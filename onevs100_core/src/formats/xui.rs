use anyhow::{bail, ensure, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XuizEntry {
    pub name: String,
    pub data: Vec<u8>,
}

fn be32(d: &[u8], o: usize) -> Result<u32> {
    ensure!(o + 4 <= d.len(), "truncated at {o:#x}");
    Ok(u32::from_be_bytes(d[o..o + 4].try_into().unwrap()))
}

fn be16(d: &[u8], o: usize) -> Result<u16> {
    ensure!(o + 2 <= d.len(), "truncated at {o:#x}");
    Ok(u16::from_be_bytes(d[o..o + 2].try_into().unwrap()))
}

fn utf16be(b: &[u8]) -> Result<String> {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    Ok(String::from_utf16(&units)?)
}

fn to_utf16be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

pub fn read_xuiz(d: &[u8]) -> Result<Vec<XuizEntry>> {
    ensure!(d.len() >= 22 && &d[..4] == b"XUIZ", "not a XUIZ archive");
    let total = be32(d, 8)? as usize;
    ensure!(total == d.len(), "header says {total} bytes, file is {}", d.len());
    let dir_size = be32(d, 16)? as usize;
    let count = be16(d, 20)? as usize;
    let data0 = 22 + dir_size;
    let mut out = Vec::with_capacity(count);
    let mut off = 22;
    for _ in 0..count {
        let len = be32(d, off)? as usize;
        let doff = be32(d, off + 4)? as usize;
        ensure!(off + 9 <= d.len(), "truncated directory");
        let nlen = d[off + 8] as usize;
        let name_end = off + 9 + nlen * 2;
        ensure!(name_end <= d.len(), "truncated entry name");
        let name = utf16be(&d[off + 9..name_end])?;
        ensure!(data0 + doff + len <= d.len(), "entry {name} out of range");
        out.push(XuizEntry { name, data: d[data0 + doff..data0 + doff + len].to_vec() });
        off = name_end;
    }
    ensure!(off == data0, "directory is {} bytes, header says {dir_size}", off - 22);
    Ok(out)
}

pub fn write_xuiz(entries: &[XuizEntry]) -> Result<Vec<u8>> {
    if entries.len() > 0xFFFF {
        bail!("too many entries for the u16 count field");
    }
    let (mut dir, mut payload) = (Vec::new(), Vec::new());
    for e in entries {
        let raw = to_utf16be(&e.name);
        ensure!(raw.len() <= 255, "name too long for the u8 length field: {}", e.name);
        dir.extend_from_slice(&(e.data.len() as u32).to_be_bytes());
        dir.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        dir.push((raw.len() / 2) as u8);
        dir.extend_from_slice(&raw);
        payload.extend_from_slice(&e.data);
    }
    let total = 22 + dir.len() + payload.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"XUIZ");
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&(total as u32).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&(dir.len() as u32).to_be_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_be_bytes());
    out.extend_from_slice(&dir);
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn read_xus(d: &[u8]) -> Result<Vec<String>> {
    ensure!(d.len() >= 12 && &d[..4] == b"XUIS", "not a XUIS string table");
    let version = u32::from_le_bytes(d[4..8].try_into().unwrap());
    ensure!(version == 1, "unsupported XUIS version {version}");
    let size = be16(d, 8)? as usize;
    ensure!(size == d.len(), "XUIS header says {size} bytes, buffer is {}", d.len());
    let (mut out, mut off) = (Vec::new(), 12);
    while off < d.len() {
        let n = be16(d, off)? as usize;
        off += 2;
        ensure!(off + n * 2 <= d.len(), "XUIS string table does not end on a boundary");
        out.push(utf16be(&d[off..off + n * 2])?);
        off += n * 2;
    }
    Ok(out)
}

pub fn write_xus(strings: &[String]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for s in strings {
        let units = s.encode_utf16().count();
        ensure!(units <= 0xFFFF, "string too long");
        body.extend_from_slice(&(units as u16).to_be_bytes());
        body.extend_from_slice(&to_utf16be(s));
    }
    let total = 12 + body.len();
    ensure!(total <= 0xFFFF, "XUIS exceeds the u16 size field");
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"XUIS");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(total as u16).to_be_bytes());
    out.extend_from_slice(&((strings.len() / 2) as u16).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn unpack_dir(entries: &[XuizEntry], dir: &std::path::Path) -> Result<()> {
    for e in entries {
        let path = dir.join(e.name.replace('\\', "/"));
        std::fs::create_dir_all(path.parent().unwrap_or(dir))?;
        std::fs::write(path, &e.data)?;
    }
    Ok(())
}

pub fn pack_dir(dir: &std::path::Path) -> Result<Vec<XuizEntry>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for item in std::fs::read_dir(&d)? {
            let path = item?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir)?.to_string_lossy().replace('/', "\\");
                out.push(XuizEntry { name: rel, data: std::fs::read(&path)? });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
