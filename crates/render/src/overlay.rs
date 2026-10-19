//! A debug overlay (the windowed app's debug UI): textured, coloured triangles in
//! target pixels (y down, origin top-left), drawn over the presented window at its
//! physical size, or over the frame when headless. One texture, the UI's font atlas.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct OverlayVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    /// RGBA, written to the target as given (not linearised).
    pub color: [u8; 4],
}

/// Indices `first .. first + count` of [`Overlay::indices`], clipped to `clip`
/// (`[x0, y0, x1, y1]` in target pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayDraw {
    pub first: u32,
    pub count: u32,
    pub clip: [f32; 4],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    pub vertices: Vec<OverlayVertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<OverlayDraw>,
}

impl OverlayDraw {
    /// The clip rectangle as a scissor `(x, y, width, height)` inside a target of
    /// `size`; `None` when nothing of it is inside.
    #[must_use]
    pub fn scissor(&self, size: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
        let [x0, y0, x1, y1] = self.clip;
        let x0 = x0.max(0.0).floor() as u32;
        let y0 = y0.max(0.0).floor() as u32;
        let x1 = (x1.ceil().max(0.0) as u32).min(size.0);
        let y1 = (y1.ceil().max(0.0) as u32).min(size.1);
        (x1 > x0 && y1 > y0).then(|| (x0, y0, x1 - x0, y1 - y0))
    }
}

impl Overlay {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.draws.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scissor_is_clamped_to_the_target() {
        let draw = OverlayDraw {
            first: 0,
            count: 3,
            clip: [-5.0, 10.5, 120.0, 40.2],
        };
        assert_eq!(draw.scissor((100, 100)), Some((0, 10, 100, 31)));
        let outside = OverlayDraw {
            clip: [150.0, 0.0, 200.0, 10.0],
            ..draw
        };
        assert_eq!(outside.scissor((100, 100)), None);
    }
}
