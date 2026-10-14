use super::bitpack::{Bits, VectorCtx, VECTOR_HEADER_BITS};
use anyhow::{ensure, Result};

pub const JOINT_NAMES: [&str; 71] = [
    "BASE",
    "BACKA",
    "LF_H",
    "RT_H",
    "SC_BASE",
    "BACKB",
    "LF_K",
    "LF_SC_H",
    "RT_K",
    "RT_SC_H",
    "SC_BACKA",
    "LF_A",
    "LF_C",
    "LF_SC_K",
    "NECK",
    "RT_A",
    "RT_C",
    "RT_SC_K",
    "SC_BACKB",
    "HEAD",
    "LF_S",
    "LF_T",
    "RT_S",
    "RT_T",
    "SC_NECK",
    "LF_E",
    "LF_SC_S",
    "LF_SC_TWIST_S",
    "RT_E",
    "RT_SC_S",
    "RT_SC_TWIST_S",
    "LF_E_TWIST",
    "LF_SC_E",
    "LF_W",
    "RT_E_TWIST",
    "RT_SC_E",
    "RT_W",
    "LF_FINGA",
    "LF_FINGB",
    "LF_FINGC",
    "LF_FINGD",
    "LF_PROP",
    "LF_SPECIAL",
    "LF_THUMB",
    "RT_FINGA",
    "RT_FINGB",
    "RT_FINGC",
    "RT_FINGD",
    "RT_PROP",
    "RT_SPECIAL",
    "RT_THUMB",
    "LF_FINGA1",
    "LF_FINGB1",
    "LF_FINGC1",
    "LF_FINGD1",
    "LF_THUMB1",
    "RT_FINGA1",
    "RT_FINGB1",
    "RT_FINGC1",
    "RT_FINGD1",
    "RT_THUMB1",
    "LF_FINGA2",
    "LF_FINGB2",
    "LF_FINGC2",
    "LF_FINGD2",
    "LF_THUMB2",
    "RT_FINGA2",
    "RT_FINGB2",
    "RT_FINGC2",
    "RT_FINGD2",
    "RT_THUMB2",
];

const PAYLOAD_HEADER_BYTES: usize = 40;
const ANIMATION_BLOCK: u32 = 1;
const MAX_JOINTS: usize = 72;
const MAX_MOTION: usize = 3;
const MAX_TEXTURES: usize = 5;
const COUNT_BITS: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub frames: u32,
    pub fps: f32,
    pub joints: u32,
    pub carryable_joints: u32,
    pub motion_channels: u32,
    pub texture_channels: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionKey {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

fn exp_quaternion(v: [f32; 3]) -> [f32; 4] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let k = if n < 1e-7 { 1.0 } else { n.sin() / n };
    [v[0] * k, v[1] * k, v[2] * k, n.cos()]
}

fn element_blocks<const PARTS: usize>(bits: &Bits, elements: usize) -> Vec<[VectorCtx; PARTS]> {
    let mut blocks: Vec<[VectorCtx; PARTS]> = (0..elements)
        .map(|b| std::array::from_fn(|k| VectorCtx::parse(bits, COUNT_BITS + (b * PARTS + k) * VECTOR_HEADER_BITS)))
        .collect();
    blocks.reverse();
    blocks
}

struct Layout {
    count: usize,
    header_bits: usize,
    frame_bits: usize,
}

impl Layout {
    fn segment_bytes(&self) -> usize {
        (self.header_bits + self.count * self.frame_bits).div_ceil(8)
    }

    fn frame_start(&self, frame: usize) -> Result<usize> {
        ensure!(frame < self.count, "frame {frame} out of range ({} frames)", self.count);
        Ok(self.header_bits + frame * self.frame_bits)
    }
}

struct JointStream {
    layout: Layout,
    joints: Vec<[VectorCtx; 3]>,
}

