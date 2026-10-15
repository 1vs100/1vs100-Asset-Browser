use crate::browser::Browser;
use crate::{anim, inspect, source, strb, ytgr};
use std::path::PathBuf;

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../testdata/avatar/avataritems_raw"
);

fn item(group: &str, name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(CORPUS)
            .join(group)
            .join(name)
            .join("asset_v2.bin"),
    )
    .unwrap()
}

fn slippers() -> Vec<u8> {
    item("#Trending", "Grumpy Cat Slippers")
}

fn bmx() -> Vec<u8> {
    item("#Trending", "BMX Fail")
}

fn builds() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common")
}

#[test]
fn strb_block_walk_explains_the_standalone_animation_files() {
    let sponsor = crate::tests_fixtures::package_file(
        "episodes",
        "83136FF343124795DC1806A2DB044476589BD5C0",
        "media/sponsor/of_lu_sp.fbx.msliveavatarasset",
    );
    let s = strb::parse(&sponsor).unwrap();
    assert_eq!((s.version, s.big_endian), (0, true));
    assert_eq!(
        s.blocks
            .iter()
            .map(|b| (b.id, b.start, b.length))
            .collect::<Vec<_>>(),
        [(1, 35, 9858)]
    );
    let retail = std::fs::read(builds().join("v2.0.1149.0.02-2009-10-26_retail/media/avataranims/xuiavataranims/hf_ch_02.fbx.msliveavatarasset")).unwrap();
    let r = strb::parse(&retail).unwrap();
    assert_eq!(
        r.blocks
            .iter()
            .map(|b| (b.id, b.start, b.length))
            .collect::<Vec<_>>(),
        [(6, 44, 13), (1, 72, 11220)]
    );
}

#[test]
fn strb_rejects_garbage_and_truncation() {
    assert!(strb::parse(b"NOPE").is_err());
    let mut data = vec![0u8; 40];
    data[..4].copy_from_slice(b"STRB");
    assert!(strb::parse(&data).is_err());
}

#[test]
fn ytgr_wrapper_fields_match_the_python_reference() {
    let d = slippers();
    let y = ytgr::parse(&d).unwrap();
    assert_eq!(
        (y.version, y.header_size, y.payload_size),
        (1, 0x130, 32648)
    );
    assert_eq!(y.timestamp, "2014-04-03 20:31:51");
    assert_eq!(y.asset_id, "00000020-6d4c-b262-c9a5-746e58570b10");
    assert_eq!(hex(&y.digest), "c7f53c5eaab0a48de5a32150b5c508a80192862e");
    assert_eq!(y.signature.len(), 256);
    assert!(y.strb.big_endian);
    assert_eq!(
        y.strb
            .blocks
            .iter()
            .map(|b| (b.id, b.length))
            .collect::<Vec<_>>(),
        [(8, 15), (3, 30092), (7, 28), (4, 2414)]
    );
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn animation_items_expose_a_decodable_animation_block() {
    let d = bmx();
    let y = ytgr::parse(&d).unwrap();
    assert_eq!(
        y.strb
            .blocks
            .iter()
            .map(|b| (b.id, b.length))
            .collect::<Vec<_>>(),
        [(8, 15), (3, 29032), (5, 188), (7, 28), (1, 114059)]
    );
    let block = y.block_data(&d, 1).unwrap();
    let a = anim::parse_block(block).unwrap();
    assert_eq!((a.header.frames, a.header.joints), (221, 71));
    assert!((a.header.fps - 14.965_986).abs() < 1e-3);
    assert!(a.pose(0).unwrap()[19]
        .rotation
        .iter()
        .zip([0.0366, 0.00655, 0.0248, 0.999])
        .all(|(x, y)| (x - y).abs() < 2e-4));
    assert!(anim::parse(&d[0x140..]).is_ok());
}

#[test]
fn metadata_block_is_decoded() {
    let d = slippers();
    let y = ytgr::parse(&d).unwrap();
    let m = ytgr::metadata(y.block_data(&d, 8).unwrap()).unwrap();
    assert_eq!(
        (m.version, m.flags, m.asset_type, m.mask, m.id),
        (2, 2, 1, 0x20, 407)
    );
    assert_eq!(m.extra, 2);
}

#[test]
fn describe_summarises_the_asset_and_its_blocks() {
    let d = slippers();
    let det = inspect::describe("asset_v2.bin", &d);
    assert_eq!(det.kind, "Avatar item asset (YTGR)");
    for want in [
        "asset id 00000020-6d4c-b262-c9a5-746e58570b10",
        "created 2014-04-03 20:31:51 UTC",
        "block 8 metadata  15 bytes",
        "block 3 model  30092 bytes",
        "block 4 texture  2414 bytes",
        "asset type 1",
    ] {
        assert!(
            det.lines.iter().any(|l| l.contains(want)),
            "missing {want:?} in {:#?}",
            det.lines
        );
    }
}

#[test]
fn ytgr_expands_into_block_entries_and_the_animation_block_describes_itself() {
    let d = bmx();
    let kids = source::expand(&d).unwrap().unwrap();
    let names: Vec<_> = kids.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "block8.metadata",
            "block3.model",
            "block5.block5",
            "block7.colours",
            "block1.animation"
        ]
    );
    let anim_entry = kids.iter().find(|k| k.name == "block1.animation").unwrap();
    let det = inspect::describe(&anim_entry.name, &anim_entry.payload.read().unwrap());
    assert_eq!(det.kind, "Avatar animation (STRB)");
    assert!(
        det.lines
            .iter()
            .any(|l| l.contains("221 frames at 14.965986 fps")),
        "{:#?}",
        &det.lines[..4]
    );
}

