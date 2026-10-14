use crate::inspect::Preview;
use crate::model::{Batch, Header, Model, Vertex};
use crate::render::{self, Camera, Face, Part, Scene};

fn vertex(position: [f32; 3]) -> Vertex {
    Vertex { position, normal: [0.0, 0.0, 1.0], weights: [1.0, 0.0, 0.0, 0.0], bones: [0; 4], color: 0xFFFF_FFFF, uvs: vec![[0.0, 0.0]] }
}

fn slab() -> Model {
    let corners = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]];
    let batch = Batch { shader: 0, triangles: 2, uv_sets: 1, vertex_stride: 32, params: vec![], vertices: corners.iter().map(|c| vertex(*c)).collect(), indices: vec![0, 1, 2, 0, 2, 3] };
    Model { header: Header { cpu_buffer: 0, gpu_buffer: 0, texture_data: 0, vertex_buffer: 0, index_buffer: 0 }, batches: vec![batch], textures: vec![], trailing_bytes: 0 }
}

fn scene() -> Scene {
    Scene { parts: vec![render::ScenePart { model: slab(), custom: None, tintable: false, depth_bias: 0.0 }], face: Face::default() }
}

fn lit(p: &Preview) -> usize {
    p.rgba.chunks(4).filter(|c| c[3] > 0).count()
}

fn centre_of_mass(p: &Preview) -> (f32, f32) {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for (i, c) in p.rgba.chunks(4).enumerate().filter(|(_, c)| c[3] > 0) {
        let _ = c;
        sx += (i % p.width as usize) as f32;
        sy += (i / p.width as usize) as f32;
        n += 1.0;
    }
    (sx / n, sy / n)
}

#[test]
fn the_default_camera_matches_the_plain_preview() {
    let m = slab();
    let plain = render::preview(&m, 96, 96);
    let via_scene = scene().render(&Camera::default(), 96, 96);
    assert_eq!(plain.rgba, via_scene.rgba);
    let parts = [Part { model: &m, custom: None, depth_bias: 0.0 }];
    assert_eq!(plain.rgba, render::preview_avatar(&parts, &Face::default(), 96, 96).rgba);
}

#[test]
fn rotating_the_camera_changes_the_view_and_a_full_turn_does_not() {
    let s = scene();
    let face_on = Camera { yaw: std::f32::consts::PI, pitch: 0.0, ..Camera::default() };
    let base = s.render(&face_on, 96, 96);
    let turned = s.render(&Camera { yaw: face_on.yaw + std::f32::consts::FRAC_PI_2, ..face_on }, 96, 96);
    let full = s.render(&Camera { yaw: face_on.yaw + std::f32::consts::TAU, ..face_on }, 96, 96);
    assert!(lit(&turned) < lit(&base) / 20, "{} {}", lit(&turned), lit(&base));
    assert!(base.rgba.iter().zip(&full.rgba).all(|(a, b)| a.abs_diff(*b) <= 1));
}

#[test]
fn zoom_scales_and_pan_moves_the_model() {
    let s = scene();
    let flat = Camera { yaw: std::f32::consts::PI, pitch: 0.0, ..Camera::default() };
    let zoomed = s.render(&Camera { zoom: 0.5, ..flat }, 128, 128);
    let flat = Camera { zoom: 0.5, ..flat };
    let base = s.render(&flat, 128, 128);
    let quarter = lit(&s.render(&Camera { zoom: 0.25, ..flat }, 128, 128)) as f32;
    assert!((lit(&zoomed) as f32 / quarter - 4.0).abs() < 0.4, "{} {}", lit(&zoomed), quarter);
    let moved = s.render(&Camera { pan: [20.0, -10.0], ..flat }, 128, 128);
    let (bx, by) = centre_of_mass(&base);
    let (mx, my) = centre_of_mass(&moved);
    assert!((mx - bx - 20.0).abs() < 1.5 && (my - by + 10.0).abs() < 1.5, "{bx},{by} -> {mx},{my}");
}

#[test]
fn an_empty_scene_renders_a_blank_image() {
    let s = Scene { parts: vec![], face: Face::default() };
    assert_eq!(lit(&s.render(&Camera::default(), 32, 32)), 0);
}

#[test]
fn tinting_only_touches_tintable_parts() {
    let mut s = scene();
    s.parts.push(render::ScenePart { model: slab(), custom: None, tintable: true, depth_bias: 0.0 });
    let tinted = s.tinted([[1.0, 0.0, 0.0]; 3]);
    assert_eq!(tinted.parts[0].custom, None);
    assert_eq!(tinted.parts[1].custom, Some([[1.0, 0.0, 0.0]; 3]));
    assert!(s.is_tintable() && !scene().is_tintable());
}

#[test]
fn textures_are_sampled_bilinearly_with_wrapping() {
    let tex = render::Tex { rgba: vec![0, 0, 0, 255, 255, 255, 255, 255], width: 2, height: 1 };
    let at = |u: f32| tex.sample([u, 0.5]);
    assert_eq!(at(0.25), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(at(0.75), [1.0, 1.0, 1.0, 1.0]);
    let mid = at(0.5);
    assert!((mid[0] - 0.5).abs() < 1e-5 && mid[3] == 1.0);
    let wrapped = at(0.0);
    assert!((wrapped[0] - 0.5).abs() < 1e-5);
    assert_eq!(at(1.25), at(0.25));
}

fn coloured(z: f32, argb: u32) -> Model {
    let mut m = slab();
    for v in &mut m.batches[0].vertices {
        v.position[2] = z;
        v.color = argb;
    }
    m
}

fn centre_pixel(p: &Preview) -> [u8; 3] {
    let o = ((p.height / 2 * p.width + p.width / 2) * 4) as usize;
    [p.rgba[o], p.rgba[o + 1], p.rgba[o + 2]]
}

#[test]
fn a_depth_bias_lets_a_garment_just_behind_the_body_win() {
    let face_on = Camera { yaw: std::f32::consts::PI, pitch: 0.0, ..Camera::default() };
    let tint = Some([[1.0; 3]; 3]);
    let render_with = |bias: f32| {
        let parts = [
            render::ScenePart { model: coloured(0.0, 0xFF00_00FF), custom: tint, tintable: false, depth_bias: bias },
            render::ScenePart { model: coloured(-0.01, 0xFFFF_0000), custom: tint, tintable: false, depth_bias: 0.0 },
        ];
        Scene { parts: parts.into(), face: Face::default() }.render(&face_on, 64, 64)
    };
    let plain = centre_pixel(&render_with(0.0));
    let biased = centre_pixel(&render_with(0.05));
    assert!(plain[2] > plain[0], "body should win without a bias: {plain:?}");
    assert!(biased[0] > biased[2], "garment should win with a bias: {biased:?}");
}
