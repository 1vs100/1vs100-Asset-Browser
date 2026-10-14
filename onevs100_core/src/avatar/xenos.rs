use anyhow::{bail, Result};

pub type Rgba = [u8; 4];

const TILED_BIT: u32 = 0x100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Dxt1,
    Dxt3,
    Dxt5,
    Argb8,
}

impl Format {
    pub fn from_d3d(word: u32) -> Result<Self> {
        Ok(match word & !TILED_BIT {
            0x1A200052 => Format::Dxt1,
            0x1A200053 => Format::Dxt3,
            0x1A200054 => Format::Dxt5,
            0x1A200086 => Format::Argb8,
            _ => bail!("unsupported D3DFORMAT {word:#010x}"),
        })
    }

    pub fn block_bytes(self) -> usize {
        match self {
            Format::Dxt1 => 8,
            Format::Dxt3 | Format::Dxt5 => 16,
            Format::Argb8 => 4,
        }
    }

    pub fn block_dim(self) -> u32 {
        if self == Format::Argb8 { 1 } else { 4 }
    }

    fn log2_block_bytes(self) -> u32 {
        self.block_bytes().trailing_zeros()
    }
}

pub fn tiled_offset_2d(x: u32, y: u32, pitch_blocks: u32, bpp_log2: u32) -> usize {
    let macro_tile = ((x >> 5) + (y >> 5) * (pitch_blocks >> 5)) << (bpp_log2 + 7);
    let micro = ((x & 7) + ((y & 0xE) << 2)) << bpp_log2;
    let off = macro_tile + ((micro & !0xF) << 1) + (micro & 0xF) + ((y & 1) << 4);
    (((off & !0x1FF) << 3)
        + ((y & 16) << 7)
        + ((off & 0x1C0) << 2)
        + (((((y & 8) >> 2) + (x >> 3)) & 3) << 6)
        + (off & 0x3F)) as usize
}

pub fn swap_bytes(data: &[u8], width: usize) -> Vec<u8> {
    let mut out = data.to_vec();
    out.chunks_exact_mut(width).for_each(<[u8]>::reverse);
    out
}

pub fn aligned_blocks(n: u32) -> u32 {
    n.div_ceil(32) * 32
}

