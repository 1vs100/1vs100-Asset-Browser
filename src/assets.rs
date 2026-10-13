//! Asset management for 1 vs 100 frontend
//! Loads textures, meshes, XUI scenes, and animations from extracted content packages

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use wgpu::util::DeviceExt;
use image::{ImageFormat, DynamicImage};
use bcndecode::{decode_bc1, decode_bc3, decode_bc4, decode_bc5, decode_bc7};

use crate::xenos::{XenosTexture, XenosTiling};

#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Dxt1,
    Dxt3,
    Dxt5,
    Rgba8,
    Bgra8,
    Rgb8,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub vertex_count: u32,
}

impl Mesh {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3, // position
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3, // normal
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2, // uv
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct AssetManager {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    content_root: PathBuf,
    
    textures: HashMap<String, Texture>,
    meshes: HashMap<String, Mesh>,
    scenes: HashMap<String, XuiScene>,
}

impl AssetManager {
    pub fn new<P: AsRef<Path>>(content_root: P, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device,
            queue,
            content_root: content_root.as_ref().to_path_buf(),
            textures: HashMap::new(),
            meshes: HashMap::new(),
            scenes: HashMap::new(),
        }
    }
    
    pub fn reload(&mut self) {
        self.textures.clear();
        self.meshes.clear();
        self.scenes.clear();
        self.load_all();
    }
    
    fn load_all(&mut self) {
        self.load_textures();
        self.load_meshes();
        self.load_scenes();
    }
    
    fn load_textures(&mut self) {
        // Walk the content directory for texture files
        // Priority: .png, .jpg, .dds, .xpr (Xbox 360 texture)
        let texture_dirs = [
            "media/textures",
            "media/set",
            "media/themes",
            "media/sponsor",
        ];
        
        for dir in &texture_dirs {
            let full_path = self.content_root.join(dir);
            if full_path.exists() {
                self.load_textures_from_dir(&full_path);
            }
        }
    }
    
    fn load_textures_from_dir(&mut self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    match ext.to_lowercase().as_str() {
                        "png" | "jpg" | "jpeg" | "bmp" | "tga" => {
                            self.load_image_texture(&path);
                        }
                        "dds" => {
                            self.load_dds_texture(&path);
                        }
                        "xpr" | "bin" => {
                            // Try to load as Xbox 360 texture
                            self.load_xenos_texture(&path);
                        }
                        _ => {}
                    }
                } else if path.is_dir() {
                    self.load_textures_from_dir(&path);
                }
            }
        }
    }
    
    fn load_image_texture(&mut self, path: &Path) {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            
            let texture = self.create_texture_from_rgba(&name, &rgba, width, height);
            self.textures.insert(name, texture);
        }
    }
    
    fn load_dds_texture(&mut self, path: &Path) {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        
        if let Ok(data) = std::fs::read(path) {
            if let Some(texture) = self.parse_dds(&name, &data) {
                self.textures.insert(name, texture);
            }
        }
    }
    
    fn load_xenos_texture(&mut self, path: &Path) {
        // Try to parse as Xenos tiled texture
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        
        if let Ok(data) = std::fs::read(path) {
            // This would use our xenos_tex logic to decode tiled DXT textures
            // For now, skip - we'll use the decoded PNGs from our Python tools
        }
    }
    
    fn parse_dds(&self, name: &str, data: &[u8]) -> Option<Texture> {
        // Minimal DDS parser for DXT1/3/5
        if data.len() < 128 || &data[0..4] != b"DDS " {
            return None;
        }
        
        let height = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let width = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let flags = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let fourcc = &data[84..88];
        
        let format = match fourcc {
            b"DXT1" => TextureFormat::Dxt1,
            b"DXT3" => TextureFormat::Dxt3,
            b"DXT5" => TextureFormat::Dxt5,
            _ => return None,
        };
        
        let mip_count = if flags & 0x20000 != 0 { u32::from_le_bytes([data[28], data[29], data[30], data[31]]) } else { 1 };
        let pixel_data = &data[128..];
        
        // Decode based on format
        let rgba = match format {
            TextureFormat::Dxt1 => decode_bc1(width, height, pixel_data),
            TextureFormat::Dxt3 => decode_bc3(width, height, pixel_data),
            TextureFormat::Dxt5 => decode_bc3(width, height, pixel_data), // BC3 = DXT5
            _ => return None,
        }?;
        
        self.create_texture_from_rgba(name, &rgba, width, height)
    }
    
    fn create_texture_from_rgba(&mut self, name: &str, rgba: &[u8], width: u32, height: u32) -> Option<Texture> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{} Bind Group Layout", name)),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} Bind Group", name)),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        
        Some(Texture {
            name: name.to_string(),
            texture,
            view,
            sampler,
            bind_group,
            width,
            height,
            format: TextureFormat::Rgba8,
        })
    }
    
    fn load_meshes(&mut self) {
        // Load meshes from .mb.bin files (CAFF rendergraph)
        // This would use our caff_rendergraph parser
        // For now, we'll load from exported OBJ/GLTF if available
    }
    
    fn load_scenes(&mut self) {
        // Load XUI scenes from .xur files
        let scene_dirs = [
            "media/launch_xzp",
            "media/episode_xzp",
        ];
        
        for dir in &scene_dirs {
            let full_path = self.content_root.join(dir);
            if full_path.exists() {
                self.load_scenes_from_dir(&full_path);
            }
        }
    }
    
    fn load_scenes_from_dir(&mut self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("xur") {
                    if let Ok(scene) = self.parse_xur(&path) {
                        self.scenes.insert(scene.name.clone(), scene);
                    }
                }
            }
        }
    }
    
    fn parse_xur(&self, path: &Path) -> Result<XuiScene, Box<dyn std::error::Error>> {
        // Parse XUR v5 format
        // This is complex - for now return a basic scene
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        
        // TODO: Implement full XUR parsing using xui_formats logic
        // For now, create a placeholder
        Ok(XuiScene {
            name,
            elements: vec![],
            textures: vec![],
        })
    }
    
    pub fn get_texture(&self, name: &str) -> Option<&Texture> {
        self.textures.get(name)
    }
    
    pub fn get_mesh(&self, name: &str) -> Option<&Mesh> {
        self.meshes.get(name)
    }
    
    pub fn get_scene(&self, name: &str) -> Option<&XuiScene> {
        self.scenes.get(name)
    }
    
    pub fn list_textures(&self) -> Vec<String> {
        self.textures.keys().cloned().collect()
    }
    
    pub fn list_scenes(&self) -> Vec<XuiScene> {
        self.scenes.values().cloned().collect()
    }
}