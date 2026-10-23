//! `Mycelium`'s sprites: a strand of an overseer or of the room's `WallMycelia` as a long
//! thin mesh. The strands themselves are `rw-sim`'s ([`rw_sim::Mycelium`]). Behaviour:
//! `rainworld-re/notes/superstructure-coral.md`, `overseer.md`.
//!
//! `WallMycelia`'s sprites: one long thin mesh per strand in `Items`, from its root
//! through its points, black fading in from nothing (the object never sets a colour, so
//! the strand's `color` is the default struct) with a blue tip.
//!
//! Mirrors: CoralBrain/Mycelium.cs (InitiateSprites, DrawSprites, UpdateColor), TriangleMesh.cs (MakeLongMesh), CoralBrain/WallMycelia.cs (InitiateSprites, DrawSprites, AddToContainer)

use std::borrow::Cow;

use rw_sim::Mycelium;
use rw_sim::Vec2;
use rw_sim::{MyceliumPool, WallMycelia};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader};
use crate::core::math::{CustomVec, Mathf};

/// `TriangleMesh.MakeLongMesh(segments, pointyTip: false, customColor: true)`.
pub struct LongMesh;

impl LongMesh {
    /// The triangles.
    #[must_use]
    pub fn triangles(segments: usize) -> Vec<[u16; 3]> {
        let mut out = Vec::with_capacity((segments - 1) * 4 + 2);
        for i in 0..segments - 1 {
            for j in 0..4 {
                let k = u16::try_from(i * 4 + j).unwrap_or(0);
                out.push([k, k + 1, k + 2]);
            }
        }
        let last = u16::try_from((segments - 1) * 4).unwrap_or(0);
        out.push([last, last + 1, last + 2]);
        out.push([last + 1, last + 2, last + 3]);
        out
    }

    /// `UVvertices`: x alternating 0, 1; y up the mesh in steps of
    /// `1 / ((segments - 1) * 2 + 1)`.
    #[must_use]
    pub fn uvs(segments: usize) -> Vec<Vec2> {
        let step = 1.0 / ((segments - 1) * 2 + 1) as f32;
        (0..segments * 4)
            .map(|k| Vec2::new(if k % 2 == 0 { 0.0 } else { 1.0 }, (k / 2) as f32 * step))
            .collect()
    }
}

/// A strand's draw.
pub trait MyceliumSprites {
    /// `DrawSprites` and `UpdateColor`: a 1 px wide strip from `from` (the owner's
    /// connection point) through the points at `t`, shading from `color` to a dark
    /// blue-green after `gradient_start`, its last two vertices blue.
    fn mesh(
        &self,
        index: usize,
        from: Vec2,
        t: f32,
        color: Color,
        gradient_start: f32,
        layer: Layer,
    ) -> Mesh;
}

impl MyceliumSprites for Mycelium {
    fn mesh(
        &self,
        index: usize,
        from: Vec2,
        t: f32,
        color: Color,
        gradient_start: f32,
        layer: Layer,
    ) -> Mesh {
        let n = self.points.len();
        let mut vertices = vec![Vec2::ZERO; n * 4];
        let mut prev = from;
        for (i, p) in self.points.iter().enumerate() {
            let pos = p[1].lerp_clamped(p[0], t);
            let dir = (prev - pos).normalized();
            let perp = dir.perpendicular() * 0.5;
            let overlap = prev.distance(pos) / 5.0;
            vertices[i * 4] = prev - dir * overlap - perp;
            vertices[i * 4 + 1] = prev - dir * overlap + perp;
            vertices[i * 4 + 2] = pos + dir * overlap - perp;
            vertices[i * 4 + 3] = pos + dir * overlap + perp;
            prev = pos;
        }
        let dark = Color::hsl(22.0 / 45.0, 0.5, 0.2);
        let last = (n * 4 - 1) as f32;
        let mut vertex_colors: Vec<Color> = (0..n * 4)
            .map(|i| color.lerp(dark, (i as f32 / last).inverse_lerp(gradient_start, 1.0)))
            .collect();
        for c in vertex_colors.iter_mut().rev().take(2) {
            *c = Color::rgb(0.0, 0.0, 1.0);
        }
        Mesh {
            index,
            vertices,
            uvs: LongMesh::uvs(n),
            triangles: Cow::Owned(LongMesh::triangles(n)),
            color,
            vertex_colors,
            shader: Shader::Basic,
            layer,
        }
    }
}

/// A `WallMycelia`'s draw.
pub trait WallMyceliaSprites {
    /// `DrawSprites` at `t` between ticks: the strands that are not culled.
    fn draw(&self, pool: &MyceliumPool, t: f32) -> DrawList;
}

impl WallMyceliaSprites for WallMycelia {
    fn draw(&self, pool: &MyceliumPool, t: f32) -> DrawList {
        let items = self
            .mycelia
            .iter()
            .enumerate()
            .filter_map(|(i, &id)| Some((i, pool.get(id)?)))
            .filter(|(_, m)| !m.culled)
            .map(|(i, m)| {
                let from = self.roots.root_positions[i];
                DrawItem::Mesh(m.mesh(i, from, t, Color::default(), 0.0, Layer::Items))
            })
            .collect();
        DrawList { items }
    }
}

#[cfg(test)]
#[path = "tests/mycelium.rs"]
mod tests;
