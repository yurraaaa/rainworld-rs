//! CPU tessellation of debug shapes into a coloured triangle list.
//!
//! Mirrors: -

use bytemuck::{Pod, Zeroable};

/// Straight (non-premultiplied) RGBA, components `0..=1`, written to the target as is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);

    /// Opaque colour.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b, 1.0])
    }

    /// Colour with alpha.
    #[must_use]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }

    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        let [r, g, b, _] = self.0;
        Self([r, g, b, a])
    }

    /// Componentwise blend, `t = 0` is `self`.
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        let mut out = self.0;
        for (o, b) in out.iter_mut().zip(to.0) {
            *o += (b - *o) * t;
        }
        Self(out)
    }
}

/// One vertex of the triangle list: room-pixel position and colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    /// Vertex at `pos` with `color`.
    #[must_use]
    pub const fn new(pos: [f32; 2], color: Color) -> Self {
        Self {
            pos,
            color: color.0,
        }
    }
}

/// Segments used for a full circle.
const CIRCLE_SEGMENTS: usize = 32;

/// A triangle list (every three vertices form one triangle).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    vertices: Vec<Vertex>,
}

impl Mesh {
    /// Empty mesh.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// All vertices; the length is a multiple of 3.
    #[must_use]
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Number of vertices, saturating at `u32::MAX` (for draw calls).
    #[must_use]
    pub fn vertex_count(&self) -> u32 {
        u32::try_from(self.vertices.len()).unwrap_or(u32::MAX)
    }

    /// True if there is nothing to draw.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Appends all triangles of `other`.
    pub fn extend(&mut self, other: &Self) {
        self.vertices.extend_from_slice(&other.vertices);
    }

    /// Removes all triangles, keeping the allocation.
    pub fn clear(&mut self) {
        self.vertices.clear();
    }

    /// One triangle.
    pub fn triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: Color) {
        self.vertices.extend([
            Vertex::new(a, color),
            Vertex::new(b, color),
            Vertex::new(c, color),
        ]);
    }

    /// Quad `a b c d` in order around its edge.
    pub fn quad(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2], color: Color) {
        self.triangle(a, b, c, color);
        self.triangle(a, c, d, color);
    }

    /// Axis-aligned rectangle.
    pub fn rect(&mut self, min: [f32; 2], max: [f32; 2], color: Color) {
        self.quad(min, [max[0], min[1]], max, [min[0], max[1]], color);
    }

    /// Outline of an axis-aligned rectangle, strokes centred on its edges.
    pub fn rect_outline(&mut self, min: [f32; 2], max: [f32; 2], width: f32, color: Color) {
        let corners = [min, [max[0], min[1]], max, [min[0], max[1]], min];
        for edge in corners.windows(2) {
            self.line(edge[0], edge[1], width, color);
        }
    }

    /// Segment `a`-`b` of the given width; nothing for a zero-length segment.
    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: Color) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len <= f32::EPSILON {
            return;
        }
        let h = width / 2.0;
        let (nx, ny) = (-dy / len * h, dx / len * h);
        self.quad(
            [a[0] + nx, a[1] + ny],
            [b[0] + nx, b[1] + ny],
            [b[0] - nx, b[1] - ny],
            [a[0] - nx, a[1] - ny],
            color,
        );
    }

    /// Filled circle as a triangle fan.
    pub fn circle(&mut self, centre: [f32; 2], radius: f32, color: Color) {
        let circle = Circle { centre, radius };
        let mut prev = circle.point(0);
        for i in 1..=CIRCLE_SEGMENTS {
            let next = circle.point(i);
            self.triangle(centre, prev, next, color);
            prev = next;
        }
    }

    /// Circle outline: a band from `radius - width/2` to `radius + width/2`.
    pub fn ring(&mut self, centre: [f32; 2], radius: f32, width: f32, color: Color) {
        let inner = Circle {
            centre,
            radius: (radius - width / 2.0).max(0.0),
        };
        let outer = Circle {
            centre,
            radius: radius + width / 2.0,
        };
        for i in 0..CIRCLE_SEGMENTS {
            self.quad(
                inner.point(i),
                outer.point(i),
                outer.point(i + 1),
                inner.point(i + 1),
                color,
            );
        }
    }
}

/// A circle being tessellated into [`CIRCLE_SEGMENTS`] segments.
struct Circle {
    centre: [f32; 2],
    radius: f32,
}

impl Circle {
    /// Vertex `i` of the outline, counter-clockwise from +x.
    fn point(&self, i: usize) -> [f32; 2] {
        let a = i as f32 / CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;
        [
            self.centre[0] + self.radius * a.cos(),
            self.centre[1] + self.radius * a.sin(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(1.0, 0.0, 0.0);

    #[test]
    fn line_is_a_quad_of_the_given_width() {
        let mut m = Mesh::new();
        m.line([0.0, 0.0], [10.0, 0.0], 2.0, RED);
        assert_eq!(m.vertices().len(), 6);
        for v in m.vertices() {
            assert!((v.pos[1].abs() - 1.0).abs() < 1e-6);
            assert!(v.pos[0] == 0.0 || v.pos[0] == 10.0);
        }
    }

    #[test]
    fn zero_length_line_draws_nothing() {
        let mut m = Mesh::new();
        m.line([3.0, 3.0], [3.0, 3.0], 2.0, RED);
        assert!(m.is_empty());
    }

    #[test]
    fn circle_vertices_lie_on_or_at_the_centre() {
        let mut m = Mesh::new();
        m.circle([5.0, -5.0], 9.0, RED);
        assert_eq!(m.vertices().len(), CIRCLE_SEGMENTS * 3);
        for v in m.vertices() {
            let d = (v.pos[0] - 5.0).hypot(v.pos[1] + 5.0);
            assert!(d < 1e-4 || (d - 9.0).abs() < 1e-4, "{d}");
        }
    }

    #[test]
    fn ring_spans_its_band() {
        let mut m = Mesh::new();
        m.ring([0.0, 0.0], 8.0, 2.0, RED);
        for v in m.vertices() {
            let d = v.pos[0].hypot(v.pos[1]);
            assert!((d - 7.0).abs() < 1e-4 || (d - 9.0).abs() < 1e-4, "{d}");
        }
    }

    #[test]
    fn rect_covers_its_corners_and_keeps_colour() {
        let mut m = Mesh::new();
        m.rect([0.0, 0.0], [20.0, 20.0], RED);
        assert_eq!(m.vertices().len(), 6);
        assert!(m.vertices().iter().all(|v| v.color == RED.0));
        for corner in [[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]] {
            assert!(m.vertices().iter().any(|v| v.pos == corner));
        }
    }
}
