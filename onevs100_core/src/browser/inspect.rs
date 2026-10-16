use crate::{avatar, body, caff, lzx, model, render, strb, texture, video, xcontent, xdbf, xex, xsb, xui, xur, xwb, ytgr};
use anyhow::{anyhow, ensure, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBody {
    pub content: String,
    pub syntax: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    pub kind: &'static str,
    pub lines: Vec<String>,
    pub image: Option<Preview>,
    pub text: Option<TextBody>,
    pub pending_preview: bool,
    pub scene: Option<render::SceneHandle>,
    pub figure: Option<crate::figure::FigureHandle>,
}

const HEX_ROWS: usize = 16;
const ASF_MAGIC: [u8; 4] = [0x30, 0x26, 0xb2, 0x75];
pub fn preview_png(p: &Preview) -> Result<Vec<u8>> {
    let img = image::RgbaImage::from_raw(p.width, p.height, p.rgba.clone()).ok_or_else(|| anyhow!("bad preview buffer"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}

pub use crate::support::is_supported as is_supported_name;

const TEXT_SYNTAX: [(&str, &str); 7] = [(".xlast", "xml"), (".lua", "lua"), (".resx", "xml"), (".updb", "xml"), (".xml", "xml"), (".ext", "txt"), (".txt", "txt")];

fn plain(kind: &'static str, lines: Vec<String>) -> Detail {
    Detail { kind, lines, image: None, text: None, pending_preview: false, scene: None, figure: None }
}

fn be32(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(d.get(o..o + 4).ok_or_else(|| anyhow!("truncated at {o:#x}"))?.try_into()?))
}

fn hex_dump(data: &[u8]) -> Vec<String> {
    let mut lines = vec![format!("{} bytes", data.len())];
    for (row, chunk) in data.chunks(16).take(HEX_ROWS).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
        lines.push(format!("{:08x}  {:<47}  {ascii}", row * 16, hex.join(" ")));
    }
    lines
}

fn text(data: &[u8], name: &str, syntax: &'static str) -> Detail {
    let body = String::from_utf8_lossy(data);
    let content = body.trim_start_matches('\u{feff}').to_owned();
    let mut lines = vec![format!("{} lines, {} bytes", content.lines().count(), data.len())];
    if name.to_lowercase().ends_with(".resx") {
        lines.push(format!("{} data entries", content.matches("<data ").count()));
    }
    Detail { kind: "Text", lines, image: None, text: Some(TextBody { content, syntax }), pending_preview: false, scene: None, figure: None }
}

fn decode_image(bytes: &[u8]) -> Option<Preview> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(Preview { width, height, rgba: img.into_raw() })
}

fn live_package(data: &[u8]) -> Result<Detail> {
    let package = xcontent::parse(data)?;
    let image = decode_image(&package.thumbnail).or_else(|| decode_image(&package.title_image));
    Ok(Detail { kind: "Xbox LIVE package", lines: package.lines, image, text: None, pending_preview: false, scene: None, figure: None })
}

fn executable(data: &[u8]) -> Result<Detail> {
    let mut lines = xex::describe(data)?;
    let table = xex::resource_table(data).unwrap_or_default();
    if !table.is_empty() {
        lines.push(format!("{} embedded resources (open the file to browse them)", table.len()));
        lines.extend(table.iter().map(|(name, address, size)| format!("  {name}  {size} bytes at {address:08X}")));
    }
    Ok(plain("Xbox 360 executable (XEX)", lines))
}

fn xui_archive(data: &[u8]) -> Result<Detail> {
    let entries = xui::read_xuiz(data)?;
    let mut lines = vec![format!("{} entries (double-click to open)", entries.len())];
    lines.extend(entries.iter().map(|e| format!("{:>9}  {}", e.data.len(), e.name)));
    Ok(plain("XUI archive", lines))
}