#[test]
fn browser_can_enter_an_item_asset() {
    let _ = slippers();
    let mut b = Browser::new(&PathBuf::from(CORPUS).join("#Trending/Grumpy Cat Slippers")).unwrap();
    let i = b
        .entries()
        .iter()
        .position(|e| e.name == "asset_v2.bin")
        .unwrap();
    b.open(i).unwrap();
    assert_eq!(b.detail().unwrap().kind, "Avatar item asset (YTGR)");
    assert!(b.entries().iter().any(|e| e.name == "block3.model"));
}

#[test]
fn a_sample_of_the_whole_corpus_parses_exactly() {
    let groups = std::fs::read_dir(CORPUS).unwrap();
    let (mut checked, mut animations) = (0, 0);
    for group in groups.flatten().take(40) {
        for item in std::fs::read_dir(group.path()).unwrap().flatten().take(6) {
            let Ok(d) = std::fs::read(item.path().join("asset_v2.bin")) else {
                continue;
            };
            let y = ytgr::parse(&d).unwrap_or_else(|e| panic!("{}: {e}", item.path().display()));
            if let Ok(block) = y.block_data(&d, 1) {
                anim::parse_block(block)
                    .unwrap_or_else(|e| panic!("{}: {e}", item.path().display()));
                animations += 1;
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 20 && animations > 0,
        "{checked} items, {animations} animations"
    );
}

#[test]
fn truncated_item_assets_are_errors() {
    let d = slippers();
    assert!(ytgr::parse(&d[..d.len() - 10]).is_err());
    assert!(ytgr::parse(&d[..0x200]).is_err());
    assert!(ytgr::parse(b"YTGR").is_err());
}

#[test]
fn block_seven_is_a_custom_colour_table() {
    let d = slippers();
    let y = ytgr::parse(&d).unwrap();
    let t = ytgr::colour_table(y.block_data(&d, 7).unwrap()).unwrap();
    assert_eq!(t.version, 1);
    assert_eq!(
        t.entries,
        vec![(0xFFFF0000, 0), (0xFF00FF00, 0), (0xFF0000FF, 0)]
    );
    assert!(ytgr::colour_table(&[0; 27]).is_err());
}

#[test]
fn item_details_list_the_colour_table() {
    let d = slippers();
    let lines = inspect::describe("asset_v2.bin", &d).lines.join("\n");
    assert!(
        lines.contains("custom colours: 3  0xffff0000 0xff00ff00 0xff0000ff"),
        "{lines}"
    );
}
