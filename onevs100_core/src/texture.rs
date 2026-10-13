use crate::{caff, xenos};
use anyhow::{bail, ensure, Result};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub index: usize,
    pub record_offset: usize,
    pub format: xenos::Format,
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    pub gpu: Option<Range<usize>>,
}

struct Blocks<'a> {
    meta: &'a [u8],
    data: &'a [u8],
    gpu: &'a [u8],
    count: usize,
}

#[derive(Clone, Copy)]
struct Item {
    kind: u8,
    sub: u8,
    id: u32,
    offset: usize,
    len: usize,
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().unwrap())
}

fn blocks(d: &[u8]) -> Result<Blocks<'_>> {
    let c = caff::parse(d)?;
    let at = |i: usize| &d[c.starts[i]..c.starts[i] + c.sizes[i] as usize];
    let (data, gpu) = match c.sizes.len() {
        5 => (3, 2),
        4 => (2, 3),
        n => bail!("no texture layout for {n}-block CAFF"),
    };
    Ok(Blocks { meta: at(0), data: at(data), gpu: at(gpu), count: c.sizes.len() })
}

fn item_at(meta: &[u8], p: usize) -> Option<Item> {
    let raw = meta.get(p..p + 14)?;
    let item = Item { kind: raw[0], sub: raw[1], id: be32(raw, 2), offset: be32(raw, 6) as usize, len: be32(raw, 10) as usize };
    let sane = (1..=4).contains(&item.kind) && (1..=15).contains(&item.sub) && item.id <= 0x400 && item.offset <= 0x400_0000 && item.len <= 0x400_0000;
    sane.then_some(item)
}

fn items(meta: &[u8]) -> Vec<Item> {
    let end = meta.windows(5).rposition(|w| w == b".adb\0").map_or(4, |p| p + 5);
    (end..end + 48)
        .map(|start| (start..).step_by(14).map_while(|p| item_at(meta, p)).collect::<Vec<_>>())
        .fold(Vec::new(), |best, run| if run.len() > best.len() { run } else { best })
}

fn record_offsets(data: &[u8]) -> Vec<usize> {
    let is_record = |w: &[u8]| {
        let version = &w[8..];
        w.starts_with(b"texture\0")
            && version[..2].iter().all(u8::is_ascii_digit)
            && version[2] == b'.'
            && version[3..5].iter().all(u8::is_ascii_digit)
            && version[5] == b'.'
            && version[6..8].iter().all(u8::is_ascii_digit)
            && version[8] == b'.'
            && version[9..13].iter().all(u8::is_ascii_digit)
            && version[13] == 0
    };
    (0..data.len().saturating_sub(22)).filter(|&i| is_record(&data[i..i + 22])).collect()
}

pub fn list(d: &[u8]) -> Result<Vec<Texture>> {
    let b = blocks(d)?;
    let table = items(b.meta);
    let records = record_offsets(b.data);
    let mut out = Vec::new();
    for (index, &r) in records.iter().enumerate() {
        let format = xenos::Format::from_d3d(be32(b.data, r + 0x18))?;
        let width = u16::from_be_bytes([b.data[r + 0x24], b.data[r + 0x25]]) as u32;
        let height = u16::from_be_bytes([b.data[r + 0x26], b.data[r + 0x27]]) as u32;
        let id = table.iter().find(|i| i.len == 0x70 && i.offset == r).map(|i| i.id);
        let chunk = id.and_then(|id| {
            table
                .iter()
                .filter(|i| i.id == id && (i.kind, i.sub) == (2, 5) && i.len >= 0x100 && i.offset + i.len <= b.gpu.len())
                .last()
                .map(|i| i.offset..i.offset + i.len)
        });
        let whole_font = (records.len() == 1 && b.count == 4).then(|| 0..b.gpu.len());
        out.push(Texture { index, record_offset: r, format, width, height, mips: b.data[r + 0x30] as u32, gpu: chunk.or(whole_font) });
    }
    Ok(out)
}

pub fn decode(d: &[u8], t: &Texture, level: u32) -> Result<Vec<xenos::Rgba>> {
    let range = t.gpu.clone().ok_or_else(|| anyhow::anyhow!("texture {} is external", t.index))?;
    let (levels, _) = xenos::mip_layout(t.width, t.height, t.format, t.mips);
    let lvl = levels.get(level as usize).ok_or_else(|| anyhow::anyhow!("no mip level {level}"))?;
    let gpu = blocks(d)?.gpu;
    ensure!(range.end <= gpu.len(), "texture chunk outside .gpu block");
    Ok(xenos::decode_level(&gpu[range], lvl, t.format, true))
}

pub fn level_size(t: &Texture, level: u32) -> (u32, u32) {
    ((t.width >> level).max(1), (t.height >> level).max(1))
}

pub fn to_png(d: &[u8], t: &Texture, level: u32) -> Result<Vec<u8>> {
    let (w, h) = level_size(t, level);
    let img = image::RgbaImage::from_raw(w, h, decode(d, t, level)?.concat()).ok_or_else(|| anyhow::anyhow!("bad pixel buffer"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}
