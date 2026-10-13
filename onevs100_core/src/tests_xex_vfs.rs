use crate::browser::Browser;
use crate::{inspect, source, xdbf, xex};
use std::path::PathBuf;

const GAME_XEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/common/v2.0.1317.0.02-2010-04-14/default.xex");
const EDITOR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/su/AvatarEditor.xex");
const DASH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/su/dash.xex");

fn read(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

#[test]
fn an_xex_lists_its_embedded_resources_with_their_content() {
    let Some(d) = read(EDITOR) else { return };
    let r = xex::resources(&d).unwrap();
    let names: Vec<&str> = r.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["584D07D1", "common", "controlp", "MEDIA"]);
    assert_eq!(&r[0].data[..4], b"XDBF");
    assert!(r[1..].iter().all(|x| x.data.starts_with(b"XUIZ")));
    assert_eq!(r[3].data.len(), 0xce6060);
    assert!(xex::resources(b"not an xex").is_err());
}

#[test]
fn xex_files_expand_into_named_entries_like_other_containers() {
    let Some(d) = read(EDITOR) else { return };
    let kids = source::expand(&d).unwrap().unwrap();
    let names: Vec<&str> = kids.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["584D07D1.xdbf", "common.xzp", "controlp.xzp", "MEDIA.xzp"]);
    let dash = source::expand(&read(DASH).unwrap()).unwrap().unwrap();
    assert!(dash.len() > 25);
    let unique: std::collections::HashSet<&str> = dash.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(unique.len(), dash.len());
}

#[test]
fn xdbf_archives_expose_icons_strings_and_the_embedded_xlast_source() {
    let Some(d) = read(GAME_XEX) else { return };
    let xdbf_bytes = xex::resources(&d).unwrap().remove(0).data;
    let a = xdbf::parse(&xdbf_bytes).unwrap();
    assert_eq!(a.entries.len(), 28);
    let title = a.title(&xdbf_bytes).unwrap();
    assert_eq!(title.title_id, 0x584A07E4);
    assert_eq!(title.version, (1, 0, 0x8b, 0));
    let kids = source::expand(&xdbf_bytes).unwrap().unwrap();
    let names: Vec<&str> = kids.iter().map(|k| k.name.as_str()).collect();
    assert!(names.contains(&"icon.png") && names.contains(&"image_0003.png") && names.contains(&"strings_lang1.xstr"));
    assert!(names.contains(&"584A07E4.original.xlast"), "{names:?}");
    let xml = kids.iter().find(|k| k.name.ends_with(".xlast")).unwrap().payload.read().unwrap();
    assert!(String::from_utf8_lossy(&xml).contains("<?xml") || String::from_utf8_lossy(&xml).contains('<'));
    let strings = xdbf::strings(&kids.iter().find(|k| k.name == "strings_lang1.xstr").unwrap().payload.read().unwrap()).unwrap();
    assert!(strings.contains(&(32768, "1 vs 100".to_owned())), "{strings:?}");
}

#[test]
fn the_inspector_describes_xdbf_and_string_tables_and_previews_icons() {
    let Some(d) = read(GAME_XEX) else { return };
    let x = xex::resources(&d).unwrap().remove(0).data;
    let det = inspect::describe("584A07E4.xdbf", &x);
    assert_eq!(det.kind, "XDBF title resources");
    assert!(det.lines.iter().any(|l| l.contains("584A07E4")), "{:?}", det.lines);
    let kids = source::expand(&x).unwrap().unwrap();
    let icon = kids.iter().find(|k| k.name == "icon.png").unwrap();
    assert!(inspect::describe(&icon.name, &icon.payload.read().unwrap()).image.is_some());
    let table = kids.iter().find(|k| k.name == "strings_lang1.xstr").unwrap();
    let t = inspect::describe(&table.name, &table.payload.read().unwrap());
    assert_eq!(t.kind, "XDBF string table");
    assert!(t.lines.iter().any(|l| l.contains("1 vs 100")));
}

#[test]
fn the_browser_walks_xex_then_xdbf_then_pictures() {
    let mut b = Browser::new(&PathBuf::from(EDITOR).parent().unwrap()).unwrap();
    let at = b.entries().iter().position(|e| e.name == "AvatarEditor.xex").unwrap();
    b.open(at).unwrap();
    assert_eq!(b.entries().len(), 4);
    b.open(0).unwrap();
    assert!(b.entries().iter().any(|e| e.name == "icon.png"));
    let icon = b.entries().iter().position(|e| e.name == "icon.png").unwrap();
    b.open(icon).unwrap();
    assert!(b.detail().unwrap().image.is_some());
}

#[test]
fn malformed_xdbf_is_an_error_not_a_panic() {
    assert!(xdbf::parse(b"XDBF").is_err());
    assert!(xdbf::parse(&[0u8; 64]).is_err());
    let mut bad = b"XDBF\x00\x01\x00\x00\x00\x00\x00\x10\x00\x00\x00\x10\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
    bad.extend_from_slice(&[0u8; 8]);
    assert!(xdbf::parse(&bad).is_err());
    assert!(xdbf::strings(b"XSTR").is_err());
}

#[test]
fn the_xex_details_list_the_embedded_resources() {
    let Some(d) = read(EDITOR) else { return };
    let det = inspect::describe("AvatarEditor.xex", &d);
    assert!(det.lines.iter().any(|l| l.contains("4 embedded resources")), "{:?}", det.lines);
    assert!(det.lines.iter().any(|l| l.contains("MEDIA") && l.contains("13525088")));
}
