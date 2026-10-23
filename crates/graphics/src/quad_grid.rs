//! The grid `TriangleMesh` a placed quad object (`LightBeam`, `SpotLight`) lays over its
//! quad: `TriangleMesh.MakeGridMesh`'s triangles and UVs, the objects' `GetIdealGridDiv`
//! and `UpdateVerts` (each vertex the crossing of two lerped lines, `Custom.LineIntersection`).
//!
//! Mirrors: TriangleMesh.cs (MakeGridMesh), LightBeam.cs (GetIdealGridDiv, UpdateVerts), SpotLight.cs (GetIdealGridDiv, UpdateVerts)

use rw_sim::Vec2;

/// A `div` x `div` grid over a quad.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadGrid {
    pub div: usize,
    /// Room pixels, `j * (div + 1) + i` for the crossing of the `j`th line from
    /// `quad[0]`-`quad[1]` and the `i`th from `quad[0]`-`quad[3]`.
    pub vertices: Vec<Vec2>,
    /// `UVvertices` over `Futile_White`: `(i / div, j / div)`.
    pub uvs: Vec<Vec2>,
    pub triangles: Vec<[u16; 3]>,
}

impl QuadGrid {
    /// `GetIdealGridDiv`: the longest of the quad's three consecutive sides and the
    /// side from corner 0 to 3, over `px_per_div`, rounded half to even (`RoundToInt`),
    /// clamped to 1 to 20.
    #[must_use]
    pub fn ideal_div(quad: &[Vec2; 4], px_per_div: f32) -> usize {
        let mut longest = 0.0f32;
        for i in 0..3 {
            longest = longest.max(quad[i].distance(quad[i + 1]));
        }
        longest = longest.max(quad[0].distance(quad[3]));
        ((longest / px_per_div).round_ties_even() as i32).clamp(1, 20) as usize
    }

    /// The mesh of `quad` with the ideal division for `px_per_div`.
    #[must_use]
    pub fn new(quad: &[Vec2; 4], px_per_div: f32) -> Self {
        let div = Self::ideal_div(quad, px_per_div);
        Self {
            div,
            vertices: Self::vertices_of(quad, div),
            uvs: Self::uvs_of(div),
            triangles: Self::triangles_of(div),
        }
    }

    /// `UpdateVerts`' loop.
    #[must_use]
    pub fn vertices_of(quad: &[Vec2; 4], div: usize) -> Vec<Vec2> {
        let n = div as f32;
        let mut verts = vec![Vec2::ZERO; (div + 1) * (div + 1)];
        for i in 0..=div {
            for j in 0..=div {
                let a = quad[0].lerp(quad[1], j as f32 / n);
                let b = quad[1].lerp(quad[2], i as f32 / n);
                let b2 = quad[3].lerp(quad[2], j as f32 / n);
                let a2 = quad[0].lerp(quad[3], i as f32 / n);
                verts[j * (div + 1) + i] = a.line_intersection(b2, a2, b);
            }
        }
        verts
    }

    /// `MakeGridMesh`'s UVs.
    #[must_use]
    pub fn uvs_of(div: usize) -> Vec<Vec2> {
        let n = div as f32;
        let mut uvs = vec![Vec2::ZERO; (div + 1) * (div + 1)];
        for k in 0..=div {
            for l in 0..=div {
                uvs[l * (div + 1) + k] = Vec2::new(k as f32 / n, l as f32 / n);
            }
        }
        uvs
    }

    /// `MakeGridMesh`'s triangles, two per cell in the game's order.
    #[must_use]
    pub fn triangles_of(div: usize) -> Vec<[u16; 3]> {
        let mut triangles = vec![[0; 3]; div * div * 2];
        let w = div + 1;
        for i in 0..div {
            for j in 0..div {
                let top = (j + 1) * w + i;
                let a = j * w + i;
                let right = j * w + i + 1;
                let c = (j + 1) * w + i + 1;
                let at = (j * div + i) * 2;
                triangles[at] = [a as u16, right as u16, top as u16];
                triangles[at + 1] = [right as u16, top as u16, c as u16];
            }
        }
        triangles
    }
}

#[cfg(test)]
mod tests {
    use rw_sim::Vec2;

    use super::QuadGrid;

    fn square(side: f32) -> [Vec2; 4] {
        [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, side),
            Vec2::new(side, side),
            Vec2::new(side, 0.0),
        ]
    }

    /// `Mathf.RoundToInt(longest / 250)` rounds half to even and clamps to 1..20; the
    /// SpotLight divides by 150.
    #[test]
    fn ideal_div_rounds_and_clamps() {
        assert_eq!(QuadGrid::ideal_div(&square(100.0), 250.0), 1);
        assert_eq!(QuadGrid::ideal_div(&square(125.0), 250.0), 1);
        assert_eq!(QuadGrid::ideal_div(&square(375.0), 250.0), 2);
        assert_eq!(QuadGrid::ideal_div(&square(625.0), 250.0), 2);
        assert_eq!(QuadGrid::ideal_div(&square(700.0), 250.0), 3);
        assert_eq!(QuadGrid::ideal_div(&square(100_000.0), 250.0), 20);
        assert_eq!(QuadGrid::ideal_div(&square(300.0), 150.0), 2);
    }

    /// The corners are the quad's corners, the interior crossings of a square are the
    /// lerped points; UVs are `(i, j) / div` at index `j * (div + 1) + i`.
    #[test]
    fn square_grid_vertices_and_uvs() {
        let g = QuadGrid::new(&square(500.0), 250.0);
        assert_eq!(g.div, 2);
        assert_eq!(g.vertices.len(), 9);
        // index j * 3 + i: i along quad[0]-quad[3] (x), j along quad[0]-quad[1] (y).
        assert_eq!(g.vertices[0], Vec2::new(0.0, 0.0));
        assert_eq!(g.vertices[2], Vec2::new(500.0, 0.0));
        assert_eq!(g.vertices[3 + 1], Vec2::new(250.0, 250.0));
        assert_eq!(g.vertices[6], Vec2::new(0.0, 500.0));
        assert_eq!(g.vertices[8], Vec2::new(500.0, 500.0));
        assert_eq!(g.uvs[5], Vec2::new(1.0, 0.5));
        assert_eq!(g.uvs[6], Vec2::new(0.0, 1.0));
    }

    /// `MakeGridMesh`: cell (i, j) holds triangles (a, right, top) and (right, top, c)
    /// at `(j * div + i) * 2`.
    #[test]
    fn triangles_are_the_games() {
        let t = QuadGrid::triangles_of(2);
        assert_eq!(t.len(), 8);
        assert_eq!(t[0], [0, 1, 3]);
        assert_eq!(t[1], [1, 3, 4]);
        // Cell (i = 1, j = 1): a = 4, right = 5, top = 7, c = 8, at index 6.
        assert_eq!(t[6], [4, 5, 7]);
        assert_eq!(t[7], [5, 7, 8]);
    }

    /// A non-parallelogram quad: the crossings stay on the quad's edges at the border.
    #[test]
    fn skewed_quad_borders_follow_its_edges() {
        let quad = [
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 300.0),
            Vec2::new(200.0, 400.0),
            Vec2::new(300.0, 20.0),
        ];
        let g = QuadGrid::new(&quad, 250.0);
        assert_eq!(g.div, 2);
        let mid = quad[0].lerp(quad[1], 0.5);
        assert!(g.vertices[3].distance(mid) < 1e-3, "{:?}", g.vertices[3]);
    }
}