impl JointStream {
    fn parse(bits: &Bits) -> Result<Self> {
        let (count, elements) = (bits.read(0, 32) as usize, bits.read(32, 32) as usize);
        ensure!(elements <= MAX_JOINTS, "{elements} joints exceed the maximum of {MAX_JOINTS}");
        let joints = element_blocks::<3>(bits, elements);
        let frame_bits = joints.iter().flatten().map(VectorCtx::bit_count).sum();
        Ok(JointStream { layout: Layout { count, header_bits: COUNT_BITS + MAX_JOINTS * 3 * VECTOR_HEADER_BITS, frame_bits }, joints })
    }

    fn frame(&self, bits: &Bits, frame: usize) -> Result<Vec<Pose>> {
        let mut pos = self.layout.frame_start(frame)?;
        Ok(self
            .joints
            .iter()
            .map(|[p, r, s]| {
                let position = p.decode(bits, pos);
                pos += p.bit_count();
                let rotation = exp_quaternion(r.decode(bits, pos));
                pos += r.bit_count();
                let scale = s.decode(bits, pos);
                pos += s.bit_count();
                Pose { position, rotation, scale }
            })
            .collect())
    }
}

struct MotionStream {
    layout: Layout,
    items: Vec<[VectorCtx; 2]>,
}

impl MotionStream {
    fn parse(bits: &Bits) -> Result<Self> {
        let (count, elements) = (bits.read(0, 32) as usize, bits.read(32, 32) as usize);
        ensure!(elements <= MAX_MOTION, "{elements} motion channels exceed the maximum of {MAX_MOTION}");
        let items = element_blocks::<2>(bits, elements);
        let frame_bits = items.iter().flatten().map(VectorCtx::bit_count).sum();
        Ok(MotionStream { layout: Layout { count, header_bits: COUNT_BITS + MAX_MOTION * 2 * VECTOR_HEADER_BITS, frame_bits }, items })
    }

    fn frame(&self, bits: &Bits, frame: usize) -> Result<Vec<MotionKey>> {
        let mut pos = self.layout.frame_start(frame)?;
        Ok(self
            .items
            .iter()
            .map(|[p, r]| {
                let position = p.decode(bits, pos);
                pos += p.bit_count();
                let rotation = exp_quaternion(r.decode(bits, pos));
                pos += r.bit_count();
                MotionKey { position, rotation }
            })
            .collect())
    }
}

struct TextureStream {
    layout: Layout,
    items: Vec<(u32, usize)>,
}

impl TextureStream {
    fn parse(bits: &Bits) -> Result<Self> {
        let (count, elements) = (bits.read(0, 32) as usize, bits.read(32, 32) as usize);
        ensure!(elements <= MAX_TEXTURES, "{elements} texture channels exceed the maximum of {MAX_TEXTURES}");
        let mut items: Vec<(u32, usize)> = (0..elements).map(|b| (bits.read(COUNT_BITS + b * 64, 32), bits.read(COUNT_BITS + b * 64 + 32, 32) as usize)).collect();
        items.reverse();
        let frame_bits = items.iter().map(|(_, w)| w).sum();
        Ok(TextureStream { layout: Layout { count, header_bits: COUNT_BITS + MAX_TEXTURES * 64, frame_bits }, items })
    }

    fn frame(&self, bits: &Bits, frame: usize) -> Result<Vec<u32>> {
        let mut pos = self.layout.frame_start(frame)?;
        Ok(self
            .items
            .iter()
            .map(|(min, width)| {
                let v = bits.read(pos, *width).wrapping_add(*min);
                pos += width;
                v
            })
            .collect())
    }
}

pub struct Animation {
    pub header: Header,
    pub extra_bytes: usize,
    payload: Vec<u8>,
    ranges: [std::ops::Range<usize>; 4],
    pose: JointStream,
    motion: MotionStream,
    textures: TextureStream,
}

fn le32(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(d.get(o..o + 4).ok_or_else(|| anyhow::anyhow!("truncated at {o:#x}"))?.try_into()?))
}

