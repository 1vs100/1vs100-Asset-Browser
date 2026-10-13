use crate::audio::{self, Player};
use crate::browser::Browser;
use crate::{inspect, xsb, xwb};
use std::path::PathBuf;

fn open_package(package: &str) -> Browser {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/dlcs/shows");
    let mut browser = Browser::new(&root).unwrap();
    let index = browser.entries().iter().position(|e| e.name == package).unwrap();
    browser.open(index).unwrap();
    browser
}

const UK: &str = "77205BC1241C708A8A10B7303AF90E5EDD0286B1";
const PLAIN: &str = "7BEC809CCC282E8C9D18EFAD8F4407AA6204DFC8";

fn read(package: &str, name: &str) -> Vec<u8> {
    crate::tests_fixtures::package_file("shows", package, &format!("media/audio/{name}"))
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01b3))
}

#[test]
fn wave_bank_entries_match_the_python_reference() {
    let d = read(UK, "vo.xwb");
    let b = xwb::parse(&d).unwrap();
    assert_eq!((b.name.as_str(), b.entries.len()), ("VO_UK", 460));
    let e = &b.entries;
    assert_eq!((e[0].codec, e[0].channels, e[0].rate, e[0].samples, e[0].offset, e[0].length), (xwb::Codec::Xma, 1, 48047, 35328, 0, 8192));
    assert_eq!((e[1].codec, e[1].samples, e[1].offset, e[1].length), (xwb::Codec::Pcm, 75988, 8192, 151_976));
    assert_eq!((e[100].codec, e[100].rate, e[100].samples, e[100].offset, e[100].length), (xwb::Codec::Xma, 44080, 93184, 2_220_032, 24576));
    assert_eq!((e[459].samples, e[459].offset, e[459].length), (215_040, 10_874_880, 45056));
}

#[test]
fn xma_entries_are_wrapped_exactly_like_the_python_reference() {
    let d = read(UK, "vo.xwb");
    let b = xwb::parse(&d).unwrap();
    for (i, len, hash) in [(0, 8272, 0xd82e_1e60_9c8b_c6fa), (100, 24656, 0x1ac9_517e_6801_4c9e), (459, 45136, 0xb2b0_4c0b_d57c_492b)] {
        let wav = audio::wav(&d, &b, i).unwrap();
        assert_eq!((wav.len(), fnv(&wav)), (len, hash), "entry {i}");
    }
}

#[test]
fn pcm_entries_become_little_endian_wav_files() {
    let d = read(UK, "vo.xwb");
    let b = xwb::parse(&d).unwrap();
    let wav = audio::wav(&d, &b, 1).unwrap();
    assert_eq!((&wav[..4], &wav[8..12], &wav[12..16]), (&b"RIFF"[..], &b"WAVE"[..], &b"fmt "[..]));
    assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1);
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 48000);
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 151_976);
    let first_be = i16::from_be_bytes([d[b.data_offset + 8192], d[b.data_offset + 8193]]);
    assert_eq!(i16::from_le_bytes([wav[44], wav[45]]), first_be);
}

#[test]
fn out_of_range_entries_are_errors() {
    let d = read(UK, "vo.xwb");
    let b = xwb::parse(&d).unwrap();
    assert!(audio::wav(&d, &b, 460).is_err());
    assert!(xwb::parse(&d[..100]).is_err());
    assert!(xwb::parse(b"RIFFxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").is_err());
}

#[test]
fn sound_bank_cues_resolve_to_wave_indices() {
    let s = xsb::parse(&read(UK, "vo.xsb")).unwrap();
    assert_eq!((s.cues.len(), s.bank_name.as_str()), (228, "VO_UK"));
    assert_eq!((s.cues[0].name.as_str(), s.cues[0].waves.clone()), ("00_Tx_01_ThxLH", vec![254, 255, 256, 257, 258]));
    assert_eq!(s.cues[2].waves, vec![250, 251, 252]);
    assert_eq!((s.cues[62].name.as_str(), s.cues[62].waves.clone()), ("MobRemaining2", vec![429]));
    assert!(s.cues.iter().all(|c| !c.waves.is_empty() && c.waves.iter().all(|&w| w < 460)));
    let plain = xsb::parse(&read(PLAIN, "vo.xsb")).unwrap();
    assert_eq!((plain.cues.len(), plain.cues[0].name.as_str(), plain.cues[0].waves.clone()), (225, "03_QA_17_VO_ThatsIt", vec![163]));
}

