//! What the GPU shader tests share: HLSL's scalar functions and a frame pixel's `scrPos`.
//!
//! Mirrors: - (port-only: tests)

use super::full_screen_tests::{H, W};

/// HLSL's `frac` and `lerp` (not Rust's `fract`, which is `x - trunc(x)`).
pub(super) trait Hlsl {
    fn frac(self) -> f32;
    fn lerp_to(self, to: f32, t: f32) -> f32;
}

impl Hlsl for f32 {
    fn frac(self) -> f32 {
        self - self.floor()
    }

    fn lerp_to(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t
    }
}

/// Frame pixel `(x, y)`, y down.
#[derive(Clone, Copy)]
pub(super) struct FramePixel(pub u32, pub u32);

impl FramePixel {
    /// Its `scrPos` (0 to 1 over the screen, y up) at the pixel's centre.
    pub(super) fn scr(self) -> [f32; 2] {
        [
            (self.0 as f32 + 0.5) / W as f32,
            (H as f32 - self.1 as f32 - 0.5) / H as f32,
        ]
    }

    /// The pixel's place in the level image of a view at the window's origin
    /// (`floor(scrPos * _screenSize) / _screenSize` through `_spriteRect`).
    pub(super) fn level_coord(self) -> [f32; 2] {
        let (w, h) = (W as f32, H as f32);
        let s = self.scr();
        let rect = [-0.5 / w, 0.5 / h, (w - 0.5) / w, (h + 0.5) / h];
        [
            ((s[0] * w).floor() / w - rect[0]) / (rect[2] - rect[0]),
            ((s[1] * h).floor() / h - rect[1]) / (rect[3] - rect[1]),
        ]
    }
}
