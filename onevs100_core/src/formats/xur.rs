use anyhow::{bail, ensure, Result};

const HEADER: usize = 0x14;
const COUNT_BLOCK: usize = 40;
const SECTION_ENTRY: usize = 12;
const COUNTS_FLAG: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub tag: String,
    pub offset: usize,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xur {
    pub version: u32,
    pub flags: u32,
    pub xui_version: u16,
    pub size: u32,
    pub counts: Vec<u32>,
    pub sections: Vec<Section>,
    pub strings: Vec<String>,
}

fn be32(d: &[u8], o: usize) -> Result<u32> {
    ensure!(o + 4 <= d.len(), "truncated at {o:#x}");
    Ok(u32::from_be_bytes(d[o..o + 4].try_into()?))
}

fn be16(d: &[u8], o: usize) -> Result<u16> {
    ensure!(o + 2 <= d.len(), "truncated at {o:#x}");
    Ok(u16::from_be_bytes(d[o..o + 2].try_into()?))
}

fn strings(block: &[u8]) -> Result<Vec<String>> {
    let (mut out, mut at) = (Vec::new(), 0);
    while at < block.len() {
        let units = be16(block, at)? as usize;
        let raw = block.get(at + 2..at + 2 + 2 * units).ok_or_else(|| anyhow::anyhow!("string table does not end on a boundary"))?;
        let utf16: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        out.push(String::from_utf16_lossy(&utf16));
        at += 2 + 2 * units;
    }
    Ok(out)
}

pub fn parse(d: &[u8]) -> Result<Xur> {
    ensure!(d.len() >= HEADER && &d[..4] == b"XUIB", "not an XUR file");
    let (version, flags, size) = (be32(d, 4)?, be32(d, 8)?, be32(d, 14)?);
    ensure!(size as usize == d.len(), "header says {size} bytes, file is {}", d.len());
    let count = be16(d, 18)? as usize;
    let has_counts = flags & COUNTS_FLAG != 0;
    let counts = if has_counts { (0..COUNT_BLOCK / 4).map(|i| be32(d, HEADER + 4 * i)).collect::<Result<_>>()? } else { Vec::new() };
    let table = HEADER + if has_counts { COUNT_BLOCK } else { 0 };
    let sections: Vec<Section> = (0..count)
        .map(|i| {
            let at = table + i * SECTION_ENTRY;
            let tag = String::from_utf8_lossy(d.get(at..at + 4).ok_or_else(|| anyhow::anyhow!("truncated section table"))?).into_owned();
            Ok(Section { tag, offset: be32(d, at + 4)? as usize, size: be32(d, at + 8)? as usize })
        })
        .collect::<Result<_>>()?;
    for s in &sections {
        ensure!(s.offset + s.size <= d.len(), "section {} outside file", s.tag);
    }
    let strn = sections.iter().find(|s| s.tag == "STRN").ok_or_else(|| anyhow::anyhow!("no STRN section"))?;
    Ok(Xur { version, flags, xui_version: be16(d, 12)?, size, counts, strings: strings(&d[strn.offset..strn.offset + strn.size])?, sections })
}

#[derive(Debug)]
pub struct Prop {
    pub name: &'static str,
    pub kind: u8,
    pub flags: u32,
    pub nested: Option<&'static [Prop]>,
}

