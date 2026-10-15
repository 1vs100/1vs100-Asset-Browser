use crate::xwb::{Bank, Codec};
use anyhow::{anyhow, bail, ensure, Result};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};

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

pub fn decode_pcm(wav: &[u8]) -> Result<Vec<u8>> {
    if wav.len() >= 22 && &wav[..4] == b"RIFF" && &wav[8..12] == b"WAVE" && &wav[12..16] == b"fmt " && u16::from_le_bytes([wav[20], wav[21]]) == 1 {
        return Ok(wav.to_vec());
    }
    let audio = xma2_rs::decode_audio_file(wav).map_err(|e| anyhow!("could not decode this wave: {e}"))?;
    ensure!(!audio.planes.is_empty(), "decoded wave has no samples");
    let samples = audio.to_interleaved_i16();
    let channels = audio.channels as u16;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&audio.sample_rate.to_le_bytes());
    fmt.extend_from_slice(&(audio.sample_rate * u32::from(channels) * 2).to_le_bytes());
    fmt.extend_from_slice(&(channels * 2).to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let pcm: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    Ok(riff(&fmt, &pcm))
}

pub struct Player {
    stream: Option<OutputStream>,
    sink: Option<Sink>,
}

impl Player {
    pub fn new() -> Self {
        Player { stream: None, sink: None }
    }

    fn sink(&mut self) -> Result<&Sink> {
        if self.sink.is_none() {
            let (stream, handle) = OutputStream::try_default().map_err(|e| anyhow!("no audio output: {e}"))?;
            let sink = Sink::try_new(&handle).map_err(|e| anyhow!("could not open audio output: {e}"))?;
            self.stream = Some(stream);
            self.sink = Some(sink);
        }
        Ok(self.sink.as_ref().unwrap())
    }

    pub fn play(&mut self, wav: &[u8]) -> Result<()> {
        let (rate, channels, samples) = pcm_i16(&decode_pcm(wav)?)?;
        let sink = self.sink()?;
        sink.stop();
        sink.append(SamplesBuffer::new(channels, rate, samples));
        sink.play();
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
    }
}

fn pcm_i16(wav: &[u8]) -> Result<(u32, u16, Vec<i16>)> {
    ensure!(wav.len() >= 44 && &wav[..4] == b"RIFF" && &wav[8..12] == b"WAVE", "not a wav");
    let mut off = 12usize;
    let mut rate = None;
    let mut channels = None;
    let mut data = None;
    while off + 8 <= wav.len() {
        let id = &wav[off..off + 4];
        let size = u32::from_le_bytes(wav[off + 4..off + 8].try_into()?) as usize;
        let body = wav.get(off + 8..off + 8 + size).ok_or_else(|| anyhow!("truncated wav"))?;
        match id {
            b"fmt " => {
                ensure!(body.len() >= 16 && u16::from_le_bytes([body[0], body[1]]) == 1, "not pcm");
                channels = Some(u16::from_le_bytes([body[2], body[3]]));
                rate = Some(u32::from_le_bytes(body[4..8].try_into()?));
                ensure!(u16::from_le_bytes([body[14], body[15]]) == 16, "not 16-bit pcm");
            }
            b"data" => data = Some(body),
            _ => {}
        }
        off += 8 + size + (size & 1);
    }
    let samples = data.ok_or_else(|| anyhow!("wav has no data"))?.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    Ok((rate.ok_or_else(|| anyhow!("wav has no fmt"))?, channels.ok_or_else(|| anyhow!("wav has no fmt"))?, samples))
}

impl Default for Player {
    fn default() -> Self {
        Player::new()
    }
}

#[cfg(test)]
mod pcm_tests {
    use super::*;

    fn fmt(bits: u16) -> Vec<u8> {
        let mut f = vec![1, 0, 2, 0];
        f.extend_from_slice(&44100u32.to_le_bytes());
        f.extend_from_slice(&[0; 6]);
        f.extend_from_slice(&bits.to_le_bytes());
        f
    }

    #[test]
    fn pcm_i16_reads_rate_channels_samples() {
        let wav = riff(&fmt(16), &[1, 0, 255, 255, 0, 0, 0, 0]);
        assert_eq!(pcm_i16(&wav).unwrap(), (44100, 2, vec![1, -1, 0, 0]));
    }

    #[test]
    fn pcm_i16_rejects_bad_input() {
        assert!(pcm_i16(&riff(&fmt(8), &[0; 40])).is_err());
        let mut truncated = riff(&fmt(16), &[0; 40]);
        truncated.truncate(truncated.len() - 4);
        assert!(pcm_i16(&truncated).is_err());
        assert!(pcm_i16(&[0; 64]).is_err());
    }
}
