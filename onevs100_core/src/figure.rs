use crate::model::{self, Model};
use crate::render::{self, Face, Part, Tex};
use crate::shade::{self, Rgb, Tones};
use crate::body::Gender;
use crate::{anim, avatar, blend, body, lzx, pose, skeleton, strb, toc};
use anyhow::{ensure, Result};
use std::sync::Arc;

const SLOT_TABLE: std::ops::Range<usize> = 0x10..0xe0;
const SLOT_SIZE: usize = 16;
const COLOUR_TABLE: usize = 0xfc;
const MODEL_TYPE: u32 = 1;
const TEXTURE_TYPE: u32 = 2;
const BODY_COMPONENT: u32 = 2;
const HEAD_COMPONENT: u32 = 1;
const HAIR_COMPONENT: u32 = 4;
const WHITE: Rgb = [1.0; 3];
const ANIMATION_TYPE: u32 = 4;

#[derive(Clone, Default)]
pub struct FaceLayers {
    pub mouth: Vec<Arc<Tex>>,
    pub eye: Vec<Arc<Tex>>,
    pub eyebrow: Vec<Arc<Tex>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expression {
    Neutral,
    Happy,
    Sad,
    Angry,
    Confused,
    Laughing,
    Shocked,
}

impl Expression {
    pub const ALL: [Expression; 7] = [Expression::Neutral, Expression::Happy, Expression::Sad, Expression::Angry, Expression::Confused, Expression::Laughing, Expression::Shocked];

