use super::bitpack::{Bits, VectorCtx, VECTOR_HEADER_BITS};
use crate::xenos;
use anyhow::{ensure, Result};

const HEADER_BYTES: usize = 48;
const BATCH_HEADER_BYTES: usize = 33;
const PARAM_BYTES: usize = 24;
const TEXTURE_ENTRY_BYTES: usize = 8;
const TEXTURE_HEADER_BYTES: usize = 33;
const COUNT_BITS: usize = 32;
const INTEGER_HEADER_BITS: usize = 64;
const INTEGER_MEMBERS: usize = 4;
const UV_SET_BITS: usize = 32;
const MAX_UV_SETS: u32 = 7;
const INDEX_HEADER_BYTES: usize = 8;
const TEXTURE_PARAM: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub cpu_buffer: u32,
    pub gpu_buffer: u32,
    pub texture_data: u32,
    pub vertex_buffer: u32,
    pub index_buffer: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Texture { uv_set: u16, index: u16, flags: u32 },
    Vector([f32; 4]),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderParam {
    pub kind: u32,
    pub usage: u32,
    pub value: ParamValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub weights: [f32; 4],
    pub bones: [u8; 4],
    pub color: u32,
    pub uvs: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub shader: u32,
    pub triangles: u32,
    pub uv_sets: u32,
    pub vertex_stride: u32,
    pub params: Vec<ShaderParam>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub row_bytes: u32,
    pub rows: u32,
    pub placeholder: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub header: Header,
    pub batches: Vec<Batch>,
    pub textures: Vec<Texture>,
    pub trailing_bytes: usize,
}

fn le32(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(d.get(o..o + 4).ok_or_else(|| anyhow::anyhow!("truncated at {o:#x}"))?.try_into()?))
}

fn le16(d: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(d.get(o..o + 2).ok_or_else(|| anyhow::anyhow!("truncated at {o:#x}"))?.try_into()?))
}

fn half(h: u16) -> f32 {
    let (sign, exp, frac) = (h >> 15 & 1, i32::from(h >> 10 & 31), f32::from(h & 1023));
    let magnitude = match exp {
        0 => frac / 1024.0 * 2f32.powi(-14),
        31 => if frac == 0.0 { f32::INFINITY } else { f32::NAN },
        e => (1.0 + frac / 1024.0) * 2f32.powi(e - 15),
    };
    if sign == 1 { -magnitude } else { magnitude }
}

pub(crate) fn unpack_normal(word: u32) -> [f32; 3] {
    [signed_normalized(word, 11), signed_normalized(word >> 11, 11), signed_normalized(word >> 22, 10)]
}

fn signed_normalized(raw: u32, bits: u32) -> f32 {
    let v = raw & ((1 << bits) - 1);
    let v = if v >= 1 << (bits - 1) { v as i32 - (1 << bits) } else { v as i32 };
    (v as f32 / ((1 << (bits - 1)) - 1) as f32).max(-1.0)
}

impl Batch {
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        self.vertices.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), v| (std::array::from_fn(|i| lo[i].min(v.position[i])), std::array::from_fn(|i| hi[i].max(v.position[i]))))
    }
}

impl Texture {
    pub fn rgba(&self) -> Result<Vec<u8>> {
        self.rgba_layer(0)
    }

    pub fn rgba_layer(&self, layer: u32) -> Result<Vec<u8>> {
        let format = xenos::Format::from_d3d(self.format)?;
        ensure!(!self.placeholder, "placeholder texture has no pixels");
        ensure!(layer < self.layers.max(1), "texture has {} layers", self.layers);
        let pitch = self.row_bytes / format.block_bytes() as u32;
        let size = self.row_bytes as usize * self.rows as usize;
        let layer_data = &self.data[layer as usize * size..(layer as usize + 1) * size];
        Ok(xenos::decode_linear(layer_data, self.width, self.height, format, pitch, true).concat())
    }
}

fn params(d: &[u8], at: usize, count: usize) -> Result<Vec<ShaderParam>> {
    (0..count)
        .map(|i| {
            let o = at + i * PARAM_BYTES;
            let (kind, usage) = (le32(d, o)?, le32(d, o + 4)?);
            let value = if kind == TEXTURE_PARAM {
                ParamValue::Texture { uv_set: le16(d, o + 8)?, index: le16(d, o + 10)?, flags: le32(d, o + 12)? }
            } else {
                ParamValue::Vector(std::array::from_fn(|k| f32::from_bits(le32(d, o + 8 + 4 * k).unwrap_or(0))))
            };
            Ok(ShaderParam { kind, usage, value })
        })
        .collect()
}

