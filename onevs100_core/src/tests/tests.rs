use crate::{caff, texture, xenos, xui};
use std::path::PathBuf;

fn game(rel: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common/v1.0.808.0_debug_unmodified");
    std::fs::read(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn xuiz_roundtrips_real_archives() {
    for rel in ["show/media/show.xzp", "media/launch.xzp"] {
        let raw = game(rel);
        let entries = xui::read_xuiz(&raw).unwrap();
        assert!(!entries.is_empty());
        assert_eq!(xui::write_xuiz(&entries).unwrap(), raw, "{rel}");
    }
}

#[test]
fn xus_roundtrips_every_string_table() {
    let mut seen = 0;
    for rel in ["show/media/show.xzp", "media/launch.xzp"] {
        for e in xui::read_xuiz(&game(rel)).unwrap().iter().filter(|e| e.data.starts_with(b"XUIS")) {
            let strings = xui::read_xus(&e.data).unwrap();
            assert_eq!(xui::write_xus(&strings).unwrap(), e.data, "{}", e.name);
            seen += 1;
        }
    }
    assert!(seen > 0);
}

#[test]
fn xuiz_rejects_wrong_magic_and_size() {
    assert!(xui::read_xuiz(b"NOPE0000000000000000000000").is_err());
    let mut raw = xui::write_xuiz(&[xui::XuizEntry { name: "a".into(), data: vec![1] }]).unwrap();
    raw.push(0);
    assert!(xui::read_xuiz(&raw).is_err());
}

#[test]
fn caff_parses_anim_with_ascending_relocs() {
    let raw = game("media/avataranims/Of_LO_02.anim.bin");
    let c = caff::parse(&raw).unwrap();
    assert_eq!(c.sizes.len(), 3);
    assert_eq!(c.sizes.iter().sum::<u32>() as usize, raw.len() - caff::HDR);
    assert_eq!(c.base, Some(c.starts[2]));
    let relocs = c.relocs.unwrap();
    assert!(relocs.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn tiled_offsets_match_python_reference() {
    let pts = [(0, 0), (1, 0), (0, 1), (8, 0), (0, 8), (33, 17), (63, 63)];
    let at = |lg| pts.map(|(x, y)| xenos::tiled_offset_2d(x, y, 64, lg));
    assert_eq!(at(3), [0, 8, 16, 64, 4224, 10264, 32632]);
    assert_eq!(at(4), [0, 32, 16, 64, 8320, 18480, 65392]);
}

#[test]
fn mip_layout_matches_python_reference() {
    let (levels, total) = xenos::mip_layout(256, 256, xenos::Format::Dxt1, 9);
    assert_eq!(total, 65536);
    let got: Vec<_> = levels.iter().take(3).map(|l| (l.width, l.offset, l.pitch, l.packed)).collect();
    assert_eq!(got, [(256, 0, 64, false), (128, 32768, 32, false), (64, 40960, 32, false)]);
}

#[test]
fn dxt1_solid_colour_block_decodes() {
    let block = [0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0];
    let px = xenos::decode_block(&block, xenos::Format::Dxt1);
    assert!(px.iter().all(|p| *p == [255, 0, 0, 255]));
}

#[test]
fn dxt1_three_colour_mode_has_transparent_index_three() {
    let block = [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let px = xenos::decode_block(&block, xenos::Format::Dxt1);
    assert!(px.iter().all(|p| *p == [0, 0, 0, 0]));
}

#[test]
fn texture_decode_places_block_at_tiled_position() {
    let (levels, total) = xenos::mip_layout(64, 64, xenos::Format::Dxt1, 1);
    let mut buf = vec![0u8; total];
    let at = xenos::tiled_offset_2d(1, 0, levels[0].pitch, 3);
    buf[at..at + 8].copy_from_slice(&[0x1F, 0x00, 0x1F, 0x00, 0, 0, 0, 0]);
    let px = xenos::decode_level(&buf, &levels[0], xenos::Format::Dxt1, false);
    assert_eq!(px[0], [0, 0, 0, 255]);
    assert_eq!(px[4], [0, 0, 255, 255]);
    assert_eq!(px[64 * 3 + 7], [0, 0, 255, 255]);
}

#[test]
fn unpack_then_pack_dir_reproduces_archive() {
    let raw = game("show/media/show.xzp");
    let dir = std::env::temp_dir().join(format!("xzp-roundtrip-{}", std::process::id()));
    let entries = xui::read_xuiz(&raw).unwrap();
    xui::unpack_dir(&entries, &dir).unwrap();
    let mut repacked = xui::pack_dir(&dir).unwrap();
    let mut original = entries;
    repacked.sort_by(|a, b| a.name.cmp(&b.name));
    original.sort_by(|a, b| a.name.cmp(&b.name));
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(repacked, original);
}


fn textures_of(name: &str) -> (Vec<u8>, Vec<texture::Texture>) {
    let raw = game(&format!("media/set/{name}"));
    let list = texture::list(&raw).unwrap();
    (raw, list)
}

#[test]
fn texture_list_matches_python_reference() {
    let (_, list) = textures_of("lobbyExtBloom.mb.bin");
    let got: Vec<_> = list.iter().take(4).map(|t| (t.format, t.width, t.height, t.mips, t.gpu.clone())).collect();
    assert_eq!(
        got,
        [
            (xenos::Format::Argb8, 256, 256, 9, Some(0xd000..0xd000 + 0x56000)),
            (xenos::Format::Dxt1, 512, 256, 9, Some(0x63000..0x63000 + 0x1a000)),
            (xenos::Format::Dxt1, 256, 128, 8, Some(0x7d000..0x7d000 + 0xa000)),
            (xenos::Format::Dxt1, 64, 64, 7, Some(0x87000..0x87000 + 0x6000)),
        ]
    );
    assert_eq!(list.len(), 4);
}

#[test]
fn font_texture_uses_whole_gpu_block() {
    let (_, list) = textures_of("tahoma.ttf.bin");
    assert_eq!((list.len(), list[0].gpu.clone()), (1, Some(0..0x2000)));
}

#[test]
fn texture_pixels_match_python_reference() {
    let (raw, list) = textures_of("lobbyExtBloom.mb.bin");
    let px = texture::decode(&raw, &list[0], 0).unwrap();
    let n = px.len();
    assert_eq!((px[0], px[n / 3], px[n / 2 + 7], px[n - 1]), ([232, 214, 158, 242], [111, 36, 31, 147], [73, 21, 15, 102], [53, 16, 0, 75]));
    let small = texture::decode(&raw, &list[0], 3).unwrap();
    assert_eq!((small.len(), small[0]), (32 * 32, [223, 206, 134, 230]));
    let dxt = texture::decode(&raw, &list[1], 0).unwrap();
    assert_eq!((dxt[0], dxt[dxt.len() / 2 + 7]), ([18, 24, 41, 255], [30, 34, 51, 255]));
}

#[test]
fn decode_rejects_bad_level() {
    let (raw, list) = textures_of("tahoma.ttf.bin");
    assert!(texture::decode(&raw, &list[0], 5).is_err());
}

#[test]
fn png_export_has_png_magic() {
    let (raw, list) = textures_of("tahoma.ttf.bin");
    let png = texture::to_png(&raw, &list[0], 0).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
}
