use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const ZIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/su/SystemUpdate_17559_USB.zip");
const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/avatar/FemaleHost.avd");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("onevs100_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn run(data: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_onevs100_cli")).args(args).env("ONEVS100_DATA_DIR", data).env_remove("ONEVS100_AVATAR_PACK").stdin(Stdio::null()).output().unwrap()
}

#[test]
fn every_command_is_refused_until_the_assets_are_imported() {
    let data = scratch("guard");
    for args in [&["info", SAMPLE][..], &["xzp-list", "x.xzp"], &["scan-assets", "/tmp"]] {
        let out = run(&data, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(text.contains("import-assets") && text.contains("17559"), "{text}");
    }
    assert!(!data.exists());
}

#[test]
fn importing_unlocks_the_commands_and_persists() {
    let data = scratch("unlock");
    let bad = run(&data, &["import-assets", SAMPLE]);
    assert_ne!(bad.status.code(), Some(0));
    let good = run(&data, &["import-assets", ZIP]);
    assert_eq!(good.status.code(), Some(0), "{}", String::from_utf8_lossy(&good.stderr));
    assert!(data.join("AvatarAssetPack.toc").exists());
    let info = run(&data, &["info", SAMPLE]);
    assert_eq!(info.status.code(), Some(0), "{}", String::from_utf8_lossy(&info.stderr));
    assert!(String::from_utf8_lossy(&info.stdout).contains("Avatar metadata"));
    let again = run(&data, &["import-assets", ZIP]);
    assert_eq!(again.status.code(), Some(0));
}

#[test]
fn help_does_not_need_the_assets() {
    let out = run(&scratch("help"), &["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("import-assets"));
}