fn xui_strings(data: &[u8]) -> Result<Detail> {
    let strings = xui::read_xus(data)?;
    let mut lines = vec![format!("{} strings", strings.len())];
    lines.extend(strings.chunks(2).map(|kv| kv.join(" = ")));
    Ok(plain("XUI string table", lines))
}

fn xui_scene(data: &[u8]) -> Result<Detail> {
    let x = xur::parse(data)?;
    let mut lines = vec![format!("version {}  flags {}  xui version {}  {} bytes", x.version, x.flags, x.xui_version, x.size)];
    lines.extend(x.sections.iter().map(|s| format!("section {}  @{:#x}  {:#x} bytes", s.tag, s.offset, s.size)));
    lines.push(format!("{} strings (class names, ids, text)", x.strings.len()));
    match xur::parse_scene(data) {
        Ok(scene) => {
            lines.push("scene tree".into());
            lines.extend(xur::render(&scene));
        }
        Err(e) => {
            lines.push(format!("scene tree unavailable: {e}"));
            lines.extend(x.strings.iter().enumerate().map(|(i, s)| format!("  {i:>3}  {s}")));
        }
    }
    Ok(plain("XUI scene (XUR)", lines))
}

const MODEL_PREVIEW: u32 = 384;

fn model_summary(compressed: &[u8], gender: Option<body::Gender>) -> (Vec<String>, Option<Preview>, Option<render::SceneHandle>) {
    let decoded = lzx::decompress(compressed).and_then(|raw| model::parse(&raw));
    let m = match decoded {
        Ok(m) => m,
        Err(e) => return (vec![format!("model not decoded: {e}")], None, None),
    };
    let mut lines = vec![format!("model: {} batches, {} textures", m.batches.len(), m.textures.len())];
    for (i, b) in m.batches.iter().enumerate() {
        let (lo, hi) = b.bounds();
        lines.push(format!("batch {i}  shader {}  {} triangles  {} vertices  {} uv sets  {} params  bounds ({:.3}, {:.3}, {:.3}) to ({:.3}, {:.3}, {:.3})", b.shader, b.triangles, b.vertices.len(), b.uv_sets, b.params.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]));
    }
    lines.extend(m.textures.iter().enumerate().map(|(i, t)| format!("texture {i}  {}x{}  format {:#010x}  {} bytes{}", t.width, t.height, t.format, t.data.len(), if t.placeholder { "  placeholder" } else { "" })));
    let base = gender.and_then(body::locate);
    if base.is_some() {
        lines.push("preview shown on the base avatar".to_owned());
    }
    if m.batches.is_empty() {
        return (lines, None, None);
    }
    let item = render::ScenePart { model: m, custom: None, tintable: true, depth_bias: 0.0 };
    let parts = base.into_iter().flatten().enumerate().map(|(i, model)| render::ScenePart { model: model.clone(), custom: None, tintable: false, depth_bias: if i == 0 { render::BODY_DEPTH_BIAS } else { 0.0 } }).chain(std::iter::once(item)).collect();
    let scene = render::Scene { parts, face: render::Face::default() };
    let image = scene.render(&render::Camera::default(), MODEL_PREVIEW, MODEL_PREVIEW);
    (lines, Some(image), Some(render::SceneHandle(std::sync::Arc::new(scene))))
}