#[derive(Debug)]
pub struct Class {
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub props: &'static [Prop],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Uint(u32),
    Float(f32),
    Text(String),
    Color(u32),
    Vector([f32; 3]),
    Quat([f32; 4]),
    Custom(u32),
    Compound(Vec<Property>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: &'static str,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub class: &'static str,
    pub props: Vec<Property>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedFrame {
    pub name: String,
    pub frame: u32,
    pub command: u8,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub frame: u32,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub target: String,
    pub paths: Vec<String>,
    pub keys: Vec<Key>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pub frames: Vec<NamedFrame>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub class: String,
    pub groups: Vec<Group>,
    pub children: Vec<Object>,
    pub timeline: Option<Timeline>,
}

#[derive(Debug)]
pub struct Scene {
    pub header: Xur,
    pub root: Object,
}

const FLAG_PROPERTIES: u8 = 1;
const FLAG_CHILDREN: u8 = 2;
const FLAG_TIMELINE: u8 = 4;
const ARRAY_FLAG: u32 = 1;
const VECTOR_BYTES: usize = 12;
const QUAT_BYTES: usize = 16;
const INDEX_MASK: u32 = 0x00FF_FFFF;
const KEY_HEADER_BYTES: usize = 4;
const PATH_EXTRA_FLAG: u8 = 0x80;
const TYPE_COMPOUND: u8 = 9;
const TYPE_CUSTOM: u8 = 10;

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
    end: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        ensure!(self.at + n <= self.end, "read past the end of DATA at {:#x}", self.at);
        let out = &self.data[self.at..self.at + n];
        self.at += n;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into()?))
    }
}

struct Context<'a> {
    strings: &'a [String],
    vectors: &'a [u8],
    quats: &'a [u8],
}

fn class(name: &str) -> Result<&'static Class> {
    super::xur_schema::CLASSES.iter().find(|c| c.name == name).ok_or_else(|| anyhow::anyhow!("unknown class {name}"))
}

fn lineage(name: &str) -> Result<Vec<&'static Class>> {
    let mut out = vec![class(name)?];
    while let Some(parent) = out.last().and_then(|c| c.parent) {
        out.push(class(parent)?);
    }
    out.reverse();
    Ok(out)
}

fn value_size(kind: u8) -> usize {
    match kind {
        1 => 1,
        5 => 2,
        _ => 4,
    }
}

