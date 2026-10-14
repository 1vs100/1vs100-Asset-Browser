use crate::inspect::{self, Detail};
use std::path::PathBuf;

fn builds() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common")
}

fn files_with_extension(ext: &str) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut stack = vec![builds()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().to_lowercase().ends_with(ext) {
                found.push((p.display().to_string(), std::fs::read(p).unwrap()));
            }
        }
    }
    found.extend(crate::tests_fixtures::packages().into_iter().flat_map(|(role, id, files)| {
        files
            .into_iter()
            .filter(|f| f.name.to_lowercase().ends_with(ext))
            .map(move |f| (format!("{role}/{id}/{}", f.name), f.payload.read().unwrap()))
    }));
    found
}

fn describe(name: &str, data: &[u8]) -> Detail {
    inspect::describe(name, data)
}

fn has(d: &Detail, needle: &str) -> bool {
    d.lines.iter().any(|l| l.contains(needle))
}

fn episode_sponsor() -> Vec<u8> {
    crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "media/sponsor/of_lu_sp.fbx.msliveavatarasset")
}

fn episode_avatar(name: &str) -> Vec<u8> {
    crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", &format!("media/avatars/{name}"))
}

#[test]
fn every_avd_in_the_game_files_decodes_with_nine_opaque_colours() {
    let files = files_with_extension(".avd");
    assert!(!files.is_empty());
    for (name, data) in files {
        let d = describe(&name, &data);
        assert_eq!(d.kind, "Avatar metadata (AVD)", "{name}");
        assert_eq!(d.lines.iter().filter(|l| l.starts_with("colour ")).count(), 9, "{name}");
        assert!(d.lines.iter().filter(|l| l.starts_with("colour ")).all(|l| l.contains(" FF")), "{name}");
        let swatches = d.image.expect("colour swatch preview");
        assert_eq!(swatches.rgba.len(), (swatches.width * swatches.height * 4) as usize);
    }
}

#[test]
fn avd_host_live_matches_the_hand_decoded_fields() {
    let data = episode_avatar("host_live.avd");
    let d = describe("host_live.avd", &data);
    for want in [
        "header  value1 -0.5  value2 0",
        "@0x10  asset id 799  kind 3  mask 0x00080000",
        "@0x30  asset id 823  kind 3  mask 0x00008000",
        "colour 0  FFE3A976",
        "colour 8  FFCF5969",
        "@0x340  asset id 72  kind 1  flags 0x00000008  mask 0x00080000  colours FF808080 FF808080 FF808080",
    ] {
        assert!(has(&d, want), "missing {want:?} in {:#?}", d.lines);
    }
}

#[test]
fn avd_female_host_reports_float_header_and_trailer() {
    let data = episode_avatar("femalehost.avd");
    let d = describe("femalehost.avd", &data);
    assert!(has(&d, "header  value1 -0.5  value2 -0.5"), "{:#?}", d.lines);
    assert!(has(&d, "trailer @0x380  e0002cb8fabee1b0092bee1b01"), "{:#?}", d.lines);
}

#[test]
fn avd_per_asset_colours_are_listed_when_set() {
    let data = episode_avatar("femalehost_holiday.avd");
    let d = describe("femalehost_holiday.avd", &data);
    assert!(has(&d, "colours FFD90000 FFD90000 FFD90000"), "{:#?}", d.lines);
}

#[test]
fn avd_with_wrong_size_is_not_decoded_as_avatar_metadata() {
    assert_ne!(inspect::describe("x.avd", &[0u8; 999]).kind, "Avatar metadata (AVD)");
}

#[test]
fn every_strb_asset_reports_its_header() {
    let files = files_with_extension(".msliveavatarasset");
    assert!(!files.is_empty());
    for (name, raw) in files {
        let d = inspect::describe("a.msliveavatarasset", &raw);
        assert_eq!(d.kind, "Avatar animation (STRB)", "{name}");
        assert!(has(&d, "format id 0x000059bd"), "{name}");
        assert!(has(&d, "asset guid E08B49094EE4B991ADA87B7C0B6B0104"), "{name}");
        let sized = has(&d, &format!("payload length {} (file size - 35)", raw.len() - 35));
        assert_eq!(sized, raw[4] == 0, "{name}: {:#?}", &d.lines[..5]);
        assert!(!has(&d, "note:"), "{name}");
    }
    assert!(has(&inspect::describe("a.msliveavatarasset", &episode_sponsor()), "layout flag 0"));
    let old = std::fs::read(builds().join("v2.0.1149.0.02-2009-10-26_retail/media/avataranims/xuiavataranims/hf_ch_02.fbx.msliveavatarasset")).unwrap();
    assert!(has(&describe("hf_ch_02.fbx.msliveavatarasset", &old), "layout flag 1"));
}

#[test]
fn every_fxlobj_lists_compiler_shaders_and_source() {
    let files = files_with_extension(".fxlobj");
    assert!(!files.is_empty());
    for (name, raw) in files {
        let d = inspect::describe("a.fxlobj", &raw);
        assert_eq!(d.kind, "Effect object (FXL)", "{name}");
        assert!(has(&d, &format!("payload bytes {} (file size - 12)", raw.len() - 12)), "{name}");
        assert!(has(&d, "compiler 2.0."), "{name}");
        assert!(has(&d, "vs_3_0 x") && has(&d, "ps_3_0 x"), "{name}: {:#?}", d.lines);
    }
}

#[test]
fn fxlobj_avatar_base_lists_constants_and_database_path() {
    let data = std::fs::read(builds().join("OneVsLive/media/avatars/avatarBase.fxlobj")).unwrap();
    let d = describe("avatarBase.fxlobj", &data);
    for want in ["vs_3_0 x11", "ps_3_0 x8", "compiler 2.0.7978.3", "c_modelview", "c_projection", "c_shadow", "avatarBase.updb"] {
        assert!(has(&d, want), "missing {want:?} in {:#?}", d.lines);
    }
}
