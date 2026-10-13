//! Xbox 360 Xenos GPU texture handling and format conversion

use std::path::Path;

/// Xenos texture format (D3DFORMAT values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum XenosFormat {
    Dxt1 = 0x1A200152,
    Dxt2 = 0x1A200153,
    Dxt3 = 0x1A200153,
    Dxt4 = 0x1A200154,
    Dxt5 = 0x1A200154,
    Argb8 = 0x1A200186,
    Rgb565 = 0x1A200156,
    Rgba8 = 0x1A200186,
    // Add more as needed
}

impl XenosFormat {
    pub fn from_d3dformat(val: u32) -> Option<Self> {
        match val {
            0x1A200152 => Some(XenosFormat::Dxt1),
            0x1A200153 => Some(XenosFormat::Dxt3),
            0x1A200154 => Some(XenosFormat::Dxt5),
            0x1A200186 => Some(XenosFormat::Argb8),
            _ => None,
        }
    }
    
    pub fn block_size(&self) -> u32 {
        match self {
            XenosFormat::Dxt1 => 8,
            XenosFormat::Dxt3 | XenosFormat::Dxt4 | XenosFormat::Dxt5 => 16,
            XenosFormat::Argb8 | XenosFormat::Rgba8 => 4,
            _ => 8,
        }
    }
    
    pub fn block_dim(&self) -> u32 {
        match self {
            XenosFormat::Dxt1 | XenosFormat::Dxt3 | XenosFormat::Dxt4 | XenosFormat::Dxt5 => 4,
            _ => 1,
        }
    }
}

/// Xenos 2D tiling (from Xenia's GetTiledOffset2D)
/// 
/// Xenos uses a tiled memory layout for textures:
/// - Macro tiles: 32x32 blocks
/// - Micro tiles: 8x8 blocks within macro tiles
/// - Each block is 4x4 texels for DXT, 1x1 for ARGB8
/// 
/// This is the tiling formula from Xenia's texture_util.cc
pub fn tiled_offset_2d(
    x: u32,           // block x coordinate
    y: u32,           // block y coordinate  
    pitch_blocks: u32, // pitch in blocks
    bpp_log2: u32,    // log2(bytes per block): 3 for 8-byte (DXT1), 4 for 16-byte (DXT3/5)
) -> usize {
    let macro_tile = ((x >> 5) + (y >> 5) * (pitch_blocks >> 5)) << (bpp_log2 + 7);
    let micro = ((x & 7) + ((y & 0xE) << 2)) << bpp_log2;
    let off = macro_tile + ((micro & !0xF) << 1) + (micro & 0xF) + ((y & 1) << 4);
    ((off & !0x1FF) << 3) + ((y & 16) << 7) + ((off & 0x1C0) << 2) + (((((y & 8) >> 2) + (x >> 3)) & 3) << 6) + (off & 0x3F)
}

/// Linear (untiled) offset
pub fn linear_offset_2d(x: u32, y: u32, pitch_blocks: u32, block_bytes: u32) -> usize {
    (y * pitch_blocks + x) * block_bytes
}

/// Swap 16-bit words (GPUENDIAN_8IN16)
pub fn swap16(data: &mut [u8]) {
    for chunk in data.chunks_exact_mut(2) {
        chunk.swap(0, 1);
    }
}

/// Swap 32-bit words (GPUENDIAN_8IN32)
pub fn swap32(data: &mut [u8]) {
    for chunk in data.chunks_exact_mut(4) {
        chunk.reverse();
    }
}

/// Align to 32-block boundary
pub fn aligned_blocks(n: u32, alignment: u32) -> u32 {
    (n + alignment - 1) / alignment * alignment
}

/// Mip level size for tiled textures
pub fn mip_level_size(width: u32, height: u32, format: XenosFormat) -> u32 {
    let blocks_w = (width + 3) / 4;
    let blocks_h = (height + 3) / 4;
    let pitch = aligned_blocks(blocks_w, 32);
    pitch * aligned_blocks(blocks_h, 32) * format.block_size()
}

