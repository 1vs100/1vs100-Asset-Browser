use crate::bitpack::{Bits, VectorCtx, VECTOR_HEADER_BITS};
use crate::model::{self, Model};
use anyhow::{ensure, Result};

const BLOCK_ID: u32 = 4;
const TRIANGLE_HEADER_BYTES: usize = 32;
const VERTEX_HEADER_BYTES: usize = 28;
const WIDTH_BITS: usize = 32;
const INTEGER_HEADERS: usize = 4;
const INTEGER_HEADER_BITS: usize = 64;
const MAX_ENTRIES: usize = 1 << 16;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub offset: u32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub target_vertex_bytes: u32,
    pub entries: Vec<Entry>,
}

fn word(d: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(d.get(at..at + 4).ok_or_else(|| anyhow::anyhow!("truncated shape block"))?.try_into()?) as usize)
}

pub fn shape(asset: &[u8]) -> Result<Shape> {
    let s = crate::strb::parse(asset)?;
    let b = s.blocks.iter().find(|b| b.id == BLOCK_ID).ok_or_else(|| anyhow::anyhow!("asset has no shape block"))?;
    let d = &asset[b.start..b.start + b.length];
    let (triangles, width) = (word(d, 0)?, word(d, 28)?);
    let part = TRIANGLE_HEADER_BYTES + (triangles * width).div_ceil(8);
    let (count, target_vertex_bytes) = (word(d, part)?, word(d, part + 4)? as u32);
    ensure!(count > 0 && count <= MAX_ENTRIES, "no vertex overrides ({count} entries)");
    let bits = Bits(d.get(part + VERTEX_HEADER_BYTES..).ok_or_else(|| anyhow::anyhow!("truncated vertex overrides"))?);
    let index_bits = bits.read(0, WIDTH_BITS) as usize;
    let position = VectorCtx::parse(&bits, WIDTH_BITS);
    let head = WIDTH_BITS + VECTOR_HEADER_BITS;
    let ints: Vec<(u32, usize)> = (0..INTEGER_HEADERS).map(|i| (bits.read(head + i * INTEGER_HEADER_BITS, 32), bits.read(head + i * INTEGER_HEADER_BITS + 32, 32) as usize)).collect();
    let data = head + INTEGER_HEADERS * INTEGER_HEADER_BITS;
    let stride = index_bits + position.bit_count() + ints.iter().map(|(_, w)| w).sum::<usize>();
    ensure!(index_bits <= 32 && ints.iter().all(|(_, w)| *w <= 32), "implausible vertex override widths");
    ensure!(part + VERTEX_HEADER_BYTES + (data + count * stride).div_ceil(8) <= d.len(), "vertex overrides run past the block");
    let entries = (0..count)
        .map(|i| {
            let at = data + i * stride;
            let normal_at = at + index_bits + position.bit_count();
            Entry { offset: bits.read(at, index_bits), position: position.decode(&bits, at + index_bits), normal: model::unpack_normal(bits.read(normal_at, ints[0].1).wrapping_add(ints[0].0)) }
        })
        .collect();
    Ok(Shape { target_vertex_bytes, entries })
}

impl Shape {
    pub fn locate(&self, model: &Model, entry: &Entry) -> Option<(usize, usize)> {
        let mut base = 0usize;
        for (b, batch) in model.batches.iter().enumerate() {
            let stride = batch.vertex_stride as usize;
            let bytes = batch.vertices.len() * stride;
            if (entry.offset as usize) < base + bytes {
                return Some((b, (entry.offset as usize - base) / stride));
            }
            base += bytes;
        }
        None
    }
}

pub fn apply(model: &Model, shape: &Shape) -> Model {
    let mut out = model.clone();
    for e in &shape.entries {
        if let Some((b, v)) = shape.locate(model, e) {
            out.batches[b].vertices[v].position = e.position;
            out.batches[b].vertices[v].normal = e.normal;
        }
    }
    out
}
