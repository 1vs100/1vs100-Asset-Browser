use crate::browser::Browser;
use std::path::PathBuf;

fn shows() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/dlcs/shows")
}

fn index_of(b: &Browser, name: &str) -> usize {
    b.entries().iter().position(|e| e.name.ends_with(name)).unwrap_or_else(|| panic!("{name} not listed"))
}

fn show_root() -> Browser {
    let mut b = Browser::new(&shows()).unwrap();
    let live = index_of(&b, "77205BC1241C708A8A10B7303AF90E5EDD0286B1");
    b.open(live).unwrap();
    b
}

#[test]
fn starts_in_the_given_folder_with_folders_first() {
    let b = Browser::new(&shows()).unwrap();
    assert_eq!(b.breadcrumb(), shows().display().to_string());
    assert_eq!(b.entries().len(), 4);
    assert!(b.detail().is_none());
}

#[test]
fn opening_a_live_package_enters_it_and_describes_it() {
    let b = show_root();
    assert_eq!(b.detail().unwrap().kind, "Xbox LIVE package");
    assert!(b.breadcrumb().ends_with("77205BC1241C708A8A10B7303AF90E5EDD0286B1"));
    assert!(b.entries().iter().any(|e| e.name == "media/show.xzp" || e.name == "media\\show.xzp"));
}

#[test]
fn opening_a_file_selects_it_without_entering() {
    let mut b = show_root();
    let before = b.entries().len();
    let jpg = index_of(&b, "prize_arcade1.jpg");
    b.open(jpg).unwrap();
    assert_eq!(b.entries().len(), before);
    assert_eq!(b.detail().unwrap().kind, "Image");
    assert!(b.detail().unwrap().image.is_some());
    assert_eq!(b.selected_name(), Some("media/prizes/prize_arcade1.jpg"));
}

#[test]
fn nested_container_then_back_restores_previous_listing() {
    let mut b = show_root();
    let before: Vec<_> = b.entries().iter().map(|e| e.name.clone()).collect();
    let xzp = index_of(&b, "show.xzp");
    b.open(xzp).unwrap();
    assert!(b.breadcrumb().ends_with("show.xzp"));
    assert_eq!(b.detail().unwrap().kind, "XUI archive");
    assert!(b.entries().iter().any(|e| e.name.ends_with(".xur")));
    assert!(b.back());
    assert_eq!(b.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>(), before);
}

#[test]
fn back_at_the_root_does_nothing() {
    let mut b = Browser::new(&shows()).unwrap();
    assert!(!b.back());
}

#[test]
fn up_from_a_disk_folder_goes_to_its_parent() {
    let mut b = Browser::new(&shows()).unwrap();
    assert!(b.up());
    assert!(b.entries().iter().any(|e| e.name == "shows"));
}

#[test]
fn caff_file_exposes_textures_and_switching_changes_the_preview() {
    let mut b = Browser::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common/v1.0.808.0_debug_unmodified/media/set")).unwrap();
    let i = index_of(&b, "NoSponsorEN.mb.bin");
    b.open(i).unwrap();
    assert!(b.texture_count() >= 4);
    let first = b.detail().unwrap().image.clone().unwrap();
    b.select_texture(1).unwrap();
    let second = b.detail().unwrap().image.clone().unwrap();
    assert_eq!((first.width, first.height), (512, 384));
    assert_eq!((second.width, second.height), (256, 128));
}

#[test]
fn export_writes_the_selected_file_bytes() {
    let mut b = show_root();
    let i = index_of(&b, "prize_arcade1.jpg");
    b.open(i).unwrap();
    let dir = std::env::temp_dir().join(format!("browser-export-{}", std::process::id()));
    let out = b.export_selected(&dir).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(out.file_name().unwrap(), "prize_arcade1.jpg");
    assert!(bytes.starts_with(&[0xff, 0xd8]));
}

#[test]
fn export_png_writes_the_current_texture() {
    let mut b = Browser::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common/v1.0.808.0_debug_unmodified/media/set")).unwrap();
    let i = index_of(&b, "NoSponsorEN.mb.bin");
    b.open(i).unwrap();
    let dir = std::env::temp_dir().join(format!("browser-png-{}", std::process::id()));
    let out = b.export_texture_png(&dir).unwrap();
    let head = std::fs::read(&out).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(&head[..4], b"\x89PNG");
}

#[test]
fn export_without_selection_is_an_error() {
    let b = Browser::new(&shows()).unwrap();
    assert!(b.export_selected(&std::env::temp_dir()).is_err());
}

#[test]
fn go_to_a_missing_folder_reports_an_error_and_keeps_the_listing() {
    let mut b = Browser::new(&shows()).unwrap();
    let n = b.entries().len();
    assert!(b.go_to(&shows().join("nope")).is_err());
    assert_eq!(b.entries().len(), n);
}

#[test]
fn mct_selection_offers_data_to_decode_and_accepts_the_preview_for_that_file_only() {
    let mut b = Browser::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/dlcs/episodes")).unwrap();
    let live = index_of(&b, "83136FF343124795DC1806A2DB044476589BD5C0");
    b.open(live).unwrap();
    let xzp = index_of(&b, "episode.xzp");
    b.open(xzp).unwrap();
    let mct = index_of(&b, "answer_glow.png");
    b.open(mct).unwrap();
    let (name, data) = b.pending_preview().expect("mct needs decoding");
    assert_eq!(&data[..4], b".mct");
    let preview = crate::inspect::Preview { width: 1, height: 1, rgba: vec![1, 2, 3, 4] };
    assert!(!b.set_preview("another.png", preview.clone()));
    assert!(b.detail().unwrap().image.is_none());
    assert!(b.set_preview(&name, preview.clone()));
    assert_eq!(b.detail().unwrap().image, Some(preview));
    assert!(b.pending_preview().is_none());
}