/// Full mip chain layout for Xenos
/// 
/// Levels 0..packed_base-1: full tiles
/// Levels packed_base..end: share one 32x32 macro tile (packed mip tail)
pub fn mip_layout(width: u32, height: u32, format: XenosFormat, mips: u32) -> Vec<MipLevel> {
    let block_dim = format.block_dim();
    let bpp_log2 = if format.block_size() == 8 { 3 } else { 4 };
    
    let log2_w = if width <= 1 { 0 } else { (width - 1).bit_length() as u32 };
    let log2_h = if height <= 1 { 0 } else { (height - 1).bit_length() as u32 };
    let log2_size = log2_w.min(log2_h);
    let packed_base = (log2_size.saturating_sub(4)).max(0);
    
    let mut levels = Vec::with_capacity(mips as usize);
    let mut offset = 0;
    let mut tail_offset = None;
    
    for m in 0..mips {
        let mw = (width >> m).max(1);
        let mh = (height >> m).max(1);
        
        if m < packed_base {
            let pw = if m == 0 { aligned_blocks((width + 3) / 4, 32) } else { aligned_blocks(((1u32 << (log2_w - m)) + 3) / 4, 32) };
            let ph = if m == 0 { aligned_blocks((height + 3) / 4, 32) } else { aligned_blocks(((1u32 << (log2_h - m)) + 3) / 4, 32) };
            
            let size = pw * ph * format.block_size();
            
            levels.push(MipLevel {
                level: m,
                width: mw,
                height: mh,
                offset,
                pitch: pw,
                block_x: 0,
                block_y: 0,
                packed: false,
            });
            
            offset += size;
        } else {
            if tail_offset.is_none() {
                tail_offset = Some(offset);
            }
            
            let pm = m - packed_base;
            let (bx, by) = if log2_w > log2_h {
                if pm < 3 {
                    (0, 16 >> pm)
                } else {
                    (1 << (log2_w - packed_base - (pm - 2)), 0)
                }
            } else {
                if pm < 3 {
                    (16 >> pm, 0)
                } else {
                    (0, 1 << (log2_h - packed_base - (pm - 2)))
                }
            };
            
            let size = 32 * 32 * format.block_size();
            
            levels.push(MipLevel {
                level: m,
                width: mw,
                height: mh,
                offset: tail_offset.unwrap(),
                pitch: 32,
                block_x: bx / block_dim,
                block_y: by / block_dim,
                packed: true,
            });
        }
    }
    
    levels
}

#[derive(Debug, Clone, Copy)]
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

/// Decode a single mip level from tiled data
pub fn decode_mip_level(
    data: &[u8],
    level: &MipLevel,
    format: XenosFormat,
    swap: bool,
) -> Vec<u8> {
    let block_dim = format.block_dim();
    let bpb = format.block_size();
    let lg = if bpb == 8 { 3 } else { 4 };
    
    let mut buf = data.to_vec();
    if swap {
        if bpb == 4 {
            swap32(&mut buf);
        } else {
            swap16(&mut buf);
        }
    }
    
    let bw = (level.width + block_dim - 1) / block_dim;
    let bh = (level.height + block_dim - 1) / block_dim;
    let mut output = vec![0u8; (level.width * level.height * 4) as usize];
    
    for by in 0..bh {
        for bx in 0..bw {
            let tiled_off = level.offset + tiled_offset_2d(
                bx + level.block_x,
                by + level.block_y,
                level.pitch,
                lg,
            );
            
            if tiled_off + bpb as usize > buf.len() {
                continue;
            }
            
            let block = &buf[tiled_off..tiled_off + bpb as usize];
            let pixels = decode_block(block, format);
            
            for i in 0..(block_dim * block_dim) {
                let x = bx * block_dim + i % block_dim;
                let y = by * block_dim + i / block_dim;
                if x < level.width && y < level.height {
                    let idx = ((y * level.width + x) * 4) as usize;
                    output[idx..idx+4].copy_from_slice(&pixels[i * 4..i * 4 + 4]);
                }
            }
        }
    }
    
    output
}

/// Decode a single DXT/ARGB block to RGBA pixels
fn decode_block(block: &[u8], format: XenosFormat) -> Vec<u8> {
    match format {
        XenosFormat::Dxt1 => decode_dxt1_block(block),
        XenosFormat::Dxt3 => decode_dxt3_block(block),
        XenosFormat::Dxt5 => decode_dxt5_block(block),
        XenosFormat::Argb8 | XenosFormat::Rgba8 => {
            // ARGB8: 1 pixel per block, 4 bytes per pixel
            if block.len() >= 4 {
                vec![block[1], block[2], block[3], block[0]] // ARGB -> RGBA
            } else {
                vec![0, 0, 0, 0]
            }
        },
        _ => vec![0; 16 * 4],
    }
}