    pub fn layers(self) -> (usize, usize, usize) {
        match self {
            Expression::Neutral => (0, 0, 0),
            Expression::Happy => (6, 6, 0),
            Expression::Sad => (1, 1, 1),
            Expression::Angry => (2, 2, 2),
            Expression::Confused => (3, 3, 3),
            Expression::Laughing => (4, 4, 0),
            Expression::Shocked => (5, 5, 4),
        }
    }
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn poses(pack: &[u8]) -> Vec<(usize, String)> {
    toc::split(pack).iter().filter(|a| a.asset_type == ANIMATION_TYPE).map(|a| (a.id, toc::name(pack, a.id).unwrap_or_default())).collect()
}
const WEIGHT_AT: usize = 4;
const HEIGHT_AT: usize = 8;
const STAND_ANIMATION: &str = "Animation Generic Stand 0";

#[derive(Clone)]
pub struct Worn {
    pub component: u32,
    pub model: Model,
    pub custom: Option<[Rgb; 3]>,
}

#[derive(Clone)]
pub struct Figure {
    pub parts: Vec<Worn>,
    pub face: Face,
    pub layers: FaceLayers,
    pub gender: Gender,
    pub weight: f32,
    pub height: f32,
}

fn palette(d: &[u8]) -> Vec<Rgb> {
    (0..9).map(|i| shade::rgb(u32::from_be_bytes(d[COLOUR_TABLE + 4 * i..COLOUR_TABLE + 4 * i + 4].try_into().unwrap()))).collect()
}

fn texture_asset(pack: &[u8], id: usize) -> Option<(u32, Vec<Arc<Tex>>)> {
    let asset = toc::entry(pack, id).filter(|a| a.asset_type == TEXTURE_TYPE)?;
    let s = strb::parse(asset.data).ok()?;
    let kind = s.blocks.iter().find(|b| b.id == 8).and_then(|b| crate::ytgr::metadata(&asset.data[b.start..b.start + b.length]).ok())?.mask;
    let b = s.blocks.iter().find(|b| b.id == 2)?;
    let t = model::parse_texture(&lzx::decompress(&asset.data[b.start..b.start + b.length]).ok()?).ok()?;
    let layers = (0..t.layers.max(1)).map(|l| Some(Arc::new(Tex { rgba: t.rgba_layer(l).ok()?, width: t.width as usize, height: t.height as usize }))).collect::<Option<Vec<_>>>()?;
    Some((kind, layers))
}

fn face(d: &[u8], pack: &[u8]) -> (Face, FaceLayers) {
    let c = palette(d);
    let mut face = Face {
        tones: Tones { skin: c[0], hair: c[1], mouth: c[2], iris: c[3], eyebrow: c[4], eye_shadow: c[5], facial_hair: c[6], skin_feature_1: c[7], skin_feature_2: c[8] },
        ..Face::default()
    };
    let mut all = FaceLayers::default();
    for o in SLOT_TABLE.step_by(SLOT_SIZE).filter(|&o| d[o + 4..o + 12] == avatar::GUID_SUFFIX) {
        let Some((kind, layers)) = texture_asset(pack, usize::from(u16::from_be_bytes([d[o], d[o + 1]]))) else { continue };
        let tex = layers[0].clone();
        match kind {
            0 => all.mouth = layers,
            1 => all.eye = layers,
            2 => all.eyebrow = layers,
            _ => {}
        }
        let slot = match kind {
            0 => &mut face.mouth,
            1 => &mut face.eye,
            2 => &mut face.eyebrow,
            3 => &mut face.facial_hair,
            4 => &mut face.eye_shadow,
            _ => &mut face.skin_features,
        };
        *slot = Some(tex);
    }
    (face, all)
}

fn custom_colours(d: &[u8], id: usize, component: u32, face: &Face) -> Option<[Rgb; 3]> {
    match component {
        BODY_COMPONENT => Some([face.tones.skin; 3]),
        HAIR_COMPONENT => Some([face.tones.hair; 3]),
        _ => avatar::record_colours(d, id).map(|c| std::array::from_fn(|i| c.get(i).copied().map_or(WHITE, shade::rgb))),
    }
}

pub fn build(avd: &[u8], pack: &[u8]) -> Result<Figure> {
    ensure!(avd.len() == avatar::AVD_SIZE, "AvatarMetadata blobs are {} bytes", avatar::AVD_SIZE);
    let (face, layers) = face(avd, pack);
    let parts = avatar::worn_ids(avd)
        .into_iter()
        .filter_map(|id| {
            let asset = toc::entry(pack, id).filter(|a| a.asset_type == MODEL_TYPE)?;
            let component = strb::parse(asset.data).ok().and_then(|s| s.blocks.iter().find(|b| b.id == 8).and_then(|b| crate::ytgr::metadata(&asset.data[b.start..b.start + b.length]).ok()))?.mask;
            Some((id, Worn { component, model: body::decode(asset.data).ok()?, custom: custom_colours(avd, id, component, &face) }))
        })
        .collect::<Vec<_>>();
    let shapes = blend_shapes(avd, pack);
    let parts = parts
        .into_iter()
        .map(|(_, w)| match w.component {
            BODY_COMPONENT => w,
            HEAD_COMPONENT => Worn { model: shapes.iter().fold(w.model, |m, s| blend::apply(&m, s)), ..w },
            _ => w,
        })
        .collect();
    let gender = body_gender(pack, avd);
    let factor = |at: usize| f32::from_be_bytes(avd[at..at + 4].try_into().unwrap());
    Ok(Figure { parts, face, layers, gender, weight: factor(WEIGHT_AT), height: factor(HEIGHT_AT) })
}

fn body_gender(pack: &[u8], avd: &[u8]) -> Gender {
    let flags = avatar::worn_ids(avd).into_iter().filter_map(|id| toc::entry(pack, id)).find_map(|a| {
        let s = strb::parse(a.data).ok()?;
        let m = s.blocks.iter().find(|b| b.id == 8).and_then(|b| crate::ytgr::metadata(&a.data[b.start..b.start + b.length]).ok())?;
        (m.mask == BODY_COMPONENT).then_some(m.flags)
    });
    Gender::from_flags(flags.unwrap_or(1))
}

fn blend_shapes(avd: &[u8], pack: &[u8]) -> Vec<blend::Shape> {
    SLOT_TABLE.step_by(SLOT_SIZE).filter(|&o| avd[o + 4..o + 12] == avatar::GUID_SUFFIX).filter_map(|o| toc::entry(pack, usize::from(u16::from_be_bytes([avd[o], avd[o + 1]])))).filter_map(|a| blend::shape(a.data).ok()).collect()
}

pub fn animation_rotations(pack: &[u8], id: usize, frame: usize) -> Result<Vec<skeleton::Rotation>> {
    let asset = toc::entry(pack, id).ok_or_else(|| anyhow::anyhow!("no asset {id}"))?;
    let s = strb::parse(asset.data)?;
    let b = s.blocks.iter().find(|b| b.id == 1).ok_or_else(|| anyhow::anyhow!("asset {id} has no animation"))?;
    let animation = anim::parse_block(&asset.data[b.start..b.start + b.length])?;
    Ok(animation.pose(frame)?.iter().map(|p| p.rotation).collect())
}

impl Figure {
    pub fn posed(&self, rotations: &[skeleton::Rotation]) -> Result<Figure> {
        let scales = skeleton::joint_scales(self.gender, self.weight, self.height);
        let matrices = skeleton::embedded(skeleton::PACK_VERSION)?.skinning_scaled(rotations, &scales);
        Ok(Figure { parts: self.parts.iter().map(|w| Worn { component: w.component, model: pose::skin(&w.model, &matrices), custom: w.custom }).collect(), face: self.face.clone(), layers: self.layers.clone(), ..*self })
    }

