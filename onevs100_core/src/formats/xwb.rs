use anyhow::{ensure, Result};

const SEGMENTS_AT: usize = 12;
const SEGMENT_COUNT: usize = 5;
const BANK_DATA: usize = 0;
const ENTRY_METADATA: usize = 1;
const ENTRY_NAMES: usize = 3;
const ENTRY_WAVE_DATA: usize = 4;
const NAME_BYTES: usize = 64;
const ENTRY_BYTES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Pcm,
    Xma,
    Adpcm,
    Wma,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub index: usize,
    pub codec: Codec,
    pub channels: u32,
    pub rate: u32,
    pub block_align: u32,
    pub sixteen_bit: bool,
    pub samples: u32,
    pub offset: usize,
    pub length: usize,
    pub loop_start: u32,
    pub loop_total: u32,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank {
    pub name: String,
    pub entries: Vec<Entry>,
    pub data_offset: usize,
    pub data_length: usize,
}

fn be32(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(d.get(o..o + 4).ok_or_else(|| anyhow::anyhow!("truncated at {o:#x}"))?.try_into()?))
}

impl Entry {
    pub fn seconds(&self) -> f64 {
        if self.rate == 0 { 0.0 } else { f64::from(self.samples) / f64::from(self.rate) }
    }

    pub fn codec_name(&self) -> &'static str {
        match self.codec {
            Codec::Pcm => "PCM",
            Codec::Xma => "XMA",
            Codec::Adpcm => "ADPCM",
            Codec::Wma => "WMA",
        }
    }
}

pub fn parse(d: &[u8]) -> Result<Bank> {
    ensure!(d.len() > 0x80 && &d[..4] == b"DNBW", "not a big-endian wave bank");
    let segment = |i: usize| -> Result<(usize, usize)> { Ok((be32(d, SEGMENTS_AT + 8 * i)? as usize, be32(d, SEGMENTS_AT + 8 * i + 4)? as usize)) };
    let segments: Vec<(usize, usize)> = (0..SEGMENT_COUNT).map(segment).collect::<Result<_>>()?;
    let (bank_at, _) = segments[BANK_DATA];
    let count = be32(d, bank_at + 4)? as usize;
    let name_raw = d.get(bank_at + 8..bank_at + 8 + NAME_BYTES).ok_or_else(|| anyhow::anyhow!("truncated bank data"))?;
    let name = String::from_utf8_lossy(name_raw.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
    let (meta_size, name_size) = (be32(d, bank_at + 72)? as usize, be32(d, bank_at + 76)? as usize);
    let (meta_at, _) = segments[ENTRY_METADATA];
    let (names_at, names_len) = segments[ENTRY_NAMES];
    let (data_offset, data_length) = segments[ENTRY_WAVE_DATA];
    ensure!(meta_size >= ENTRY_BYTES && data_offset + data_length <= d.len(), "bank layout is inconsistent");
    let entries = (0..count)
        .map(|index| {
            let at = meta_at + index * meta_size;
            let w: Vec<u32> = (0..6).map(|i| be32(d, at + 4 * i)).collect::<Result<_>>()?;
            let mini = w[1];
            let codec = match mini & 3 {
                0 => Codec::Pcm,
                1 => Codec::Xma,
                2 => Codec::Adpcm,
                _ => Codec::Wma,
            };
            let entry_name = (names_len > 0 && name_size > 0).then(|| d.get(names_at + index * name_size..names_at + (index + 1) * name_size)).flatten().map(|raw| String::from_utf8_lossy(raw.split(|&b| b == 0).next().unwrap_or_default()).into_owned());
            let entry = Entry {
                index,
                codec,
                channels: (mini >> 2) & 7,
                rate: (mini >> 5) & 0x3_ffff,
                block_align: (mini >> 23) & 0xff,
                sixteen_bit: mini >> 31 == 1,
                samples: w[0] >> 4,
                offset: w[2] as usize,
                length: w[3] as usize,
                loop_start: w[4],
                loop_total: w[5],
                name: entry_name,
            };
            ensure!(data_offset + entry.offset + entry.length <= d.len(), "entry {index} runs past the end of the bank");
            Ok(entry)
        })
        .collect::<Result<_>>()?;
    Ok(Bank { name, entries, data_offset, data_length })
}
