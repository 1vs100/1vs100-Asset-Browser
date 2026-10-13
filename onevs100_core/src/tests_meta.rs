use crate::browser::Browser;
use crate::inspect::{self, Detail};
use crate::{mct, timefmt, video};
use std::path::PathBuf;

fn testdata(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata").join(rel)
}

fn describe_file(rel: &str) -> Detail {
    let path = testdata(rel);
    inspect::describe(&path.file_name().unwrap().to_string_lossy(), &std::fs::read(&path).unwrap_or_else(|e| panic!("{rel}: {e}")))
}

fn has(d: &Detail, needle: &str) -> bool {
    d.lines.iter().any(|l| l.contains(needle))
}

const PROMO: &str = "xex";
const SHOW_LIVE: &str = "dlcs/shows/77205BC1241C708A8A10B7303AF90E5EDD0286B1";
const SHOW_ID: &str = "77205BC1241C708A8A10B7303AF90E5EDD0286B1";

fn show_file(rel: &str) -> Vec<u8> {
    crate::tests_fixtures::package_file("shows", SHOW_ID, rel)
}

fn describe_show(rel: &str) -> Detail {
    let data = show_file(rel);
    inspect::describe(rel.rsplit('/').next().unwrap(), &data)
}

#[test]
fn xex_details_match_xextool_reference() {
    let d = describe_file(&format!("{PROMO}/12209-08CCD74CAAC0DA6C_PT.xex"));
    assert_eq!(d.kind, "Xbox 360 executable (XEX)");
    for want in [
        "original PE name Primetime.dll",
        "load address 9ED40000",
        "entry point 9ED47000",
        "image size 71000",
        "compression Compressed",
        "encryption Encrypted",
        "version 2.0.12209.0",
        "base version 2.0.12209.0",
        "bounding path \\SAP\\online\\202FB100\\Primetime.xex",
        "LIBCMT v2.0.11569.0",
        "LINK v10.0.10224.0",
        "xboxkrnl.exe v2.0.11569.0 (min v2.0.12209.0)",
        "xam.xex v2.0.12209.0 (min v2.0.12209.0)",
        "programg",
    ] {
        assert!(has(&d, want), "missing {want:?} in {:#?}", d.lines);
    }
}

#[test]
fn xex_image_sizes_match_other_reference_files() {
    for (file, size) in [("12365-0112B3CAF7EE6F13_PT.xex", "6C000"), ("12379-011D43A46C303B02_PT.xex", "6B000")] {
        let d = describe_file(&format!("{PROMO}/{file}"));
        assert!(has(&d, &format!("image size {size}")), "{file}: {:#?}", d.lines);
    }
}

#[test]
fn game_executable_names_its_title() {
    let d = describe_file("common/v2.0.1317.0.02-2010-04-14/default.xex");
    assert_eq!(d.kind, "Xbox 360 executable (XEX)");
    assert!(has(&d, "title id 584A07E4"), "{:#?}", d.lines);
}

#[test]
fn every_xex_in_the_game_builds_is_readable() {
    let found: Vec<_> = [
        "common/v1.0.808.0_debug_unmodified/1vs100-fre.xex",
        "common/v2.0.1149.0.02-2009-10-26_retail/default.xex",
        "common/v2.0.1167.0.02-2009-12-07/default.xex",
        "common/v2.0.1317.0.02-2010-04-14/tickettracker/tickettrackeruploader.xex",
        "common/primetime-beta/Default.xex",
    ]
    .iter()
    .map(|r| (r, describe_file(r)))
    .collect();
    for (rel, d) in found {
        assert_eq!(d.kind, "Xbox 360 executable (XEX)", "{rel}: {:?}", d.lines.first());
        assert!(has(&d, "load address"), "{rel}");
    }
}

#[test]
fn xcontent_details_list_header_metadata() {
    let d = describe_file(SHOW_LIVE);
    assert_eq!(d.kind, "Xbox LIVE package");
    for want in ["signature LIVE", "content type MarketPlaceContent", "title id 584A07DC", "display name 1 vs 100 Content", "media id", "console id", "profile id", "content size", "files"] {
        assert!(has(&d, want), "missing {want:?} in {:#?}", d.lines);
    }
}

#[test]
fn xcontent_previews_its_thumbnail_image() {
    let d = describe_file(SHOW_LIVE);
    let img = d.image.clone().expect("package thumbnail");
    assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
    assert!(has(&d, "thumbnail") && has(&d, "title image"));
}

#[test]
fn wmv_details_report_streams_like_ffprobe() {
    let d = describe_show("media/movies/titleintroaudio.wmv");
    assert!(has(&d, "video WMV3 1280x720"), "{:#?}", d.lines);
    assert!(has(&d, "audio 2 ch 44100 Hz"), "{:#?}", d.lines);
    let promo = describe_show("media/ads/promo_1.wmv");
    assert!(has(&promo, "video WMV3 424x240"), "{:#?}", promo.lines);
}

