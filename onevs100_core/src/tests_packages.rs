use crate::inspect::{self, Detail};
use crate::source::{self, Entry};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn containers() -> Vec<(String, String, Vec<Entry>)> {
    crate::tests_fixtures::packages()
}

fn describe(name: &str, data: &[u8]) -> Detail {
    inspect::describe(name, data)
}

fn kind_for(rel: &str) -> &'static str {
    let lower = rel.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap();
    match ext {
        "ext" | "lua" | "resx" | "updb" => "Text",
        "xzp" => "XUI archive",
        "xsb" => "XACT sound bank",
        "xwb" => "XACT wave bank",
        "wmv" => "Windows Media video",
        "jpg" | "png" => "Image",
        "psd" => "Photoshop image",
        "msliveavatarasset" => "Avatar animation (STRB)",
        "bin" => "CAFF container",
        "avd" => "Avatar metadata (AVD)",
        "fxlobj" => "Effect object (FXL)",
        other => panic!("unexpected extension {other}"),
    }
}

#[test]
fn all_eight_live_packages_can_be_read() {
    assert_eq!(containers().len(), 8);
}

#[test]
fn live_packages_list_files_with_readable_bytes() {
    for (role, id, files) in containers() {
        assert!(!files.is_empty(), "{role}/{id}");
        for file in files {
            assert!(!file.payload.read().unwrap().is_empty(), "{role}/{id}/{}", file.name);
        }
    }
}

#[test]
fn live_container_details_name_title_and_display_name() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/dlcs/shows/77205BC1241C708A8A10B7303AF90E5EDD0286B1");
    let d = describe("pkg", &std::fs::read(path).unwrap());
    assert_eq!(d.kind, "Xbox LIVE package");
    assert!(d.lines.iter().any(|l| l.contains("584A07DC")), "{:?}", d.lines);
}

#[test]
fn every_file_in_every_package_has_the_expected_kind_and_details() {
    let mut seen = BTreeSet::new();
    for (role, id, files) in containers() {
        for file in files {
            let rel = file.name.replace('\\', "/");
            if rel.chars().any(char::is_control) {
                continue;
            }
            let d = describe(&rel, &file.payload.read().unwrap());
            let want = kind_for(&rel);
            assert_eq!(d.kind, want, "{role}/{id}/{rel}");
            assert!(!d.lines.is_empty(), "{role}/{id}/{rel} has no details");
            seen.insert(want);
        }
    }
    assert_eq!(seen.len(), 11, "{seen:?}");
}

#[test]
fn image_files_offer_a_preview_with_their_real_size() {
    let jpg = describe("p.jpg", &crate::tests_fixtures::package_file("shows", "77205BC1241C708A8A10B7303AF90E5EDD0286B1", "media/prizes/prize_arcade1.jpg"));
    let p = jpg.image.unwrap();
    assert_eq!((p.width, p.height, p.rgba.len()), (219, 300, 219 * 300 * 4));
    let psd = describe("p.psd", &crate::tests_fixtures::package_file("shows", "77205BC1241C708A8A10B7303AF90E5EDD0286B1", "media/prizes/source/prizeboxes.psd"));
    assert!(psd.lines.iter().any(|l| l.contains("219") && l.contains("300")), "{:?}", psd.lines);
    assert!(psd.image.is_none());
}

#[test]
fn every_png_and_jpg_previews() {
    for (role, id, files) in containers() {
        for file in files.into_iter().filter(|f| f.name.to_lowercase().ends_with(".png") || f.name.to_lowercase().ends_with(".jpg")) {
            let img = describe(&file.name, &file.payload.read().unwrap()).image.unwrap_or_else(|| panic!("{role}/{id}/{}: no preview", file.name));
            assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize, "{role}/{id}/{}", file.name);
        }
    }
}

