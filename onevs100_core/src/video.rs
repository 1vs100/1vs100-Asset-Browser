use anyhow::{anyhow, bail, ensure, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const STREAM_PROPERTIES: [u8; 16] = [0x91, 0x07, 0xdc, 0xb7, 0xb7, 0xa9, 0xcf, 0x11, 0x8e, 0xe6, 0x00, 0xc0, 0x0c, 0x20, 0x53, 0x65];
const VIDEO_MEDIA: [u8; 16] = [0xc0, 0xef, 0x19, 0xbc, 0x4d, 0x5b, 0xcf, 0x11, 0xa8, 0xfd, 0x00, 0x80, 0x5f, 0x5c, 0x44, 0x2b];
const AUDIO_MEDIA: [u8; 16] = [0x40, 0x9e, 0x69, 0xf8, 0x4d, 0x5b, 0xcf, 0x11, 0xa8, 0xfd, 0x00, 0x80, 0x5f, 0x5c, 0x44, 0x2b];
const FILE_PROPERTIES: [u8; 16] = [0xa1, 0xdc, 0xab, 0x8c, 0x47, 0xa9, 0xcf, 0x11, 0x8e, 0xe4, 0x00, 0xc0, 0x0c, 0x20, 0x53, 0x65];
const OBJECT_HEADER: usize = 24;
const HEADER_OBJECT: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct VideoStream {
    pub fourcc: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStream {
    pub format_tag: u16,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AsfInfo {
    pub seconds: f64,
    pub video: Option<VideoStream>,
    pub audio: Option<AudioStream>,
}

fn u16le(d: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(d.get(o..o + 2).ok_or_else(|| anyhow!("truncated at {o:#x}"))?.try_into()?))
}

fn u32le(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(d.get(o..o + 4).ok_or_else(|| anyhow!("truncated at {o:#x}"))?.try_into()?))
}

fn u64le(d: &[u8], o: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(d.get(o..o + 8).ok_or_else(|| anyhow!("truncated at {o:#x}"))?.try_into()?))
}

fn stream(object: &[u8], info: &mut AsfInfo) -> Result<()> {
    let kind = object.get(OBJECT_HEADER..OBJECT_HEADER + 16).ok_or_else(|| anyhow!("short stream object"))?;
    let specific = OBJECT_HEADER + 16 + 16 + 8 + 4 + 4 + 2 + 4;
    if kind == VIDEO_MEDIA {
        let bitmap = specific + 11;
        let fourcc = String::from_utf8_lossy(object.get(bitmap + 16..bitmap + 20).ok_or_else(|| anyhow!("short video format"))?).into_owned();
        info.video = Some(VideoStream { fourcc, width: u32le(object, specific)?, height: u32le(object, specific + 4)? });
    } else if kind == AUDIO_MEDIA {
        info.audio = Some(AudioStream { format_tag: u16le(object, specific)?, channels: u16le(object, specific + 2)?, sample_rate: u32le(object, specific + 4)? });
    }
    Ok(())
}

pub fn asf_info(data: &[u8]) -> Result<AsfInfo> {
    ensure!(data.len() > HEADER_OBJECT && data[..4] == [0x30, 0x26, 0xb2, 0x75], "not an ASF file");
    let header_size = (u64le(data, 16)? as usize).min(data.len());
    let mut info = AsfInfo::default();
    let mut at = HEADER_OBJECT;
    while at + OBJECT_HEADER <= header_size {
        let size = u64le(data, at + 16)? as usize;
        ensure!(size >= OBJECT_HEADER && at + size <= data.len(), "bad ASF object at {at:#x}");
        let object = &data[at..at + size];
        let guid = &object[..16];
        if guid == FILE_PROPERTIES {
            info.seconds = u64le(object, 64)? as f64 / 1e7 - u64le(object, 80)? as f64 / 1000.0;
        } else if guid == STREAM_PROPERTIES {
            stream(object, &mut info)?;
        }
        at += size;
    }
    Ok(info)
}

fn ffmpeg_output(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("ffmpeg").args(["-hide_banner", "-v", "error"]).args(args).stdin(Stdio::null()).output()
}

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

pub fn decoder_available() -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-decoders"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.split_whitespace().nth(1) == Some("wmv3")))
}

pub fn frame_png(data: &[u8], seconds: f64) -> Result<Vec<u8>> {
    let path = std::env::temp_dir().join(format!("onevs100-video-{}-{}.bin", std::process::id(), data.len()));
    std::fs::File::create(&path)?.write_all(data)?;
    let input = path.to_string_lossy().into_owned();
    let output = ffmpeg_output(&["-ss", &format!("{seconds:.3}"), "-i", &input, "-frames:v", "1", "-f", "image2pipe", "-vcodec", "png", "pipe:1"]);
    std::fs::remove_file(&path).ok();
    let output = output?;
    if output.stdout.is_empty() {
        bail!("no frame decoded: {}", String::from_utf8_lossy(&output.stderr).lines().last().unwrap_or("ffmpeg not found"));
    }
    Ok(output.stdout)
}
