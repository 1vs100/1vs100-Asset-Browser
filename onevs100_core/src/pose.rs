use crate::model::Model;
use crate::skeleton::{transform, Matrix};

fn blend(matrices: &[Matrix], bones: [u8; 4], weights: [f32; 4], p: [f32; 3], rotate_only: bool) -> [f32; 3] {
    let total: f32 = (0..4).filter(|&k| usize::from(bones[k]) < matrices.len()).map(|k| weights[k]).sum();
    if total <= 0.0 {
        return p;
    }
    let mut out = [0.0f32; 3];
    for k in (0..4).filter(|&k| usize::from(bones[k]) < matrices.len() && weights[k] > 0.0) {
        let m = &matrices[usize::from(bones[k])];
        let moved = if rotate_only { transform(&[[m[0][0], m[0][1], m[0][2], 0.0], [m[1][0], m[1][1], m[1][2], 0.0], [m[2][0], m[2][1], m[2][2], 0.0]], p) } else { transform(m, p) };
        for (o, v) in out.iter_mut().zip(moved) {
            *o += v * weights[k] / total;
        }
    }
    out
}

pub fn skin(model: &Model, matrices: &[Matrix]) -> Model {
    let mut posed = model.clone();
    for batch in &mut posed.batches {
        for v in &mut batch.vertices {
            v.position = blend(matrices, v.bones, v.weights, v.position, false);
            v.normal = blend(matrices, v.bones, v.weights, v.normal, true);
        }
    }
    posed
}
