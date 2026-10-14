use crate::{strb, ytgr};

const MAGIC: &[u8] = b"STRB";
const TABLE_START: usize = 0xec;
const ENTRY_SIZE: usize = 0x10c;
const NAME_OFFSETS: usize = 18;
const POINTER_AT: usize = NAME_OFFSETS * 4;

pub struct PackedAsset<'a> {
    pub id: usize,
    pub offset: usize,
    pub asset_type: u32,
    pub data: &'a [u8],
}

fn be32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn entry_count(d: &[u8]) -> usize {
    be32(d, TABLE_START).map_or(0, |first_name| first_name.saturating_sub(TABLE_START) / ENTRY_SIZE)
}

fn load(d: &[u8], id: usize) -> Option<PackedAsset<'_>> {
    let at = TABLE_START + id * ENTRY_SIZE + POINTER_AT;
    let (offset, size) = (be32(d, at)?, be32(d, at + 4)?);
    let data = d.get(offset..offset.checked_add(size)?)?;
    if !data.starts_with(MAGIC) {
        return None;
    }
    let s = strb::parse(data).ok()?;
    let b = s.blocks.iter().find(|b| b.id == 8)?;
    let meta = ytgr::metadata(&data[b.start..b.start + b.length]).ok()?;
    Some(PackedAsset { id, offset, asset_type: meta.asset_type, data })
}

pub fn entry(d: &[u8], id: usize) -> Option<PackedAsset<'_>> {
    load(d, id)
}

pub fn split(d: &[u8]) -> Vec<PackedAsset<'_>> {
    (0..entry_count(d)).filter_map(|id| load(d, id)).collect()
}

pub fn name(d: &[u8], id: usize) -> Option<String> {
    let start = be32(d, TABLE_START + id * ENTRY_SIZE)?;
    let units: Vec<u16> = d.get(start..)?.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
    (!units.is_empty()).then(|| String::from_utf16_lossy(&units))
}

const HIDING_SUFFIX: &str = " Hiding Template";

pub fn hiding_template_for(d: &[u8], id: usize) -> Option<usize> {
    find_by_name(d, &format!("{}{HIDING_SUFFIX}", name(d, id)?))
}

pub fn find_by_name(d: &[u8], wanted: &str) -> Option<usize> {
    (0..entry_count(d)).find(|&id| name(d, id).as_deref() == Some(wanted))
}
