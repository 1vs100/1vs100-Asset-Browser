use crate::model::Model;
use anyhow::{ensure, Result};

const HEADER_BYTES: usize = 32;
const BLOCK_ID: u32 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub target_index_bytes: u32,
    pub triangles: Vec<u32>,
}

pub fn template(asset: &[u8]) -> Result<Template> {
    let s = crate::strb::parse(asset)?;
    let b = s.blocks.iter().find(|b| b.id == BLOCK_ID).ok_or_else(|| anyhow::anyhow!("asset has no shape override block"))?;
    let d = &asset[b.start..b.start + b.length];
    ensure!(d.len() >= HEADER_BYTES, "shape override block is {} bytes", d.len());
    let word = |at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap());
    let (count, target_index_bytes, width) = (word(0) as usize, word(4), word(28) as usize);
    if count == 0 {
        return Ok(Template { target_index_bytes, triangles: Vec::new() });
    }
    ensure!(width > 0 && width <= 16 && HEADER_BYTES + (count * width).div_ceil(8) <= d.len(), "bad triangle list ({count} values of {width} bits)");
    let bits = super::bitpack::Bits(&d[HEADER_BYTES..]);
    Ok(Template { target_index_bytes, triangles: (0..count).map(|i| bits.read(i * width, width)).collect() })
}

pub fn apply(model: &Model, hidden: &[u32]) -> Model {
    let mut out = model.clone();
    for batch in &mut out.batches {
        let kept: Vec<u16> = batch.indices.chunks_exact(3).enumerate().filter(|(i, _)| hidden.binary_search(&(*i as u32)).is_err()).flat_map(|(_, t)| t.iter().copied()).collect();
        batch.triangles = (kept.len() / 3) as u32;
        batch.indices = kept;
    }
    out
}

const FIT_RADIUS: f32 = 0.03;
const MIN_FIT: f32 = 0.6;
const TEMPLATE_TYPE: u32 = 3;

pub fn all_templates(pack: &[u8]) -> Vec<Template> {
    crate::toc::split(pack).iter().filter(|a| a.asset_type == TEMPLATE_TYPE).filter_map(|a| template(a.data).ok()).filter(|t| !t.triangles.is_empty()).collect()
}

fn near(body: &Model, garments: &[&Model]) -> Vec<bool> {
    let key = |p: [f32; 3]| p.map(|v| (v / FIT_RADIUS).floor() as i32);
    let mut grid: std::collections::HashMap<[i32; 3], Vec<[f32; 3]>> = std::collections::HashMap::new();
    for v in garments.iter().flat_map(|g| g.batches.iter().flat_map(|b| &b.vertices)) {
        grid.entry(key(v.position)).or_default().push(v.position);
    }
    let near = |c: [f32; 3]| {
        let k = key(c);
        (-1..=1).any(|dx| (-1..=1).any(|dy| (-1..=1).any(|dz| grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).is_some_and(|pts| pts.iter().any(|p| (0..3).map(|i| (p[i] - c[i]).powi(2)).sum::<f32>() < FIT_RADIUS * FIT_RADIUS)))))
    };
    let Some(b) = body.batches.first() else { return Vec::new() };
    b.indices.chunks_exact(3).map(|t| near(std::array::from_fn(|k| (0..3).map(|i| b.vertices[usize::from(t[i])].position[k]).sum::<f32>() / 3.0))).collect()
}

pub fn best_fit<'a>(body: &Model, garment: &Model, templates: &'a [Template]) -> Option<&'a Template> {
    let index_bytes = body.batches.first()?.indices.len() as u32 * 2;
    let cover = near(body, &[garment]);
    let total = cover.iter().filter(|c| **c).count() as f32;
    let score = |t: &Template| {
        let hit = t.triangles.iter().filter(|&&i| cover.get(i as usize).copied().unwrap_or(false)).count() as f32;
        hit / (total + t.triangles.len() as f32 - hit).max(1.0)
    };
    templates.iter().filter(|t| t.target_index_bytes == index_bytes).map(|t| (score(t), t)).filter(|(s, _)| *s >= MIN_FIT).max_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, t)| t)
}

