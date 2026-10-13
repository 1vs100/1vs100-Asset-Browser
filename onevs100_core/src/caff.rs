use anyhow::{ensure, Result};

pub const HDR: usize = 0x78;

#[derive(Debug, Clone)]
pub struct CaffFile {
    pub sizes: Vec<u32>,
    pub starts: Vec<usize>,
    pub relocs: Option<Vec<u32>>,
    pub reloc_pos: Option<usize>,
    pub base: Option<usize>,
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap())
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

pub fn block_sizes(d: &[u8]) -> Vec<u32> {
    let head = &d[..d.len().min(0x400)];
    let end = [&b".data\0"[..], b".gpu\0", b".stream\0"]
        .iter()
        .filter_map(|n| find(head, n))
        .min()
        .unwrap_or(0x100);
    let mut out = Vec::new();
    let mut i = 0x48;
    while i + 8 < end {
        let s = be32(d, i);
        if s != 0 {
            for z in [12, 16] {
                let j = i + 4 + z;
                if j + 4 <= d.len() && d[i + 4..j].iter().all(|&b| b == 0) && be32(d, j) == s {
                    out.push(s);
                    i = j + 3;
                    break;
                }
            }
        }
        i += 1;
    }
    if let Some(last) = out.len().checked_sub(1) {
        let rest: u32 = out[..last].iter().sum();
        out[last] = (d.len() - HDR) as u32 - rest;
    }
    out
}

pub fn parse(d: &[u8]) -> Result<CaffFile> {
    ensure!(d.len() > HDR && &d[..4] == b"CAFF", "not a CAFF file");
    let sizes = block_sizes(d);
    ensure!(sizes.len() >= 2, "CAFF has fewer than two blocks");
    let mut starts = Vec::with_capacity(sizes.len());
    let mut o = HDR;
    for s in &sizes {
        starts.push(o);
        o += *s as usize;
    }
    let n = be32(d, 0x40) as usize;
    let (mut relocs, mut reloc_pos) = (None, None);
    let hi = (starts[1] + sizes[1] as usize).saturating_sub(4 * n + 3);
    for t in starts[1]..hi.min(d.len().saturating_sub(4 * n + 4)) {
        if be32(d, t) as usize != n {
            continue;
        }
        let r: Vec<u32> = (0..n).map(|i| be32(d, t + 4 + 4 * i)).collect();
        if r.windows(2).all(|w| w[0] < w[1]) {
            relocs = Some(r);
            reloc_pos = Some(t);
            break;
        }
    }
    let base = (starts.len() == 3).then(|| starts[2]);
    Ok(CaffFile { sizes, starts, relocs, reloc_pos, base })
}