impl Context<'_> {
    fn string(&self, index: u16) -> Result<String> {
        match index {
            0 => Ok(String::new()),
            i => self.strings.get(usize::from(i) - 1).cloned().ok_or_else(|| anyhow::anyhow!("string index {i} out of range")),
        }
    }

    fn floats<const N: usize>(&self, table: &[u8], index: u32, width: usize) -> Option<[f32; N]> {
        let at = (index & INDEX_MASK) as usize * width;
        let raw = table.get(at..at + width)?;
        Some(std::array::from_fn(|i| f32::from_be_bytes([raw[4 * i], raw[4 * i + 1], raw[4 * i + 2], raw[4 * i + 3]])))
    }

    fn scalar(&self, kind: u8, raw: &[u8]) -> Result<Value> {
        let word = || u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
        Ok(match kind {
            1 => Value::Bool(raw[0] != 0),
            2 => Value::Int(word() as i32),
            3 => Value::Uint(word()),
            4 => Value::Float(f32::from_bits(word())),
            5 => Value::Text(self.string(u16::from_be_bytes([raw[0], raw[1]]))?),
            6 => Value::Color(word()),
            7 => self.floats(self.vectors, word(), VECTOR_BYTES).map_or(Value::Uint(word()), Value::Vector),
            8 => self.floats(self.quats, word(), QUAT_BYTES).map_or(Value::Uint(word()), Value::Quat),
            other => bail!("unsupported property type {other}"),
        })
    }

    fn value(&self, cur: &mut Cursor, prop: &Prop) -> Result<Value> {
        match (prop.kind, prop.nested) {
            (TYPE_COMPOUND, Some(nested)) => {
                cur.u16()?;
                Ok(Value::Compound(self.group(cur, nested)?))
            }
            (TYPE_CUSTOM, _) => Ok(Value::Custom(cur.u32()?)),
            (kind, _) => self.scalar(kind, cur.take(value_size(kind))?),
        }
    }

    fn group(&self, cur: &mut Cursor, props: &'static [Prop]) -> Result<Vec<Property>> {
        let mask_bytes = usize::from(cur.u8()? & 7);
        let raw = cur.take(mask_bytes)?;
        let mask = if matches!(mask_bytes, 1 | 2 | 4) { raw.iter().fold(0u32, |m, &b| (m << 8) | u32::from(b)) } else { 0 };
        let mut out = Vec::new();
        for prop in props.iter().enumerate().filter(|(i, _)| mask >> i & 1 == 1).map(|(_, p)| p) {
            let count = if prop.flags & ARRAY_FLAG != 0 { self.array_len(cur)? } else { 1 };
            let values = (0..count).map(|_| self.value(cur, prop)).collect::<Result<Vec<_>>>()?;
            out.push(Property { name: prop.name, values });
        }
        Ok(out)
    }

    fn array_len(&self, cur: &mut Cursor) -> Result<usize> {
        let b = cur.u8()?;
        if b & 0x80 == 0 {
            return Ok(usize::from(b));
        }
        Ok(cur.take(usize::from(b & 0x7f))?.iter().fold(0usize, |n, &x| (n << 8) | usize::from(x)))
    }

    fn object(&self, cur: &mut Cursor) -> Result<Object> {
        let index = cur.u16()?;
        let flags = cur.u8()?;
        let name = self.string(index)?;
        let mut obj = Object { class: name.clone(), groups: Vec::new(), children: Vec::new(), timeline: None };
        if flags & FLAG_PROPERTIES != 0 {
            let declared = usize::from(cur.u16()?);
            let mut total = 0;
            for c in lineage(&name)? {
                let props = self.group(cur, c.props)?;
                total += props.len();
                obj.groups.push(Group { class: c.name, props });
            }
            ensure!(total == declared, "{name}: decoded {total} values, header says {declared}");
        }
        if flags & FLAG_CHILDREN != 0 {
            let n = cur.u32()?;
            obj.children = (0..n).map(|_| self.object(cur)).collect::<Result<_>>()?;
        }
        if flags & FLAG_TIMELINE != 0 {
            obj.timeline = Some(self.timeline(cur, &obj)?);
        }
        Ok(obj)
    }

    fn child_classes(&self, obj: &Object) -> Vec<(String, String)> {
        obj.children
            .iter()
            .filter_map(|c| {
                let id = c.groups.iter().flat_map(|g| &g.props).find(|p| p.name == "Id")?;
                match id.values.first()? {
                    Value::Text(t) => Some((t.clone(), c.class.clone())),
                    _ => None,
                }
            })
            .collect()
    }

    fn path(&self, cur: &mut Cursor, class_name: &str) -> Result<(Vec<String>, u8)> {
        let head = cur.u8()?;
        let depth = usize::from(head & 0x7f);
        let (mut names, mut kind) = (Vec::new(), 0);
        if depth > 0 {
            let (up, index) = (usize::from(cur.u8()?), usize::from(cur.u8()?));
            let mut c = class(class_name)?;
            for _ in 0..up {
                c = class(c.parent.ok_or_else(|| anyhow::anyhow!("path climbs past the root class"))?)?;
            }
            let mut prop = c.props.get(index).ok_or_else(|| anyhow::anyhow!("no property {index} in {}", c.name))?;
            names.push(format!("{}.{}", c.name, prop.name));
            for _ in 1..depth {
                let nested = prop.nested.ok_or_else(|| anyhow::anyhow!("{} has no nested properties", prop.name))?;
                prop = nested.get(usize::from(cur.u8()?)).ok_or_else(|| anyhow::anyhow!("nested index out of range"))?;
                names.push(prop.name.to_owned());
            }
            kind = prop.kind;
        }
        if head & PATH_EXTRA_FLAG != 0 {
            cur.u32()?;
        }
        Ok((names, kind))
    }

    fn timeline(&self, cur: &mut Cursor, owner: &Object) -> Result<Timeline> {
        let frames = (0..cur.u32()?)
            .map(|_| Ok(NamedFrame { name: self.string(cur.u16()?)?, frame: cur.u32()?, command: cur.u8()?, target: self.string(cur.u16()?)? }))
            .collect::<Result<Vec<_>>>()?;
        let mut tracks = Vec::new();
        if !owner.children.is_empty() {
            let classes = self.child_classes(owner);
            for _ in 0..cur.u32()? {
                let target = self.string(cur.u16()?)?;
                let class_name = classes.iter().find(|(id, _)| *id == target).map(|(_, c)| c.clone()).ok_or_else(|| anyhow::anyhow!("timeline target {target} is not a child"))?;
                let paths: Vec<(Vec<String>, u8)> = (0..cur.u32()?).map(|_| self.path(cur, &class_name)).collect::<Result<_>>()?;
                let mut keys = Vec::new();
                for _ in 0..cur.u32()? {
                    let frame = cur.u32()?;
                    cur.take(KEY_HEADER_BYTES)?;
                    let values = paths
                        .iter()
                        .filter(|(_, kind)| !matches!(*kind, TYPE_COMPOUND | TYPE_CUSTOM))
                        .map(|(_, kind)| self.scalar(*kind, cur.take(value_size(*kind))?))
                        .collect::<Result<_>>()?;
                    keys.push(Key { frame, values });
                }
                tracks.push(Track { target, paths: paths.into_iter().map(|(n, _)| n.join(".")).collect(), keys });
            }
        }
        Ok(Timeline { frames, tracks })
    }
}

