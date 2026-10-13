use crate::{lzx, model, strb, toc, ytgr};
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::Mutex;

const ENV_PACK: &str = "ONEVS100_AVATAR_PACK";
const PACK_FILE: &str = "AvatarAssetPack.toc";
const MODEL_TYPE: u32 = 1;
const BODY_CATEGORY: u32 = 2;
const HEAD_CATEGORY: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Male = 1,
    Female = 2,
}

impl Gender {
    pub fn from_flags(flags: u8) -> Gender {
        if flags & Gender::Male as u8 == 0 && flags & Gender::Female as u8 != 0 { Gender::Female } else { Gender::Male }
    }
}

pub fn decode(asset: &[u8]) -> Result<model::Model> {
    let s = strb::parse(asset)?;
    let b = s.blocks.iter().find(|b| b.id == 3).ok_or_else(|| anyhow!("asset has no model block"))?;
    model::parse(&lzx::decompress(&asset[b.start..b.start + b.length])?)
}

fn meta(asset: &[u8]) -> Option<ytgr::Metadata> {
    let s = strb::parse(asset).ok()?;
    let b = s.blocks.iter().find(|b| b.id == 8)?;
    ytgr::metadata(&asset[b.start..b.start + b.length]).ok()
}

pub fn from_pack(pack: &[u8], gender: Gender) -> Result<Vec<model::Model>> {
    let assets = toc::split(pack);
    let pick = |category: u32, wanted: Option<u8>| {
        assets
            .iter()
            .filter(|a| a.asset_type == MODEL_TYPE)
            .find(|a| meta(a.data).is_some_and(|m| m.mask == category && wanted.is_none_or(|g| m.flags == g)))
            .ok_or_else(|| anyhow!("pack has no base asset of category {category}"))
    };
    [pick(BODY_CATEGORY, Some(gender as u8))?, pick(HEAD_CATEGORY, None)?].iter().map(|a| decode(a.data)).collect()
}

pub fn candidates() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::env::var_os(ENV_PACK).map(PathBuf::from).into_iter().collect();
    paths.push(crate::assets::Store::user().pack_path());
    paths.extend(std::env::current_dir().ok().map(|d| d.join(PACK_FILE)));
    paths.extend(std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(PACK_FILE))));
    paths
}

pub fn first_readable(paths: &[PathBuf]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

pub fn pack_bytes() -> Option<&'static [u8]> {
    static PACK: Mutex<Option<&'static [u8]>> = Mutex::new(None);
    let mut cached = PACK.lock().ok()?;
    if cached.is_none() {
        *cached = first_readable(&candidates()).map(|bytes| &*Box::leak(bytes.into_boxed_slice()));
    }
    *cached
}

pub fn locate(gender: Gender) -> Option<&'static [model::Model]> {
    static CACHE: [Mutex<Option<&'static [model::Model]>>; 2] = [Mutex::new(None), Mutex::new(None)];
    let mut cached = CACHE[gender as usize - 1].lock().ok()?;
    if cached.is_none() {
        *cached = pack_bytes().and_then(|d| from_pack(d, gender).ok()).map(|models| &*Box::leak(models.into_boxed_slice()));
    }
    *cached
}
