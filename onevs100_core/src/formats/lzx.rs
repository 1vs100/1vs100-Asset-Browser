use anyhow::{ensure, Result};
use lzxd::{Lzxd, WindowSize};

const HEADER_BYTES: usize = 12;
const MAX_COMPRESSED: u32 = 0x9800;
const MAX_UNCOMPRESSED: u32 = 0x8000;
const MAX_TRAILER: usize = 64;

fn le32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

pub fn chain(data: &[u8]) -> Result<Vec<(u32, u32, u32)>> {
    ensure!(!data.is_empty(), "empty compressed stream");
    let (mut at, mut start, mut out) = (0usize, 0u32, Vec::new());
    while at + HEADER_BYTES <= data.len() {
        let (compressed, block_start, size) = (le32(data, at), le32(data, at + 4), le32(data, at + 8));
        let plausible = compressed <= MAX_COMPRESSED && size <= MAX_UNCOMPRESSED && size > 0 && block_start == start;
        if !plausible && !out.is_empty() && data.len() - at <= MAX_TRAILER {
            break;
        }
        ensure!(compressed <= MAX_COMPRESSED && size <= MAX_UNCOMPRESSED && size > 0, "implausible block sizes {compressed}/{size}");
        ensure!(block_start == start, "block starts at {block_start}, expected {start}");
        ensure!(at + HEADER_BYTES + compressed as usize <= data.len(), "block runs past the end of the stream");
        out.push((compressed, block_start, size));
        at += HEADER_BYTES + compressed as usize;
        start += size;
    }
    ensure!(data.len() - at <= MAX_TRAILER, "unexpected data after the last block at {at:#x}");
    Ok(out)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let blocks = chain(data)?;
    let (mut at, mut out) = (0usize, Vec::new());
    for (compressed, _, size) in blocks {
        let mut lzx = Lzxd::new(WindowSize::KB32);
        let chunk = &data[at + HEADER_BYTES..at + HEADER_BYTES + compressed as usize];
        let produced = lzx.decompress_next(chunk, size as usize).map_err(|e| anyhow::anyhow!("LZX decode failed: {e}"))?;
        out.extend_from_slice(produced);
        at += HEADER_BYTES + compressed as usize;
    }
    Ok(out)
}