pub fn parse_scene(d: &[u8]) -> Result<Scene> {
    let header = parse(d)?;
    let section = |tag: &str| header.sections.iter().find(|s| s.tag == tag).map_or(&d[..0], |s| &d[s.offset..s.offset + s.size]);
    let data = header.sections.iter().find(|s| s.tag == "DATA").ok_or_else(|| anyhow::anyhow!("no DATA section"))?;
    let ctx = Context { strings: &header.strings, vectors: section("VECT"), quats: section("QUAT") };
    let mut cur = Cursor { data: d, at: data.offset, end: data.offset + data.size };
    let root = ctx.object(&mut cur)?;
    ensure!(cur.at == cur.end, "{} bytes of DATA left unread", cur.end - cur.at);
    Ok(Scene { header, root })
}

fn show(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Uint(u) => u.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(t) => format!("{t:?}"),
        Value::Color(c) => format!("#{c:08X}"),
        Value::Vector(v) => format!("({}, {}, {})", v[0], v[1], v[2]),
        Value::Quat(q) => format!("({}, {}, {}, {})", q[0], q[1], q[2], q[3]),
        Value::Custom(o) => format!("custom@{o:#x}"),
        Value::Compound(props) => format!("{{ {} }}", props.iter().map(property_text).collect::<Vec<_>>().join(", ")),
    }
}

fn property_text(p: &Property) -> String {
    let values: Vec<String> = p.values.iter().map(show).collect();
    format!("{} = {}", p.name, if values.len() == 1 { values[0].clone() } else { format!("[{}]", values.join(", ")) })
}

fn render_object(o: &Object, depth: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    let id = o.groups.iter().flat_map(|g| &g.props).find(|p| p.name == "Id").and_then(|p| p.values.first());
    out.push(match id {
        Some(Value::Text(t)) if !t.is_empty() => format!("{pad}{} {:?}", o.class, t),
        _ => format!("{pad}{}", o.class),
    });
    for p in o.groups.iter().flat_map(|g| &g.props).filter(|p| p.name != "Id") {
        out.push(format!("{pad}  {}", property_text(p)));
    }
    if let Some(t) = &o.timeline {
        out.extend(t.frames.iter().map(|f| format!("{pad}  named frame {} {}{}", f.frame, f.name, if f.target.is_empty() { String::new() } else { format!(" -> {}", f.target) })));
        for track in &t.tracks {
            let keys: Vec<String> = track.keys.iter().map(|k| format!("{}:{}", k.frame, k.values.iter().map(show).collect::<Vec<_>>().join("/"))).collect();
            out.push(format!("{pad}  animate {} [{}] keys {}", track.target, track.paths.join(", "), keys.join(" ")));
        }
    }
    o.children.iter().for_each(|c| render_object(c, depth + 1, out));
}

pub fn render(scene: &Scene) -> Vec<String> {
    let mut out = Vec::new();
    render_object(&scene.root, 0, &mut out);
    out
}