pub fn keep_covered(body: &Model, garments: &[&Model], hidden: &mut Vec<u32>) {
    let cover = near(body, garments);
    hidden.retain(|&t| cover.get(t as usize).copied().unwrap_or(false));
}

const RAY_REACH: f32 = 0.06;
const RAY_BEHIND: f32 = 0.004;
const CELL: f32 = 0.08;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn hits(origin: [f32; 3], dir: [f32; 3], tri: &[[f32; 3]; 3]) -> Option<f32> {
    let (e1, e2) = (sub(tri[1], tri[0]), sub(tri[2], tri[0]));
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let t = sub(origin, tri[0]);
    let u = dot(t, p) / det;
    let q = cross(t, e1);
    let v = dot(dir, q) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0).then(|| dot(e2, q) / det)
}

fn cell(p: [f32; 3]) -> [i32; 3] {
    p.map(|v| (v / CELL).floor() as i32)
}

pub fn covered_by(body: &Model, garments: &[&Model]) -> Vec<u32> {
    let Some(b) = body.batches.first() else { return Vec::new() };
    let mut grid: std::collections::HashMap<[i32; 3], Vec<[[f32; 3]; 3]>> = std::collections::HashMap::new();
    for g in garments.iter().flat_map(|g| g.batches.iter()) {
        for t in g.indices.chunks_exact(3) {
            let tri: [[f32; 3]; 3] = std::array::from_fn(|i| g.vertices[usize::from(t[i])].position);
            let (lo, hi) = (cell(std::array::from_fn(|k| tri.iter().map(|p| p[k]).fold(f32::MAX, f32::min))), cell(std::array::from_fn(|k| tri.iter().map(|p| p[k]).fold(f32::MIN, f32::max))));
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        grid.entry([x, y, z]).or_default().push(tri);
                    }
                }
            }
        }
    }
    let blocked = |origin: [f32; 3], normal: [f32; 3]| {
        let far: [f32; 3] = std::array::from_fn(|k| origin[k] + normal[k] * RAY_REACH);
        let (lo, hi) = (cell(std::array::from_fn(|k| origin[k].min(far[k]))), cell(std::array::from_fn(|k| origin[k].max(far[k]))));
        (lo[0]..=hi[0]).any(|x| (lo[1]..=hi[1]).any(|y| (lo[2]..=hi[2]).any(|z| grid.get(&[x, y, z]).is_some_and(|tris| tris.iter().any(|tri| hits(origin, normal, tri).is_some_and(|d| d > -RAY_BEHIND && d <= RAY_REACH))))))
    };
    let unit = |n: [f32; 3]| {
        let len = dot(n, n).sqrt().max(1e-12);
        n.map(|c| c / len)
    };
    let covered = |t: &[u16]| {
        let v: [&crate::model::Vertex; 3] = std::array::from_fn(|i| &b.vertices[usize::from(t[i])]);
        let centre: [f32; 3] = std::array::from_fn(|k| (v[0].position[k] + v[1].position[k] + v[2].position[k]) / 3.0);
        let shading: [f32; 3] = std::array::from_fn(|k| v[0].normal[k] + v[1].normal[k] + v[2].normal[k]);
        let face = cross(sub(v[1].position, v[0].position), sub(v[2].position, v[0].position));
        let face = if dot(face, shading) < 0.0 { face.map(|c| -c) } else { face };
        blocked(centre, unit(face)) || v.iter().filter(|x| blocked(x.position, unit(x.normal))).count() >= 2
    };
    b.indices.chunks_exact(3).enumerate().filter(|(_, t)| covered(t)).map(|(i, _)| i as u32).collect()
}
