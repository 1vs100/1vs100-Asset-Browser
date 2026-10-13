use crate::anim::{self, Animation};
use crate::inspect;
use std::collections::HashSet;
use std::path::PathBuf;

fn builds() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/common")
}

fn assets() -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut stack = vec![builds()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().ends_with(".msliveavatarasset") {
                found.push((p.display().to_string(), std::fs::read(p).unwrap()));
            }
        }
    }
    found.extend(
        crate::tests_fixtures::packages()
            .into_iter()
            .flat_map(|(role, id, files)| {
                files
                    .into_iter()
                    .filter(|f| f.name.to_lowercase().ends_with(".msliveavatarasset"))
                    .map(move |f| (format!("{role}/{id}/{}", f.name), f.payload.read().unwrap()))
            }),
    );
    found
}

fn sponsor() -> Vec<u8> {
    crate::tests_fixtures::package_file("episodes", "83136FF343124795DC1806A2DB044476589BD5C0", "media/sponsor/of_lu_sp.fbx.msliveavatarasset")
}

fn widget_sad() -> Vec<u8> {
    crate::tests_fixtures::package_file("shows", "77205BC1241C708A8A10B7303AF90E5EDD0286B1", "media/widgetanims/widgetcam_sad.fbx.msliveavatarasset")
}

fn retail() -> Vec<u8> {
    std::fs::read(builds().join("v2.0.1149.0.02-2009-10-26_retail/media/avataranims/xuiavataranims/hf_ch_02.fbx.msliveavatarasset")).unwrap()
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2e-5)
}

fn parsed(data: &[u8]) -> Animation {
    anim::parse(data).unwrap()
}

#[test]
fn every_distinct_animation_in_the_game_files_decodes_all_streams_exactly() {
    let mut seen = HashSet::new();
    let mut count = 0;
    for (path, data) in assets() {
        if !seen.insert(data.clone()) {
            continue;
        }
        let a = anim::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(a.header.joints, 71, "{path}");
        assert_eq!((a.header.motion_channels, a.header.texture_channels), (3, 5), "{path}");
        assert!(a.header.frames > 0 && a.header.fps > 0.0, "{path}");
        count += 1;
    }
    assert_eq!(count, 13);
}

#[test]
fn header_matches_the_python_reference() {
    let a = parsed(&sponsor());
    assert_eq!((a.header.frames, a.header.fps, a.header.joints, a.header.carryable_joints), (16, 15.0, 71, 0));
    assert!((a.duration() - 16.0 / 15.0).abs() < 1e-6);
    let b = parsed(&retail());
    assert_eq!(b.header.frames, 15);
    assert!((b.header.fps - 31.111_109).abs() < 1e-3);
    assert!(b.extra_bytes > 400, "older builds carry extra data: {}", b.extra_bytes);
    assert_eq!(parsed(&sponsor()).extra_bytes, 0);
}

#[test]
fn pose_values_match_the_python_reference() {
    let a = parsed(&sponsor());
    let first = a.pose(0).unwrap();
    let last = a.pose(15).unwrap();
    assert_eq!(first.len(), 71);
    assert!(close(&first[0].position, &[-0.02531, -0.03098, 0.00648]));
    assert!(close(&last[0].position, &[-0.02515, -0.03145, 0.00974]));
    assert!(close(&first[19].rotation, &[-0.08393, 0.01855, 0.07494, 0.99348]));
    assert!(close(&last[19].rotation, &[-0.08087, 0.0124, 0.06235, 0.9947]));
    assert!(close(&first[50].rotation, &[0.24251, 0.1353, -0.00976, 0.96062]));
    assert!(close(&first[19].scale, &[1.0, 1.0, 1.0]));
}

