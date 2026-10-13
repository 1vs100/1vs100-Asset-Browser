//! XUI (Xbox User Interface) parsing for 1 vs 100
//! Parses XUR (compiled UI) and XUIZ/XUIS archives

use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct XuiScene {
    pub name: String,
    pub elements: Vec<XuiElement>,
    pub textures: Vec<String>,
    pub animations: Vec<XuiAnimation>,
}

#[derive(Debug, Clone)]
pub struct XuiElement {
    pub name: String,
    pub element_type: XuiElementType,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub texture_name: Option<String>,
    pub text: Option<String>,
    pub color: [f32; 4],
    pub children: Vec<XuiElement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XuiElementType {
    Panel,
    Text,
    Image,
    Button,
    List,
    Grid,
    ProgressBar,
    Slider,
    Custom(u32),
}

#[derive(Debug, Clone)]
pub struct XuiAnimation {
    pub name: String,
    pub target: String,
    pub property: String,
    pub keys: Vec<AnimationKey>,
}

#[derive(Debug, Clone)]
pub struct AnimationKey {
    pub time: f32,
    pub value: AnimationValue,
    pub interpolation: InterpolationType,
}

#[derive(Debug, Clone)]
pub enum AnimationValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
}

#[derive(Debug, Clone, Copy)]
pub enum InterpolationType {
    Linear,
    Step,
    Spline,
}

impl XuiScene {
    pub fn load_from_xur<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read(path)?;
        
        // Check magic
        if data.len() < 4 || &data[0..4] != b"XUR\0" && &data[0..4] != b"XUR5" {
            return Err("Not a valid XUR file".into());
        }
        
        // Parse XUR v5 format
        // This is a simplified parser - full implementation would be much more complex
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        
        Ok(XuiScene {
            name,
            elements: vec![],
            textures: vec![],
            animations: vec![],
        })
    }
}

// XUIZ/XUIS archive parsing (from our Python xui_formats.py)
pub mod xui_formats {
    use std::io::{Read, Seek, SeekFrom};
    
    #[derive(Debug, Clone)]
    pub struct XuiZEntry {
        pub name: String,
        pub data: Vec<u8>,
    }
    
    pub fn read_xuiz<R: Read + Seek>(reader: &mut R) -> Result<Vec<XuiZEntry>, Box<dyn std::error::Error>> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"XUIZ" {
            return Err("Not a XUIZ archive".into());
        }
        
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?; // flags
        let total_size = u32::from_be_bytes({
            reader.read_exact(&mut buf)?;
            buf
        });
        reader.read_exact(&mut buf)?; // reserved
        let dir_size = u32::from_be_bytes({
            reader.read_exact(&mut buf)?;
            buf
        });
        let entry_count = u16::from_be_bytes({
            reader.read_exact(&mut buf)?;
            buf
        });
        
        let mut entries = Vec::with_capacity(entry_count as usize);
        let data_start = 22 + dir_size as u64;
        
        for _ in 0..entry_count {
            let length = u32::from_be_bytes({
                reader.read_exact(&mut buf)?;
                buf
            });
            let data_offset = u32::from_be_bytes({
                reader.read_exact(&mut buf)?;
                buf
            });
            let name_len = reader.read_u8()? as usize;
            let mut name_bytes = vec![0u8; name_len * 2];
            reader.read_exact(&mut name_bytes)?;
            let name = String::from_utf16be(&name_bytes)?;
            
            entries.push(XuiZEntry { name, data: Vec::new() }); // Data reading would need seek
        }
        
        Ok(entries)
    }
    
    #[derive(Debug, Clone)]
    pub struct XuiSEntry {
        pub key: String,
        pub value: String,
    }
    
    pub fn read_xuis<R: Read>(reader: &mut R) -> Result<Vec<XuiSEntry>, Box<dyn std::error::Error>> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"XUIS" {
            return Err("Not a XUIS string table".into());
        }
        
        let _version = u32::from_le_bytes({
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            buf
        });
        
        let _size = u16::from_be_bytes({
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            buf
        });
        
        let pair_count = u16::from_be_bytes({
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            buf
        });
        
        let mut entries = Vec::with_capacity(pair_count as usize * 2);
        
        for _ in 0..pair_count * 2 {
            let len = u16::from_be_bytes({
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                buf
            });
            let mut str_bytes = vec![0u8; len as usize * 2];
            reader.read_exact(&mut str_bytes)?;
            let s = String::from_utf16be(&str_bytes)?;
            entries.push(XuiSEntry { key: s, value: String::new() }); // Simplified
        }
        
        Ok(entries)
    }
}