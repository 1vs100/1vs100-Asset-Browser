use crate::anim;
use crate::inspect::{Detail, Preview};
use anyhow::{ensure, Result};

pub const AVD_SIZE: usize = 1000;
pub(crate) const GUID_SUFFIX: [u8; 8] = [0xc1, 0xc8, 0xf1, 0x09, 0xa1, 0x9c, 0xb2, 0xe0];
const SLOT_TABLE: std::ops::Range<usize> = 0x10..0xe0;
const SLOT_SIZE: usize = 16;
const COLOUR_TABLE: std::ops::Range<usize> = 0xfc..0x120;
const COLOUR_RECORDS: std::ops::Range<usize> = 0x120..0x380;
const WORN_RECORDS: std::ops::Range<usize> = 0x120..0x300;
const COLOUR_RECORD_SIZE: usize = 32;
const TRAILER: std::ops::Range<usize> = 0x380..0x390;
const SWATCH: u32 = 48;
const STRB_HEADER_LEN: usize = 35;
const FXL_HEADER_LEN: usize = 12;

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(d[o..o + 4].try_into().expect("slice length checked by caller"))
}

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn swatches(colours: &[u32]) -> Preview {
    let width = SWATCH * colours.len() as u32;
    let mut rgba = Vec::with_capacity((width * SWATCH * 4) as usize);
    for _ in 0..SWATCH {
        for c in colours {
            let pixel = [(c >> 16) as u8, (c >> 8) as u8, *c as u8, 255];
            (0..SWATCH).for_each(|_| rgba.extend_from_slice(&pixel));
        }
    }
    Preview { width, height: SWATCH, rgba }
}

pub fn describe_avd(d: &[u8]) -> Result<Detail> {
    describe_avd_with(d, crate::body::pack_bytes())
}

const MODEL_TYPE: u32 = 1;

