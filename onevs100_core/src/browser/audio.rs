use crate::xwb::{Bank, Codec};
use anyhow::{anyhow, bail, ensure, Result};
use std::io::Write;
use std::process::{Child, Command, Stdio};

const STANDARD_RATES: [u32; 8] = [8000, 11025, 16000, 22050, 24000, 32000, 44100, 48000];
const XMA2_TAG: u16 = 0x166;
const XMA2_BLOCK_BYTES: u32 = 65536;
const XMA2_EXTRA_BYTES: u16 = 34;

fn snap_rate(rate: u32) -> u32 {
    *STANDARD_RATES.iter().min_by_key(|&&s| s.abs_diff(rate)).expect("rate table is not empty")
}

fn riff(fmt: &[u8], data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(20 + fmt.len() + data.len());
    body.extend_from_slice(b"WAVEfmt ");
    body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    body.extend_from_slice(fmt);
    body.extend_from_slice(b"data");
    body.extend_from_slice(&(data.len() as u32).to_le_bytes());
    body.extend_from_slice(data);
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

fn xma2(stream: &[u8], rate: u32, channels: u32, samples: u32) -> Vec<u8> {
    let streams = ((channels + 1) / 2) as u16;
    let mask: u32 = if channels == 2 { 3 } else { 1 };
    let blocks = (stream.len() as u32).div_ceil(XMA2_BLOCK_BYTES).max(1) as u16;
    let mut extra = Vec::with_capacity(usize::from(XMA2_EXTRA_BYTES));
    extra.extend_from_slice(&streams.to_le_bytes());
    for v in [mask, samples, XMA2_BLOCK_BYTES, 0, samples, 0, 0] {
        extra.extend_from_slice(&v.to_le_bytes());
    }
    extra.extend_from_slice(&[0, 4]);
    extra.extend_from_slice(&blocks.to_le_bytes());
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&XMA2_TAG.to_le_bytes());
    fmt.extend_from_slice(&(channels as u16).to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * channels * 2).to_le_bytes());
    fmt.extend_from_slice(&((channels * 2) as u16).to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    fmt.extend_from_slice(&XMA2_EXTRA_BYTES.to_le_bytes());
    fmt.extend_from_slice(&extra);
    riff(&fmt, stream)
}

fn pcm(stream: &[u8], rate: u32, channels: u32, sixteen_bit: bool) -> Vec<u8> {
    let (bits, bytes) = if sixteen_bit { (16u16, 2usize) } else { (8, 1) };
    let block = channels as u16 * bits / 8;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&(channels as u16).to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    fmt.extend_from_slice(&block.to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());
    let data: Vec<u8> = if bytes == 2 { stream.chunks_exact(2).flat_map(|c| [c[1], c[0]]).collect() } else { stream.to_vec() };
    riff(&fmt, &data)
}

pub fn wav(bank_bytes: &[u8], bank: &Bank, index: usize) -> Result<Vec<u8>> {
    let e = bank.entries.get(index).ok_or_else(|| anyhow!("no wave {index} in bank {} ({} entries)", bank.name, bank.entries.len()))?;
    let start = bank.data_offset + e.offset;
    let stream = bank_bytes.get(start..start + e.length).ok_or_else(|| anyhow!("wave {index} is outside the bank"))?;
    let rate = snap_rate(e.rate);
    match e.codec {
        Codec::Xma => Ok(xma2(stream, rate, e.channels, e.samples)),
        Codec::Pcm => Ok(pcm(stream, e.rate, e.channels, e.sixteen_bit)),
        other => bail!("{other:?} waves cannot be played yet"),
    }
}

pub fn xma_decoder_available() -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-decoders"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| matches!(l.split_whitespace().nth(1), Some("xma1" | "xma2"))))
}

pub fn decode_pcm(wav: &[u8]) -> Result<Vec<u8>> {
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-i", "pipe:0", "-f", "wav", "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("could not start ffmpeg: {e}"))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let input = wav.to_vec();
    let feeder = std::thread::spawn(move || stdin.write_all(&input));
    let out = child.wait_with_output()?;
    feeder.join().ok();
    ensure!(out.status.success() && !out.stdout.is_empty(), "ffmpeg could not decode this wave: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("no output"));
    Ok(out.stdout)
}

pub struct Player {
    program: String,
    args: Vec<String>,
    child: Option<Child>,
}

impl Player {
    pub fn new() -> Self {
        Player::with_command("ffplay", &["-nodisp", "-autoexit", "-loglevel", "error", "-i", "pipe:0"])
    }

    pub fn with_command(program: &str, args: &[&str]) -> Self {
        Player { program: program.to_owned(), args: args.iter().map(|a| (*a).to_owned()).collect(), child: None }
    }

    pub fn play(&mut self, wav: &[u8]) -> Result<()> {
        self.stop();
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("could not start {} for playback ({e}); install ffmpeg's ffplay or put another player on PATH", self.program))?;
        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let data = wav.to_vec();
        std::thread::spawn(move || stdin.write_all(&data));
        self.child = Some(child);
        Ok(())
    }

    pub fn is_playing(&mut self) -> bool {
        match self.child.as_mut().map(Child::try_wait) {
            Some(Ok(None)) => true,
            Some(_) => {
                self.child = None;
                false
            }
            None => false,
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().ok();
            child.wait().ok();
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Player::new()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}
