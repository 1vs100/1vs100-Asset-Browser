use crate::bitpack::{Bits, VectorCtx, VECTOR_HEADER_BITS};
use crate::body::Gender;
use anyhow::{ensure, Result};

const TABLE: &[u8] = include_bytes!("../data/embedded_skeletons.bin");
const VERSION_STRIDE: usize = 0x220;
const COUNT_BITS: usize = 32;
const PARENT_BITS: usize = 8;
const ROOT_PARENT: u32 = 255;
const MAX_JOINTS: usize = 72;

pub type Rotation = [f32; 4];
pub type Matrix = [[f32; 4]; 3];
pub type Scale = [f32; 3];

const BASE: usize = 0;
const HEAD: usize = 19;
const LEG_JOINTS: [usize; 4] = [7, 13, 9, 17];
const ARM_JOINTS: [usize; 6] = [32, 35, 26, 29, 27, 30];
const NECK: usize = 24;
const BACK_B: usize = 18;
const BACK_A: usize = 10;
const SC_BASE: usize = 4;

struct Weights {
    arms: Scale,
    legs: Scale,
    neck: Scale,
    back_b: Scale,
    back_a: Scale,
    base: Scale,
}

const MALE_FAT: Weights = Weights { arms: [1.0, 1.3, 1.3], legs: [1.3, 1.0, 1.3], neck: [1.54, 1.0, 1.3], back_b: [1.3, 1.0, 1.24], back_a: [1.48, 1.0, 1.54], base: [1.36, 1.36, 1.72] };
const FEMALE_FAT: Weights = Weights { arms: [1.0, 1.36, 1.36], legs: [1.36, 1.0, 1.36], neck: [1.6, 1.0, 1.36], back_b: [1.36, 1.0, 1.36], back_a: [1.36, 1.0, 1.6], base: [1.3, 1.3, 1.6] };
const MALE_THIN: Weights = Weights { arms: [1.0, 0.76, 0.76], legs: [0.76, 1.0, 0.76], neck: [0.76, 1.0, 0.76], back_b: [0.92, 1.0, 0.84], back_a: [0.68, 1.0, 0.68], base: [0.84, 1.0, 0.92] };
const FEMALE_THIN: Weights = Weights { arms: [1.0, 0.82, 0.82], legs: [0.82, 1.0, 0.82], neck: [0.82, 1.0, 0.82], back_b: [0.88, 1.0, 0.82], back_a: [0.82, 1.0, 0.7], base: [0.79, 1.0, 0.82] };
const TALL: (f32, f32) = (1.1, 0.9);
const SHORT: (f32, f32) = (0.9, 1.05);

fn lerp(extreme: Scale, amount: f32) -> Scale {
    extreme.map(|e| 1.0 + (e - 1.0) * amount)
}

pub fn joint_scales(gender: Gender, weight: f32, height: f32) -> Vec<Scale> {
    let mut scales = vec![[1.0; 3]; MAX_JOINTS - 1];
    let table = match (gender, weight >= 0.0) {
        (Gender::Male, true) => &MALE_FAT,
        (Gender::Female, true) => &FEMALE_FAT,
        (Gender::Male, false) => &MALE_THIN,
        (Gender::Female, false) => &FEMALE_THIN,
    };
    let amount = weight.abs().min(1.0);
    for j in ARM_JOINTS {
        scales[j] = lerp(table.arms, amount);
    }
    for j in LEG_JOINTS {
        scales[j] = lerp(table.legs, amount);
    }
    scales[NECK] = lerp(table.neck, amount);
    scales[BACK_B] = lerp(table.back_b, amount);
    scales[BACK_A] = lerp(table.back_a, amount);
    scales[SC_BASE] = lerp(table.base, amount);
    let (base, head) = if height >= 0.0 { TALL } else { SHORT };
    let amount = height.abs().min(1.0);
    scales[BASE] = lerp([base; 3], amount);
    scales[HEAD] = lerp([head; 3], amount);
    scales
}

#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    pub parents: Vec<Option<usize>>,
    pub positions: Vec<[f32; 3]>,
}

pub fn embedded(version: usize) -> Result<Skeleton> {
    ensure!((1..=TABLE.len() / VERSION_STRIDE).contains(&version), "no embedded skeleton version {version}");
    decode(&TABLE[(version - 1) * VERSION_STRIDE..])
}

fn decode(d: &[u8]) -> Result<Skeleton> {
    let bits = Bits(d);
    let count = bits.read(0, COUNT_BITS) as usize;
    ensure!(count > 0 && count <= MAX_JOINTS, "implausible joint count {count}");
    let position = VectorCtx::parse(&bits, COUNT_BITS);
    let rotation = VectorCtx::parse(&bits, COUNT_BITS + VECTOR_HEADER_BITS);
    let data = COUNT_BITS + 2 * VECTOR_HEADER_BITS;
    let stride = PARENT_BITS + position.bit_count() + rotation.bit_count();
    ensure!((data + count * stride).div_ceil(8) <= d.len(), "skeleton stream is truncated");
    let (mut parents, mut positions) = (Vec::new(), Vec::new());
    for j in 0..count {
        let at = data + j * stride;
        let parent = bits.read(at, PARENT_BITS);
        parents.push((parent != ROOT_PARENT).then_some(parent as usize));
        positions.push(position.decode(&bits, at + PARENT_BITS));
    }
    Ok(Skeleton { parents, positions })
}

fn matrix_from(q: Rotation, translation: [f32; 3]) -> Matrix {
    let [x, y, z, w] = q;
    let n = (x * x + y * y + z * z + w * w).sqrt().max(1e-9);
    let (x, y, z, w) = (x / n, y / n, z / n, w / n);
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w), translation[0]],
        [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w), translation[1]],
        [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y), translation[2]],
    ]
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + if c == 3 { a[r][3] } else { 0.0 }))
}

pub fn transform(m: &Matrix, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
}

impl Skeleton {
    pub fn skinning(&self, rotations: &[Rotation]) -> Vec<Matrix> {
        self.skinning_scaled(rotations, &[])
    }

    pub fn skinning_scaled(&self, rotations: &[Rotation], scales: &[Scale]) -> Vec<Matrix> {
        let mut world: Vec<Matrix> = Vec::with_capacity(self.parents.len());
        for (j, parent) in self.parents.iter().enumerate() {
            let rotation = rotations.get(j).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]);
            let offset = parent.map_or(self.positions[j], |p| std::array::from_fn(|i| self.positions[j][i] - self.positions[p][i]));
            let scale = scales.get(j).copied().unwrap_or([1.0; 3]);
            let mut rotate = matrix_from(rotation, [0.0; 3]);
            for (row, k) in rotate.iter_mut().zip(scale) {
                row.iter_mut().for_each(|v| *v *= k);
            }
            let local = multiply(&matrix_from([0.0, 0.0, 0.0, 1.0], offset), &rotate);
            world.push(parent.map_or(local, |p| multiply(&world[p], &local)));
        }
        world.iter().zip(&self.positions).map(|(w, bind)| multiply(w, &matrix_from([0.0, 0.0, 0.0, 1.0], bind.map(|v| -v)))).collect()
    }
}

pub const PACK_VERSION: usize = 2;