fn vertices(d: &[u8], at: usize, expected: usize, uv_sets: u32) -> Result<(Vec<Vertex>, usize)> {
    ensure!((1..=MAX_UV_SETS).contains(&uv_sets), "unsupported UV set count {uv_sets}");
    let bits = Bits(d.get(at..).ok_or_else(|| anyhow::anyhow!("vertex stream outside the model"))?);
    let count = bits.read(0, COUNT_BITS) as usize;
    ensure!(count == expected, "vertex stream has {count} vertices, batch header says {expected}");
    let position = VectorCtx::parse(&bits, COUNT_BITS);
    let ints: Vec<(u32, usize)> = (0..INTEGER_MEMBERS)
        .map(|i| {
            let o = COUNT_BITS + VECTOR_HEADER_BITS + i * INTEGER_HEADER_BITS;
            (bits.read(o, 32), bits.read(o + 32, 32) as usize)
        })
        .collect();
    let header_bits = COUNT_BITS + VECTOR_HEADER_BITS + INTEGER_MEMBERS * INTEGER_HEADER_BITS;
    let per_vertex = position.bit_count() + ints.iter().map(|(_, w)| w).sum::<usize>() + uv_sets as usize * UV_SET_BITS;
    let bytes = (header_bits + count * per_vertex).div_ceil(8);
    ensure!(at + bytes <= d.len(), "vertex stream runs past the end of the model");
    let out = (0..count)
        .map(|i| {
            let mut p = header_bits + i * per_vertex;
            let position_value = position.decode(&bits, p);
            p += position.bit_count();
            let mut fields = [0u32; INTEGER_MEMBERS];
            for (field, (min, width)) in fields.iter_mut().zip(&ints) {
                *field = bits.read(p, *width).wrapping_add(*min);
                p += width;
            }
            let [normal, weights, bones, color] = fields;
            let uvs = (0..uv_sets as usize).map(|s| [half(bits.read(p + s * 32, 16) as u16), half(bits.read(p + s * 32 + 16, 16) as u16)]).collect();
            Vertex {
                position: position_value,
                normal: unpack_normal(normal),
                weights: std::array::from_fn(|k| ((weights >> (8 * k)) & 255) as f32 / 255.0),
                bones: std::array::from_fn(|k| ((bones >> (8 * k)) & 255) as u8),
                color,
                uvs,
            }
        })
        .collect();
    Ok((out, bytes))
}

fn indices(d: &[u8], at: usize, expected: usize) -> Result<(Vec<u16>, usize)> {
    let count = le32(d, at)? as usize;
    ensure!(count == expected, "index stream has {count} indices, batch header says {expected}");
    let (min, width) = (le16(d, at + 4)?, usize::from(le16(d, at + 6)?));
    ensure!(width <= 16, "index width {width} is not a 16-bit index");
    let bits = Bits(d.get(at + INDEX_HEADER_BYTES..).ok_or_else(|| anyhow::anyhow!("index stream outside the model"))?);
    let bytes = (count * width + 71) >> 3;
    ensure!(at + bytes <= d.len(), "index stream runs past the end of the model");
    Ok(((0..count).map(|i| (bits.read(i * width, width) as u16).wrapping_add(min)).collect(), bytes))
}

fn batch(d: &[u8], at: usize) -> Result<(Batch, usize)> {
    let h = Bits(d.get(at..at + BATCH_HEADER_BYTES).ok_or_else(|| anyhow::anyhow!("truncated batch header at {at:#x}"))?);
    let (shader, param_count) = (h.read(0, 32), h.read(32, 5) as usize);
    let (triangles, vertex_count, uv_sets, vertex_stride) = (h.read(37, 32), h.read(69, 32) as usize, h.read(101, 32), h.read(133, 32));
    let params = params(d, at + BATCH_HEADER_BYTES, param_count)?;
    let vertex_at = at + BATCH_HEADER_BYTES + param_count * PARAM_BYTES;
    let (vertices, vertex_bytes) = vertices(d, vertex_at, vertex_count, uv_sets)?;
    let (indices, index_bytes) = indices(d, vertex_at + vertex_bytes, 3 * triangles as usize)?;
    ensure!(indices.iter().all(|&i| usize::from(i) < vertices.len()), "an index points past the vertex list");
    Ok((Batch { shader, triangles, uv_sets, vertex_stride, params, vertices, indices }, vertex_at + vertex_bytes + index_bytes))
}

fn texture(d: &[u8], at: usize) -> Result<(Texture, usize)> {
    header_and_data(d, at + TEXTURE_ENTRY_BYTES)
}

pub fn parse_texture(d: &[u8]) -> Result<Texture> {
    header_and_data(d, 0).map(|(t, _)| t)
}

fn header_and_data(d: &[u8], at: usize) -> Result<(Texture, usize)> {
    let h = Bits(d.get(at..at + TEXTURE_HEADER_BYTES).ok_or_else(|| anyhow::anyhow!("truncated texture header at {at:#x}"))?);
    let (format, width, height, layers) = (h.read(0, 32), h.read(32, 32), h.read(64, 32), h.read(160, 32));
    let placeholder = h.read(192, 1) == 1;
    let (row_bytes, rows) = (h.read(194, 32), h.read(226, 32));
    let size = if placeholder { 0 } else { row_bytes as usize * rows as usize * layers.max(1) as usize };
    let start = at + TEXTURE_HEADER_BYTES;
    let data = d.get(start..start + size).ok_or_else(|| anyhow::anyhow!("texture data runs past the end of the model"))?.to_vec();
    Ok((Texture { format, width, height, layers, row_bytes, rows, placeholder, data }, start + size))
}

pub fn parse(d: &[u8]) -> Result<Model> {
    ensure!(d.len() > HEADER_BYTES, "model stream is {} bytes", d.len());
    let w: Vec<u32> = (0..7).map(|i| le32(d, 4 * i)).collect::<Result<_>>()?;
    let header = Header { cpu_buffer: w[0], gpu_buffer: w[1], texture_data: w[2], vertex_buffer: w[3], index_buffer: w[4] };
    let (batch_count, texture_count) = (w[5] as usize, w[6] as usize);
    ensure!(batch_count <= 64 && texture_count <= 64, "implausible batch/texture counts {batch_count}/{texture_count}");
    let (mut at, mut batches, mut textures) = (HEADER_BYTES, Vec::new(), Vec::new());
    for _ in 0..batch_count {
        let (b, next) = batch(d, at)?;
        batches.push(b);
        at = next;
    }
    for _ in 0..texture_count {
        let (t, next) = texture(d, at)?;
        textures.push(t);
        at = next;
    }
    Ok(Model { header, batches, textures, trailing_bytes: d.len() - at })
}
