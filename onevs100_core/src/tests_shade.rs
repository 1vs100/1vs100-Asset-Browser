use crate::shade::{self, FaceLayers, Rgb, Tones};

const WHITE: Rgb = [1.0, 1.0, 1.0];

fn close(a: Rgb, b: Rgb) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5)
}

#[test]
fn argb_colours_split_into_unit_rgb() {
    assert!(close(shade::rgb(0xFF336699), [0.2, 0.4, 0.6]));
}

#[test]
fn intensity_map_replaces_the_base_by_the_custom_colours_where_alpha_is_set() {
    let custom = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let base = [0.5, 0.5, 0.5];
    assert!(close(shade::intensity(base, [1.0, 0.0, 0.0, 1.0], custom), [1.0, 0.0, 0.0]));
    assert!(close(shade::intensity(base, [0.5, 0.5, 0.0, 1.0], custom), [0.5, 0.5, 0.0]));
    assert!(close(shade::intensity(base, [1.0, 0.0, 0.0, 0.0], custom), base));
    assert!(close(shade::intensity(base, [1.0, 0.0, 0.0, 0.5], custom), [0.75, 0.25, 0.25]));
}

#[test]
fn decals_blend_by_their_alpha() {
    assert!(close(shade::decal([0.0; 3], [1.0, 1.0, 1.0, 0.25]), [0.25; 3]));
}

fn tones() -> Tones {
    Tones { skin: [0.8, 0.6, 0.4], hair: WHITE, mouth: [1.0, 0.0, 0.0], iris: [0.0, 0.0, 1.0], eyebrow: [0.1, 0.1, 0.1], eye_shadow: [0.5, 0.0, 0.5], facial_hair: [0.2; 3], skin_feature_1: [0.3; 3], skin_feature_2: [0.4; 3] }
}

#[test]
fn a_face_without_layers_is_the_skin_tone() {
    assert!(close(shade::face(&tones(), &FaceLayers::default()), [0.8, 0.6, 0.4]));
}

#[test]
fn face_layers_blend_in_the_shader_order() {
    let mouth = FaceLayers { mouth: Some([1.0, 0.0, 0.0, 1.0]), ..FaceLayers::default() };
    assert!(close(shade::face(&tones(), &mouth), [1.0, 0.0, 0.0]));
    let teeth = FaceLayers { mouth: Some([0.0, 1.0, 0.0, 1.0]), ..FaceLayers::default() };
    assert!(close(shade::face(&tones(), &teeth), WHITE));
    let skin_channel = FaceLayers { mouth: Some([0.0, 0.0, 1.0, 1.0]), ..FaceLayers::default() };
    assert!(close(shade::face(&tones(), &skin_channel), [0.8, 0.6, 0.4]));
    let brow_over_eye = FaceLayers { eye: Some([0.0, 0.0, 1.0, 1.0]), eyebrow: Some([1.0, 0.0, 0.0, 1.0]), ..FaceLayers::default() };
    assert!(close(shade::face(&tones(), &brow_over_eye), [0.1, 0.1, 0.1]));
    let half = FaceLayers { eye_shadow: Some([1.0, 0.0, 0.0, 0.5]), ..FaceLayers::default() };
    assert!(close(shade::face(&tones(), &half), [0.65, 0.3, 0.45]));
}