fn avatar_item(data: &[u8]) -> Result<Detail> {
    let y = ytgr::parse(data)?;
    let mut lines = vec![
        format!("asset id {}", y.asset_id),
        format!("created {} UTC", y.timestamp),
        format!("version {}  header {:#x} bytes  payload {} bytes", y.version, y.header_size, y.payload_size),
        format!("digest {}", y.digest.iter().map(|b| format!("{b:02x}")).collect::<String>()),
        format!("signature {} bytes (not verified)", y.signature.len()),
        format!("structured binary version {} {}-endian", y.strb.version, if y.strb.big_endian { "big" } else { "little" }),
    ];
    for b in &y.strb.blocks {
        let kind = ytgr::block_kind(b.id);
        lines.push(format!("block {} {}  {} bytes", b.id, if kind.is_empty() { "unknown" } else { kind }, b.length));
    }
    if let Ok(m) = y.block_data(data, 8).and_then(ytgr::metadata) {
        lines.push(format!("metadata: asset type {}  flags {}  value A {:#x}  value B {}  extra {}", m.asset_type, m.flags, m.mask, m.id, m.extra));
    }
    if let Some(t) = y.block_data(data, 7).ok().and_then(|b| ytgr::colour_table(b).ok()) {
        let colours: Vec<String> = t.entries.iter().map(|(c, _)| format!("{c:#010x}")).collect();
        lines.push(format!("custom colours: {}  {}", t.entries.len(), colours.join(" ")));
    }
    let gender = y.block_data(data, 8).ok().and_then(|b| ytgr::metadata(b).ok()).map(|m| body::Gender::from_flags(m.flags));
    let (model_lines, image, scene) = y.block_data(data, 3).map_or((Vec::new(), None, None), |b| model_summary(b, gender));
    lines.extend(model_lines);
    Ok(Detail { kind: "Avatar item asset (YTGR)", lines, image, text: None, pending_preview: false, scene, figure: None })
}

fn packed_asset(data: &[u8]) -> Result<Detail> {
    let s = strb::parse(data)?;
    let mut lines = vec![format!("asset id {}", ytgr::guid_text(&s.guid))];
    for b in &s.blocks {
        let kind = ytgr::block_kind(b.id);
        lines.push(format!("block {} {}  {} bytes", b.id, if kind.is_empty() { "unknown" } else { kind }, b.length));
    }
    if let Some(m) = s.blocks.iter().find(|b| b.id == 8).and_then(|b| ytgr::metadata(&data[b.start..b.start + b.length]).ok()) {
        lines.push(format!("metadata: asset type {}  flags {}  value A {:#x}  value B {}  extra {}", m.asset_type, m.flags, m.mask, m.id, m.extra));
    }
    let (model_lines, image, scene) = s.blocks.iter().find(|b| b.id == 3).map_or((Vec::new(), None, None), |b| model_summary(&data[b.start..b.start + b.length], None));
    lines.extend(model_lines);
    Ok(Detail { kind: "Avatar asset (STRB)", lines, image, text: None, pending_preview: false, scene, figure: None })
}

fn avatar_block(name: &str, data: &[u8]) -> Option<Detail> {
    let lower = name.to_lowercase();
    if lower.ends_with(".animation") {
        return Some(avatar::describe_animation_block(data));
    }
    if lower.ends_with(".model") {
        let (lines, image, scene) = model_summary(data, None);
        return Some(Detail { kind: "Avatar model", lines, image, text: None, pending_preview: false, scene, figure: None });
    }
    if lower.ends_with(".colours") {
        return Some(match ytgr::colour_table(data) {
            Ok(t) => plain("Avatar custom colour table", t.entries.iter().enumerate().map(|(i, (c, v))| format!("entry {i}  colour {c:#010x}  value {v}")).collect()),
            Err(e) => plain("Avatar custom colour table", vec![e.to_string()]),
        });
    }
    if lower.ends_with(".metadata") {
        let lines = ytgr::metadata(data).map_or_else(|e| vec![e.to_string()], |m| vec![format!("asset type {}", m.asset_type), format!("flags {}", m.flags), format!("value A {:#x}", m.mask), format!("value B {}", m.id), format!("extra {}", m.extra)]);
        return Some(plain("Avatar asset metadata", lines));
    }
    let kind = lower.rsplit('.').next().filter(|_| lower.starts_with("block"))?;
    let mut lines = vec![format!("{kind} block, {} bytes (not decoded yet)", data.len())];
    lines.extend(hex_dump(data).into_iter().skip(1));
    Some(plain("Avatar asset block", lines))
}

