use anyhow::{bail, ensure, Result};

const MAGIC_BYTES: usize = 6;
const VERSION0_HEADER: usize = 20;
const VERSION1_HEADER: usize = 24;
const ALIGNMENT_AT: usize = 26;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: u32,
    pub start: usize,
    pub length: usize,
    pub field_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strb {
    pub version: u8,
    pub big_endian: bool,
    pub guid: [u8; 16],
    pub blocks: Vec<Block>,
}

fn round_up(n: usize, to: usize) -> usize {
    n.div_ceil(to) * to
}

fn field(d: &[u8], at: usize, size: usize, big: bool) -> Result<usize> {
    ensure!(matches!(size, 1 | 2 | 4 | 8), "unsupported field size {size}");
    let raw = d.get(at..at + size).ok_or_else(|| anyhow::anyhow!("truncated block header at {at:#x}"))?;
    let value = if big { raw.iter().fold(0u64, |v, &b| (v << 8) | u64::from(b)) } else { raw.iter().rev().fold(0u64, |v, &b| (v << 8) | u64::from(b)) };
    Ok(value as usize)
}

pub fn parse(d: &[u8]) -> Result<Strb> {
    ensure!(d.len() > 32 && &d[..4] == b"STRB", "not a Structured Binary");
    let version = d[4];
    let big_endian = d[5] == 0;
    let (header, align) = match version {
        0 => (VERSION0_HEADER, 1),
        1 => (VERSION1_HEADER, usize::from(d[ALIGNMENT_AT])),
        v => bail!("Structured Binary version {v} is too high"),
    };
    ensure!(align.is_power_of_two(), "bad block alignment {align}");
    let (id_size, span) = (usize::from(d[22]), usize::from(d[23]));
    let header_size = round_up(2 * span + id_size, align);
    let mut off = round_up(MAGIC_BYTES + header, align);
    let mut blocks = Vec::new();
    while off + header_size <= d.len() {
        let id = field(d, off, id_size, big_endian)? as u32;
        let length = field(d, off + id_size, span, big_endian)?;
        let field_size = field(d, off + id_size + span, span, big_endian)?;
        let start = off + header_size;
        ensure!(start + length <= d.len(), "block {id} runs past the end of the file");
        blocks.push(Block { id, start, length, field_size });
        off = start + round_up(length, align);
    }
    ensure!(off == d.len(), "blocks end at {off:#x}, file is {:#x} bytes", d.len());
    Ok(Strb { version, big_endian, guid: d[MAGIC_BYTES..MAGIC_BYTES + 16].try_into()?, blocks })
}

impl Strb {
    pub fn block<'a>(&self, d: &'a [u8], id: u32) -> Option<&'a [u8]> {
        self.blocks.iter().find(|b| b.id == id).map(|b| &d[b.start..b.start + b.length])
    }
}