fn decode_dxt1_block(block: &[u8]) -> Vec<u8> {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    
    let c0_rgb = rgb565_to_rgba(c0);
    let c1_rgb = rgb565_to_rgba(c1);
    
    let c2 = if c0 > c1 {
        [
            (2 * c0_rgb[0] + c1_rgb[0]) / 3,
            (2 * c0_rgb[1] + c1_rgb[1]) / 3,
            (2 * c0_rgb[2] + c1_rgb[2]) / 3,
            255,
        ]
    } else {
        [
            (c0_rgb[0] + c1_rgb[0]) / 2,
            (c0_rgb[1] + c1_rgb[1]) / 2,
            (c0_rgb[2] + c1_rgb[2]) / 2,
            255,
        ]
    };
    
    let c3 = if c0 > c1 { [0, 0, 0, 0] } else { c2 };
    
    let palette = [c0_rgb, c1_rgb, c2, c3];
    let mut out = Vec::with_capacity(16 * 4);
    
    for i in 0..16 {
        let idx = (indices >> (2 * i)) & 3;
        out.extend_from_slice(&palette[idx as usize]);
    }
    
    out
}

fn decode_dxt3_block(block: &[u8]) -> Vec<u8> {
    let alpha = u64::from_le_bytes([
        block[0], block[1], block[2], block[3],
        block[4], block[5], block[6], block[7],
    ]);
    
    let color_block = &block[8..16];
    let mut rgb = decode_dxt1_block(color_block);
    
    // Apply DXT3 alpha (4 bits per pixel)
    for i in 0..16 {
        let a = ((alpha >> (4 * i)) & 0xF) as u8;
        rgb[i * 4 + 3] = a * 17; // 4-bit to 8-bit
    }
    
    rgb
}

fn decode_dxt5_block(block: &[u8]) -> Vec<u8> {
    let a0 = block[0];
    let a1 = block[1];
    let bits = u64::from_le_bytes([
        block[2], block[3], block[4], block[5],
        block[6], block[7], 0, 0,
    ]);
    
    let color_block = &block[8..16];
    let mut rgb = decode_dxt1_block(color_block);
    
    // DXT5 alpha interpolation
    let alphas = if a0 > a1 {
        // 8-alpha mode
        let mut a = [0u8; 8];
        a[0] = a0;
        a[1] = a1;
        for i in 1..7 {
            a[i + 1] = ((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7;
        }
        a
    } else {
        // 6-alpha mode
        let mut a = [0u8; 8];
        a[0] = a0;
        a[1] = a1;
        for i in 1..5 {
            a[i + 1] = ((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5;
        }
        a[6] = 0;
        a[7] = 255;
        a
    };
    
    for i in 0..16 {
        let idx = (bits >> (3 * i)) & 7;
        rgb[i * 4 + 3] = alphas[idx as usize];
    }
    
    rgb
}

fn rgb565_to_rgba(c: u16) -> [u8; 4] {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    [
        (r * 255 + 15) / 31,
        (g * 255 + 31) / 63,
        (b * 255 + 15) / 31,
        255,
    ]
}

/// Parse CAFF texture record from .data block
pub fn parse_caff_texture_record(data: &[u8], offset: usize) -> Option<CaffTextureRecord> {
    if offset + 0x80 > data.len() {
        return None;
    }
    
    // Check for "texture\0" signature
    if &data[offset..offset+7] != b"texture" || data[offset+7] != 0 {
        return None;
    }
    
    // Skip version string (variable length, null-terminated)
    let mut pos = offset + 8;
    while pos < data.len() && data[pos] != 0 {
        pos += 1;
    }
    pos += 1; // skip null
    
    if pos + 0x80 > data.len() {
        return None;
    }
    
    // Read texture record fields (big-endian)
    use byteorder::{BigEndian, ReadBytesExt};
    use std::io::Cursor;
    
    let mut cursor = Cursor::new(&data[pos..]);
    
    // Skip to known offsets
    // +0x18 = D3DFORMAT, +0x24 = width, +0x26 = height, +0x30 = mip count
    cursor.set_position(0x18);
    let format = XenosFormat::from_d3dformat(cursor.read_u32::<BigEndian>().ok()?)?;
    
    cursor.set_position(0x24);
    let width = cursor.read_u16::<BigEndian>().ok()?;
    let height = cursor.read_u16::<BigEndian>().ok()?;
    
    cursor.set_position(0x30);
    let mips = cursor.read_u8().ok()?;
    
    Some(CaffTextureRecord {
        format,
        width,
        height,
        mips,
        data_offset: offset,
    })
}

#[derive(Debug, Clone)]
pub struct CaffTextureRecord {
    pub format: XenosFormat,
    pub width: u16,
    pub height: u16,
    pub mips: u8,
    pub data_offset: usize,
}