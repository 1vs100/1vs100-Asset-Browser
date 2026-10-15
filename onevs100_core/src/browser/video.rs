use anyhow::{anyhow, ensure, Result};
use std::io::Cursor;

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

pub struct RgbaFrame {
    pub pts_ms: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct FramePump {
    dec: wmv_decoder::AsfWmv2Decoder<Cursor<Vec<u8>>>,
}

impl FramePump {
    pub fn open(data: Vec<u8>) -> Result<Self> {
        Ok(Self { dec: wmv_decoder::AsfWmv2Decoder::open(Cursor::new(data)).map_err(|e| anyhow!("{e}"))? })
    }

    pub fn next(&mut self) -> Result<Option<RgbaFrame>> {
        let Some(frame) = self.dec.next_frame().map_err(|e| anyhow!("{e}"))? else {
            return Ok(None);
        };
        let (w, h) = (frame.frame.width.max(1), frame.frame.height.max(1));
        let rgba = wmv_decoder::yuv420p_to_rgba_scaled(&frame.frame, w, h);
        Ok(Some(RgbaFrame { pts_ms: frame.pts_ms, width: w, height: h, rgba }))
    }

    pub fn next_yuv(&mut self) -> Result<Option<YuvPicture>> {
        let Some(frame) = self.dec.next_frame().map_err(|e| anyhow!("{e}"))? else {
            return Ok(None);
        };
        Ok(Some(YuvPicture { pts_ms: frame.pts_ms, width: frame.frame.width, height: frame.frame.height, y: frame.frame.y, cb: frame.frame.cb, cr: frame.frame.cr }))
    }
}

pub struct Soundtrack {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

pub fn soundtrack(data: &[u8]) -> Result<Option<Soundtrack>> {
    let mut dec = match wmv_decoder::AsfWmaDecoder::open(Cursor::new(data.to_vec())) {
        Ok(dec) => dec,
        Err(wmv_decoder::DecoderError::Unsupported(_)) => return Ok(None),
        Err(e) => return Err(anyhow!("{e}")),
    };
    let mut samples = Vec::new();
    let mut rate = 0u32;
    let mut channels = 0u16;
    while let Some(frame) = dec.next_frame().map_err(|e| anyhow!("{e}"))? {
        rate = frame.frame.sample_rate;
        channels = frame.frame.channels;
        samples.extend_from_slice(&frame.frame.samples);
    }
    if samples.is_empty() || rate == 0 || channels == 0 {
        return Ok(None);
    }
    Ok(Some(Soundtrack { sample_rate: rate, channels, samples }))
}
pub struct YuvPicture {
    pub pts_ms: u32,
    pub width: u32,
    pub height: u32,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
}

/// Newest frame whose pts is at or before the audio clock. Later frames stay queued.
/// This is the appsink `drop=true` rule: a late backlog is not played through.
pub fn frame_for_clock(clock_ms: u32, pts: &[u32]) -> Option<usize> {
    pts.iter().enumerate().filter(|(_, pts)| **pts <= clock_ms).max_by_key(|(_, pts)| *pts).map(|(i, _)| i)
}

pub fn frame_rgba(data: &[u8], seconds: f64) -> Result<RgbaFrame> {
    // shortcut: walks frames from the start, upgrade if long seeks get slow
    let mut pump = FramePump::open(data.to_vec())?;
    let target_ms = (seconds.max(0.0) * 1000.0) as u32;
    let mut chosen = None;
    while let Some(frame) = pump.next()? {
        let reached = frame.pts_ms >= target_ms;
        chosen = Some(frame);
        if reached {
            break;
        }
    }
    chosen.ok_or_else(|| anyhow!("no frame decoded"))
}

pub fn frame_png(data: &[u8], seconds: f64) -> Result<Vec<u8>> {
    let decoded = frame_rgba(data, seconds)?;
    let img = image::RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba).ok_or_else(|| anyhow!("bad frame size"))?;
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok(png)
}