fn caff_container(data: &[u8]) -> Result<Detail> {
    let c = caff::parse(data)?;
    let mut lines = vec![format!("CAFF  {} blocks  {} relocs", c.sizes.len(), c.relocs.as_ref().map_or(0, |r| r.len()))];
    lines.extend(c.starts.iter().zip(&c.sizes).enumerate().map(|(i, (at, n))| format!("block {i}  @{at:#x}  {n:#x} bytes")));
    let textures = texture::list(data).unwrap_or_default();
    lines.extend(textures.iter().map(|t| format!("texture {}  {:?} {}x{} mips {}", t.index, t.format, t.width, t.height, t.mips)));
    let image = textures.iter().find_map(|t| {
        let (width, height) = texture::level_size(t, 0);
        texture::decode(data, t, 0).ok().map(|px| Preview { width, height, rgba: px.concat() })
    });
    Ok(Detail { kind: "CAFF container", lines, image, text: None, pending_preview: false, scene: None, figure: None })
}

fn wave_bank(data: &[u8]) -> Result<Detail> {
    let bank = xwb::parse(data)?;
    let first = bank.entries.first();
    let mut lines = vec![format!("bank {}  {} entries", bank.name, bank.entries.len())];
    if let Some(e) = first {
        lines.push(format!("first entry {} {} ch {} Hz", e.codec_name(), e.channels, e.rate));
    }
    lines.push(format!("{} bytes", data.len()));
    lines.extend(bank.entries.iter().map(|e| format!("entry {}  {} {} ch {} Hz  {:.2} s  {} bytes", e.index, e.codec_name(), e.channels, e.rate, e.seconds(), e.length)));
    Ok(plain("XACT wave bank", lines))
}

fn sound_bank(data: &[u8]) -> Result<Detail> {
    let sb = xsb::parse(data)?;
    let mut lines = vec![format!("{} cues, wave bank {}", sb.cues.len(), sb.bank_name), format!("{} bytes", data.len())];
    lines.extend(sb.cues.iter().map(|c| format!("{}  waves {}", c.name, c.waves.iter().map(u16::to_string).collect::<Vec<_>>().join(" "))));
    Ok(plain("XACT sound bank", lines))
}

fn video_details(data: &[u8]) -> Result<Detail> {
    let info = video::asf_info(data)?;
    let mut lines = vec![format!("duration {:.1} s", info.seconds), format!("{} bytes", data.len())];
    if let Some(v) = &info.video {
        lines.push(format!("video {} {}x{}", v.fourcc, v.width, v.height));
    }
    if let Some(a) = &info.audio {
        lines.push(format!("audio {} ch {} Hz (format {:#06x})", a.channels, a.sample_rate, a.format_tag));
    }
    lines.push("press Play to watch".into());
    Ok(Detail { kind: "Windows Media video", lines, image: None, text: None, pending_preview: false, scene: None, figure: None })
}

fn photoshop(data: &[u8]) -> Result<Detail> {
    ensure!(data.len() >= 26, "truncated PSD header");
    let channels = u16::from_be_bytes([data[12], data[13]]);
    Ok(plain("Photoshop image", vec![format!("{}x{} px, {channels} channels", be32(data, 18)?, be32(data, 14)?), format!("{} bytes", data.len())]))
}

fn raster(data: &[u8]) -> Result<Detail> {
    let preview = decode_image(data).ok_or_else(|| anyhow!("undecodable image"))?;
    Ok(Detail { kind: "Image", lines: vec![format!("{}x{} px", preview.width, preview.height), format!("{} bytes", data.len())], image: Some(preview), text: None, pending_preview: false, scene: None, figure: None })
}

fn mct_texture(data: &[u8]) -> Detail {
    let mut lines = vec![format!("MCT second-stage texture codec, {} bytes", data.len()), "stored under a .png name inside the archive".into(), "LZX-packed DXT5 planes, preview decodes in the background".into()];
    lines.extend(hex_dump(data).into_iter().skip(1).take(4));
    Detail { kind: "MCT texture", lines, image: None, text: None, pending_preview: true, scene: None, figure: None }
}