fn log2_ceil(n: u32) -> u32 {
    if n <= 1 { 0 } else { 32 - (n - 1).leading_zeros() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MipLevel {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub offset: usize,
    pub pitch: u32,
    pub block_x: u32,
    pub block_y: u32,
    pub packed: bool,
}

pub fn mip_layout(width: u32, height: u32, format: Format, mips: u32) -> (Vec<MipLevel>, usize) {
    let bd = format.block_dim();
    let bytes = format.block_bytes();
    let shift = bd.trailing_zeros();
    let (lw, lh) = (log2_ceil(width), log2_ceil(height));
    let packed_base = lw.min(lh).saturating_sub(4);
    let mut out = Vec::new();
    let mut offset = 0usize;
    let mut tail = None;
    for m in 0..mips {
        let (w, h) = ((width >> m).max(1), (height >> m).max(1));
        if m < packed_base {
            let edge = |full: u32, log: u32| {
                let texels = if m == 0 { full } else { (1u32 << log) >> m };
                aligned_blocks(texels.div_ceil(bd))
            };
            let (pw, ph) = (edge(width, lw), edge(height, lh));
            out.push(MipLevel { level: m, width: w, height: h, offset, pitch: pw, block_x: 0, block_y: 0, packed: false });
            offset += (pw * ph) as usize * bytes;
        } else {
            let tail_offset = *tail.get_or_insert(offset);
            let pm = m - packed_base;
            let wide = lw > lh;
            let (x, y) = match (pm < 3, wide) {
                (true, true) => (0, 16 >> pm),
                (true, false) => (16 >> pm, 0),
                (false, true) => ((1u32 << (lw - packed_base)) >> (pm - 2), 0),
                (false, false) => (0, (1u32 << (lh - packed_base)) >> (pm - 2)),
            };
            out.push(MipLevel {
                level: m,
                width: w,
                height: h,
                offset: tail_offset,
                pitch: 32,
                block_x: x >> shift,
                block_y: y >> shift,
                packed: true,
            });
        }
    }
    (out, tail.map_or(offset, |t| t + 32 * 32 * bytes))
}

fn rgb565(c: u16) -> [u32; 3] {
    let (r, g, b) = ((c >> 11) & 31, (c >> 5) & 63, c & 31);
    [r as u32 * 255 / 31, g as u32 * 255 / 63, b as u32 * 255 / 31]
}

fn mix(a: [u32; 3], b: [u32; 3], wa: u32, wb: u32) -> Rgba {
    let c = |i: usize| ((wa * a[i] + wb * b[i]) / (wa + wb)) as u8;
    [c(0), c(1), c(2), 255]
}

fn colour_palette(c0: u16, c1: u16, four_colour: bool) -> [Rgba; 4] {
    let (a, b) = (rgb565(c0), rgb565(c1));
    let opaque = |c: [u32; 3]| [c[0] as u8, c[1] as u8, c[2] as u8, 255];
    if four_colour || c0 > c1 {
        [opaque(a), opaque(b), mix(a, b, 2, 1), mix(a, b, 1, 2)]
    } else {
        [opaque(a), opaque(b), mix(a, b, 1, 1), [0, 0, 0, 0]]
    }
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn colour_pixels(block: &[u8], four_colour: bool) -> [Rgba; 16] {
    let pal = colour_palette(le16(block, 0), le16(block, 2), four_colour);
    let idx = u32::from_le_bytes(block[4..8].try_into().unwrap());
    std::array::from_fn(|i| pal[((idx >> (2 * i)) & 3) as usize])
}

pub fn dxt5_alphas(a0: u8, a1: u8) -> [u8; 8] {
    let (x, y) = (a0 as u32, a1 as u32);
    let lerp = |i: u32, n: u32| (((n - i) * x + i * y) / n) as u8;
    if a0 > a1 {
        std::array::from_fn(|i| match i {
            0 => a0,
            1 => a1,
            _ => lerp(i as u32 - 1, 7),
        })
    } else {
        std::array::from_fn(|i| match i {
            0 => a0,
            1 => a1,
            6 => 0,
            7 => 255,
            _ => lerp(i as u32 - 1, 5),
        })
    }
}

pub fn decode_block(block: &[u8], format: Format) -> [Rgba; 16] {
    match format {
        Format::Dxt1 => colour_pixels(block, false),
        Format::Dxt3 => {
            let alpha = u64::from_le_bytes(block[..8].try_into().unwrap());
            let mut px = colour_pixels(&block[8..], true);
            px.iter_mut().enumerate().for_each(|(i, p)| p[3] = ((alpha >> (4 * i)) & 15) as u8 * 17);
            px
        }
        Format::Dxt5 => {
            let table = dxt5_alphas(block[0], block[1]);
            let bits = u64::from_le_bytes([block[2], block[3], block[4], block[5], block[6], block[7], 0, 0]);
            let mut px = colour_pixels(&block[8..], true);
            px.iter_mut().enumerate().for_each(|(i, p)| p[3] = table[((bits >> (3 * i)) & 7) as usize]);
            px
        }
        Format::Argb8 => {
            let mut px = [[0; 4]; 16];
            px[0] = [block[1], block[2], block[3], block[0]];
            px
        }
    }
}

fn decode_blocks(buf: &[u8], width: u32, height: u32, format: Format, offset_of: impl Fn(u32, u32) -> usize) -> Vec<Rgba> {
    let (bd, bytes) = (format.block_dim(), format.block_bytes());
    let mut out = vec![[0, 0, 0, 255]; (width * height) as usize];
    for by in 0..height.div_ceil(bd) {
        for bx in 0..width.div_ceil(bd) {
            let at = offset_of(bx, by);
            let Some(block) = buf.get(at..at + bytes) else { continue };
            for (i, p) in decode_block(block, format).iter().enumerate().take((bd * bd) as usize) {
                let (x, y) = (bx * bd + i as u32 % bd, by * bd + i as u32 / bd);
                if x < width && y < height {
                    out[(y * width + x) as usize] = *p;
                }
            }
        }
    }
    out
}

fn unswapped(chunk: &[u8], format: Format, swapped: bool) -> std::borrow::Cow<'_, [u8]> {
    if swapped {
        std::borrow::Cow::Owned(swap_bytes(chunk, if format == Format::Argb8 { 4 } else { 2 }))
    } else {
        std::borrow::Cow::Borrowed(chunk)
    }
}

pub fn decode_level(chunk: &[u8], level: &MipLevel, format: Format, swapped: bool) -> Vec<Rgba> {
    let buf = unswapped(chunk, format, swapped);
    let lg = format.log2_block_bytes();
    decode_blocks(&buf, level.width, level.height, format, |bx, by| level.offset + tiled_offset_2d(bx + level.block_x, by + level.block_y, level.pitch, lg))
}

pub fn decode_linear(chunk: &[u8], width: u32, height: u32, format: Format, pitch_blocks: u32, swapped: bool) -> Vec<Rgba> {
    let buf = unswapped(chunk, format, swapped);
    let bytes = format.block_bytes();
    decode_blocks(&buf, width, height, format, |bx, by| (by as usize * pitch_blocks as usize + bx as usize) * bytes)
}
