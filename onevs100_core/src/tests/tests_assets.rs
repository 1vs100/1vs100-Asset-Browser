use crate::assets::{self, Store};
use std::path::{Path, PathBuf};

const ZIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/su/SystemUpdate_17559_USB.zip");
const UPDATE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../testdata/su"
);
const PIRS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../testdata/su/FFFE07DF00000002"
);
const EXTRACTED_TOC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../testdata/avatar/AvatarAssetPack.toc"
);

fn scratch(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "onevs100_assets_{name}_{}_{id}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn installed(source: &str) {
    let store = Store::at(scratch("ok"));
    let _ = std::fs::metadata(source).unwrap();
    assert!(!store.is_installed());
    let done = store.import(Path::new(source)).unwrap();
    assert!(store.is_installed());
    assert_eq!(done.bytes, 14_662_168);
    assert_eq!(std::fs::read(store.pack_path()).unwrap().len(), 14_662_168);
}

#[test]
fn a_fresh_store_is_not_installed() {
    let store = Store::at(scratch("fresh"));
    assert!(!store.is_installed());
    assert!(store.pack_path().ends_with(assets::PACK_FILE));
}

#[test]
fn the_system_update_zip_installs_the_pack() {
    installed(ZIP);
}

#[test]
fn an_extracted_update_folder_installs_the_pack() {
    installed(UPDATE_DIR);
}

#[test]
fn the_avatar_package_file_installs_the_pack() {
    installed(PIRS);
}

#[test]
fn the_extracted_pack_file_installs_itself() {
    installed(EXTRACTED_TOC);
}

#[test]
fn unrelated_input_is_rejected_with_a_helpful_message_and_installs_nothing() {
    let store = Store::at(scratch("bad"));
    let junk = store.dir().join("junk.bin");
    std::fs::write(&junk, vec![7u8; 4096]).unwrap();
    let message = store.import(&junk).unwrap_err().to_string();
    assert!(
        message.contains("17559") && message.contains("junk.bin"),
        "{message}"
    );
    assert!(store.import(Path::new("/definitely/not/here")).is_err());
    assert!(store.import(store.dir()).is_err());
    assert!(!store.is_installed());
}

#[test]
fn a_pack_of_the_wrong_size_does_not_count_as_installed() {
    let store = Store::at(scratch("partial"));
    std::fs::write(store.pack_path(), b"truncated").unwrap();
    assert!(!store.is_installed());
}

#[test]
fn the_user_store_honours_the_data_dir_override() {
    let dir = scratch("env");
    std::env::set_var("ONEVS100_DATA_DIR", &dir);
    assert_eq!(Store::user().dir(), dir.as_path());
    std::env::remove_var("ONEVS100_DATA_DIR");
}

#[test]
fn the_first_readable_candidate_wins() {
    let dir = scratch("cand");
    let (a, b) = (dir.join("a.toc"), dir.join("b.toc"));
    std::fs::write(&b, b"second").unwrap();
    assert_eq!(
        crate::body::first_readable(&[a.clone(), b.clone()]),
        Some(b"second".to_vec())
    );
    assert_eq!(crate::body::first_readable(&[a]), None);
}

#[test]
fn the_search_path_includes_the_user_store() {
    assert!(crate::body::candidates().contains(&Store::user().pack_path()));
}
