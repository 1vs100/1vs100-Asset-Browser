//! 1 vs 100 Episode/Show Display Frontend
//! 
//! A Rust renderer for Xbox 360 "1 vs 100" content packages.
//! Supports rendering extracted assets: textures, meshes, UI scenes (XUR), animations.

use std::path::PathBuf;
use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::{
    event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod render;
mod assets;
mod xenos;
mod xui;
mod stfs;

use render::Renderer;
use assets::{AssetManager, TextureFormat, Mesh};
use xui::{XuiScene, XuiElement};

/// Main application state
struct App {
    renderer: Renderer,
    asset_manager: AssetManager,
    current_scene: Option<XuiScene>,
    show_ui: bool,
    egui_renderer: egui_wgpu::Renderer,
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    last_frame_time: std::time::Instant,
    frame_count: u64,
}

impl App {
    async fn new(window: Arc<Window>, content_path: PathBuf) -> Self {
        let size = window.inner_size();
        let renderer = Renderer::new(window.clone(), size.width, size.height).await;
        
        let asset_manager = AssetManager::new(content_path);
        
        // Initialize egui
        let egui_context = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            &renderer.device,
            renderer.config.format,
            None,
            1,
            true,
        );
        
        Self {
            renderer,
            asset_manager,
            current_scene: None,
            show_ui: true,
            egui_renderer,
            egui_context,
            egui_state,
            last_frame_time: std::time::Instant::now(),
            frame_count: 0,
        }
    }
    
    fn handle_event(&mut self, event: &WindowEvent) -> bool {
        // Let egui handle events first
        let _ = self.egui_state.on_window_event(&self.egui_context, event);
        
        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                if let KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    state: ElementState::Pressed,
                    ..
                } = input {
                    *self.renderer.control_flow.lock().unwrap() = ControlFlow::Exit;
                    return true;
                }
                
                if let KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::F1),
                    state: ElementState::Pressed,
                    ..
                } = input {
                    self.show_ui = !self.show_ui;
                    return true;
                }
                
                if let KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::F2),
                    state: ElementState::Pressed,
                    ..
                } = input {
                    // Reload assets
                    self.asset_manager.reload();
                    return true;
                }
                
                if let KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::F3),
                    state: ElementState::Pressed,
                    ..
                } = input {
                    // Cycle through scenes
                    self.load_next_scene();
                    return true;
                }
            }
            WindowEvent::Resized(size) => {
                self.renderer.resize(*size);
                self.egui_renderer.resize(&self.renderer.device, *size);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.egui_state = egui_winit::State::new(
                    self.egui_context.clone(),
                    egui::ViewportId::ROOT,
                    &self.renderer.window,
                    Some(*scale_factor as f32),
                    None,
                );
            }
            _ => {}
        }
        false
    }
    
    fn load_next_scene(&mut self) {
        let scenes = self.asset_manager.list_scenes();
        if scenes.is_empty() {
            return;
        }
        
        let current_idx = self.current_scene.as_ref()
            .and_then(|s| scenes.iter().position(|x| x == &s.name))
            .unwrap_or(0);
        
        let next_idx = (current_idx + 1) % scenes.len();
        let scene_name = &scenes[next_idx].name;
        
        if let Ok(scene) = self.asset_manager.load_scene(scene_name) {
            self.current_scene = Some(scene);
            println!("Loaded scene: {}", scene_name);
        }
    }
    
    fn render_ui(&mut self) {
        if !self.show_ui {
            return;
        }
        
        let egui_ctx = &self.egui_context;
        let window = &self.renderer.window;
        
        // Main menu bar
        egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load Scene...").clicked() {
                        // TODO: File dialog
                    }
                    if ui.button("Reload Assets (F2)").clicked() {
                        self.asset_manager.reload();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        *self.renderer.control_flow.lock().unwrap() = ControlFlow::Exit;
                    }
                });
                
                ui.menu_button("View", |ui| {
                    if ui.button("Toggle UI (F1)").clicked() {
                        self.show_ui = !self.show_ui;
                    }
                    if ui.button("Next Scene (F3)").clicked() {
                        self.load_next_scene();
                    }
                });
                
                ui.menu_button("Render", |ui| {
                    if ui.button("Wireframe").clicked() {
                        self.renderer.toggle_wireframe();
                    }
                    if ui.button("Show Normals").clicked() {
                        self.renderer.toggle_normals();
                    }
                });
            });
        });
        
        // Scene info panel
        egui::SidePanel::left("scene_info").show(egui_ctx, |ui| {
            ui.heading("1 vs 100 Frontend");
            ui.separator();
            
            if let Some(scene) = &self.current_scene {
                ui.label(format!("Scene: {}", scene.name));
                ui.label(format!("Elements: {}", scene.elements.len()));
                ui.label(format!("Textures: {}", scene.textures.len()));
            } else {
                ui.label("No scene loaded");
                ui.label("Press F3 to load first scene");
            }
            
            ui.separator();
            
            ui.label("Controls:");
            ui.label("  F1 - Toggle UI");
            ui.label("  F2 - Reload Assets");
            ui.label("  F3 - Next Scene");
            ui.label("  ESC - Exit");
            
            ui.separator();
            
            // Asset browser
            ui.heading("Available Assets");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for texture in self.asset_manager.list_textures() {
                    if ui.button(&texture).clicked() {
                        println!("Selected texture: {}", texture);
                    }
                }
            });
        });
        
        // Performance panel
        egui::TopBottomPanel::bottom("performance").show(egui_ctx, |ui| {
            let dt = self.last_frame_time.elapsed().as_secs_f32();
            let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
            
            ui.horizontal(|ui| {
                ui.label(format!("FPS: {:.1}", fps));
                ui.label(format!("Frame: {}", self.frame_count));
                ui.label(format!("Frame time: {:.2} ms", dt * 1000.0));
            });
        });
    }
    
    fn render_frame(&mut self) {
        let frame = self.renderer.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self.renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Frame Encoder"),
        });
        
        // Render scene
        if let Some(scene) = &self.current_scene {
            self.renderer.render_scene(&mut encoder, &view, scene, &self.asset_manager);
        } else {
            // Clear to dark gray
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.renderer.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
            });
        }
        
        // Render UI
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.renderer.config.width, self.renderer.config.height],
            pixels_per_point: self.renderer.window.as_ref().unwrap().scale_factor() as f32,
        };
        
        let egui_ctx = &self.egui_context;
        let paint_jobs = egui_ctx.tessellate(egui_ctx.take_shapes());
        self.egui_renderer.render(
            &self.renderer.device,
            &self.renderer.queue,
            &mut encoder,
            &view,
            &screen_descriptor,
            &paint_jobs,
            None,
        ).unwrap();
        
        self.renderer.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        
        self.frame_count += 1;
        self.last_frame_time = std::time::Instant::now();
    }
    
    async fn run(mut self) {
        let event_loop = self.renderer.event_loop.take().unwrap();
        let window = self.renderer.window.take().unwrap();
        
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            
            match event {
                Event::WindowEvent { event, .. } => {
                    if self.handle_event(&event) {
                        return;
                    }
                }
                Event::MainEventsCleared => {
                    self.render_frame();
                    window.request_redraw();
                }
                _ => {}
            }
        }).unwrap();
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Parse command line args
    let args: Vec<String> = std::env::args().collect();
    let content_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("../../raw_content/xblive_dlcs")
    };
    
    println!("1 vs 100 Frontend");
    println!("Content path: {:?}", content_path);
    
    // Create window
    let event_loop = EventLoop::new();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("1 vs 100 - Episode/Show Display")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
    );
    
    let app = App::new(window, content_path).await;
    app.run().await;
}