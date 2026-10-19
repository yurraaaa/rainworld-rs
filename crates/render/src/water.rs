//! The water surface as drawn this frame, and the debug view's flat translucent water
//! (`rainworld-re/notes/water.md` sections 1 and 6). The game's own water drawing is
//! [`crate::WaterSprites`].
//!
//! Mirrors: -

use rw_assets::{Room, TILE_SIZE};

use crate::Mesh;

/// Colours of the debug water.
mod palette {
    use crate::Color;

    pub const BODY: Color = Color::rgba(0.15, 0.40, 0.75, 0.40);
    pub const SURFACE: Color = Color::rgba(0.55, 0.80, 1.0, 0.85);
}

/// Water surface points left to right, in room pixels (y up).
///
/// The game's surface points are about 20 px apart from `x = -220` to
/// `pixel_width + 220`; the simulation's wavy surface is passed in per frame with
/// [`WaterSurface::from_rows`]: the front row (the physical surface) and the back row
/// (half a step right, drawn only), at their draw positions.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterSurface {
    points: Vec<[f32; 2]>,
    back: Vec<[f32; 2]>,
    /// Still water level (`fWaterLevel` between ticks), px.
    level: f32,
}

impl WaterSurface {
    /// How far the surface reaches past each side of the room, px.
    pub const OVERHANG: f32 = 220.0;
    /// The game's surface point spacing, px.
    pub const SPACING: f32 = TILE_SIZE;
    /// How far below the room's bottom edge the water body is filled, px.
    pub const DEPTH_BELOW_ROOM: f32 = 20.0;

    /// Surface points `left, left + spacing, ...` with the given heights; the back row
    /// is the front one half a step to the right, the still level the first height.
    #[must_use]
    pub fn new(left: f32, spacing: f32, heights: Vec<f32>) -> Self {
        let level = heights.first().copied().unwrap_or(0.0);
        let points: Vec<[f32; 2]> = heights
            .into_iter()
            .enumerate()
            .map(|(i, h)| [left + i as f32 * spacing, h])
            .collect();
        let back = points
            .iter()
            .map(|p| [p[0] + spacing / 2.0, p[1]])
            .collect();
        Self {
            points,
            back,
            level,
        }
    }

    /// Surface through the front `points` and `back` points (left to right, same
    /// count) over still water at `level`.
    #[must_use]
    pub fn from_rows(points: Vec<[f32; 2]>, back: Vec<[f32; 2]>, level: f32) -> Self {
        debug_assert_eq!(points.len(), back.len());
        Self {
            points,
            back,
            level,
        }
    }

    /// A flat surface at `level` over the game's span for a room `pixel_width` wide.
    #[must_use]
    pub fn flat(pixel_width: f32, level: f32) -> Self {
        let span = pixel_width + 2.0 * Self::OVERHANG;
        let count = (span / Self::SPACING).ceil() as usize + 1;
        Self::new(-Self::OVERHANG, Self::SPACING, vec![level; count])
    }

    /// Flat surface at the room's water row (`20 * L + 10` px), or `None` without water.
    #[must_use]
    pub fn for_room(room: &Room) -> Option<Self> {
        let row = room.water_level()?;
        Some(Self::flat(
            room.pixel_width(),
            row as f32 * TILE_SIZE + TILE_SIZE / 2.0,
        ))
    }

    /// Heights of the surface points.
    pub fn heights(&self) -> impl Iterator<Item = f32> + '_ {
        self.points.iter().map(|p| p[1])
    }

    /// `[first x, last x]` of the surface points.
    #[must_use]
    pub fn span(&self) -> [f32; 2] {
        match (self.points.first(), self.points.last()) {
            (Some(a), Some(b)) => [a[0], b[0]],
            _ => [0.0, 0.0],
        }
    }

    /// Surface points as room-pixel `[x, y]`.
    pub fn points(&self) -> impl Iterator<Item = [f32; 2]> + '_ {
        self.points.iter().copied()
    }

    /// Front point `i`.
    #[must_use]
    pub fn front(&self, i: usize) -> [f32; 2] {
        self.points[i]
    }

    /// Back point `i`.
    #[must_use]
    pub fn back(&self, i: usize) -> [f32; 2] {
        self.back[i]
    }

    /// Number of points per row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Still water level, px.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Translucent body from each surface segment down to `bottom`, with a bright
    /// strip along the surface.
    #[must_use]
    pub fn mesh(&self, bottom: f32) -> Mesh {
        let mut mesh = Mesh::new();
        let points: Vec<[f32; 2]> = self.points().collect();
        for w in points.windows(2) {
            let [a, b] = [w[0], w[1]];
            mesh.quad([a[0], bottom], [b[0], bottom], b, a, palette::BODY);
        }
        for w in points.windows(2) {
            mesh.line(w[0], w[1], 2.0, palette::SURFACE);
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_surface_spans_the_overhang() {
        let s = WaterSurface::flat(960.0, 110.0);
        assert_eq!(s.span(), [-220.0, 1180.0]);
        assert_eq!(s.heights().count(), 71);
        assert!(s.points().all(|p| p[1] == 110.0));
    }

    #[test]
    fn mesh_fills_from_bottom_to_surface() {
        let s = WaterSurface::new(0.0, 20.0, vec![50.0, 60.0, 45.0]);
        let mesh = s.mesh(-10.0);
        let body: Vec<[f32; 2]> = mesh
            .vertices()
            .iter()
            .filter(|v| v.color == palette::BODY.0)
            .map(|v| v.pos)
            .collect();
        // Two segments, two triangles each.
        assert_eq!(body.len(), 12);
        for p in [
            [0.0, -10.0],
            [40.0, -10.0],
            [0.0, 50.0],
            [20.0, 60.0],
            [40.0, 45.0],
        ] {
            assert!(body.contains(&p), "{p:?}");
        }
        assert!(body.iter().all(|p| (-10.0..=60.0).contains(&p[1])));
    }
}