#[test]
fn wmv_previews_a_frame_at_the_video_size() {
    if !video::decoder_available() {
        return;
    }
    let d = describe_show("media/movies/titleintroaudio.wmv");
    let img = d.image.expect("video frame");
    assert_eq!((img.width, img.height), (1280, 720));
    assert!(img.rgba.chunks(4).any(|p| p[0] != img.rgba[0]), "frame is a single flat colour");
}

#[test]
fn seeking_a_video_changes_the_frame() {
    if !video::decoder_available() {
        return;
    }
    let root = testdata("dlcs/shows");
    let mut b = Browser::new(&root).unwrap();
    let package = b.entries().iter().position(|e| e.name == SHOW_ID).unwrap();
    b.open(package).unwrap();
    let i = b.entries().iter().position(|e| e.name.ends_with("promo_1.wmv")).unwrap();
    b.open(i).unwrap();
    assert!(b.video_seconds().unwrap() > 25.0);
    let start = b.detail().unwrap().image.clone().unwrap();
    b.seek_video(0.9).unwrap();
    assert_ne!(b.detail().unwrap().image.clone().unwrap().rgba, start.rgba);
}

#[test]
fn text_files_expose_full_content_and_syntax() {
    let lua = describe_show("luaengine/gameservice.lua");
    let body = lua.text.expect("lua body");
    assert_eq!(body.syntax, "lua");
    assert_eq!(body.content, String::from_utf8_lossy(&show_file("luaengine/gameservice.lua")));
    let ext = describe_show("show.ext").text.unwrap();
    assert_eq!(ext.syntax, "txt");
    assert!(ext.content.contains("This is the show package file"));
}

#[test]
fn xml_flavoured_text_is_highlighted_as_xml_without_bom() {
    let resx_data = crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "media/strings/epsetscreens.resx");
    let resx = inspect::describe("a.resx", &resx_data);
    let body = resx.text.unwrap();
    assert_eq!(body.syntax, "xml");
    assert!(!body.content.starts_with('\u{feff}'));
    let updb_data = crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "media/avatars/avatarbase.updb");
    let updb = inspect::describe("a.updb", &updb_data);
    assert_eq!(updb.text.unwrap().syntax, "xml");
}

#[test]
fn non_text_files_have_no_text_body() {
    assert!(describe_show("media/prizes/prize_arcade1.jpg").text.is_none());
}

#[test]
fn frame_extraction_works_with_any_ffmpeg_decodable_clip() {
    if !video::ffmpeg_available() {
        return;
    }
    let clip = std::env::temp_dir().join(format!("onevs100-clip-{}.avi", std::process::id()));
    let made = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=64x48:rate=10:duration=2", "-c:v", "mpeg4"])
        .arg(&clip)
        .status()
        .unwrap();
    assert!(made.success());
    let png = video::frame_png(&std::fs::read(&clip).unwrap(), 1.0).unwrap();
    std::fs::remove_file(&clip).ok();
    assert_eq!(&png[..4], b"\x89PNG");
    assert!(video::frame_png(b"not a video", 0.0).is_err());
}

#[test]
fn time_formatting_handles_unix_and_fat_stamps() {
    assert_eq!(timefmt::unix(0), "1970-01-01 00:00:00");
    assert_eq!(timefmt::unix(0x4C11DF5B), "2010-06-11 07:01:47");
    assert_eq!(timefmt::fat(0x2d6f0020), "2002-11-15 00:01:00");
    assert_eq!(timefmt::fat(0x4d3a_bf7d), "2018-09-26 23:59:58");
}

#[test]
fn xex_details_include_build_timestamp_and_checksum() {
    let d = describe_file(&format!("{PROMO}/12209-08CCD74CAAC0DA6C_PT.xex"));
    assert!(has(&d, "checksum 00043568"), "{:#?}", d.lines);
    assert!(has(&d, "timestamp 2010-06-11 07:01:47 UTC (4C11DF5B)"), "{:#?}", d.lines);
}

#[test]
fn xcontent_details_include_file_stamps() {
    let d = describe_file(SHOW_LIVE);
    assert!(has(&d, "files created 2002-11-15 00:01:00"), "{:#?}", d.lines);
    assert!(has(&d, "files accessed 2002-11-15 00:01:00"), "{:#?}", d.lines);
}

fn launch_children() -> Vec<crate::source::Entry> {
    let live = crate::source::Entry {
        name: "launch.xzp".into(),
        is_dir: false,
        payload: crate::source::Payload::Disk(testdata("common/v1.0.808.0_debug_unmodified/media/launch.xzp")),
    };
    crate::source::children(&live).unwrap().unwrap()
}