pub(crate) fn worn_ids(d: &[u8]) -> Vec<usize> {
    let mut ids: Vec<usize> = WORN_RECORDS.step_by(COLOUR_RECORD_SIZE).filter(|&o| d[o..o + COLOUR_RECORD_SIZE].iter().any(|&b| b != 0)).map(|o| usize::from(be16(d, o + 4))).collect();
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

pub(crate) fn record_colours(d: &[u8], id: usize) -> Option<Vec<u32>> {
    WORN_RECORDS
        .step_by(COLOUR_RECORD_SIZE)
        .find(|&o| usize::from(be16(d, o + 4)) == id)
        .map(|o| (0..3).map(|i| be32(d, o + 20 + 4 * i)).filter(|&c| c != 0).collect::<Vec<_>>())
        .filter(|c| !c.is_empty())
}

pub fn models_for(d: &[u8], pack: &[u8]) -> Result<Vec<crate::model::Model>> {
    ensure!(d.len() == AVD_SIZE, "AvatarMetadata blobs are {AVD_SIZE} bytes");
    Ok(worn_ids(d).into_iter().filter_map(|id| crate::toc::entry(pack, id).filter(|a| a.asset_type == MODEL_TYPE)).filter_map(|a| crate::body::decode(a.data).ok()).collect())
}

pub fn describe_avd_with(d: &[u8], pack: Option<&[u8]>) -> Result<Detail> {
    ensure!(d.len() == AVD_SIZE, "AvatarMetadata blobs are {AVD_SIZE} bytes");
    let mut lines = vec![
        format!("AvatarMetadata blob, {} bytes (layout inferred from the distinct blobs in the game files)", d.len()),
        format!("header  value1 {}  value2 {}  flags {:#010x}", f32::from_bits(be32(d, 4)), f32::from_bits(be32(d, 8)), be32(d, 12)),
        "asset slots (id, kind, 32-bit slot mask)".into(),
    ];
    for o in SLOT_TABLE.step_by(SLOT_SIZE).filter(|&o| d[o..o + SLOT_SIZE].iter().any(|&b| b != 0)) {
        let mask = be32(d, o + 12);
        if d[o + 4..o + 12] == GUID_SUFFIX {
            lines.push(format!("  @{o:#x}  asset id {}  kind {}  mask {mask:#010x}", be16(d, o), be16(d, o + 2)));
        } else {
            lines.push(format!("  @{o:#x}  empty slot  mask {mask:#010x}"));
        }
    }
    let colours: Vec<u32> = COLOUR_TABLE.step_by(4).map(|o| be32(d, o)).collect();
    lines.push("colours (ARGB)".into());
    lines.extend(colours.iter().enumerate().map(|(i, c)| format!("colour {i}  {c:08X}")));
    lines.push("coloured assets (flags, id, kind, mask, up to 3 colours)".into());
    for o in COLOUR_RECORDS.step_by(COLOUR_RECORD_SIZE).filter(|&o| d[o..o + COLOUR_RECORD_SIZE].iter().any(|&b| b != 0)) {
        let tints: Vec<String> = (0..3).map(|i| be32(d, o + 20 + 4 * i)).filter(|&c| c != 0).map(|c| format!("{c:08X}")).collect();
        let tint = if tints.is_empty() { String::new() } else { format!("  colours {}", tints.join(" ")) };
        lines.push(format!("  @{o:#x}  asset id {}  kind {}  flags {:#010x}  mask {:#010x}{tint}", be16(d, o + 4), be16(d, o + 6), be32(d, o), be32(d, o + 16)));
    }
    if d[TRAILER].iter().any(|&b| b != 0) {
        let trailer = &d[TRAILER.start..TRAILER.start + 13];
        lines.push(format!("trailer @{:#x}  {}", TRAILER.start, hex(trailer).to_lowercase()));
    }
    let mut image = swatches(&colours);
    let mut scene = None;
    let mut figure = None;
    if let Some(models) = pack.and_then(|p| models_for(d, p).ok()).filter(|m| !m.is_empty()) {
        lines.push(format!("rendered {} of {} assets from the avatar pack", models.len(), worn_ids(d).len()));
        if let Ok(f) = crate::figure::build(d, pack.unwrap_or_default()) {
            let s = f.standing(pack.unwrap_or_default()).unwrap_or_else(|_| f.clone()).scene();
            image = s.render(&crate::render::Camera::default(), 384, 384);
            scene = Some(crate::render::SceneHandle(std::sync::Arc::new(s)));
            figure = Some(crate::figure::FigureHandle(std::sync::Arc::new(f)));
        }
    }
    Ok(Detail { kind: "Avatar metadata (AVD)", lines, image: Some(image), text: None, pending_preview: false, scene, figure })
}

fn animation_lines(a: &anim::Animation) -> Vec<String> {
    let h = &a.header;
    let first = a.pose(0).ok().and_then(|p| p.first().map(|j| j.position));
    let animated = a.animated_joints();
    let mut lines = vec![
        format!("animation {} frames at {} fps ({:.2} s)", h.frames, h.fps, a.duration()),
        format!("{} joints, carryable joints {}", h.joints, h.carryable_joints),
        format!("motion channels {}, texture channels {}", h.motion_channels, h.texture_channels),
    ];
    if let Some([x, y, z]) = first {
        lines.push(format!("root position at frame 0: ({x:.4}, {y:.4}, {z:.4})"));
    }
    lines.push(format!("animated joints {} of {} (bits per frame: position, rotation, scale)", animated.len(), h.joints));
    lines.extend(animated.iter().map(|(i, [p, r, s])| format!("  {:<14} {p} {r} {s}", anim::JOINT_NAMES.get(*i).copied().unwrap_or("?"))));
    lines.extend(a.texture_ranges().iter().enumerate().map(|(i, (lo, hi))| format!("texture channel {i} values {lo}..{hi}")));
    if a.extra_bytes > 0 {
        lines.push(format!("extra data after the streams: {} bytes (older builds)", a.extra_bytes));
    }
    lines
}

pub fn describe_animation_block(block: &[u8]) -> Detail {
    match anim::parse_block(block) {
        Ok(a) => Detail { kind: "Avatar animation (STRB)", lines: animation_lines(&a), image: None, text: None, pending_preview: false, scene: None, figure: None },
        Err(e) => Detail { kind: "Avatar asset block", lines: vec![format!("animation payload not decoded: {e}")], image: None, text: None, pending_preview: false, scene: None, figure: None },
    }
}

pub fn describe_strb(d: &[u8]) -> Result<Detail> {
    ensure!(d.len() > 0x20 && &d[..4] == b"STRB", "not an STRB asset");
    let layout = d[4];
    let mut lines = vec![
        format!("STRB resource, {} bytes", d.len()),
        format!("layout flag {layout}"),
        format!("format id {:#010x}", be16(d, 6)),
        format!("asset guid {}", hex(&d[8..24])),
    ];
    if layout == 0 {
        lines.push(format!("payload length {} (file size - {STRB_HEADER_LEN})", d.len() - STRB_HEADER_LEN));
        if usize::from(be16(d, 0x1d)) != d.len() - STRB_HEADER_LEN {
            lines.push("note: length field does not match the layout seen in the game files".into());
        }
    }
    let kind = match anim::parse(d) {
        Ok(a) => {
            lines.extend(animation_lines(&a));
            "Avatar animation (STRB)"
        }
        Err(e) => {
            lines.push(format!("animation payload not decoded: {e}"));
            "Avatar asset (STRB)"
        }
    };
    lines.extend(hex_rows(&d[..0x40]));
    Ok(Detail { kind, lines, image: None, text: None, pending_preview: false, scene: None, figure: None })
}

fn hex_rows(d: &[u8]) -> Vec<String> {
    d.chunks(16).enumerate().map(|(i, c)| format!("{:04x}  {}", i * 16, c.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "))).collect()
}

fn ascii_runs(d: &[u8]) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = Vec::new();
    for &b in d.iter().chain(std::iter::once(&0)) {
        if (0x20..0x7f).contains(&b) {
            current.push(b);
        } else {
            if current.len() >= 4 {
                runs.push(String::from_utf8_lossy(&current).into_owned());
            }
            current.clear();
        }
    }
    runs
}

fn is_version(s: &str) -> bool {
    s.split('.').count() == 4 && s.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn constant(s: &str) -> Option<&str> {
    let start = s.find("c_")?;
    let name = &s[start..];
    name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_').then_some(name)
}

fn source_path(s: &str) -> Option<&str> {
    let at = s.find(":\\")?;
    at.checked_sub(1).map(|i| &s[i..])
}

pub fn describe_fxl(d: &[u8]) -> Result<Detail> {
    ensure!(d.len() > FXL_HEADER_LEN && d[..4] == [0xa3, 0xd7, 0x01, 0x41], "not an FXL effect object");
    let runs = ascii_runs(d);
    let count = |p: &str| runs.iter().filter(|s| s.as_str() == p).count();
    let mut lines = vec![
        format!("Xbox 360 effect object, {} bytes", d.len()),
        format!("version word {}", hex(&d[..4])),
        format!("payload bytes {} (file size - {FXL_HEADER_LEN})", be32(d, 4) as usize),
        format!("second section {} bytes", be32(d, 8)),
    ];
    let mut versions: Vec<&String> = runs.iter().filter(|s| is_version(s)).collect();
    versions.sort();
    versions.dedup();
    lines.extend(versions.iter().map(|v| format!("compiler {v}")));
    for profile in ["vs_3_0", "ps_3_0"] {
        if count(profile) > 0 {
            lines.push(format!("{profile} x{}", count(profile)));
        }
    }
    let mut constants: Vec<&str> = runs.iter().filter_map(|s| constant(s)).collect();
    constants.sort();
    constants.dedup();
    lines.extend(constants.iter().map(|c| format!("constant {c}")));
    let mut sources: Vec<&str> = runs.iter().filter_map(|s| source_path(s)).collect();
    sources.sort();
    sources.dedup();
    lines.extend(sources.iter().map(|s| format!("debug database {s}")));
    Ok(Detail { kind: "Effect object (FXL)", lines, image: None, text: None, pending_preview: false, scene: None, figure: None })
}
