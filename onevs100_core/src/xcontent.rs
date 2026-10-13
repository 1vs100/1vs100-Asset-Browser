use anyhow::Result;
use crate::timefmt;
use std::collections::BTreeSet;
use stfs::{BytesStfsReader, StfsPackageReader};

pub struct PackageDetails {
    pub lines: Vec<String>,
    pub thumbnail: Vec<u8>,
    pub title_image: Vec<u8>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

pub fn parse(data: &[u8]) -> Result<PackageDetails> {
    let reader = BytesStfsReader::open(data)?;
    let h = &reader.package().header;
    let files = crate::stfs::list(data)?;
    let entries = reader.package().file_table.walk_files();
    let stamps = |pick: fn(&stfs::StfsFileEntry) -> usize| -> String {
        let distinct: BTreeSet<u32> = entries.iter().map(|e| pick(&e.entry) as u32).collect();
        match (distinct.len(), distinct.first()) {
            (1, Some(&only)) => format!("{} (all {} files)", timefmt::fat(only), entries.len()),
            (_, Some(&first)) => format!("{} to {}", timefmt::fat(first), timefmt::fat(*distinct.last().unwrap_or(&first))),
            _ => "none".into(),
        }
    };
    let mut lines = vec![
        format!("signature {}", String::from_utf8_lossy(&data[..4]).trim_end()),
        format!("content type {:?}", h.content_type),
        format!("title id {:08X}", h.title_id.0),
        format!("title name {}", h.title_name),
        format!("display name {}", h.display_name),
        format!("description {}", h.display_description),
        format!("publisher {}", h.publisher_name),
        format!("media id {:08X}", h.media_id.0),
        format!("version {}  base version {}", h.version, h.base_version),
        format!("platform {}  executable type {}  disc {}/{}", h.platform, h.executable_type, h.disc_number, h.disc_in_set),
        format!("savegame id {:08X}", h.savegame_id.0),
        format!("console id {}", hex(&h.console_id.0)),
        format!("profile id {}", hex(&h.profile_id.0)),
        format!("content size {} bytes", h.content_size),
        format!("metadata version {}  filesystem {:?}", h.metadata_version, h.filesystem_type),
        format!("thumbnail {} bytes  title image {} bytes", h.thumbnail_image.len(), h.title_image.len()),
        format!("{} files, {} bytes of file data", files.len(), files.iter().map(|f| f.size).sum::<u64>()),
    ];
    lines.push(format!("files created {}", stamps(|e| e.created_time_stamp as usize)));
    lines.push(format!("files accessed {}", stamps(|e| e.access_time_stamp as usize)));
    Ok(PackageDetails { lines, thumbnail: h.thumbnail_image.clone(), title_image: h.title_image.clone() })
}