#[test]
fn xur_scene_lists_header_sections_and_strings() {
    let kids = launch_children();
    let bug = kids.iter().find(|k| k.name == "Bug.xur").unwrap();
    let d = inspect::describe(&bug.name, &bug.payload.read().unwrap());
    assert_eq!(d.kind, "XUI scene (XUR)");
    for want in ["version 5", "71 strings", "STRN", "VECT", "CUST", "DATA", "scene tree", "XuiCanvas", "XuiScene \"Bug_scene\"", "Width = 1280", "XuiFigure"] {
        assert!(has(&d, want), "missing {want:?} in {:#?}", &d.lines[..d.lines.len().min(12)]);
    }
}

#[test]
fn every_xur_in_the_game_parses() {
    let kids = launch_children();
    let scenes: Vec<_> = kids.iter().filter(|k| k.name.ends_with(".xur")).collect();
    assert!(scenes.len() > 20);
    for k in scenes {
        let d = inspect::describe(&k.name, &k.payload.read().unwrap());
        assert_eq!(d.kind, "XUI scene (XUR)", "{}: {:?}", k.name, d.lines.first());
    }
}

#[test]
fn every_xus_in_the_game_lists_its_strings() {
    let kids = launch_children();
    let tables: Vec<_> = kids.iter().filter(|k| k.name.ends_with(".xus")).collect();
    assert!(!tables.is_empty());
    for k in tables {
        let d = inspect::describe(&k.name, &k.payload.read().unwrap());
        assert_eq!(d.kind, "XUI string table", "{}", k.name);
        assert!(d.lines[0].ends_with("strings"), "{}", k.name);
    }
}

fn first_mct(name_part: &str) -> Vec<u8> {
    let live = crate::source::Entry {
        name: "episode.xzp".into(),
        is_dir: false,
        payload: crate::source::Payload::Bytes(crate::tests_fixtures::package_file(
            "episodes",
            "83136FF343124795DC1806A2DB044476589BD5C0",
            "media/episode.xzp",
        )),
    };
    let kids = crate::source::children(&live).unwrap().unwrap();
    kids.iter().find(|k| k.name.contains(name_part)).unwrap().payload.read().unwrap()
}

#[test]
fn mct_files_are_recognised_without_decoding() {
    let data = first_mct("answer_glow.png");
    assert_eq!(&data[..4], b".mct");
    let d = inspect::describe("answer_glow.png", &data);
    assert_eq!(d.kind, "MCT texture");
    assert!(d.pending_preview && d.image.is_none());
    assert!(has(&d, "stored under a .png name"), "{:#?}", d.lines);
}

#[test]
fn real_png_has_no_pending_preview() {
    assert!(!describe_show("media/prizes/prize_arcade1.jpg").pending_preview);
}

#[test]
fn mct_decodes_through_the_research_helper() {
    if !mct::decoder_available() {
        return;
    }
    let data = first_mct("answer_glow.png");
    let img = mct::decode(&data).unwrap();
    assert_eq!((img.width, img.height), (273, 213));
    assert!(img.rgba.chunks(4).any(|p| p[3] > 0), "fully transparent");
    assert!(mct::cached(&data));
    assert_eq!(mct::decode(&data).unwrap(), img);
}

#[test]
fn mct_decode_rejects_garbage() {
    if !mct::decoder_available() {
        return;
    }
    assert!(mct::decode(b".mct\x00\x01\x02").is_err());
}

#[test]
fn mct_tools_are_found_in_the_first_candidate_that_has_the_helper() {
    let root = std::env::temp_dir().join(format!("onevs100-tools-{}", std::process::id()));
    let (empty, good) = (root.join("empty"), root.join("good"));
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::create_dir_all(&good).unwrap();
    std::fs::write(good.join("mct_decode.py"), "").unwrap();
    let found = mct::locate_tools([root.join("missing"), empty.clone(), good.clone()]);
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(found, Some(good));
    assert_eq!(mct::locate_tools([PathBuf::from("/nonexistent/x")]), None);
}

#[test]
fn mct_missing_helper_error_says_how_to_fix_it() {
    let msg = mct::missing_tools_message(&[PathBuf::from("/a/b"), PathBuf::from("/c")]);
    for want in ["ONEVS100_TOOLS", "mct_decode.py", "/a/b", "/c", "title-1317.bin"] {
        assert!(msg.contains(want), "{msg}");
    }
}

#[test]
fn mct_candidates_include_the_environment_override_first() {
    let c = mct::tool_candidates(Some(PathBuf::from("/custom/tools")), Some(PathBuf::from("/opt/app/bin")), Some(PathBuf::from("/work/dir")));
    assert_eq!(c[0], PathBuf::from("/custom/tools"));
    assert!(c.contains(&PathBuf::from("/opt/app/common/content_package_editor")), "{c:?}");
    assert!(c.contains(&PathBuf::from("/work/common/content_package_editor")), "{c:?}");
}
