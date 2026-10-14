use crate::render::Camera;

const TURN_PER_PIXEL: f32 = 0.01;
const ZOOM_PER_NOTCH: f32 = 1.1;
const MIN_ZOOM: f32 = 0.2;
const MAX_ZOOM: f32 = 8.0;
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.05;

#[derive(Debug, Clone, Default)]
pub struct ViewState {
    pub camera: Camera,
    anchor: Option<[f32; 2]>,
}

impl ViewState {
    pub fn press(&mut self, at: [f32; 2]) {
        self.anchor = Some(at);
    }

    pub fn release(&mut self) {
        self.anchor = None;
    }

    pub fn dragging(&self) -> bool {
        self.anchor.is_some()
    }

    pub fn drag(&mut self, at: [f32; 2]) -> bool {
        let Some(from) = self.anchor.replace(at) else {
            self.anchor = None;
            return false;
        };
        self.camera.yaw += (at[0] - from[0]) * TURN_PER_PIXEL;
        self.camera.pitch = (self.camera.pitch + (at[1] - from[1]) * TURN_PER_PIXEL).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        true
    }

    pub fn scroll(&mut self, notches: f32) -> bool {
        let zoom = (self.camera.zoom * ZOOM_PER_NOTCH.powf(notches)).clamp(MIN_ZOOM, MAX_ZOOM);
        let changed = zoom != self.camera.zoom;
        self.camera.zoom = zoom;
        changed
    }

    pub fn reset(&mut self) {
        *self = ViewState::default();
    }
}
