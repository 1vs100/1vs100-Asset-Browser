pub type Rgb = [f32; 3];
pub type Rgba = [f32; 4];

const WHITE: Rgb = [1.0; 3];

#[derive(Debug, Clone, PartialEq)]
pub struct Tones {
    pub skin: Rgb,
    pub hair: Rgb,
    pub mouth: Rgb,
    pub iris: Rgb,
    pub eyebrow: Rgb,
    pub eye_shadow: Rgb,
    pub facial_hair: Rgb,
    pub skin_feature_1: Rgb,
    pub skin_feature_2: Rgb,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FaceLayers {
    pub skin_features: Option<Rgba>,
    pub facial_hair: Option<Rgba>,
    pub eyebrow: Option<Rgba>,
    pub eye: Option<Rgba>,
    pub mouth: Option<Rgba>,
    pub eye_shadow: Option<Rgba>,
}

pub fn rgb(argb: u32) -> Rgb {
    [16, 8, 0].map(|shift| ((argb >> shift) & 255) as f32 / 255.0)
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn weighted(mask: Rgba, colours: [Rgb; 3]) -> Rgb {
    std::array::from_fn(|i| mask[0] * colours[0][i] + mask[1] * colours[1][i] + mask[2] * colours[2][i])
}

pub fn intensity(base: Rgb, map: Rgba, custom: [Rgb; 3]) -> Rgb {
    mix(base, weighted(map, custom), map[3])
}

pub fn decal(base: Rgb, decal: Rgba) -> Rgb {
    mix(base, [decal[0], decal[1], decal[2]], decal[3])
}

pub fn face(t: &Tones, l: &FaceLayers) -> Rgb {
    let layers = [
        (l.skin_features, [t.skin_feature_1, t.skin_feature_2, t.skin]),
        (l.eye_shadow, [t.eye_shadow, [0.0; 3], [0.0; 3]]),
        (l.mouth, [t.mouth, WHITE, t.skin]),
        (l.eye, [t.iris, WHITE, t.skin]),
        (l.facial_hair, [t.facial_hair, WHITE, t.skin]),
        (l.eyebrow, [t.eyebrow, WHITE, t.skin]),
    ];
    layers.iter().fold(t.skin, |colour, (layer, tones)| layer.map_or(colour, |m| mix(colour, weighted(m, *tones), m[3])))
}
