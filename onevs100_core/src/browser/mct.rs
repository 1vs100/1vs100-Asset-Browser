//! Xbox 360 MCT texture, as shipped by 1 vs 100 (port of the title's `XGMCTDecompressTexture` path).
//!
//! Layout (big-endian): `.mct`, 24-byte packed texture description (dword2 == dword5 holds
//! (width-1) in bits 0..12 and (height-1) in bits 13..25), then four `u32`-length-prefixed planes.
//! Each plane is an XMemCompress LZX stream (512 KiB window; frames `0xFF us:u16 cs:u16` or
//! `cs:u16` with us = 0x8000; `cs == 0` ends). The planes de-interleave 16-byte DXT5 blocks
//! (GPU order, 16-bit big-endian words): bytes 0..4, 4..8, 8..10, 10..16.
//! Verified bit-exact against the title decoder (run under emulation) on all 202 distinct MCTs.

use crate::inspect::Preview;
use anyhow::{anyhow, bail, ensure, Result};
use lzxd::{Lzxd, WindowSize};
use texpresso::Format;

const MAGIC: &[u8; 4] = b".mct";
const PLANE_SPLIT: [(usize, usize); 4] = [(0, 4), (4, 8), (8, 10), (10, 16)];

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn parse(data: &[u8]) -> Result<(u32, u32, [&[u8]; 4])> {
    ensure!(data.len() >= 28 && data.starts_with(MAGIC), "not .mct");
    ensure!(be32(data, 12) == be32(data, 24), "MCT header dword2 != dword5");
    let (w, h) = ((be32(data, 12) & 0x1fff) + 1, ((be32(data, 12) >> 13) & 0x1fff) + 1);
    let mut off = 28;
    let mut planes = [&data[..0]; 4];
    for plane in &mut planes {
        ensure!(off + 4 <= data.len(), "truncated MCT plane length");
        let n = be32(data, off) as usize;
        off += 4;
        ensure!(n <= data.len() - off, "truncated MCT plane");
        *plane = &data[off..off + n];
        off += n;
    }
    ensure!(off == data.len(), "trailing bytes after MCT planes");
    Ok((w, h, planes))
}

/// Inflate one XMemCompress LZX stream; it must yield exactly `want` bytes.
fn unpack_plane(data: &[u8], want: usize) -> Result<Vec<u8>> {
    let mut lzx = Lzxd::new(WindowSize::KB512);
    let (mut at, mut out) = (0usize, Vec::with_capacity(want));
    loop {
        let (us, cs, hdr) = match data.get(at..) {
            Some([0xff, a, b, c, d, ..]) => (u16::from_be_bytes([*a, *b]) as usize, u16::from_be_bytes([*c, *d]) as usize, 5),
            Some([a, b, ..]) => (0x8000, u16::from_be_bytes([*a, *b]) as usize, 2),
            _ => bail!("truncated LZX frame header"),
        };
        at += hdr;
        if cs == 0 {
            break;
        }
        let frame = data.get(at..at + cs).ok_or_else(|| anyhow!("truncated LZX frame"))?;
        ensure!(out.len() + us <= want, "MCT plane larger than texture");
        out.extend_from_slice(lzx.decompress_next(frame, us).map_err(|e| anyhow!("LZX decode failed: {e}"))?);
        at += cs;
    }
    ensure!(out.len() == want, "MCT plane has {} bytes, texture needs {want}", out.len());
    Ok(out)
}

/// Packed DXT5 blocks (`bw * bh * 16`, GPU byte order), plus width and height in pixels.
pub fn dxt5(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let (w, h, planes) = parse(data)?;
    let blocks = (w.div_ceil(4) * h.div_ceil(4)) as usize;
    let mut out = vec![0u8; blocks * 16];
    for (i, plane) in planes.iter().enumerate() {
        let (a, b) = PLANE_SPLIT[i];
        let raw = unpack_plane(plane, blocks * (b - a)).map_err(|e| anyhow!("MCT plane {i}: {e}"))?;
        for (block, src) in out.chunks_exact_mut(16).zip(raw.chunks_exact(b - a)) {
            block[a..b].copy_from_slice(src);
        }
    }
    Ok((w, h, out))
}

pub fn decode(data: &[u8]) -> Result<Preview> {
    let (width, height, mut dxt) = dxt5(data)?;
    for word in dxt.chunks_exact_mut(2) {
        word.swap(0, 1);
    }
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    Format::Bc3.decompress(&dxt, width as usize, height as usize, &mut rgba);
    Ok(Preview { width, height, rgba })
}
