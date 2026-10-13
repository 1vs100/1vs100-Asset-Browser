use crate::skeleton::{self, Skeleton};

#[test]
fn both_embedded_skeletons_decode_to_seventy_one_joints() {
    for version in [1, 2] {
        let s = skeleton::embedded(version).unwrap();
        assert_eq!(s.parents.len(), 71);
        assert_eq!(s.parents[0], None);
        assert!(s.parents.iter().enumerate().skip(1).all(|(i, p)| p.is_some_and(|p| p < 71 && p != i)));
    }
    assert!(skeleton::embedded(0).is_err());
    assert!(skeleton::embedded(3).is_err());
}

#[test]
fn bind_positions_follow_the_xdk_joint_order() {
    let s = skeleton::embedded(1).unwrap();
    assert_eq!(s.parents[1], Some(0));
    assert_eq!(s.parents[19], Some(14));
    assert!((s.positions[19][1] - 1.139).abs() < 0.001);
    assert!(s.positions[33][0] > 0.4 && s.positions[36][0] < -0.4);
    assert!((skeleton::embedded(2).unwrap().positions[19][1] - 1.191).abs() < 0.001);
}

#[test]
fn rest_pose_leaves_vertices_where_they_are() {
    let s = skeleton::embedded(1).unwrap();
    let rest: Vec<skeleton::Rotation> = vec![[0.0, 0.0, 0.0, 1.0]; 71];
    let m = s.skinning(&rest);
    for matrix in &m {
        let moved = skeleton::transform(matrix, [0.1, 0.2, 0.3]);
        assert!((0..3).all(|i| (moved[i] - [0.1, 0.2, 0.3][i]).abs() < 1e-4));
    }
}

#[test]
fn rotating_a_joint_swings_its_children_around_it() {
    let s = skeleton::embedded(1).unwrap();
    let mut rot: Vec<skeleton::Rotation> = vec![[0.0, 0.0, 0.0, 1.0]; 71];
    let half = std::f32::consts::FRAC_1_SQRT_2;
    rot[14] = [0.0, 0.0, half, half];
    let m = s.skinning(&rot);
    let head = s.positions[19];
    let neck = s.positions[14];
    let moved = skeleton::transform(&m[19], head);
    let offset = [head[0] - neck[0], head[1] - neck[1]];
    assert!((moved[0] - (neck[0] - offset[1])).abs() < 1e-3 && (moved[1] - (neck[1] + offset[0])).abs() < 1e-3, "{moved:?}");
    let _ = Skeleton { parents: vec![], positions: vec![] };
}

use crate::body::Gender;

#[test]
fn neutral_factors_leave_every_joint_unscaled() {
    assert!(skeleton::joint_scales(Gender::Male, 0.0, 0.0).iter().all(|s| *s == [1.0; 3]));
    assert_eq!(skeleton::joint_scales(Gender::Female, 0.0, 0.0).len(), 71);
}

#[test]
fn weight_and_height_factors_interpolate_the_documented_joint_scales() {
    let fat = skeleton::joint_scales(Gender::Female, 1.0, 0.0);
    assert_eq!(fat[4], [1.3, 1.3, 1.6]);
    assert_eq!(fat[24], [1.6, 1.0, 1.36]);
    let thin = skeleton::joint_scales(Gender::Male, -0.5, 0.0);
    assert!((thin[24][0] - 0.88).abs() < 1e-5 && thin[24][1] == 1.0);
    let tall = skeleton::joint_scales(Gender::Male, 0.0, 1.0);
    assert_eq!((tall[0], tall[19]), ([1.1; 3], [0.9; 3]));
    let short = skeleton::joint_scales(Gender::Female, 0.0, -1.0);
    assert_eq!((short[0], short[19]), ([0.9; 3], [1.05; 3]));
}

#[test]
fn a_joint_scale_stretches_the_offsets_of_its_children() {
    let s = skeleton::embedded(2).unwrap();
    let rest: Vec<skeleton::Rotation> = vec![[0.0, 0.0, 0.0, 1.0]; 71];
    let mut scales = vec![[1.0; 3]; 71];
    scales[0] = [2.0; 3];
    let m = s.skinning_scaled(&rest, &scales);
    let (base, head) = (s.positions[0], s.positions[19]);
    let moved = skeleton::transform(&m[19], head);
    for i in 0..3 {
        assert!((moved[i] - (base[i] + 2.0 * (head[i] - base[i]))).abs() < 1e-3, "{moved:?}");
    }
}
