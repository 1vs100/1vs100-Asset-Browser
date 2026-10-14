use anyhow::Result;
use stfs::types::BlockNumber;
use stfs::{BytesStfsReader, StfsFileEntry, StfsPackageReader};

const BLOCK_SIZE: usize = 0x1000;
const CONSECUTIVE_FLAG: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StfsFile {
    pub path: String,
    pub size: u64,
}

pub fn list(data: &[u8]) -> Result<Vec<StfsFile>> {
    let reader = BytesStfsReader::open(data)?;
    Ok(reader
        .package()
        .file_table
        .walk_files()
        .into_iter()
        .map(|e| StfsFile { path: e.path, size: e.entry.file_size as u64 })
        .collect())
}

pub fn extract(data: &[u8], path: &str) -> Result<Vec<u8>> {
    let reader = BytesStfsReader::open(data)?;
    let entry = reader
        .package()
        .file_table
        .walk_files()
        .into_iter()
        .find(|e| e.path == path)
        .ok_or_else(|| anyhow::anyhow!("{path} not in package"))?
        .entry;
    let mut out = Vec::new();
    if entry.flags & CONSECUTIVE_FLAG != 0 {
        read_consecutive(&reader, data, &entry, &mut out)?;
    } else {
        reader.extract_file(&mut out, &entry)?;
    }
    Ok(out)
}

fn read_consecutive(reader: &BytesStfsReader<&[u8]>, data: &[u8], entry: &StfsFileEntry, out: &mut Vec<u8>) -> Result<()> {
    let package = reader.package();
    let blocks = entry.file_size.div_ceil(BLOCK_SIZE);
    for i in 0..blocks {
        let at = package.hash_table_meta.block_to_addr(BlockNumber(entry.starting_block_num.0 + i), package.sex);
        let len = BLOCK_SIZE.min(entry.file_size - i * BLOCK_SIZE);
        out.extend_from_slice(data.get(at..at + len).ok_or_else(|| anyhow::anyhow!("block {i} outside package"))?);
    }
    Ok(())
}
