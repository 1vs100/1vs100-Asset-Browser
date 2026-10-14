use anyhow::{anyhow, bail, ensure, Context, Result};
use sha1::{Digest, Sha1};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const PACK_FILE: &str = "AvatarAssetPack.toc";
pub const PACK_PACKAGE: &str = "FFFE07DF00000002";
pub const PACK_BYTES: u64 = 14_662_168;
const PACK_SHA1: &str = "425856323f37decb1b889afd19fa6ba91b045719";
const DATA_DIR_ENV: &str = "ONEVS100_DATA_DIR";
const APP_DIR: &str = "onevs100";
const SEARCH_DEPTH: usize = 3;
const ZIP_MAGIC: &[u8] = b"PK";
const STFS_MAGICS: [&[u8]; 3] = [b"LIVE", b"PIRS", b"CON "];

pub const REQUIREMENT: &str = "The 1 vs 100 tools need the Xbox 360 avatar asset pack from system update 17559 (SystemUpdate_17559_USB.zip, file $SystemUpdate/FFFE07DF00000002). Choose that zip, the extracted $SystemUpdate folder, the FFFE07DF00000002 file or AvatarAssetPack.toc once; it is stored for later runs.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    pub source: String,
    pub bytes: usize,
}

#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn user() -> Store {
        let dir = std::env::var_os(DATA_DIR_ENV).map(PathBuf::from).or_else(|| dirs::data_dir().map(|d| d.join(APP_DIR))).unwrap_or_else(|| PathBuf::from(APP_DIR));
        Store { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn pack_path(&self) -> PathBuf {
        self.dir.join(PACK_FILE)
    }

    pub fn is_installed(&self) -> bool {
        std::fs::metadata(self.pack_path()).is_ok_and(|m| m.len() == PACK_BYTES)
    }

    pub fn import(&self, source: &Path) -> Result<Imported> {
        let pack = read_pack(source, SEARCH_DEPTH).map_err(|e| anyhow!("could not read the avatar pack from {}: {e:#}", source.display()))?;
        verify(&pack)?;
        std::fs::create_dir_all(&self.dir).with_context(|| format!("could not create {}", self.dir.display()))?;
        let partial = self.dir.join(format!("{PACK_FILE}.partial"));
        std::fs::write(&partial, &pack)?;
        std::fs::rename(&partial, self.pack_path())?;
        Ok(Imported { source: source.display().to_string(), bytes: pack.len() })
    }
}

fn verify(pack: &[u8]) -> Result<()> {
    let digest: String = Sha1::digest(pack).iter().map(|b| format!("{b:02x}")).collect();
    ensure!(digest == PACK_SHA1, "this is not the avatar asset pack from system update 17559 (SHA-1 {digest}); {REQUIREMENT}");
    Ok(())
}

fn read_pack(source: &Path, depth: usize) -> Result<Vec<u8>> {
    if source.is_dir() {
        ensure!(depth > 0, "no avatar pack found in this folder");
        let mut entries: Vec<PathBuf> = std::fs::read_dir(source)?.flatten().map(|e| e.path()).collect();
        entries.sort();
        let named = entries.iter().find(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(PACK_FILE) || n.eq_ignore_ascii_case(PACK_PACKAGE)));
        if let Some(found) = named {
            return read_pack(found, depth - 1);
        }
        return entries.iter().filter(|p| p.is_dir() || p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))).find_map(|p| read_pack(p, depth - 1).ok()).ok_or_else(|| anyhow!("no avatar pack found in this folder. {REQUIREMENT}"));
    }
    let mut file = std::fs::File::open(source)?;
    let mut magic = [0u8; 4];
    let got = file.read(&mut magic)?;
    drop(file);
    if magic[..got.min(2)] == *ZIP_MAGIC {
        return read_zip(source);
    }
    let data = std::fs::read(source)?;
    if STFS_MAGICS.iter().any(|m| data.starts_with(m)) {
        return crate::stfs::extract(&data, PACK_FILE).map_err(|e| anyhow!("the package does not contain {PACK_FILE}: {e}"));
    }
    if data.len() as u64 == PACK_BYTES {
        return Ok(data);
    }
    bail!("unrecognised file; {REQUIREMENT}")
}

fn read_zip(source: &Path) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(source)?)?;
    let wanted = [PACK_PACKAGE.to_lowercase(), PACK_FILE.to_lowercase()];
    let names: Vec<String> = archive.file_names().flatten().map(|n| n.into_owned()).collect();
    let name = names.into_iter().find(|n| wanted.iter().any(|w| n.to_lowercase().ends_with(w))).ok_or_else(|| anyhow!("the zip has no {PACK_PACKAGE} (is it the 17559 update?)"))?;
    let mut data = Vec::new();
    archive.by_name(&name)?.read_to_end(&mut data)?;
    if STFS_MAGICS.iter().any(|m| data.starts_with(m)) {
        return crate::stfs::extract(&data, PACK_FILE).map_err(|e| anyhow!("{name} does not contain {PACK_FILE}: {e}"));
    }
    Ok(data)
}

pub fn available() -> bool {
    Store::user().is_installed() || std::env::var_os("ONEVS100_AVATAR_PACK").is_some_and(|p| Path::new(&p).is_file())
}
