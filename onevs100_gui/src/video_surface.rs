use iced::widget::shader::{self, Action, Pipeline, Primitive};
use iced::Rectangle;
use onevs100_core::video::{self, YuvPicture};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Speaker delay past the samples rodio has already pulled. Matches playbin's clock, which
/// waits for the device buffer instead of the moment audio is queued.
const SPEAKER_LAG_MS: u32 = 80;
const STEP_CAP_MS: u64 = 60;
const MAX_PENDING: usize = 8;

#[derive(Debug, Default)]
pub struct FrameSlot {
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pts_ms: u32,
    pub seq: u64,
}

pub struct Session {
    pub stop: Arc<AtomicBool>,
    rx: Mutex<Receiver<YuvPicture>>,
    pending: Mutex<VecDeque<YuvPicture>>,
    pub slot: Arc<Mutex<FrameSlot>>,
    pub played: Arc<AtomicU64>,
    pub rate: AtomicU32,
    pub has_audio: AtomicBool,
    pub audio_checked: AtomicBool,
    pub audio_done: Arc<AtomicBool>,
    pub error: Mutex<Option<String>>,
    tail: Mutex<Option<(Instant, u32)>>,
    /// (played frames, when that count last changed): rodio advances `played` in device-buffer steps.
    step: Mutex<(u64, Instant)>,
    started: Instant,
    ended: AtomicBool,
}

impl Session {
    pub fn new(rx: Receiver<YuvPicture>, stop: Arc<AtomicBool>) -> Self {
        Self {
            stop,
            rx: Mutex::new(rx),
            pending: Mutex::new(VecDeque::new()),
            slot: Arc::new(Mutex::new(FrameSlot::default())),
            played: Arc::new(AtomicU64::new(0)),
            rate: AtomicU32::new(0),
            has_audio: AtomicBool::new(false),
            audio_checked: AtomicBool::new(false),
            audio_done: Arc::new(AtomicBool::new(false)),
            error: Mutex::new(None),
            tail: Mutex::new(None),
            step: Mutex::new((0, Instant::now())),
            started: Instant::now(),
            ended: AtomicBool::new(false),
        }
    }

    fn clock_ms(&self) -> u32 {
        if self.has_audio.load(Ordering::Relaxed) {
            let rate = u64::from(self.rate.load(Ordering::Relaxed).max(1));
            let played = self.played.load(Ordering::Relaxed);
            let mut step = self.step.lock().expect("clock step");
            if step.0 != played {
                *step = (played, Instant::now());
            }
            // Interpolate between buffer steps (capped) so frames are not shown in bursts.
            let since = u64::try_from(step.1.elapsed().as_millis()).unwrap_or(u64::MAX).min(STEP_CAP_MS);
            drop(step);
            let ms = played.saturating_mul(1000) / rate + since;
            let ms = u32::try_from(ms).unwrap_or(u32::MAX);
            if self.audio_done.load(Ordering::Relaxed) {
                // Audio ran out: keep the clock moving so longer video still finishes.
                let (at, base) = *self.tail.lock().expect("clock tail").get_or_insert((Instant::now(), ms));
                return base.saturating_add(u32::try_from(at.elapsed().as_millis()).unwrap_or(u32::MAX));
            }
            return ms.saturating_sub(SPEAKER_LAG_MS);
        }
        if !self.audio_checked.load(Ordering::Relaxed) {
            return 0;
        }
        u32::try_from(self.started.elapsed().as_millis()).unwrap_or(u32::MAX)
    }

    /// Pull decoded frames and keep the newest one the audio clock has reached.
    /// Returns true once the decoder is finished and nothing is left to show.
    pub fn pull(&self) -> bool {
        let rx = self.rx.lock().expect("video queue");
        let mut pending = self.pending.lock().expect("video queue");
        let mut disconnected = false;
        // Cap the backlog so the bounded channel throttles the decoder.
        while pending.len() < MAX_PENDING {
            match rx.try_recv() {
                Ok(frame) => pending.push_back(frame),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        let clock = self.clock_ms();
        let pts: Vec<u32> = pending.iter().map(|frame| frame.pts_ms).collect();
        if let Some(index) = video::frame_for_clock(clock, &pts) {
            let frame = pending.drain(..=index).last().expect("chosen frame");
            let mut slot = self.slot.lock().expect("video frame");
            if slot.pts_ms != frame.pts_ms || slot.seq == 0 {
                slot.seq += 1;
                slot.pts_ms = frame.pts_ms;
                slot.width = frame.width;
                slot.height = frame.height;
                slot.y = frame.y;
                slot.cb = frame.cb;
                slot.cr = frame.cr;
            }
        }
        disconnected && pending.is_empty()
    }
}

#[derive(Clone)]
pub struct Film<Message> {
    pub session: Arc<Session>,
    pub ended: Message,
}

impl<Message: Clone + 'static> shader::Program<Message> for Film<Message> {
    type State = ();
    type Primitive = VideoPrimitive;

    fn update(&self, _state: &mut Self::State, event: &iced::Event, _bounds: Rectangle, _cursor: iced::mouse::Cursor) -> Option<Action<Message>> {
        if self.session.ended.load(Ordering::Relaxed) {
            return None;
        }
        let iced::Event::Window(iced::window::Event::RedrawRequested(_)) = event else {
            return None;
        };
        if self.session.pull() {
            self.session.ended.store(true, Ordering::Relaxed);
            return Some(Action::publish(self.ended.clone()));
        }
        Some(Action::request_redraw_at(Instant::now() + Duration::from_millis(16)))
    }

    fn draw(&self, _state: &Self::State, _cursor: iced::mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        VideoPrimitive { frame: Arc::clone(&self.session.slot) }
    }
}

#[derive(Debug)]
pub struct VideoPrimitive {
    frame: Arc<Mutex<FrameSlot>>,
}

impl Primitive for VideoPrimitive {
    type Pipeline = VideoPipeline;

