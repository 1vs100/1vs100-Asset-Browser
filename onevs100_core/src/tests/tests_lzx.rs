use crate::{lzx, strb, ytgr};
use std::path::PathBuf;

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../testdata/avatar/avataritems_raw"
);

fn model_block(group: &str, name: &str) -> Vec<u8> {
    let d = std::fs::read(
        PathBuf::from(CORPUS)
            .join(group)
            .join(name)
            .join("asset_v2.bin"),
    )
    .unwrap();
    let y = ytgr::parse(&d).unwrap();
    y.block_data(&d, 3).unwrap().to_vec()
}

#[test]
fn model_blocks_decompress_to_the_size_their_headers_promise() {
    let b = model_block("#Trending", "Grumpy Cat Slippers");
    assert_eq!(lzx::decompress(&b).unwrap().len(), 32768 + 4085);
    let b = model_block("#Trending", "BMX Fail");
    assert_eq!(lzx::decompress(&b).unwrap().len(), 32768 + 8742);
}

#[test]
fn block_chain_is_listed_with_start_and_size() {
    let b = model_block("#Trending", "Grumpy Cat Slippers");
    assert_eq!(
        lzx::chain(&b).unwrap(),
        vec![(26810, 0, 32768), (3258, 32768, 4085)]
    );
}

#[test]
fn corrupt_or_inconsistent_streams_are_errors() {
    let mut b = model_block("#Trending", "Grumpy Cat Slippers");
    assert!(lzx::decompress(&b[..b.len() - 5]).is_err());
    b[16 + 4..16 + 8].copy_from_slice(&7u32.to_le_bytes());
    assert!(lzx::decompress(&b).is_err());
    assert!(lzx::decompress(&[]).is_err());
}

#[test]
fn a_corpus_sample_decompresses_without_errors() {
    let groups = std::fs::read_dir(CORPUS).unwrap();
    let mut checked = 0;
    for g in groups.flatten().take(30) {
        for it in std::fs::read_dir(g.path()).unwrap().flatten().take(5) {
            let Ok(d) = std::fs::read(it.path().join("asset_v2.bin")) else {
                continue;
            };
            let y = ytgr::parse(&d).unwrap();
            let Ok(block) = y.block_data(&d, 3) else {
                continue;
            };
            let out =
                lzx::decompress(block).unwrap_or_else(|e| panic!("{}: {e}", it.path().display()));
            assert!(out.len() >= 48, "out.len: {}", it.path().display());
            checked += 1;
        }
    }
    assert!(checked >= 20, "checked: {checked}");
    let _ = strb::parse;
}

#[test]
fn short_trailing_bytes_after_the_last_block_are_ignored() {
    let mut b = model_block("#Trending", "Grumpy Cat Slippers");
    let want = lzx::decompress(&b).unwrap();
    b.extend_from_slice(&[0x28, 0xa5]);
    assert_eq!(lzx::decompress(&b).unwrap(), want);
    b.extend_from_slice(&[0x5a; 40]);
    assert_eq!(lzx::decompress(&b).unwrap(), want);
    b.extend_from_slice(&[0x5a; 100]);
    assert!(lzx::decompress(&b).is_err());
}
