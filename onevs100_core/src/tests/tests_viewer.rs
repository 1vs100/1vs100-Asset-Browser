use crate::render::Camera;
use crate::viewer::ViewState;

#[test]
fn dragging_turns_the_camera_in_proportion_to_the_motion() {
    let mut v = ViewState::default();
    let start = v.camera;
    assert!(!v.drag([10.0, 10.0]));
    v.press([10.0, 10.0]);
    assert!(v.drag([110.0, 10.0]));
    assert!(v.camera.yaw > start.yaw);
    assert_eq!(v.camera.pitch, start.pitch);
    let after = v.camera.yaw;
    assert!(v.drag([110.0, 60.0]));
    assert_eq!(v.camera.yaw, after);
    assert!(v.camera.pitch > start.pitch);
}

#[test]
fn releasing_stops_the_drag() {
    let mut v = ViewState::default();
    v.press([0.0, 0.0]);
    v.release();
    assert!(!v.drag([50.0, 50.0]));
    assert_eq!(v.camera, Camera::default());
}

#[test]
fn pitch_is_clamped_so_the_model_cannot_flip_over() {
    let mut v = ViewState::default();
    v.press([0.0, 0.0]);
    v.drag([0.0, 100_000.0]);
    assert!(v.camera.pitch <= std::f32::consts::FRAC_PI_2 - 0.05);
    v.drag([0.0, -200_000.0]);
    assert!(v.camera.pitch >= -(std::f32::consts::FRAC_PI_2 - 0.05));
}

#[test]
fn scrolling_zooms_within_limits_and_reset_restores_the_default() {
    let mut v = ViewState::default();
    assert!(v.scroll(1.0));
    assert!(v.camera.zoom > 1.0);
    for _ in 0..200 {
        v.scroll(1.0);
    }
    let max = v.camera.zoom;
    assert!(!v.scroll(1.0) && v.camera.zoom == max && max <= 8.0);
    for _ in 0..400 {
        v.scroll(-1.0);
    }
    assert!(v.camera.zoom >= 0.2);
    v.press([0.0, 0.0]);
    v.drag([30.0, 30.0]);
    v.reset();
    assert_eq!(v.camera, Camera::default());
}
