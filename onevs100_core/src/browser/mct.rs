use crate::inspect::Preview;
use anyhow::{anyhow, bail, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const TOOLS_SUBDIR: &str = "common/content_package_editor";
const HELPER: &str = "mct_decode.py";

pub fn tool_candidates(env_override: Option<PathBuf>, exe_dir: Option<PathBuf>, cwd: Option<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = env_override.into_iter().collect();
    for start in [exe_dir, cwd].into_iter().flatten() {
        out.extend(start.ancestors().map(|a| a.join(TOOLS_SUBDIR)));
    }
    out.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(TOOLS_SUBDIR));
    out
}

pub fn locate_tools(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|c| c.join(HELPER).exists())
}

pub fn missing_tools_message(searched: &[PathBuf]) -> String {
    let list = searched.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ");
    format!("MCT decoder helper ({HELPER}) not found; looked in: {list}. Set ONEVS100_TOOLS to the content_package_editor folder (it needs {HELPER}, a .venv or python3 with unicorn, and rev3/tools/ppc/title-1317.bin in the project)")
}

fn candidates() -> Vec<PathBuf> {
    tool_candidates(
        std::env::var_os("ONEVS100_TOOLS").map(PathBuf::from),
        std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)),
        std::env::current_dir().ok(),
    )
}

fn tools() -> Result<PathBuf> {
    let searched = candidates();
    locate_tools(searched.clone()).ok_or_else(|| anyhow!(missing_tools_message(&searched)))
}

fn python(tools: &std::path::Path) -> PathBuf {
    std::env::var_os("ONEVS100_MCT_PYTHON").map(PathBuf::from).unwrap_or_else(|| {
        let venv = tools.join(".venv/bin/python");
        if venv.exists() { venv } else { PathBuf::from("python3") }
    })
}

fn cache_file(data: &[u8]) -> PathBuf {
    let hash = data.iter().fold(FNV_OFFSET, |h, &b| (h ^ u64::from(b)).wrapping_mul(FNV_PRIME));
    std::env::temp_dir().join("onevs100-mct-cache").join(format!("{hash:016x}.png"))
}

pub fn decoder_available() -> bool {
    tools().is_ok_and(|t| Command::new(python(&t)).args(["-I", "-c", "import unicorn"]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success()))
}

pub fn cached(data: &[u8]) -> bool {
    cache_file(data).exists()
}

fn run_helper(data: &[u8]) -> Result<Vec<u8>> {
    let tools = tools()?;
    let python = python(&tools);
    let mut child = Command::new(&python)
        .args(["-I"])
        .arg(tools.join(HELPER))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("could not start {} for the MCT decoder: {e}", python.display()))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let payload = data.to_vec();
    let feeder = std::thread::spawn(move || stdin.write_all(&payload));
    let output = child.wait_with_output()?;
    feeder.join().ok();
    if !output.status.success() || output.stdout.is_empty() {
        bail!("MCT decoder failed: {}", String::from_utf8_lossy(&output.stderr).lines().last().unwrap_or("no output"));
    }
    Ok(output.stdout)
}

pub fn decode(data: &[u8]) -> Result<Preview> {
    let path = cache_file(data);
    let png = match std::fs::read(&path) {
        Ok(png) => png,
        Err(_) => {
            let png = run_helper(data)?;
            std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
            std::fs::write(&path, &png)?;
            png
        }
    };
    let img = image::load_from_memory(&png)?.into_rgba8();
    let (width, height) = img.dimensions();
    Ok(Preview { width, height, rgba: img.into_raw() })
}
