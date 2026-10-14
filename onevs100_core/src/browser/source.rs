use crate::{stfs, toc, xdbf, xex, xui, ytgr};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Payload {
    Disk(PathBuf),
    Bytes(Vec<u8>),
    Stfs { package: Arc<Vec<u8>>, path: String },
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub payload: Payload,
}

impl Payload {
    pub fn read(&self) -> Result<Vec<u8>> {
        match self {
            Payload::Disk(path) => Ok(std::fs::read(path)?),
            Payload::Bytes(data) => Ok(data.clone()),
            Payload::Stfs { package, path } => stfs::extract(package, path),
        }
    }
}

pub fn read_dir(dir: &Path) -> Result<Vec<Entry>> {
    let mut entries: Vec<Entry> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| Entry { name: e.file_name().to_string_lossy().into_owned(), is_dir: e.path().is_dir(), payload: Payload::Disk(e.path()) })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

pub fn children(entry: &Entry) -> Result<Option<Vec<Entry>>> {
    if let (true, Payload::Disk(path)) = (entry.is_dir, &entry.payload) {
        return read_dir(path).map(Some);
    }
    expand_named(&entry.name, &entry.payload.read()?)
}

pub fn expand_named(name: &str, data: &[u8]) -> Result<Option<Vec<Entry>>> {
    if name.to_lowercase().ends_with(".toc") {
        let entries = toc::split(data).iter().map(|a| Entry { name: format!("{:04} {}.type{}.avatarasset", a.id, toc::name(data, a.id).unwrap_or_default().replace(['/', '\\'], "-"), a.asset_type), is_dir: false, payload: Payload::Bytes(a.data.to_vec()) }).collect::<Vec<_>>();
        if !entries.is_empty() {
            return Ok(Some(entries));
        }
    }
    expand(data)
}

pub fn expand(data: &[u8]) -> Result<Option<Vec<Entry>>> {
    match data.get(..4) {
        Some(b"LIVE" | b"PIRS" | b"CON ") => {
            let package = Arc::new(data.to_vec());
            let files = stfs::list(&package)?;
            Ok(Some(files.into_iter().map(|f| Entry { name: f.path.clone(), is_dir: false, payload: Payload::Stfs { package: package.clone(), path: f.path } }).collect()))
        }
        Some(b"XEX2") => {
            let mut seen = std::collections::HashMap::<String, usize>::new();
            let entries = xex::resources(data)?
                .into_iter()
                .map(|r| {
                    let extension = match r.data.get(..4) {
                        Some(b"XUIZ") => "xzp",
                        Some(b"XDBF") => "xdbf",
                        _ => "bin",
                    };
                    let base = format!("{}.{extension}", r.name);
                    let count = seen.entry(base.clone()).or_default();
                    *count += 1;
                    let name = if *count == 1 { base } else { format!("{}_{:08x}.{extension}", r.name, r.address) };
                    Entry { name, is_dir: false, payload: Payload::Bytes(r.data) }
                })
                .collect();
            Ok(Some(entries))
        }
        Some(b"XDBF") => {
            let a = xdbf::parse(data)?;
            Ok(Some(a.entries.iter().map(|e| xdbf::entry_name(&a, data, e)).map(|(name, bytes)| Entry { name, is_dir: false, payload: Payload::Bytes(bytes) }).collect()))
        }
        Some(b"YTGR") => {
            let y = ytgr::parse(data)?;
            Ok(Some(
                y.strb
                    .blocks
                    .iter()
                    .map(|b| {
                        let kind = ytgr::block_kind(b.id);
                        let name = format!("block{}.{}", b.id, if kind.is_empty() { format!("block{}", b.id) } else { kind.to_owned() });
                        Entry { name, is_dir: false, payload: Payload::Bytes(y.block_data(data, b.id).map(<[u8]>::to_vec).unwrap_or_default()) }
                    })
                    .collect(),
            ))
        }
        Some(b"XUIZ") => Ok(Some(xui::read_xuiz(data)?.into_iter().map(|e| Entry { name: e.name, is_dir: false, payload: Payload::Bytes(e.data) }).collect())),
        _ => Ok(None),
    }
}