impl Animation {
    fn at(data: &[u8], k: usize) -> Result<Animation> {
        ensure!(k + PAYLOAD_HEADER_BYTES <= data.len(), "no room for a payload header at {k:#x}");
        let w: Vec<u32> = (0..10).map(|i| le32(data, k + 4 * i)).collect::<Result<_>>()?;
        let fps = f32::from_bits(w[1]);
        let (pose_end, motion_end, second_end, size) = (w[6] as usize, w[7] as usize, w[8] as usize, w[9] as usize);
        ensure!(w[0] > 0 && fps.is_finite() && fps > 0.0, "implausible frame count or rate");
        ensure!(0 < pose_end && pose_end <= second_end && second_end <= motion_end && motion_end <= size, "segment offsets are not ascending");
        let base = k + PAYLOAD_HEADER_BYTES;
        ensure!(base + size <= data.len(), "payload runs past the end of the block");
        let payload = data[base..base + size].to_vec();
        let ranges = [0..pose_end, pose_end..second_end, second_end..motion_end, motion_end..size];
        let seg = |i: usize| Bits(&payload[ranges[i].clone()]);
        let pose = JointStream::parse(&seg(0))?;
        let carryable = JointStream::parse(&seg(1))?;
        let motion = MotionStream::parse(&seg(2))?;
        let textures = TextureStream::parse(&seg(3))?;
        ensure!(pose.layout.count == w[0] as usize, "pose stream has {} frames, header says {}", pose.layout.count, w[0]);
        let sized = [(&pose.layout, 0), (&carryable.layout, 1), (&motion.layout, 2), (&textures.layout, 3)];
        for (layout, i) in sized {
            let want = layout.segment_bytes();
            let have = ranges[i].len();
            ensure!(if i == 3 { have >= want } else { have == want }, "stream {i} is {have} bytes, layout needs {want}");
        }
        let extra_bytes = ranges[3].len() - textures.layout.segment_bytes();
        let header = Header { frames: w[0], fps, joints: pose.joints.len() as u32, carryable_joints: carryable.joints.len() as u32, motion_channels: motion.items.len() as u32, texture_channels: textures.items.len() as u32 };
        Ok(Animation { header, extra_bytes, payload, ranges, pose, motion, textures })
    }

    fn bits(&self, i: usize) -> Bits<'_> {
        Bits(&self.payload[self.ranges[i].clone()])
    }

    pub fn duration(&self) -> f32 {
        self.header.frames as f32 / self.header.fps
    }

    pub fn pose(&self, frame: usize) -> Result<Vec<Pose>> {
        self.pose.frame(&self.bits(0), frame)
    }

    pub fn motion(&self, frame: usize) -> Result<Vec<MotionKey>> {
        self.motion.frame(&self.bits(2), frame)
    }

    pub fn textures(&self, frame: usize) -> Result<Vec<u32>> {
        self.textures.frame(&self.bits(3), frame)
    }

    pub fn animated_joints(&self) -> Vec<(usize, [usize; 3])> {
        self.pose
            .joints
            .iter()
            .enumerate()
            .filter_map(|(i, [p, r, s])| ((p.bit_count() + r.bit_count() + s.bit_count()) > 0).then(|| (i, [p.bit_count(), r.bit_count(), s.bit_count()])))
            .collect()
    }

    pub fn texture_ranges(&self) -> Vec<(u32, u32)> {
        self.textures.items.iter().map(|(min, width)| (*min, min.wrapping_add(((1u64 << width) - 1) as u32))).collect()
    }
}

pub fn parse_block(block: &[u8]) -> Result<Animation> {
    Animation::at(block, 0)
}

pub fn parse(data: &[u8]) -> Result<Animation> {
    let strb = crate::strb::parse(data)?;
    let block = strb.block(data, ANIMATION_BLOCK).ok_or_else(|| anyhow::anyhow!("no animation block"))?;
    parse_block(block)
}
