use anyhow::{bail, ensure, Result};
use std::io::Read;

const MAGIC: &[u8] = b"XDBF";
const VERSION: u32 = 0x10000;
const HEADER_BYTES: usize = 24;
const ENTRY_BYTES: usize = 18;
const FREE_BYTES: usize = 8;
const STRING_MAGIC: &[u8] = b"XSTR";
const STRING_HEADER_BYTES: usize = 14;
const SOURCE_MAGIC: &[u8] = b"XSRC";
const NAMESPACE_METADATA: u16 = 1;
const NAMESPACE_IMAGE: u16 = 2;
const NAMESPACE_STRINGS: u16 = 3;
const TITLE_HEADER_ID: u64 = 0x5854_4844;
const SOURCE_ID: u64 = 0x5853_5243;
const ICON_ID: u64 = 0x8000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub namespace: u16,
    pub id: u64,
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xdbf {
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title {
    pub title_id: u32,
    pub version: (u16, u16, u16, u16),
}

fn be32(d: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_be_bytes(d.get(at..at + 4).ok_or_else(|| anyhow::anyhow!("truncated at {at:#x}"))?.try_into()?) as usize)
}

fn be16(d: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(d.get(at..at + 2).ok_or_else(|| anyhow::anyhow!("truncated at {at:#x}"))?.try_into()?))
}

pub fn parse(d: &[u8]) -> Result<Xdbf> {
    ensure!(d.starts_with(MAGIC), "not an XDBF archive");
    ensure!(be32(d, 4)? == VERSION as usize, "unsupported XDBF version");
    let (table_len, count, free_len) = (be32(d, 8)?, be32(d, 12)?, be32(d, 16)?);
    ensure!(count <= table_len && table_len <= d.len() && free_len <= d.len(), "implausible XDBF table sizes");
    let data = HEADER_BYTES + table_len * ENTRY_BYTES + free_len * FREE_BYTES;
    ensure!(data <= d.len(), "XDBF tables run past the end of the file");
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let at = HEADER_BYTES + i * ENTRY_BYTES;
        let id = u64::from_be_bytes(d[at + 2..at + 10].try_into()?);
        let (start, len) = (data + be32(d, at + 10)?, be32(d, at + 14)?);
        ensure!(start + len <= d.len(), "XDBF entry {i} runs past the end of the file");
        entries.push(Entry { namespace: be16(d, at)?, id, start, len });
    }
    Ok(Xdbf { entries })
}

impl Xdbf {
    pub fn data<'a>(&self, d: &'a [u8], e: &Entry) -> &'a [u8] {
        &d[e.start..e.start + e.len]
    }

    pub fn find(&self, namespace: u16, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.namespace == namespace && e.id == id)
    }

    pub fn title(&self, d: &[u8]) -> Result<Title> {
        let Some(e) = self.find(NAMESPACE_METADATA, TITLE_HEADER_ID) else { bail!("no title header") };
        let t = self.data(d, e);
        Ok(Title { title_id: be32(t, 12)? as u32, version: (be16(t, 20)?, be16(t, 22)?, be16(t, 24)?, be16(t, 26)?) })
    }
}

pub fn strings(d: &[u8]) -> Result<Vec<(u16, String)>> {
    ensure!(d.starts_with(STRING_MAGIC), "not an XDBF string table");
    let count = be16(d, STRING_HEADER_BYTES - 2)? as usize;
    let mut at = STRING_HEADER_BYTES;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let (id, len) = (be16(d, at)?, usize::from(be16(d, at + 2)?));
        let text = d.get(at + 4..at + 4 + len).ok_or_else(|| anyhow::anyhow!("string runs past the end of the table"))?;
        out.push((id, String::from_utf8_lossy(text).into_owned()));
        at += 4 + len;
    }
    Ok(out)
}

pub fn xlast(d: &[u8]) -> Result<(String, Vec<u8>)> {
    ensure!(d.starts_with(SOURCE_MAGIC), "not an XLAST source entry");
    let name_len = be32(d, 12)?;
    let name = String::from_utf8_lossy(d.get(16..16 + name_len).ok_or_else(|| anyhow::anyhow!("truncated source name"))?).into_owned();
    let packed = d.get(16 + name_len + 8..).ok_or_else(|| anyhow::anyhow!("truncated source data"))?;
    let mut xml = Vec::new();
    flate2::read::GzDecoder::new(packed).read_to_end(&mut xml)?;
    Ok((name, xml))
}

pub fn entry_name(a: &Xdbf, d: &[u8], e: &Entry) -> (String, Vec<u8>) {
    let data = a.data(d, e);
    match e.namespace {
        NAMESPACE_IMAGE if e.id == ICON_ID => ("icon.png".to_owned(), data.to_vec()),
        NAMESPACE_IMAGE => (format!("image_{:04x}.png", e.id), data.to_vec()),
        NAMESPACE_STRINGS => (format!("strings_lang{}.xstr", e.id), data.to_vec()),
        NAMESPACE_METADATA if e.id == SOURCE_ID => match xlast(data) {
            Ok((name, xml)) => (name, xml),
            Err(_) => ("XSRC.bin".to_owned(), data.to_vec()),
        },
        NAMESPACE_METADATA => (format!("{}.bin", String::from_utf8_lossy(&e.id.to_be_bytes()[4..]).trim_matches('\0')), data.to_vec()),
        namespace => (format!("ns{namespace}_{:x}.bin", e.id), data.to_vec()),
    }
}
