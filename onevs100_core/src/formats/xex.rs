use anyhow::{anyhow, Result};
use xex2::header::{CompressionType, EncryptionType, OptionalHeaderKey};
use xex2::Xex2;

use crate::timefmt;

const STATIC_LIBRARY_RECORD: usize = 16;
const LENGTH_PREFIX: usize = 4;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn version(v: xenon_types::Version) -> String {
    format!("{v}")
}

fn payload(raw: &[u8]) -> &[u8] {
    raw.get(LENGTH_PREFIX..).unwrap_or_default()
}

fn static_libraries(raw: &[u8]) -> Vec<String> {
    payload(raw)
        .chunks_exact(STATIC_LIBRARY_RECORD)
        .map(|r| {
            let name = String::from_utf8_lossy(&r[..8]).trim_end_matches(['\0', ' ']).to_owned();
            let field = |i: usize| u16::from_be_bytes([r[8 + 2 * i], r[9 + 2 * i]]);
            format!("{name} v{}.{}.{}.{}", field(0), field(1), field(2), field(3) & 0xff)
        })
        .collect()
}

pub fn describe(data: &[u8]) -> Result<Vec<String>> {
    let xex = Xex2::parse(data).map_err(|e| anyhow!("{e}"))?;
    let (h, s) = (&xex.header, &xex.security_info);
    let mut lines = vec![format!("module flags {:?}", h.module_flags), format!("image flags {:?}", s.image_info.image_flags)];
    if let Ok(format) = h.file_format_info() {
        let compression = match format.compression_type {
            CompressionType::None => "Uncompressed",
            CompressionType::Basic => "Compressed (basic)",
            CompressionType::Normal => "Compressed",
            CompressionType::Delta => "Delta patch",
        };
        let encryption = if format.encryption_type == EncryptionType::None { "Not encrypted" } else { "Encrypted" };
        lines.push(format!("compression {compression}"));
        lines.push(format!("encryption {encryption}"));
    }
    if let Some(Ok(name)) = h.get_optional_data(OptionalHeaderKey::OriginalPeName).map(|d| String::from_utf8(payload(d).iter().copied().take_while(|&b| b != 0).collect())) {
        lines.push(format!("original PE name {name}"));
    }
    lines.push(format!("load address {:08X}", s.image_info.load_address.0));
    if let Some(entry) = h.entry_point() {
        lines.push(format!("entry point {entry:08X}"));
    }
    lines.push(format!("image size {:X}", s.image_size));
    if let Some(base) = h.original_base_address() {
        lines.push(format!("original base address {base:08X}"));
    }
    if let Some(stack) = h.default_stack_size() {
        lines.push(format!("default stack size {stack:X}"));
    }
    if let Some(raw) = h.get_optional_data(OptionalHeaderKey::ChecksumTimestamp).filter(|d| d.len() >= 8) {
        let stamp = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]);
        lines.push(format!("checksum {:08X}", u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]])));
        lines.push(format!("timestamp {} UTC ({stamp:08X})", timefmt::unix(i64::from(stamp))));
    }
    if let Some(range) = h.date_range() {
        let show = |t: Option<u64>| t.map_or("none".to_owned(), timefmt::filetime);
        lines.push(format!("valid from {} to {}", show(range.not_before), show(range.not_after)));
    }
    lines.push(format!("game regions {:08X}", s.image_info.game_regions));
    lines.push(format!("allowed media {:?}", s.image_info.allowed_media_types));
    lines.push(format!("media id {}", hex(&s.image_info.media_id)));
    if let Some(key) = h.lan_key() {
        lines.push(format!("LAN key {}", hex(&key)));
    }
    if let Some(path) = h.bounding_path() {
        lines.push(format!("bounding path {path}"));
    }
    if let Some(e) = h.execution_info() {
        lines.extend([
            format!("title id {:08X}", e.title_id.0),
            format!("media id (execution) {:08X}", e.media_id.0),
            format!("version {}", version(e.version)),
            format!("base version {}", version(e.base_version)),
            format!("savegame id {:08X}", e.savegame_id),
            format!("platform {}  executable type {}  disc {}/{}", e.platform, e.executable_table, e.disc_number, e.disc_count),
        ]);
    }
    if let Some(raw) = h.get_optional_data(OptionalHeaderKey::StaticLibraries) {
        lines.push("static libraries".into());
        lines.extend(static_libraries(raw).into_iter().map(|l| format!("  {l}")));
    }
    if let Some(table) = h.import_table() {
        lines.push("import libraries".into());
        lines.extend(table.libraries.iter().map(|l| format!("  {} v{} (min v{})", l.name, version(l.version), version(l.version_min))));
    }
    if let Some(resources) = h.resource_info() {
        lines.push("resources".into());
        lines.extend(resources.resources.iter().map(|r| format!("  {:08X} size {:X} : {}", r.address, r.size, r.name)));
    }
    Ok(lines)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: String,
    pub address: u32,
    pub data: Vec<u8>,
}

const RESOURCE_RECORD: usize = 16;

pub fn resource_table(data: &[u8]) -> Result<Vec<(String, u32, usize)>> {
    let xex = Xex2::parse(data).map_err(|e| anyhow!("{e}"))?;
    let Some(info) = xex.header.get_optional_data(OptionalHeaderKey::ResourceInfo) else { return Ok(Vec::new()) };
    Ok(payload(info)
        .chunks_exact(RESOURCE_RECORD)
        .map(|r| (String::from_utf8_lossy(&r[..8]).trim_end_matches('\0').to_owned(), u32::from_be_bytes(r[8..12].try_into().unwrap()), u32::from_be_bytes(r[12..16].try_into().unwrap()) as usize))
        .collect())
}

pub fn resources(data: &[u8]) -> Result<Vec<Resource>> {
    let table = resource_table(data)?;
    if table.is_empty() {
        return Ok(Vec::new());
    }
    let xex = Xex2::parse(data).map_err(|e| anyhow!("{e}"))?;
    let image = xex.extract_basefile(data).map_err(|e| anyhow!("{e:?}"))?;
    let load = xex.security_info.image_info.load_address.0;
    table
        .into_iter()
        .map(|(name, address, size)| {
            let start = address.wrapping_sub(load) as usize;
            let bytes = image.get(start..start + size).ok_or_else(|| anyhow!("resource {name} lies outside the image"))?;
            Ok(Resource { name, address, data: bytes.to_vec() })
        })
        .collect()
}
