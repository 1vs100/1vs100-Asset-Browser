use crate::{inspect, lzx, model, render, source, ytgr};
use std::path::PathBuf;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/avatar/avataritems_raw");

fn asset(group: &str, name: &str) -> Option<Vec<u8>> {
    std::fs::read(PathBuf::from(CORPUS).join(group).join(name).join("asset_v2.bin")).ok()
}

fn slippers() -> Option<Vec<u8>> {
    asset("#Trending", "Grumpy Cat Slippers")
}

fn decoded(d: &[u8]) -> model::Model {
    let y = ytgr::parse(d).unwrap();
    model::parse(&lzx::decompress(y.block_data(d, 3).unwrap()).unwrap()).unwrap()
}

fn close(a: &[f32], b: &[f32], tol: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

#[test]
fn model_header_and_batches_match_the_python_reference() {
    let Some(d) = slippers() else { return };
    let m = decoded(&d);
    assert_eq!((m.batches.len(), m.textures.len()), (2, 2));
    let b = &m.batches[0];
    assert_eq!((b.shader, b.triangles, b.vertices.len(), b.uv_sets, b.indices.len()), (2, 1296, 914, 2, 3888));
    assert_eq!(*b.indices.iter().max().unwrap(), 913);
    assert_eq!(&b.indices[..6], &[2, 1, 0, 5, 4, 3]);
    let small = &m.batches[1];
    assert_eq!((small.triangles, small.vertices.len(), small.indices.len()), (36, 40, 108));
}

#[test]
fn vertex_values_match_the_python_reference() {
    let Some(d) = slippers() else { return };
    let v = &decoded(&d).batches[0].vertices;
    assert!(close(&v[0].position, &[-0.10115, 0.00059, 0.14610], 1e-4));
    assert!(close(&v[0].normal, &[0.0, -0.69306, 0.72016], 2e-3));
    assert_eq!((v[0].weights, v[0].bones, v[0].color), ([1.0, 0.0, 0.0, 0.0], [23, 0, 0, 0], 0xFFFF_FFFF));
    assert!(close(&v[0].uvs[0], &[0.007515, 0.109253], 1e-4));
    assert_eq!(v[500].bones[0], 21);
    assert!(close(&v[500].position, &[0.07381, 0.00059, 0.13840], 1e-4));
}

#[test]
fn geometry_is_physically_plausible() {
    let Some(d) = slippers() else { return };
    let b = &decoded(&d).batches[0];
    let (lo, hi) = b.bounds();
    assert!(close(&lo, &[-0.153, 0.0005, -0.11], 2e-3) && close(&hi, &[0.153, 0.156, 0.153], 2e-3), "{lo:?} {hi:?}");
    for v in &b.vertices {
        let n = v.normal.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 0.02, "normal length {n}");
        assert!((v.weights.iter().sum::<f32>() - 1.0).abs() < 0.01);
        assert!(v.uvs.iter().flatten().all(|c| (0.0..=1.0).contains(c)));
    }
}

#[test]
fn shader_parameters_are_colours_and_texture_references() {
    let Some(d) = slippers() else { return };
    let p = &decoded(&d).batches[0].params;
    assert_eq!(p.len(), 8);
    assert_eq!((p[0].usage, p[0].value.clone()), (22, model::ParamValue::Vector([0.0, 0.0, 0.0, 1.0])));
    let textures: Vec<_> = p.iter().filter_map(|x| match x.value { model::ParamValue::Texture { index, .. } => Some((x.usage, index)), _ => None }).collect();
    assert_eq!(textures, vec![(1, 0), (2, 1), (4, 65535)]);
}

#[test]
fn textures_decode_to_the_reference_pixels() {
    let Some(d) = slippers() else { return };
    let t = &decoded(&d).textures;
    assert_eq!((t[0].width, t[0].height, t[1].width, t[1].height), (128, 128, 32, 32));
    let px = t[0].rgba().unwrap();
    let at = |x: usize, y: usize| px[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4].to_vec();
    assert_eq!(at(10, 10), vec![40, 38, 40, 255]);
    assert_eq!(at(64, 100), vec![221, 202, 175, 255]);
    assert_eq!(at(120, 120), vec![106, 85, 65, 255]);
    assert_eq!(t[1].rgba().unwrap().len(), 32 * 32 * 4);
}