#[test]
fn caff_set_files_with_textures_preview_their_first_texture() {
    let mut with_textures = 0;
    for file in crate::tests_fixtures::packages().into_iter().find(|(_, id, _)| id == "83136FF343124795DC1806A2DB044476589BD5C0").unwrap().2.into_iter().filter(|f| f.name.ends_with(".mb.bin")) {
        let d = describe(&file.name, &file.payload.read().unwrap());
        assert!(d.lines.iter().any(|l| l.starts_with("CAFF")), "{}", file.name);
        if d.lines.iter().any(|l| l.starts_with("texture")) {
            with_textures += 1;
            assert!(d.image.is_some(), "{}: textures listed but no preview", file.name);
        }
    }
    assert!(with_textures > 0);
}

#[test]
fn wave_bank_details_match_python_reference() {
    let want = [("77205BC1", "VO_UK", 460), ("79E60F7D", "VO_FR", 458), ("7BEC809C", "VO", 449), ("9AF84C64", "VO_DE", 459)];
    for (id, name, count) in want {
        let package_id = containers().into_iter().find(|(_, package_id, _)| package_id.starts_with(id)).unwrap().1;
        let d = describe("vo.xwb", &crate::tests_fixtures::package_file("shows", &package_id, "media/audio/vo.xwb"));
        assert!(d.lines.iter().any(|l| l.contains(name) && l.contains(&count.to_string())), "{id}: {:?}", d.lines);
        assert!(d.lines.iter().any(|l| l.contains("XMA")), "{id}: {:?}", d.lines);
    }
}

#[test]
fn video_details_report_asf_duration() {
    let d = describe("t.wmv", &crate::tests_fixtures::package_file("shows", "77205BC1241C708A8A10B7303AF90E5EDD0286B1", "media/movies/titleintroaudio.wmv"));
    let secs: f64 = d.lines.iter().find_map(|l| l.strip_prefix("duration ")?.trim_end_matches(" s").parse().ok()).unwrap_or_else(|| panic!("{:?}", d.lines));
    assert!((9.0..16.0).contains(&secs), "{secs}");
}

#[test]
fn xui_archives_open_into_described_children() {
    let archive = crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "media/episode.xzp");
    let kids = source::expand_named("episode.xzp", &archive).unwrap().unwrap();
    assert!(kids.len() > 10);
    for k in &kids {
        let d = inspect::describe(&k.name, &k.payload.read().unwrap());
        assert!(!d.lines.is_empty() && !d.kind.is_empty(), "{}", k.name);
    }
    assert!(kids.iter().any(|k| k.name.ends_with(".xur")));
}

#[test]
fn xus_string_tables_list_their_strings() {
    let archive = crate::tests_fixtures::package_file("shows", "77205BC1241C708A8A10B7303AF90E5EDD0286B1", "media/show.xzp");
    let kids = source::expand_named("show.xzp", &archive).unwrap().unwrap();
    let xus = kids.iter().find(|k| k.payload.read().unwrap().starts_with(b"XUIS")).unwrap();
    let d = inspect::describe(&xus.name, &xus.payload.read().unwrap());
    assert_eq!(d.kind, "XUI string table");
}

#[test]
fn text_files_show_their_content_and_resx_counts_entries() {
    let lua = describe("a.lua", &crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "luaengine/logging.lua"));
    assert!(lua.text.unwrap().content.lines().count() > 5);
    let resx = describe("a.resx", &crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "media/strings/epsetscreens.resx"));
    assert!(resx.lines.iter().any(|l| l.contains("96 data entries")), "{:?}", &resx.lines[..3]);
}

#[test]
fn unknown_binary_falls_back_to_hex_dump() {
    let d = inspect::describe("x.avd", &[0u8; 40]);
    assert_eq!(d.kind, "Binary");
    assert!(d.lines.iter().any(|l| l.starts_with("00000000")));
}



#[test]
fn empty_file_is_reported_as_empty() {
    let d = inspect::describe("zero.bin", &[]);
    assert_eq!((d.kind, d.lines[0].as_str()), ("Empty file", "empty file (0 bytes)"));
}

#[test]
fn corrupt_container_reports_an_error_instead_of_failing() {
    let d = inspect::describe("bad.xzp", b"XUIZ\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
    assert_eq!(d.kind, "Unreadable");
    assert!(d.lines[0].contains("could not read"));
}