#[test]
fn motion_channels_match_the_python_reference() {
    let m = parsed(&sponsor()).motion(0).unwrap();
    assert_eq!(m.len(), 3);
    assert!(close(&m[1].position, &[0.05, 0.0, 0.0]) && close(&m[1].rotation, &[0.0, 0.13053, 0.0, 0.99144]));
    assert!(close(&m[2].position, &[-0.05, 0.0, 0.0]) && close(&m[2].rotation, &[0.0, -0.13053, 0.0, 0.99144]));
    let old = parsed(&retail()).motion(0).unwrap();
    assert!(close(&old[1].position, &[-0.00773, 0.0, 0.12629]));
}

#[test]
fn texture_channels_change_over_time() {
    let a = parsed(&widget_sad());
    assert_eq!(a.textures(0).unwrap(), vec![1, 1, 1, 1, 1]);
    assert_eq!(a.textures(12).unwrap(), vec![1, 1, 1, 13, 13]);
    assert_eq!(parsed(&sponsor()).textures(0).unwrap(), vec![4, 4, 4, 6, 6]);
}

#[test]
fn rotations_are_unit_quaternions_and_motion_is_smooth_in_every_animation() {
    let mut seen = HashSet::new();
    for (path, data) in assets() {
        if !seen.insert(data.clone()) {
            continue;
        }
        let a = parsed(&data);
        let mut previous = a.pose(0).unwrap();
        for f in 0..a.header.frames as usize {
            let pose = a.pose(f).unwrap();
            for (j, p) in pose.iter().enumerate() {
                let norm: f32 = p.rotation.iter().map(|x| x * x).sum::<f32>().sqrt();
                assert!((norm - 1.0).abs() < 1e-3, "{path} frame {f} joint {j}");
                if f > 0 {
                    let dot: f32 = p.rotation.iter().zip(&previous[j].rotation).map(|(a, b)| a * b).sum::<f32>().abs();
                    assert!(dot > 0.5, "{path} frame {f} joint {j}: rotation jumped (dot {dot})");
                }
            }
            previous = pose;
        }
    }
}

#[test]
fn out_of_range_frames_are_errors() {
    let a = parsed(&sponsor());
    assert!(a.pose(16).is_err() && a.motion(99).is_err() && a.textures(16).is_err());
}

#[test]
fn corrupt_or_truncated_assets_are_rejected() {
    let data = sponsor();
    assert!(anim::parse(&data[..data.len() - 200]).is_err());
    assert!(anim::parse(b"STRB\0\0not an animation at all").is_err());
    assert!(anim::parse(&[]).is_err());
}

#[test]
fn joint_names_cover_every_joint() {
    assert_eq!(anim::JOINT_NAMES.len(), 71);
    assert_eq!((anim::JOINT_NAMES[0], anim::JOINT_NAMES[19], anim::JOINT_NAMES[70]), ("BASE", "HEAD", "RT_THUMB2"));
}

#[test]
fn strb_details_describe_the_decoded_animation() {
    let d = inspect::describe("of_lu_sp.fbx.msliveavatarasset", &sponsor());
    let has = |s: &str| d.lines.iter().any(|l| l.contains(s));
    assert_eq!(d.kind, "Avatar animation (STRB)");
    assert!(has("16 frames at 15 fps (1.07 s)"), "{:#?}", d.lines);
    assert!(has("71 joints") && has("carryable joints 0"));
    assert!(has("motion channels 3") && has("texture channels 5"));
    assert!(has("BACKA") || has("LF_K") || has("HEAD"), "no animated joint names: {:#?}", d.lines);
    let old = inspect::describe("hf_ch_02.fbx.msliveavatarasset", &retail());
    assert!(old.lines.iter().any(|l| l.contains("extra data after the streams")));
}

#[test]
fn non_animation_strb_falls_back_to_the_header_only() {
    let mut data = vec![0u8; 64];
    data[..4].copy_from_slice(b"STRB");
    let d = inspect::describe("x.msliveavatarasset", &data);
    assert!(d.lines.iter().any(|l| l.contains("not decoded") || l.contains("could not")));
}