fn xdbf_archive(data: &[u8]) -> Result<Detail> {
    let a = xdbf::parse(data)?;
    let mut lines = Vec::new();
    if let Ok(t) = a.title(data) {
        lines.push(format!("title id {:08X}", t.title_id));
        lines.push(format!("version {}.{}.{}.{}", t.version.0, t.version.1, t.version.2, t.version.3));
    }
    lines.push(format!("{} entries", a.entries.len()));
    for e in &a.entries {
        let (name, bytes) = xdbf::entry_name(&a, data, e);
        lines.push(format!("  {name}  {} bytes", bytes.len()));
    }
    Ok(plain("XDBF title resources", lines))
}

fn string_table(data: &[u8]) -> Result<Detail> {
    let strings = xdbf::strings(data)?;
    let mut lines = vec![format!("{} strings", strings.len())];
    lines.extend(strings.into_iter().map(|(id, s)| format!("{id:>6}  {s}")));
    Ok(plain("XDBF string table", lines))
}

fn by_magic(data: &[u8]) -> Result<Option<Detail>> {
    Ok(Some(match data.get(..4) {
        Some(b"LIVE" | b"PIRS" | b"CON ") => live_package(data)?,
        Some(b"XUIZ") => xui_archive(data)?,
        Some(b"XUIS") => xui_strings(data)?,
        Some(b"XUIB") => xui_scene(data)?,
        Some(b"YTGR") => avatar_item(data)?,
        Some(b"CAFF") => caff_container(data)?,
        Some(b"DNBW") => wave_bank(data)?,
        Some(b"KBDS") => sound_bank(data).unwrap_or_else(|e| plain("XACT sound bank", vec![format!("{} bytes", data.len()), format!("cue table not decoded: {e}")])),
        Some(b"STRB") => avatar::describe_strb(data)?,
        Some([0xa3, 0xd7, 0x01, 0x41]) => avatar::describe_fxl(data)?,
        Some(b".mct") => mct_texture(data),
        Some(b"8BPS") => photoshop(data)?,
        Some(b"\x89PNG") => raster(data)?,
        Some([0xff, 0xd8, 0xff, _]) => raster(data)?,
        Some(b"XEX2") => executable(data)?,
        Some(b"XDBF") => xdbf_archive(data)?,
        Some(b"XSTR") => string_table(data)?,
        Some(head) if head == ASF_MAGIC => video_details(data)?,
        _ => return Ok(None),
    }))
}

pub fn describe(name: &str, data: &[u8]) -> Detail {
    if data.is_empty() {
        return plain("Empty file", vec!["empty file (0 bytes)".into()]);
    }
    if name.to_lowercase().ends_with(".toc") {
        let assets = crate::toc::split(data);
        if !assets.is_empty() {
            let count = |t: u32| assets.iter().filter(|a| a.asset_type == t).count();
            return plain("Avatar asset pack", vec![format!("{} assets: {} models, {} face textures, {} shape and hiding templates, {} animations", assets.len(), count(1), count(2), count(3), count(4))]);
        }
    }
    if name.to_lowercase().ends_with(".avatarasset") {
        return packed_asset(data).unwrap_or_else(|e| plain("Avatar asset (STRB)", vec![e.to_string()]));
    }
    match by_magic(data) {
        Ok(Some(detail)) => return detail,
        Ok(None) => {}
        Err(e) => {
            let mut lines = vec![format!("could not read this file: {e}")];
            lines.extend(hex_dump(data));
            return plain("Unreadable", lines);
        }
    }
    if let Some(detail) = avatar_block(name, data) {
        return detail;
    }
    let lower = name.to_lowercase();
    if lower.ends_with(".avd") && data.len() == avatar::AVD_SIZE {
        return avatar::describe_avd(data).unwrap_or_else(|e| plain("Unreadable", vec![e.to_string()]));
    }
    if let Some((_, syntax)) = TEXT_SYNTAX.iter().find(|(ext, _)| lower.ends_with(ext)) {
        return text(data, name, syntax);
    }
    plain("Binary", hex_dump(data))
}
