use crate::inspect::{self, Detail, Preview};
use crate::source::{self, Entry};
use crate::texture::{self, Texture};
use crate::video;
use crate::{audio, xsb, xwb};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

struct Level {
    title: String,
    dir: Option<PathBuf>,
    entries: Vec<Entry>,
}

enum AudioSource {
    Bank { bytes: Vec<u8>, bank: xwb::Bank },
    Cues { cues: Vec<xsb::Cue>, bank: Option<(Vec<u8>, xwb::Bank)> },
}

struct Selected {
    name: String,
    data: Vec<u8>,
    detail: Detail,
    textures: Vec<Texture>,
    texture: usize,
    video_seconds: Option<f64>,
    audio: Option<AudioSource>,
}

pub struct Browser {
    levels: Vec<Level>,
    selected: Option<Selected>,
}

fn file_name(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

impl Browser {
    pub fn new(dir: &Path) -> Result<Self> {
        let mut browser = Browser { levels: Vec::new(), selected: None };
        browser.go_to(dir)?;
        Ok(browser)
    }

    pub fn go_to(&mut self, dir: &Path) -> Result<()> {
        let entries = source::read_dir(dir)?;
        self.levels = vec![Level { title: dir.display().to_string(), dir: Some(dir.to_path_buf()), entries }];
        self.selected = None;
        Ok(())
    }

    pub fn entries(&self) -> &[Entry] {
        &self.levels.last().expect("browser always has a level").entries
    }

    pub fn breadcrumb(&self) -> String {
        self.levels.iter().map(|l| l.title.as_str()).collect::<Vec<_>>().join(" / ")
    }

    pub fn detail(&self) -> Option<&Detail> {
        self.selected.as_ref().map(|s| &s.detail)
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.selected.as_ref().map(|s| s.name.as_str())
    }

    pub fn texture_count(&self) -> usize {
        self.selected.as_ref().map_or(0, |s| s.textures.len())
    }

    pub fn pending_preview(&self) -> Option<(String, Vec<u8>)> {
        let sel = self.selected.as_ref().filter(|s| s.detail.pending_preview)?;
        Some((sel.name.clone(), sel.data.clone()))
    }

    pub fn set_preview(&mut self, name: &str, preview: Preview) -> bool {
        match self.selected.as_mut().filter(|s| s.name == name) {
            Some(sel) => {
                sel.detail.image = Some(preview);
                sel.detail.pending_preview = false;
                true
            }
            None => false,
        }
    }

    fn audio_source(&self, name: &str, data: &[u8]) -> Option<AudioSource> {
        match data.get(..4)? {
            b"DNBW" => xwb::parse(data).ok().map(|bank| AudioSource::Bank { bytes: data.to_vec(), bank }),
            b"KBDS" => {
                let cues = xsb::parse(data).ok()?.cues;
                let stem = name.rsplit_once('.').map_or(name, |(s, _)| s).to_lowercase();
                let sibling = self.entries().iter().find(|e| e.name.to_lowercase() == format!("{stem}.xwb"));
                let bank = sibling.and_then(|e| e.payload.read().ok()).and_then(|bytes| xwb::parse(&bytes).ok().map(|b| (bytes, b)));
                Some(AudioSource::Cues { cues, bank })
            }
            _ => None,
        }
    }

    pub fn audio_labels(&self) -> Vec<String> {
        match self.selected.as_ref().and_then(|s| s.audio.as_ref()) {
            Some(AudioSource::Bank { bank, .. }) => bank.entries.iter().map(|e| format!("{:03}  {} {} ch {} Hz  {:.2} s", e.index, e.codec_name(), e.channels, e.rate, e.seconds())).collect(),
            Some(AudioSource::Cues { cues, bank }) => cues.iter().map(|c| format!("{}  ({} variation{}){}", c.name, c.waves.len(), if c.waves.len() == 1 { "" } else { "s" }, if bank.is_none() { "  no wave bank next to it" } else { "" })).collect(),
            None => Vec::new(),
        }
    }

    pub fn audio_wav(&self, index: usize) -> Result<Vec<u8>> {
        match self.selected.as_ref().and_then(|s| s.audio.as_ref()).ok_or_else(|| anyhow!("nothing playable selected"))? {
            AudioSource::Bank { bytes, bank } => audio::wav(bytes, bank, index),
            AudioSource::Cues { cues, bank } => {
                let cue = cues.get(index).ok_or_else(|| anyhow!("no cue {index}"))?;
                let (bytes, bank) = bank.as_ref().ok_or_else(|| anyhow!("the wave bank for this sound bank was not found beside it"))?;
                audio::wav(bytes, bank, usize::from(cue.waves[0]))
            }
        }
    }

    pub fn selected_bytes(&self) -> Option<&[u8]> {
        self.selected.as_ref().map(|s| s.data.as_slice())
    }

    pub fn video_seconds(&self) -> Option<f64> {
        self.selected.as_ref().and_then(|s| s.video_seconds)
    }

    pub fn open(&mut self, index: usize) -> Result<()> {
        let entry = self.entries().get(index).ok_or_else(|| anyhow!("no entry {index}"))?.clone();
        if entry.is_dir {
            let entries = source::children(&entry)?.unwrap_or_default();
            let dir = match &entry.payload {
                source::Payload::Disk(p) => Some(p.clone()),
                _ => None,
            };
            self.levels.push(Level { title: entry.name, dir, entries });
            self.selected = None;
            return Ok(());
        }
        let data = entry.payload.read()?;
        let detail = inspect::describe(&entry.name, &data);
        let textures = texture::list(&data).unwrap_or_default();
        let kids = source::expand_named(&entry.name, &data)?;
        let video_seconds = video::asf_info(&data).ok().map(|i| i.seconds);
        let audio = self.audio_source(&entry.name, &data);
        self.selected = Some(Selected { name: entry.name.clone(), data, detail, textures, texture: 0, video_seconds, audio });
        if let Some(entries) = kids {
            self.levels.push(Level { title: file_name(&entry.name).to_owned(), dir: None, entries });
        }
        Ok(())
    }

    pub fn back(&mut self) -> bool {
        if self.levels.len() < 2 {
            return false;
        }
        self.levels.pop();
        self.selected = None;
        true
    }

    pub fn up(&mut self) -> bool {
        if self.back() {
            return true;
        }
        let parent = self.levels[0].dir.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        parent.is_some_and(|p| self.go_to(&p).is_ok())
    }

    pub fn select_texture(&mut self, index: usize) -> Result<()> {
        let sel = self.selected.as_mut().ok_or_else(|| anyhow!("nothing selected"))?;
        let t = sel.textures.get(index).ok_or_else(|| anyhow!("no texture {index}"))?;
        let (width, height) = texture::level_size(t, 0);
        let rgba = texture::decode(&sel.data, t, 0)?.concat();
        sel.detail.image = Some(Preview { width, height, rgba });
        sel.texture = index;
        Ok(())
    }

    pub fn export_selected(&self, dir: &Path) -> Result<PathBuf> {
        let sel = self.selected.as_ref().ok_or_else(|| anyhow!("nothing selected"))?;
        std::fs::create_dir_all(dir)?;
        let out = dir.join(file_name(&sel.name));
        std::fs::write(&out, &sel.data)?;
        Ok(out)
    }

    pub fn export_texture_png(&self, dir: &Path) -> Result<PathBuf> {
        let sel = self.selected.as_ref().ok_or_else(|| anyhow!("nothing selected"))?;
        let t = sel.textures.get(sel.texture).ok_or_else(|| anyhow!("no texture in this file"))?;
        std::fs::create_dir_all(dir)?;
        let out = dir.join(format!("{}_texture{}.png", file_name(&sel.name), sel.texture));
        std::fs::write(&out, texture::to_png(&sel.data, t, 0)?)?;
        Ok(out)
    }
}
