use crate::hiding;
use crate::model::{Batch, Header, Model, Vertex};

fn vertex(position: [f32; 3], normal: [f32; 3]) -> Vertex {
    Vertex { position, normal, weights: [1.0, 0.0, 0.0, 0.0], bones: [0; 4], color: 0xFFFF_FFFF, uvs: vec![[0.0, 0.0]] }
}

fn model(batches: Vec<Batch>) -> Model {
    Model { header: Header { cpu_buffer: 0, gpu_buffer: 0, texture_data: 0, vertex_buffer: 0, index_buffer: 0 }, batches, textures: vec![], trailing_bytes: 0 }
}

fn plane(z: f32, normal_z: f32, size: f32) -> Model {
    let c = [[-size, -size], [size, -size], [size, size], [-size, size]];
    let vertices = c.iter().map(|p| vertex([p[0], p[1], z], [0.0, 0.0, normal_z])).collect();
    model(vec![Batch { shader: 0, triangles: 2, uv_sets: 1, vertex_stride: 32, params: vec![], vertices, indices: vec![0, 1, 2, 0, 2, 3] }])
}

#[test]
fn a_garment_just_above_the_skin_hides_the_triangles_under_it() {
    let body = plane(0.0, 1.0, 1.0);
    assert_eq!(hiding::covered_by(&body, &[&plane(0.02, 1.0, 2.0)]), vec![0, 1]);
}

#[test]
fn garments_behind_or_far_in_front_or_beside_hide_nothing() {
    let body = plane(0.0, 1.0, 1.0);
    assert!(hiding::covered_by(&body, &[&plane(-0.05, 1.0, 2.0)]).is_empty());
    assert!(hiding::covered_by(&body, &[&plane(0.5, 1.0, 2.0)]).is_empty());
    let mut aside = plane(0.02, 1.0, 0.1);
    for v in &mut aside.batches[0].vertices {
        v.position[0] += 5.0;
    }
    assert!(hiding::covered_by(&body, &[&aside]).is_empty());
    assert!(hiding::covered_by(&body, &[]).is_empty());
}

#[test]
fn a_sparse_garment_only_hides_what_it_actually_crosses() {
    let body = plane(0.0, 1.0, 1.0);
    let strap = model(vec![Batch {
        shader: 0,
        triangles: 1,
        uv_sets: 1,
        vertex_stride: 32,
        params: vec![],
        vertices: vec![vertex([-1.0, 0.7, 0.02], [0.0, 0.0, 1.0]), vertex([1.0, 0.7, 0.02], [0.0, 0.0, 1.0]), vertex([0.0, 0.9, 0.02], [0.0, 0.0, 1.0])],
        indices: vec![0, 1, 2],
    }]);
    let hidden = hiding::covered_by(&body, &[&strap]);
    assert!(hidden.len() < 2);
}
