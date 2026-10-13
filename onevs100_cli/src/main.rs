use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use onevs100_core::{toc, render, anim, audio, caff, inspect, lzx, model, stfs, texture, xcontent, xui, xwb, ytgr};
use onevs100_core::assets::{self, Store};
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

const EXIT_NEEDS_ASSETS: i32 = 2;

#[derive(Parser)]
#[command(name = "onevs100", version, about = "1 vs 100 content package editor")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Import the avatar asset pack from the 17559 system update (zip, folder, package file or AvatarAssetPack.toc)")]
    ImportAssets { path: PathBuf },
    Info { path: PathBuf },
    ScanAssets { root: PathBuf },
    Preview { path: PathBuf, output: PathBuf, #[arg(long)] animation: Option<usize>, #[arg(long, default_value_t = 0)] frame: usize, #[arg(long)] yaw: Option<f32>, #[arg(long)] pitch: Option<f32>, #[arg(long)] zoom: Option<f32>, #[arg(long, default_value_t = 0.0)] pan_x: f32, #[arg(long, default_value_t = 0.0)] pan_y: f32, #[arg(long, default_value_t = 384)] size: u32 },
    AssetBlock { path: PathBuf, id: u32, output: PathBuf, #[arg(long)] raw: bool },
    WaveExport { path: PathBuf, index: usize, output: PathBuf, #[arg(long)] pcm: bool },
    XzpList { path: PathBuf },
    XzpExtract { path: PathBuf, dir: PathBuf },
    XzpPack { dir: PathBuf, output: PathBuf },
    XusStrings { path: PathBuf },
    CaffInfo { path: PathBuf },
    StfsList { path: PathBuf },
    StfsInfo { path: PathBuf },
    TocRender { path: PathBuf, dir: PathBuf },
    TocTextures { path: PathBuf, dir: PathBuf },
    StfsExtract { path: PathBuf, dir: PathBuf },
    TexList { path: PathBuf },
    TexPng { path: PathBuf, index: usize, output: PathBuf, #[arg(long, default_value_t = 0)] level: u32 },
}

fn prompt_for_assets(store: &Store) -> Result<()> {
    let stdin = std::io::stdin();
    loop {
        eprint!("Path to the 17559 system update (empty to quit): ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 || line.trim().is_empty() {
            bail!("the avatar assets are required");
        }
        match store.import(std::path::Path::new(line.trim().trim_matches('\''))) {
            Ok(done) => {
                eprintln!("Imported {} bytes from {}.", done.bytes, done.source);
                return Ok(());
            }
            Err(e) => eprintln!("{e:#}"),
        }
    }
}

fn guard() {
    let store = Store::user();
    if assets::available() {
        return;
    }
    eprintln!("{}", assets::REQUIREMENT);
    let ready = std::io::stdin().is_terminal() && prompt_for_assets(&store).is_ok();
    if !ready {
        eprintln!("Run `onevs100_cli import-assets <path>` first (data is stored in {}).", store.dir().display());
        std::process::exit(EXIT_NEEDS_ASSETS);
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Command::ImportAssets { path } = &cli.command {
        let done = Store::user().import(path)?;
        println!("Imported {} bytes from {} into {}.", done.bytes, done.source, Store::user().dir().display());
        return Ok(());
    }
    guard();
    match cli.command {
        Command::ImportAssets { .. } => unreachable!(),
        Command::Info { path } => {
            let detail = inspect::describe(&path.file_name().unwrap_or_default().to_string_lossy(), &std::fs::read(&path)?);
            println!("{}", detail.kind);
            detail.lines.iter().for_each(|l| println!("{l}"));
        }
        Command::WaveExport { path, index, output, pcm } => {
            let bytes = std::fs::read(path)?;
            let wav = audio::wav(&bytes, &xwb::parse(&bytes)?, index)?;
            std::fs::write(output, if pcm { audio::decode_pcm(&wav)? } else { wav })?;
        }
        Command::AssetBlock { path, id, output, raw } => {
            let bytes = std::fs::read(path)?;
            let block = ytgr::parse(&bytes)?.block_data(&bytes, id)?.to_vec();
            std::fs::write(output, if raw { block } else { lzx::decompress(&block).unwrap_or(block) })?;
        }
        Command::Preview { path, output, animation: Some(id), frame, .. } => {
            let pack = onevs100_core::body::pack_bytes().ok_or_else(|| anyhow::anyhow!("avatar pack not found (set ONEVS100_AVATAR_PACK)"))?;
            let figure = onevs100_core::figure::build(&std::fs::read(&path)?, pack)?;
            let posed = figure.posed(&onevs100_core::figure::animation_rotations(pack, id, frame)?)?;
            std::fs::write(output, inspect::preview_png(&posed.preview(384, 384))?)?;
        }
        Command::Preview { path, output, yaw, pitch, zoom, pan_x, pan_y, size, .. } => {
            let detail = inspect::describe(&path.file_name().unwrap_or_default().to_string_lossy(), &std::fs::read(&path)?);
            let camera = render::Camera { yaw: yaw.unwrap_or(render::Camera::default().yaw), pitch: pitch.unwrap_or(render::Camera::default().pitch), zoom: zoom.unwrap_or(1.0), pan: [pan_x, pan_y] };
            let image = match &detail.scene {
                Some(scene) => scene.0.render(&camera, size, size),
                None => detail.image.ok_or_else(|| anyhow::anyhow!("{} has no preview", path.display()))?,
            };
            std::fs::write(output, inspect::preview_png(&image)?)?;
        }
        Command::ScanAssets { root } => scan_assets(&root)?,
        Command::XzpList { path } => {
            for e in xui::read_xuiz(&std::fs::read(path)?)? {
                println!("{:>9}  {}", e.data.len(), e.name);
            }
        }
        Command::XzpExtract { path, dir } => {
            xui::unpack_dir(&xui::read_xuiz(&std::fs::read(path)?)?, &dir)?;
        }
        Command::XzpPack { dir, output } => {
            let entries = xui::pack_dir(&dir)?;
            std::fs::write(&output, xui::write_xuiz(&entries)?)?;
            println!("wrote {}: {} entries", output.display(), entries.len());
        }
        Command::XusStrings { path } => {
            for s in xui::read_xus(&std::fs::read(path)?)? {
                println!("{s:?}");
            }
        }
        Command::CaffInfo { path } => {
            let c = caff::parse(&std::fs::read(path)?)?;
            println!("blocks {:x?}", c.sizes);
            println!("base {:x?}", c.base);
            println!("relocs {}", c.relocs.map_or(0, |r| r.len()));
        }
        Command::StfsList { path } => {
            for f in stfs::list(&std::fs::read(path)?)? {
                println!("{:>9}  {}", f.size, f.path);
            }
        }
        Command::StfsExtract { path, dir } => {
            let data = std::fs::read(path)?;
            for f in stfs::list(&data)? {
                let target = dir.join(&f.path);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(target, stfs::extract(&data, &f.path)?)?;
            }
        }
        Command::TocRender { path, dir } => {
            let data = std::fs::read(path)?;
            std::fs::create_dir_all(&dir)?;
            for a in toc::split(&data).iter().filter(|a| a.asset_type == 1) {
                let i = a.id;
                let s = onevs100_core::strb::parse(a.data)?;
                let b = s.blocks.iter().find(|b| b.id == 3).ok_or_else(|| anyhow::anyhow!("asset {i} has no model"))?;
                let m = model::parse(&lzx::decompress(&a.data[b.start..b.start + b.length])?)?;
                std::fs::write(dir.join(format!("{i:04}.png")), inspect::preview_png(&render::preview(&m, 256, 256))?)?;
            }
        }
        Command::TocTextures { path, dir } => {
            let data = std::fs::read(path)?;
            std::fs::create_dir_all(&dir)?;
            for a in toc::split(&data).iter().filter(|a| a.asset_type == 2) {
                let s = onevs100_core::strb::parse(a.data)?;
                let b = s.blocks.iter().find(|b| b.id == 2).ok_or_else(|| anyhow::anyhow!("asset {} has no texture", a.id))?;
                let t = model::parse_texture(&lzx::decompress(&a.data[b.start..b.start + b.length])?)?;
                let meta = s.blocks.iter().find(|b| b.id == 8).and_then(|b| onevs100_core::ytgr::metadata(&a.data[b.start..b.start + b.length]).ok());
                let (flags, variant) = meta.map_or((0, 0), |m| (m.flags, m.mask));
                let image = inspect::Preview { width: t.width, height: t.height, rgba: t.rgba()? };
                std::fs::write(dir.join(format!("{:04}_f{flags}_v{variant}_{}x{}x{}.png", a.id, t.width, t.height, t.layers)), inspect::preview_png(&image)?)?;
            }
        }
        Command::TexList { path } => {
            for t in texture::list(&std::fs::read(path)?)? {
                println!("{}  {:?} {}x{} mips {} gpu {:x?}", t.index, t.format, t.width, t.height, t.mips, t.gpu);
            }
        }
        Command::TexPng { path, index, output, level } => {
            let raw = std::fs::read(path)?;
            let list = texture::list(&raw)?;
            let t = list.get(index).ok_or_else(|| anyhow::anyhow!("no texture {index}"))?;
            std::fs::write(output, texture::to_png(&raw, t, level)?)?;
        }
        Command::StfsInfo { path } => xcontent::parse(&std::fs::read(path)?)?.lines.iter().for_each(|l| println!("{l}")),
    }
    Ok(())
}

fn asset_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == "asset_v2.bin") {
                found.push(p);
            }
        }
    }
    found.sort();
    found
}

