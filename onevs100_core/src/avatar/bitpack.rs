pub(crate) const VECTOR_HEADER_BITS: usize = 146;
const STEP_RATIOS: [f32; 3] = [2.0, 1.632_993_2, 1.732_050_8];
const THIRD: f32 = 1.0 / 3.0;

pub(crate) struct Bits<'a>(pub(crate) &'a [u8]);

impl Bits<'_> {
    pub(crate) fn read(&self, pos: usize, n: usize) -> u32 {
        if n == 0 {
            return 0;
        }
        let (first, shift) = (pos / 8, pos % 8);
        let mut acc: u64 = 0;
        for i in 0..(shift + n).div_ceil(8) {
            acc |= u64::from(*self.0.get(first + i).unwrap_or(&0)) << (8 * i);
        }
        ((acc >> shift) & ((1u64 << n) - 1)) as u32
    }

    pub(crate) fn float(&self, pos: usize) -> f32 {
        f32::from_bits(self.read(pos, 32))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VectorCtx {
    pub(crate) mins: [f32; 3],
    pub(crate) steps: [f32; 3],
    pub(crate) widths: [usize; 3],
}

impl VectorCtx {
    pub(crate) fn parse(bits: &Bits, pos: usize) -> Self {
        let radius = bits.float(pos);
        VectorCtx {
            mins: [bits.float(pos + 32), bits.float(pos + 64), bits.float(pos + 96)],
            steps: STEP_RATIOS.map(|r| radius * r),
            widths: [bits.read(pos + 128, 6) as usize, bits.read(pos + 134, 6) as usize, bits.read(pos + 140, 6) as usize],
        }
    }

    pub(crate) fn bit_count(&self) -> usize {
        self.widths.iter().sum()
    }

    pub(crate) fn decode(&self, bits: &Bits, pos: usize) -> [f32; 3] {
        let q0 = bits.read(pos, self.widths[0]);
        let q1 = bits.read(pos + self.widths[0], self.widths[1]);
        let q2 = bits.read(pos + self.widths[0] + self.widths[1], self.widths[2]);
        let y = q1 as f32 * self.steps[1] + self.mins[1];
        let mut z = q2 as f32 * self.steps[2] + self.mins[2];
        if q1 & 1 == 1 {
            z += self.steps[2] * THIRD;
        }
        let mut x = q0 as f32 * self.steps[0] + self.mins[0];
        if (q1 ^ q2) & 1 == 1 {
            x += self.steps[0] * 0.5;
        }
        [x, y, z]
    }
}