#[test]
fn rendering_produces_a_non_trivial_deterministic_preview() {
    let Some(d) = slippers() else { return };
    let m = decoded(&d);
    let a = render::preview(&m, 256, 256);
    let b = render::preview(&m, 256, 256);
    assert_eq!((a.width, a.height, a.rgba.len()), (256, 256, 256 * 256 * 4));
    assert_eq!(a, b);
    let covered = a.rgba.chunks(4).filter(|p| p[3] > 0).count();
    assert!(covered > 256 * 256 / 20 && covered < 256 * 256 * 9 / 10, "coverage {covered}");
    let distinct: std::collections::HashSet<_> = a.rgba.chunks(4).filter(|p| p[3] > 0).map(|p| [p[0], p[1], p[2]]).collect();
    assert!(distinct.len() > 50, "flat image: {} colours", distinct.len());
}

#[test]
fn a_corpus_sample_decodes_with_consistent_geometry() {
    let Ok(groups) = std::fs::read_dir(CORPUS) else { return };
    let (mut checked, mut vertices) = (0, 0);
    for g in groups.flatten().take(40) {
        for it in std::fs::read_dir(g.path()).unwrap().flatten().take(5) {
            let Ok(d) = std::fs::read(it.path().join("asset_v2.bin")) else { continue };
            let y = ytgr::parse(&d).unwrap();
            let Ok(block) = y.block_data(&d, 3) else { continue };
            let m = model::parse(&lzx::decompress(block).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", it.path().display()));
            for b in &m.batches {
                assert!(b.indices.iter().all(|&i| usize::from(i) < b.vertices.len()), "{}", it.path().display());
                vertices += b.vertices.len();
            }
            checked += 1;
        }
    }
    assert!(checked >= 20 && vertices > 10_000, "{checked} models, {vertices} vertices");
}

#[test]
fn asset_details_include_the_model_and_a_rendered_preview() {
    let Some(d) = slippers() else { return };
    let det = inspect::describe("asset_v2.bin", &d);
    assert!(det.lines.iter().any(|l| l.contains("model: 2 batches, 2 textures")), "{:#?}", det.lines);
    assert!(det.lines.iter().any(|l| l.contains("batch 0") && l.contains("1296 triangles") && l.contains("914 vertices")));
    let img = det.image.expect("model preview");
    assert!(img.rgba.chunks(4).any(|p| p[3] > 0));
    let kids = source::expand(&d).unwrap().unwrap();
    let entry = kids.iter().find(|k| k.name == "block3.model").unwrap();
    let block = inspect::describe(&entry.name, &entry.payload.read().unwrap());
    assert_eq!(block.kind, "Avatar model");
    assert!(block.image.is_some());
}

#[test]
fn damaged_models_are_errors() {
    let Some(d) = slippers() else { return };
    let y = ytgr::parse(&d).unwrap();
    let raw = lzx::decompress(y.block_data(&d, 3).unwrap()).unwrap();
    assert!(model::parse(&raw[..raw.len() - 2000]).is_err());
    assert!(model::parse(&raw[..20]).is_err());
    assert!(model::parse(&[]).is_err());
}

#[test]
fn tint_driven_batches_with_black_textures_still_show_their_shape() {
    let Some(d) = asset("#Trending", "Forever Alone Head") else { return };
    let img = render::preview(&decoded(&d), 192, 192);
    let lit: std::collections::HashSet<_> = img.rgba.chunks(4).filter(|p| p[3] > 0).map(|p| p[0] / 8).collect();
    assert!(lit.len() > 8, "only {} brightness levels", lit.len());
    assert!(img.rgba.chunks(4).filter(|p| p[3] > 0).any(|p| p[0] > 60));
}
