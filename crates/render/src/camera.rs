//! 2D camera: room pixels (y up) to clip space.

/// Maps room pixels to the viewport: `screen = (world - centre) * scale`, centred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Room-pixel point shown at the viewport centre.
    pub centre: [f32; 2],
    /// Physical screen pixels per room pixel.
    pub scale: f32,
    /// Viewport size in physical pixels.
    pub viewport: [f32; 2],
}

impl Camera {
    /// Fits the room rectangle `[0, size]` inside `viewport` with `margin` screen pixels
    /// on every side, preserving aspect ratio, centred.
    ///
    /// Degenerate inputs (zero viewport, zero room) give a scale of at least `1e-3`.
    #[must_use]
    pub fn fit(room_size: [f32; 2], viewport: (u32, u32), margin: f32) -> Self {
        let viewport = [viewport.0 as f32, viewport.1 as f32];
        let scale = Self::axis_scale(room_size[0], viewport[0], margin).min(Self::axis_scale(
            room_size[1],
            viewport[1],
            margin,
        ));
        Self {
            centre: [room_size[0] / 2.0, room_size[1] / 2.0],
            scale: scale.max(1e-3),
            viewport,
        }
    }

    /// Shows the room rectangle `origin .. origin + size` as large as fits in
    /// `viewport`, preserving aspect ratio, centred (the game's window scaled up).
    #[must_use]
    pub fn window(origin: [f32; 2], size: [f32; 2], viewport: (u32, u32)) -> Self {
        let viewport = [viewport.0 as f32, viewport.1 as f32];
        let scale = (viewport[0] / size[0].max(1.0)).min(viewport[1] / size[1].max(1.0));
        Self {
            centre: [origin[0] + size[0] / 2.0, origin[1] + size[1] / 2.0],
            scale: scale.max(1e-3),
            viewport,
        }
    }

    /// Scale that fits `room` room pixels into `viewport - 2 * margin` screen pixels.
    fn axis_scale(room: f32, viewport: f32, margin: f32) -> f32 {
        (viewport - 2.0 * margin).max(1.0) / room.max(1.0)
    }

    /// Room pixel to clip space (`-1..1` on both axes, y up).
    #[must_use]
    pub fn world_to_clip(&self, p: [f32; 2]) -> [f32; 2] {
        let [sx, sy, ox, oy] = self.clip_transform();
        [p[0] * sx + ox, p[1] * sy + oy]
    }

    /// Room pixel to window pixel (origin top-left, y down).
    #[must_use]
    pub fn world_to_screen(&self, p: [f32; 2]) -> [f32; 2] {
        let [cx, cy] = self.world_to_clip(p);
        [
            (cx + 1.0) * 0.5 * self.viewport[0],
            (1.0 - cy) * 0.5 * self.viewport[1],
        ]
    }

    /// `[scale_x, scale_y, offset_x, offset_y]` with `clip = world * scale + offset`,
    /// the layout of the shader's camera uniform.
    #[must_use]
    pub fn clip_transform(&self) -> [f32; 4] {
        let sx = 2.0 * self.scale / self.viewport[0].max(1.0);
        let sy = 2.0 * self.scale / self.viewport[1].max(1.0);
        [sx, sy, -self.centre[0] * sx, -self.centre[1] * sy]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait ApproxEq {
        /// Equal within 1e-3 per component.
        fn approx(self, other: Self) -> bool;
    }

    impl ApproxEq for [f32; 2] {
        fn approx(self, other: Self) -> bool {
            (self[0] - other[0]).abs() < 1e-3 && (self[1] - other[1]).abs() < 1e-3
        }
    }

    #[test]
    fn fit_wide_window_is_height_limited_and_centred() {
        // Room 960x700, window 1600x800, margin 50: height limits, 700/700 = 1.0.
        let cam = Camera::fit([960.0, 700.0], (1600, 800), 50.0);
        assert!((cam.scale - 1.0).abs() < 1e-6);
        assert!(cam.world_to_screen([0.0, 0.0]).approx([320.0, 750.0]));
        assert!(cam.world_to_screen([960.0, 700.0]).approx([1280.0, 50.0]));
        assert!(cam.world_to_clip([480.0, 350.0]).approx([0.0, 0.0]));
    }

    #[test]
    fn fit_tall_window_is_width_limited() {
        let cam = Camera::fit([1000.0, 500.0], (500, 1000), 0.0);
        assert!((cam.scale - 0.5).abs() < 1e-6);
        assert!(cam.world_to_clip([0.0, 250.0]).approx([-1.0, 0.0]));
        assert!(cam.world_to_clip([1000.0, 250.0]).approx([1.0, 0.0]));
    }

    #[test]
    fn y_up_maps_to_screen_up() {
        let cam = Camera::fit([100.0, 100.0], (200, 200), 0.0);
        let low = cam.world_to_screen([50.0, 0.0]);
        let high = cam.world_to_screen([50.0, 100.0]);
        assert!(high[1] < low[1]);
        assert!(low.approx([100.0, 200.0]));
        assert!(high.approx([100.0, 0.0]));
    }

    #[test]
    fn window_fills_the_viewport_at_its_aspect() {
        // Exact fit: the window's bottom-left at the screen's bottom-left, 1:1.
        let cam = Camera::window([-143.02, -52.02], [1366.0, 768.0], (1366, 768));
        assert!(cam.world_to_screen([-143.02, -52.02]).approx([0.0, 768.0]));
        assert!(cam.world_to_screen([1222.98, 715.98]).approx([1366.0, 0.0]));
        // Taller viewport: width-limited, letterboxed above and below.
        let cam = Camera::window([0.0, 0.0], [1366.0, 768.0], (683, 768));
        assert!((cam.scale - 0.5).abs() < 1e-6);
        assert!(cam.world_to_screen([0.0, 0.0]).approx([0.0, 576.0]));
    }

    #[test]
    fn degenerate_viewport_stays_finite() {
        let cam = Camera::fit([960.0, 700.0], (0, 0), 20.0);
        assert!(cam.scale > 0.0);
        assert!(cam.clip_transform().iter().all(|v| v.is_finite()));
    }
}