    fn prepare(&self, pipeline: &mut Self::Pipeline, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, bounds: &Rectangle, _viewport: &shader::Viewport) {
        let slot = self.frame.lock().expect("video frame lock");
        pipeline.upload(device, queue, &slot, bounds);
    }

    fn draw(&self, pipeline: &Self::Pipeline, pass: &mut iced::wgpu::RenderPass<'_>) -> bool {
        pipeline.draw(pass);
        true
    }
}

struct Planes {
    y: iced::wgpu::Texture,
    u: iced::wgpu::Texture,
    v: iced::wgpu::Texture,
}

pub struct VideoPipeline {
    pipeline: iced::wgpu::RenderPipeline,
    layout: iced::wgpu::BindGroupLayout,
    sampler: iced::wgpu::Sampler,
    uniform: iced::wgpu::Buffer,
    bind: iced::wgpu::BindGroup,
    planes: Planes,
    size: (u32, u32),
    seq: u64,
    scratch: Vec<u8>,
}

impl Pipeline for VideoPipeline {
    fn new(device: &iced::wgpu::Device, _queue: &iced::wgpu::Queue, format: iced::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor { label: Some("video surface"), source: iced::wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let plane = |binding| iced::wgpu::BindGroupLayoutEntry { binding, visibility: iced::wgpu::ShaderStages::FRAGMENT, ty: iced::wgpu::BindingType::Texture { sample_type: iced::wgpu::TextureSampleType::Float { filterable: true }, view_dimension: iced::wgpu::TextureViewDimension::D2, multisampled: false }, count: None };
        let layout = device.create_bind_group_layout(&iced::wgpu::BindGroupLayoutDescriptor {
            label: Some("video surface"),
            entries: &[
                plane(0),
                plane(1),
                plane(2),
                iced::wgpu::BindGroupLayoutEntry { binding: 3, visibility: iced::wgpu::ShaderStages::FRAGMENT, ty: iced::wgpu::BindingType::Sampler(iced::wgpu::SamplerBindingType::Filtering), count: None },
                iced::wgpu::BindGroupLayoutEntry { binding: 4, visibility: iced::wgpu::ShaderStages::VERTEX, ty: iced::wgpu::BindingType::Buffer { ty: iced::wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor { label: Some("video surface"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("video surface"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: iced::wgpu::PipelineCompilationOptions::default() },
            fragment: Some(iced::wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(iced::wgpu::ColorTargetState { format, blend: Some(iced::wgpu::BlendState::REPLACE), write_mask: iced::wgpu::ColorWrites::ALL })], compilation_options: iced::wgpu::PipelineCompilationOptions::default() }),
            primitive: iced::wgpu::PrimitiveState { topology: iced::wgpu::PrimitiveTopology::TriangleStrip, ..iced::wgpu::PrimitiveState::default() },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&iced::wgpu::SamplerDescriptor { mag_filter: iced::wgpu::FilterMode::Linear, min_filter: iced::wgpu::FilterMode::Linear, ..iced::wgpu::SamplerDescriptor::default() });
        let uniform = device.create_buffer(&iced::wgpu::BufferDescriptor { label: Some("video fit"), size: 16, usage: iced::wgpu::BufferUsages::UNIFORM | iced::wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let planes = Planes { y: plane_tex(device, 1, 1), u: plane_tex(device, 1, 1), v: plane_tex(device, 1, 1) };
        let bind = bind_group(device, &layout, &planes, &sampler, &uniform);
        Self { pipeline, layout, sampler, uniform, bind, planes, size: (0, 0), seq: 0, scratch: Vec::new() }
    }
}

impl VideoPipeline {
    fn upload(&mut self, device: &iced::wgpu::Device, queue: &iced::wgpu::Queue, slot: &FrameSlot, bounds: &Rectangle) {
        if slot.width < 2 || slot.height < 2 || slot.y.len() < (slot.width * slot.height) as usize {
            return;
        }
        if self.size != (slot.width, slot.height) {
            self.planes = Planes { y: plane_tex(device, slot.width, slot.height), u: plane_tex(device, slot.width / 2, slot.height / 2), v: plane_tex(device, slot.width / 2, slot.height / 2) };
            self.bind = bind_group(device, &self.layout, &self.planes, &self.sampler, &self.uniform);
            self.size = (slot.width, slot.height);
            self.seq = 0;
        }
        if self.seq != slot.seq {
            write_plane(queue, &self.planes.y, &slot.y, slot.width, slot.height, &mut self.scratch);
            write_plane(queue, &self.planes.u, &slot.cb, slot.width / 2, slot.height / 2, &mut self.scratch);
            write_plane(queue, &self.planes.v, &slot.cr, slot.width / 2, slot.height / 2, &mut self.scratch);
            self.seq = slot.seq;
        }
        let widget = bounds.width / bounds.height.max(1.0);
        let frame = slot.width as f32 / slot.height.max(1) as f32;
        let fit = if frame > widget { [1.0, widget / frame] } else { [frame / widget, 1.0] };
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&fit[0].to_ne_bytes());
        bytes[4..8].copy_from_slice(&fit[1].to_ne_bytes());
        queue.write_buffer(&self.uniform, 0, &bytes);
    }

    fn draw(&self, pass: &mut iced::wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..4, 0..1);
    }
}

fn write_plane(queue: &iced::wgpu::Queue, texture: &iced::wgpu::Texture, plane: &[u8], width: u32, height: u32, scratch: &mut Vec<u8>) {
    let row = align_row(width);
    let tight = width as usize;
    scratch.clear();
    scratch.resize(row as usize * height as usize, 0);
    for y in 0..height as usize {
        let src = y * tight;
        let dst = y * row as usize;
        let end = src + tight;
        if end > plane.len() {
            return;
        }
        scratch[dst..dst + tight].copy_from_slice(&plane[src..end]);
    }
    queue.write_texture(iced::wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: iced::wgpu::Origin3d::ZERO, aspect: iced::wgpu::TextureAspect::All }, scratch, iced::wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) }, iced::wgpu::Extent3d { width, height, depth_or_array_layers: 1 });
}

fn align_row(width: u32) -> u32 {
    let align = iced::wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    width.div_ceil(align) * align
}

fn plane_tex(device: &iced::wgpu::Device, width: u32, height: u32) -> iced::wgpu::Texture {
    device.create_texture(&iced::wgpu::TextureDescriptor {
        label: Some("video plane"),
        size: iced::wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: iced::wgpu::TextureDimension::D2,
        format: iced::wgpu::TextureFormat::R8Unorm,
        usage: iced::wgpu::TextureUsages::TEXTURE_BINDING | iced::wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn bind_group(device: &iced::wgpu::Device, layout: &iced::wgpu::BindGroupLayout, planes: &Planes, sampler: &iced::wgpu::Sampler, uniform: &iced::wgpu::Buffer) -> iced::wgpu::BindGroup {
    let y = planes.y.create_view(&iced::wgpu::TextureViewDescriptor::default());
    let u = planes.u.create_view(&iced::wgpu::TextureViewDescriptor::default());
    let v = planes.v.create_view(&iced::wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&iced::wgpu::BindGroupDescriptor {
        label: Some("video surface"),
        layout,
        entries: &[
            iced::wgpu::BindGroupEntry { binding: 0, resource: iced::wgpu::BindingResource::TextureView(&y) },
            iced::wgpu::BindGroupEntry { binding: 1, resource: iced::wgpu::BindingResource::TextureView(&u) },
            iced::wgpu::BindGroupEntry { binding: 2, resource: iced::wgpu::BindingResource::TextureView(&v) },
            iced::wgpu::BindGroupEntry { binding: 3, resource: iced::wgpu::BindingResource::Sampler(sampler) },
            iced::wgpu::BindGroupEntry { binding: 4, resource: uniform.as_entire_binding() },
        ],
    })
}

const SHADER: &str = r#"
struct Fit { scale: vec2<f32> };
@group(0) @binding(0) var plane_y: texture_2d<f32>;
@group(0) @binding(1) var plane_u: texture_2d<f32>;
@group(0) @binding(2) var plane_v: texture_2d<f32>;
@group(0) @binding(3) var plane_sampler: sampler;
@group(0) @binding(4) var<uniform> fit: Fit;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var corner = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    var uv = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(corner[index] * fit.scale, 0.0, 1.0);
    out.uv = uv[index];
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let y = textureSample(plane_y, plane_sampler, in.uv).r;
    let u = textureSample(plane_u, plane_sampler, in.uv).r - 0.5;
    let v = textureSample(plane_v, plane_sampler, in.uv).r - 0.5;
    let luma = (y - 0.0625) * 1.164;
    let r = clamp(luma + 1.596 * v, 0.0, 1.0);
    let g = clamp(luma - 0.813 * v - 0.391 * u, 0.0, 1.0);
    let b = clamp(luma + 2.018 * u, 0.0, 1.0);
    return vec4<f32>(r, g, b, 1.0);
}
"#;