fn check_asset(data: &[u8]) -> Result<(usize, usize, bool)> {
    let y = ytgr::parse(data)?;
    let (mut triangles, mut textures) = (0, 0);
    if let Ok(block) = y.block_data(data, 3) {
        let m = model::parse(&lzx::decompress(block)?)?;
        triangles = m.batches.iter().map(|b| b.triangles as usize).sum();
        textures = m.textures.iter().filter(|t| t.rgba().is_ok()).count();
    }
    let animated = match y.block_data(data, 1) {
        Ok(block) => anim::parse_block(block).map(|_| true)?,
        Err(_) => false,
    };
    Ok((triangles, textures, animated))
}

fn scan_assets(root: &std::path::Path) -> Result<()> {
    let files = asset_files(root);
    let (mut ok, mut failures, mut triangles, mut animations) = (0usize, std::collections::BTreeMap::<String, Vec<PathBuf>>::new(), 0usize, 0usize);
    for path in &files {
        match std::fs::read(path).map_err(anyhow::Error::from).and_then(|d| check_asset(&d)) {
            Ok((t, _, animated)) => {
                ok += 1;
                triangles += t;
                animations += usize::from(animated);
            }
            Err(e) => failures.entry(e.to_string()).or_default().push(path.clone()),
        }
    }
    println!("{} assets: {ok} decoded ({animations} with animations, {triangles} triangles), {} failed", files.len(), files.len() - ok);
    for (reason, paths) in &failures {
        println!("{:>6}  {reason}  e.g. {}", paths.len(), paths[0].display());
    }
    Ok(())
}
