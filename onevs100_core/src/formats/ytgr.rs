use crate::{strb, timefmt};
use anyhow::{ensure, Result};

const HEADER_SIZE: usize = 0x130;
const DIGEST_AT: usize = 0x1c;
const SIGNATURE_AT: usize = 0x30;
const GUID_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ytgr {
    pub version: u32,
    pub header_size: usize,
    pub timestamp: String,
    pub payload_size: usize,
    pub digest: [u8; 20],
    pub signature: Vec<u8>,
    pub asset_id: String,
    pub strb: strb::Strb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub version: u8,
    pub flags: u8,
    pub asset_type: u32,
    pub mask: u32,
    pub id: u32,
    pub extra: u8,
}

fn le32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

pub fn guid_text(b: &[u8]) -> String {
    let hex: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
    [hex[..4].concat(), hex[4..6].concat(), hex[6..8].concat(), hex[8..10].concat(), hex[10..].concat()].join("-")
}

pub fn parse(d: &[u8]) -> Result<Ytgr> {
    ensure!(d.len() >= HEADER_SIZE + GUID_BYTES + 32 && &d[..4] == b"YTGR", "not a YTGR avatar asset");
    let (version, header_size) = (le32(d, 4), le32(d, 8) as usize);
    ensure!(header_size == HEADER_SIZE, "unexpected header size {header_size:#x}");
    let filetime = u64::from(le32(d, 12)) | (u64::from(le32(d, 16)) << 32);
    let payload_size = le32(d, 20) as usize;
    ensure!(payload_size == d.len() - header_size, "header says {payload_size} payload bytes, file has {}", d.len() - header_size);
    let strb = strb::parse(&d[header_size + GUID_BYTES..])?;
    Ok(Ytgr {
        version,
        header_size,
        timestamp: timefmt::filetime(filetime),
        payload_size,
        digest: d[DIGEST_AT..DIGEST_AT + 20].try_into()?,
        signature: d[SIGNATURE_AT..HEADER_SIZE].to_vec(),
        asset_id: guid_text(&d[header_size..header_size + GUID_BYTES]),
        strb,
    })
}

impl Ytgr {
    pub fn block_data<'a>(&self, d: &'a [u8], id: u32) -> Result<&'a [u8]> {
        let base = self.header_size + GUID_BYTES;
        let b = self.strb.blocks.iter().find(|b| b.id == id).ok_or_else(|| anyhow::anyhow!("no block {id}"))?;
        Ok(&d[base + b.start..base + b.start + b.length])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColourTable {
    pub version: u32,
    pub entries: Vec<(u32, u32)>,
}

pub fn colour_table(b: &[u8]) -> Result<ColourTable> {
    ensure!(b.len() >= 4 && (b.len() - 4) % 8 == 0, "colour table block is {} bytes", b.len());
    Ok(ColourTable { version: le32(b, 0), entries: b[4..].chunks_exact(8).map(|e| (le32(e, 0), le32(e, 4))).collect() })
}

pub fn block_kind(id: u32) -> &'static str {
    match id {
        1 => "animation",
        3 => "model",
        4 => "texture",
        7 => "colours",
        6 => "legacy-metadata",
        8 => "metadata",
        _ => "",
    }
}

pub fn metadata(b: &[u8]) -> Result<Metadata> {
    ensure!(b.len() == 15, "metadata block is {} bytes, expected 15", b.len());
    Ok(Metadata { version: b[0], flags: b[1], asset_type: le32(b, 2), mask: le32(b, 6), id: le32(b, 10), extra: b[14] })
}
