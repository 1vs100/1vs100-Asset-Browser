use crate::inspect::Preview;
use crate::model::{Batch, Model, ParamValue, Vertex};
use crate::shade::{self, FaceLayers, Rgb, Rgba, Tones};
use std::sync::Arc;

pub const BODY_DEPTH_BIAS: f32 = 0.03;
const COLOR_USAGE: u32 = 1;
const INTENSITY_USAGE: u32 = 2;
const DECAL_USAGE: u32 = 3;
const SKIN_FEATURES_USAGE: u32 = 5;
const FACIAL_HAIR_USAGE: u32 = 6;
const EYEBROW_USAGES: [u32; 2] = [7, 8];
const EYE_USAGES: [u32; 2] = [9, 10];
const EYE_SHADOW_USAGE: u32 = 11;
const MOUTH_USAGE: u32 = 12;
const HEAD_SHADER: u32 = 4;
const DEFAULT_YAW: f32 = std::f32::consts::PI - 0.5;
const DEFAULT_PITCH: f32 = 0.35;
const MARGIN: f32 = 0.9;
const LIGHT: [f32; 3] = [0.35, 0.55, 0.75];
const AMBIENT: f32 = 0.38;
const CUTOUT_ALPHA: f32 = 0.5;
const FALLBACK: Rgb = [0.75, 0.75, 0.78];
const DEFAULT_SKIN: Rgb = [0.8, 0.65, 0.55];
const BLACK_LUMA: f32 = 6.0 / 255.0;
const MAX_UV_INDEX: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct Tex {
    pub rgba: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone)]
pub struct Face {
    pub tones: Tones,
    pub mouth: Option<Arc<Tex>>,
    pub eye: Option<Arc<Tex>>,
    pub eyebrow: Option<Arc<Tex>>,
    pub facial_hair: Option<Arc<Tex>>,
    pub eye_shadow: Option<Arc<Tex>>,
    pub skin_features: Option<Arc<Tex>>,
}

impl Default for Face {
    fn default() -> Face {
        let tone = |c: Rgb| c;
        Face {
            tones: Tones { skin: DEFAULT_SKIN, hair: tone([0.2; 3]), mouth: [0.7, 0.3, 0.3], iris: [0.3, 0.4, 0.6], eyebrow: [0.2; 3], eye_shadow: [0.5; 3], facial_hair: [0.2; 3], skin_feature_1: DEFAULT_SKIN, skin_feature_2: DEFAULT_SKIN },
            mouth: None,
            eye: None,
            eyebrow: None,
            facial_hair: None,
            eye_shadow: None,
            skin_features: None,
        }
    }
}

pub struct Part<'a> {
    pub model: &'a Model,
    pub custom: Option<[Rgb; 3]>,
    pub depth_bias: f32,
}

struct Layer {
    tex: Arc<Tex>,
    uv_set: usize,
}

enum Material {
    Flat,
    Body { colour: Option<Layer>, intensity: Option<Layer>, decal: Option<Layer>, custom: Option<[Rgb; 3]> },
    Head { skin_features: Option<Layer>, facial_hair: Option<Layer>, eyebrow: Option<Layer>, eye: Option<Layer>, eye_shadow: Option<Layer>, mouth: Option<Layer> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: [f32; 2],
}

impl Default for Camera {
    fn default() -> Camera {
        Camera { yaw: DEFAULT_YAW, pitch: DEFAULT_PITCH, zoom: 1.0, pan: [0.0; 2] }
    }
}

#[derive(Debug)]
pub struct ScenePart {
    pub model: Model,
    pub custom: Option<[Rgb; 3]>,
    pub tintable: bool,
    pub depth_bias: f32,
}

#[derive(Debug)]
pub struct Scene {
    pub parts: Vec<ScenePart>,
    pub face: Face,
}

#[derive(Debug, Clone)]
pub struct SceneHandle(pub Arc<Scene>);

impl PartialEq for SceneHandle {
    fn eq(&self, other: &SceneHandle) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SceneHandle {}

impl Scene {
    pub fn is_tintable(&self) -> bool {
        self.parts.iter().any(|p| p.tintable)
    }

