use crate::source::{self, Entry};
use std::path::PathBuf;

fn dlcs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/dlcs")
}

pub(crate) fn packages() -> Vec<(String, String, Vec<Entry>)> {
    let mut packages = Vec::new();
    for role in ["shows", "episodes"] {
        for package in std::fs::read_dir(dlcs().join(role)).unwrap().flatten() {
            let id = package.file_name().to_string_lossy().into_owned();
            let entry = Entry {
                name: id.clone(),
                is_dir: false,
                payload: crate::source::Payload::Disk(package.path()),
            };
            let files = source::children(&entry).unwrap().unwrap();
            packages.push((role.to_owned(), id, files));
        }
    }
    packages
}

pub(crate) fn package_file(role: &str, id: &str, path: &str) -> Vec<u8> {
    let package = dlcs().join(role).join(id);
    let entry = Entry { name: id.to_owned(), is_dir: false, payload: crate::source::Payload::Disk(package) };
    let files = source::children(&entry).unwrap().unwrap();
    let path = path.replace('\\', "/");
    files
        .into_iter()
        .find(|file| file.name.replace('\\', "/").eq_ignore_ascii_case(&path))
        .unwrap_or_else(|| panic!("{role}/{id}/{path} not found in package"))
        .payload
        .read()
        .unwrap()
}
