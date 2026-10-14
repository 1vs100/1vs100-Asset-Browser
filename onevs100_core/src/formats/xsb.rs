use anyhow::{ensure, Result};

const TOTAL_CUES_AT: usize = 0x19;
const SIMPLE_CUES_AT: usize = 0x22;
const CUE_NAMES_AT: usize = 0x2a;
const BANK_NAMES_AT: usize = 0x3a;
const CUE_BYTES: usize = 5;
const SOUND_LENGTH_AT: usize = 7;
const SIMPLE_TRACK_AT: usize = 9;
const VARIATION_BYTES: usize = 5;
const MAX_VARIATIONS: usize = 64;
const VARIATION_MARKER: [u8; 4] = [0xff; 4];
const INLINE_EVENT: [u8; 2] = [0xff, 0x0c];
const NAME_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub name: String,
    pub waves: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundBank {
    pub bank_name: String,
    pub cues: Vec<Cue>,
}

fn be16(d: &[u8], o: usize) -> Result<usize> {
    Ok(usize::from(u16::from_be_bytes(d.get(o..o + 2).ok_or_else(|| anyhow::anyhow!("truncated at {o:#x}"))?.try_into()?)))
}

fn be32(d: &[u8], o: usize) -> Result<usize> {
    Ok(u32::from_be_bytes(d.get(o..o + 4).ok_or_else(|| anyhow::anyhow!("truncated at {o:#x}"))?.try_into()?) as usize)
}

fn waves_of(entry: &[u8]) -> Option<Vec<u16>> {
    let word = |at: usize| entry.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    if entry[0] & 1 == 0 {
        return word(SIMPLE_TRACK_AT).map(|w| vec![w]);
    }
    let len = entry.len();
    for n in 1..MAX_VARIATIONS {
        let Some(marker) = len.checked_sub(VARIATION_BYTES * n + VARIATION_BYTES) else { break };
        if marker >= SIMPLE_TRACK_AT && entry[marker] as usize == n && entry[marker + 1..marker + 5] == VARIATION_MARKER {
            return (0..n).map(|k| word(len - VARIATION_BYTES * n + VARIATION_BYTES * k)).collect();
        }
    }
    let at = entry.windows(2).skip(SIMPLE_TRACK_AT).position(|w| w == INLINE_EVENT)? + SIMPLE_TRACK_AT;
    word(at + 2).map(|w| vec![w])
}

pub fn parse(d: &[u8]) -> Result<SoundBank> {
    ensure!(d.len() > 0x50 && &d[..4] == b"KBDS", "not a big-endian sound bank");
    let total = be16(d, TOTAL_CUES_AT)?;
    let (cues_at, names_at, bank_at) = (be32(d, SIMPLE_CUES_AT)?, be32(d, CUE_NAMES_AT)?, be32(d, BANK_NAMES_AT)?);
    ensure!(names_at <= d.len() && cues_at + total * CUE_BYTES <= d.len(), "sound bank tables are outside the file");
    let names: Vec<String> = d[names_at..].split(|&b| b == 0).map(|n| String::from_utf8_lossy(n).into_owned()).collect();
    let bank_raw = d.get(bank_at..bank_at + NAME_BYTES.min(d.len() - bank_at)).unwrap_or_default();
    let bank_name = String::from_utf8_lossy(bank_raw.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
    let cues = (0..total)
        .map(|i| {
            let sound = be32(d, cues_at + CUE_BYTES * i + 1)?;
            let length = be16(d, sound + SOUND_LENGTH_AT)?;
            let entry = d.get(sound..sound + length).ok_or_else(|| anyhow::anyhow!("cue {i}: sound entry outside the file"))?;
            let waves = waves_of(entry).ok_or_else(|| anyhow::anyhow!("cue {i}: no wave reference found"))?;
            Ok(Cue { name: names.get(i).cloned().unwrap_or_default(), waves })
        })
        .collect::<Result<_>>()?;
    Ok(SoundBank { bank_name, cues })
}