    pub fn tinted(&self, colours: [Rgb; 3]) -> Scene {
        let parts = self.parts.iter().map(|p| ScenePart { model: p.model.clone(), custom: if p.tintable { Some(colours) } else { p.custom }, tintable: p.tintable, depth_bias: p.depth_bias }).collect();
        Scene { parts, face: self.face.clone() }
    }

    pub fn render(&self, camera: &Camera, width: u32, height: u32) -> Preview {
        let parts: Vec<Part> = self.parts.iter().map(|p| Part { model: &p.model, custom: p.custom, depth_bias: p.depth_bias }).collect();
        draw(&parts, &self.face, camera, width, height)
    }
}

fn rotate(p: [f32; 3], camera: &Camera) -> [f32; 3] {
    let (sy, cy, sp, cp) = (camera.yaw.sin(), camera.yaw.cos(), camera.pitch.sin(), camera.pitch.cos());
    let (x, z) = (p[0] * cy + p[2] * sy, -p[0] * sy + p[2] * cy);
    [x, p[1] * cp - z * sp, p[1] * sp + z * cp]
}

impl Tex {
    pub fn sample(&self, uv: [f32; 2]) -> Rgba {
        let (x, y) = (uv[0].rem_euclid(1.0) * self.width as f32 - 0.5, uv[1].rem_euclid(1.0) * self.height as f32 - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let texel = |tx: f32, ty: f32| {
            let (tx, ty) = ((tx as i64).rem_euclid(self.width as i64) as usize, (ty as i64).rem_euclid(self.height as i64) as usize);
            let o = (ty * self.width + tx) * 4;
            std::array::from_fn::<f32, 4, _>(|i| f32::from(self.rgba[o + i]) / 255.0)
        };
        let (a, b, c, d) = (texel(x0, y0), texel(x0 + 1.0, y0), texel(x0, y0 + 1.0), texel(x0 + 1.0, y0 + 1.0));
        std::array::from_fn(|i| (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy)
    }
}

fn sample(tex: &Tex, uv: [f32; 2]) -> Rgba {
    tex.sample(uv)
}

fn model_layer(model: &Model, batch: &Batch, usages: &[u32]) -> Option<Layer> {
    let p = batch.params.iter().find(|p| usages.contains(&p.usage))?;
    let ParamValue::Texture { uv_set, index, .. } = p.value else { return None };
    let t = model.textures.get(usize::from(index))?;
    Some(Layer { tex: Arc::new(Tex { rgba: t.rgba().ok()?, width: t.width as usize, height: t.height as usize }), uv_set: usize::from(uv_set).min(MAX_UV_INDEX) })
}

fn dynamic_layer(batch: &Batch, usages: &[u32], tex: &Option<Arc<Tex>>) -> Option<Layer> {
    let p = batch.params.iter().find(|p| usages.contains(&p.usage))?;
    let ParamValue::Texture { uv_set, .. } = p.value else { return None };
    tex.clone().map(|tex| Layer { tex, uv_set: usize::from(uv_set).min(MAX_UV_INDEX) })
}

fn uv_at(batch: &Batch, ids: [usize; 3], weights: [f32; 3], set: usize) -> [f32; 2] {
    let set = set.min(batch.uv_sets as usize - 1);
    [0, 1].map(|c| (0..3).map(|k| weights[k] * batch.vertices[ids[k]].uvs[set][c]).sum())
}

fn material(part: &Part, batch: &Batch, face: &Face) -> Material {
    if batch.shader == HEAD_SHADER {
        return Material::Head {
            skin_features: dynamic_layer(batch, &[SKIN_FEATURES_USAGE], &face.skin_features),
            facial_hair: dynamic_layer(batch, &[FACIAL_HAIR_USAGE], &face.facial_hair),
            eyebrow: dynamic_layer(batch, &EYEBROW_USAGES, &face.eyebrow),
            eye: dynamic_layer(batch, &EYE_USAGES, &face.eye),
            eye_shadow: dynamic_layer(batch, &[EYE_SHADOW_USAGE], &face.eye_shadow),
            mouth: dynamic_layer(batch, &[MOUTH_USAGE], &face.mouth),
        };
    }
    let colour = model_layer(part.model, batch, &[COLOR_USAGE]);
    let mean_luma = colour.as_ref().map(|l| {
        let n = batch.vertices.len().max(1) as f32;
        let uv_set = l.uv_set.min(batch.uv_sets as usize - 1);
        batch.vertices.iter().map(|v| sample(&l.tex, v.uvs[uv_set])).map(|c| (c[0] + c[1] + c[2]) / 3.0).sum::<f32>() / n
    });
    if part.custom.is_none() && mean_luma.is_none_or(|m| m < BLACK_LUMA) {
        return Material::Flat;
    }
    Material::Body { colour, intensity: model_layer(part.model, batch, &[INTENSITY_USAGE]), decal: model_layer(part.model, batch, &[DECAL_USAGE]), custom: part.custom }
}

fn vertex_tint(vs: [&Vertex; 3], weights: [f32; 3]) -> Rgb {
    std::array::from_fn(|c| (0..3).map(|k| weights[k] * shade::rgb(vs[k].color)[c]).sum())
}

fn shade_pixel(m: &Material, face: &Face, batch: &Batch, ids: [usize; 3], weights: [f32; 3]) -> Option<Rgb> {
    let at = |l: &Layer| sample(&l.tex, uv_at(batch, ids, weights, l.uv_set));
    match m {
        Material::Flat => Some(FALLBACK),
        Material::Head { skin_features, facial_hair, eyebrow, eye, eye_shadow, mouth } => {
            let layers = FaceLayers { skin_features: skin_features.as_ref().map(at), facial_hair: facial_hair.as_ref().map(at), eyebrow: eyebrow.as_ref().map(at), eye: eye.as_ref().map(at), mouth: mouth.as_ref().map(at), eye_shadow: eye_shadow.as_ref().map(at) };
            Some(shade::face(&face.tones, &layers))
        }
        Material::Body { colour, intensity, decal, custom } => {
            let base = colour.as_ref().map_or([FALLBACK[0], FALLBACK[1], FALLBACK[2], 1.0], at);
            if base[3] < CUTOUT_ALPHA {
                return None;
            }
            let mut rgb = [base[0], base[1], base[2]];
            if let (Some(i), Some(c)) = (intensity, custom) {
                rgb = shade::intensity(rgb, at(i), *c);
            }
            if let Some(d) = decal {
                rgb = shade::decal(rgb, at(d));
            }
            let tint = vertex_tint(ids.map(|i| &batch.vertices[i]), weights);
            Some(std::array::from_fn(|c| rgb[c] * tint[c]))
        }
    }
}

pub fn preview(model: &Model, width: u32, height: u32) -> Preview {
    preview_scene(&[model], width, height)
}

pub fn preview_scene(models: &[&Model], width: u32, height: u32) -> Preview {
    let parts: Vec<Part> = models.iter().map(|&model| Part { model, custom: None, depth_bias: 0.0 }).collect();
    preview_avatar(&parts, &Face::default(), width, height)
}

pub fn preview_avatar(parts: &[Part], face: &Face, width: u32, height: u32) -> Preview {
    draw(parts, face, &Camera::default(), width, height)
}

fn draw(parts: &[Part], face: &Face, camera: &Camera, width: u32, height: u32) -> Preview {
    let batches: Vec<(&Part, &Batch)> = parts.iter().flat_map(|p| p.model.batches.iter().map(move |b| (p, b))).collect();
    let (w, h) = (width as usize, height as usize);
    let mut rgba = vec![0u8; w * h * 4];
    let mut depth = vec![f32::MAX; w * h];
    let (lo, hi) = batches.iter().flat_map(|(_, b)| b.vertices.iter().map(|v| v.position)).fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| (std::array::from_fn(|i| lo[i].min(p[i])), std::array::from_fn(|i| hi[i].max(p[i]))));
    let centre: [f32; 3] = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0);
    let extent = (0..3).map(|i| hi[i] - lo[i]).fold(1e-6, f32::max);
    let scale = MARGIN * w.min(h) as f32 / extent * camera.zoom;
    let transformed: Vec<Vec<[f32; 3]>> = batches.iter().map(|(_, b)| b.vertices.iter().map(|v| rotate(std::array::from_fn(|i| v.position[i] - centre[i]), camera)).collect()).collect();
    let screen = |p: [f32; 3]| [p[0] * scale + w as f32 / 2.0 + camera.pan[0], h as f32 / 2.0 - p[1] * scale + camera.pan[1], p[2]];
    let light_len = LIGHT.iter().map(|x| x * x).sum::<f32>().sqrt();
    for (&(part, batch), points) in batches.iter().zip(&transformed) {
        let mat = material(part, batch, face);
        for tri in batch.indices.chunks_exact(3) {
            let ids = [usize::from(tri[0]), usize::from(tri[1]), usize::from(tri[2])];
            let p = ids.map(|i| screen(points[i]));
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
            if area.abs() < 1e-9 {
                continue;
            }
            let (x0, x1) = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil() as usize).min(w - 1));
            let (y0, y1) = (p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil() as usize).min(h - 1));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[1][1] - py) * (p[2][0] - px)) / area;
                    let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[2][1] - py) * (p[0][0] - px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2] + part.depth_bias;
                    if z >= depth[y * w + x] {
                        continue;
                    }
                    let weights = [w0, w1, w2];
                    let verts = ids.map(|i| &batch.vertices[i]);
                    let normal: [f32; 3] = std::array::from_fn(|k| (0..3).map(|v| weights[v] * verts[v].normal[k]).sum());
                    let rotated = rotate(normal, camera);
                    let facing = (rotated[0] * LIGHT[0] + rotated[1] * LIGHT[1] + rotated[2] * LIGHT[2]) / light_len / rotated.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-6);
                    let lit = (AMBIENT + (1.0 - AMBIENT) * facing.abs()).min(1.0);
                    let Some(colour) = shade_pixel(&mat, face, batch, ids, weights) else { continue };
                    depth[y * w + x] = z;
                    let o = (y * w + x) * 4;
                    rgba[o..o + 3].copy_from_slice(&colour.map(|c| (c.clamp(0.0, 1.0) * lit * 255.0) as u8));
                    rgba[o + 3] = 255;
                }
            }
        }
    }
    Preview { width, height, rgba }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TintPreset {
    Original,
    Warm,
    Cool,
    Forest,
    Royal,
    Mono,
}

impl TintPreset {
    pub const ALL: [TintPreset; 6] = [TintPreset::Original, TintPreset::Warm, TintPreset::Cool, TintPreset::Forest, TintPreset::Royal, TintPreset::Mono];

    pub fn colours(self) -> Option<[Rgb; 3]> {
        match self {
            TintPreset::Original => None,
            TintPreset::Warm => Some([[0.9, 0.35, 0.15], [0.95, 0.75, 0.2], [0.6, 0.1, 0.1]]),
            TintPreset::Cool => Some([[0.2, 0.45, 0.9], [0.3, 0.8, 0.85], [0.15, 0.2, 0.5]]),
            TintPreset::Forest => Some([[0.2, 0.55, 0.25], [0.55, 0.45, 0.2], [0.1, 0.3, 0.15]]),
            TintPreset::Royal => Some([[0.45, 0.2, 0.7], [0.9, 0.8, 0.3], [0.2, 0.1, 0.4]]),
            TintPreset::Mono => Some([[0.9; 3], [0.5; 3], [0.15; 3]]),
        }
    }
}

impl std::fmt::Display for TintPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