    pub fn express(&self, expression: Expression) -> Figure {
        let (mouth, eye, eyebrow) = expression.layers();
        let pick = |layers: &[Arc<Tex>], at: usize, current: &Option<Arc<Tex>>| layers.get(at.min(layers.len().saturating_sub(1))).cloned().or_else(|| current.clone());
        let face = Face { mouth: pick(&self.layers.mouth, mouth, &self.face.mouth), eye: pick(&self.layers.eye, eye, &self.face.eye), eyebrow: pick(&self.layers.eyebrow, eyebrow, &self.face.eyebrow), ..self.face.clone() };
        Figure { face, ..self.clone() }
    }

    pub fn scene(&self) -> render::Scene {
        render::Scene { parts: self.parts.iter().map(|w| render::ScenePart { model: w.model.clone(), custom: w.custom, tintable: false, depth_bias: if w.component == BODY_COMPONENT { render::BODY_DEPTH_BIAS } else { 0.0 } }).collect(), face: self.face.clone() }
    }

    pub fn styled(&self, pack: &[u8], pose: Option<usize>, expression: Expression) -> Result<render::Scene> {
        let figure = self.express(expression);
        let figure = match pose {
            Some(id) => figure.posed(&animation_rotations(pack, id, 0)?)?,
            None => figure.posed(&[])?,
        };
        Ok(figure.scene())
    }

    pub fn standing(&self, pack: &[u8]) -> Result<Figure> {
        match toc::find_by_name(pack, STAND_ANIMATION) {
            Some(id) => self.posed(&animation_rotations(pack, id, 0)?),
            None => self.posed(&[]),
        }
    }

    pub fn preview(&self, width: u32, height: u32) -> crate::inspect::Preview {
        let parts: Vec<Part> = self.parts.iter().map(|w| Part { model: &w.model, custom: w.custom, depth_bias: if w.component == BODY_COMPONENT { render::BODY_DEPTH_BIAS } else { 0.0 } }).collect();
        render::preview_avatar(&parts, &self.face, width, height)
    }
}

#[derive(Clone)]
pub struct FigureHandle(pub Arc<Figure>);

impl std::fmt::Debug for FigureHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FigureHandle({} parts)", self.0.parts.len())
    }
}

impl PartialEq for FigureHandle {
    fn eq(&self, other: &FigureHandle) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FigureHandle {}