#[test]
fn sound_bank_rejects_other_data() {
    assert!(xsb::parse(b"KBDS").is_err());
    assert!(xsb::parse(&read(UK, "vo.xwb")).is_err());
}

#[test]
fn details_list_entries_and_cues() {
    let wb = inspect::describe("vo.xwb", &read(UK, "vo.xwb"));
    assert!(wb.lines.iter().any(|l| l.contains("entry 0  XMA 1 ch 48047 Hz  0.74 s  8192 bytes")), "{:#?}", &wb.lines[..6]);
    assert!(wb.lines.iter().any(|l| l.contains("entry 1  PCM")));
    let sb = inspect::describe("vo.xsb", &read(UK, "vo.xsb"));
    assert_eq!(sb.kind, "XACT sound bank");
    assert!(sb.lines.iter().any(|l| l.contains("228 cues")));
    assert!(sb.lines.iter().any(|l| l.contains("00_Tx_01_ThxLH") && l.contains("254")));
}

#[test]
fn browser_offers_wave_entries_and_cues_for_playback() {
    let mut b = open_package(UK);
    let wb = b.entries().iter().position(|e| e.name.ends_with("vo.xwb")).unwrap();
    b.open(wb).unwrap();
    let labels = b.audio_labels();
    assert_eq!(labels.len(), 460);
    assert!(labels[0].contains("XMA 1 ch 48047 Hz"), "{}", labels[0]);
    assert_eq!(&b.audio_wav(0).unwrap()[..4], b"RIFF");
    let sb = b.entries().iter().position(|e| e.name.ends_with("vo.xsb")).unwrap();
    b.open(sb).unwrap();
    let cues = b.audio_labels();
    assert_eq!(cues.len(), 228);
    assert!(cues[0].starts_with("00_Tx_01_ThxLH"));
    let wb_bytes = read(UK, "vo.xwb");
    let bank = xwb::parse(&wb_bytes).unwrap();
    assert_eq!(b.audio_wav(0).unwrap(), audio::wav(&wb_bytes, &bank, 254).unwrap());
    assert!(b.audio_wav(228).is_err());
}

#[test]
fn files_without_audio_offer_nothing_to_play() {
    let mut b = open_package(UK);
    let i = b.entries().iter().position(|e| e.name.ends_with("vo.xsb")).unwrap();
    b.open(i).unwrap();
    assert!(!b.audio_labels().is_empty());
    let mut r = open_package(UK);
    let j = r.entries().iter().position(|e| e.name.ends_with("show.ext")).unwrap();
    r.open(j).unwrap();
    assert!(r.audio_labels().is_empty());
}

#[test]
fn player_starts_and_stops_a_command() {
    let mut p = Player::with_command("cat", &[]);
    assert!(!p.is_playing());
    p.play(b"RIFFxxxx").unwrap();
    assert!(p.is_playing());
    p.stop();
    assert!(!p.is_playing());
}

#[test]
fn missing_player_gives_a_helpful_error() {
    let mut p = Player::with_command("definitely-not-a-real-player", &[]);
    let err = p.play(b"x").unwrap_err().to_string();
    assert!(err.contains("definitely-not-a-real-player") && err.contains("ffplay"), "{err}");
}

#[test]
fn xma_decodes_through_ffmpeg_when_the_codec_is_available() {
    if !audio::xma_decoder_available() {
        return;
    }
    let d = read(UK, "vo.xwb");
    let b = xwb::parse(&d).unwrap();
    let pcm = audio::decode_pcm(&audio::wav(&d, &b, 0).unwrap()).unwrap();
    assert_eq!(&pcm[..4], b"RIFF");
    assert!(pcm.len() > 40_000, "{} bytes", pcm.len());
}
