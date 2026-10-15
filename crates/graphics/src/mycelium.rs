//! `Mycelium`: a hair of an overseer (the only owner ported), a short chain of points
//! pulled toward a fixed length, reaching out from its body and linking up with a
//! conversation partner's. Behaviour: `rainworld-re/notes/overseer.md`.

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::Vec2;
use rw_sim::creature::EntityId;

use crate::draw::{Color, Layer, Mesh, Shader};
use crate::math::{CustomVec, Mathf, Vec2Bezier};

/// One end of a `MyceliaConnection`: the other overseer's mycelium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyceliumLink {
    pub owner: EntityId,
    pub index: usize,
}

/// A `Mycelium` without a neuron system (the base game's overseers have none).
#[derive(Debug, Clone, PartialEq)]
pub struct Mycelium {
    /// Per point: position, last position, velocity.
    pub points: Vec<[Vec2; 3]>,
    pub con_rad: f32,
    pub length: f32,
    pub connection: Option<MyceliumLink>,
    pub rest: i32,
}

impl Mycelium {
    /// The constructor: `IntClamp(max(length, lerp(length, 40, 0.5)) / 15, 2, 20)`
    /// points, then `Reset(init)`.
    pub fn new(length: f32, init: Vec2, rng: &mut UnityRandom) -> Self {
        let reach = length.max(length + (40.0 - length) * 0.5);
        let count = ((reach / 15.0) as i32).clamp(2, 20);
        let n = usize::try_from(count).unwrap_or(2);
        let mut m = Self {
            points: vec![[Vec2::ZERO; 3]; n],
            con_rad: length / count as f32,
            length,
            connection: None,
            rest: 0,
        };
        m.reset(init, rng);
        m
    }

    /// `Reset`: a random curve from `at` (the owner's `ResetDir` is `Custom.RNV`), every
    /// point jittered, every velocity random.
    pub fn reset(&mut self, at: Vec2, rng: &mut UnityRandom) {
        let end = at + Vec2::random_unit(rng) * (self.length * 0.6);
        let half = at.distance(end) * 0.5;
        let ca = at + (at.direction_to(end) + Vec2::random_unit(rng)).normalized() * half;
        let cb = end + (end.direction_to(at) + Vec2::random_unit(rng)).normalized() * half;
        let last = (self.points.len() - 1) as f32;
        for (i, p) in self.points.iter_mut().enumerate() {
            let pos = at.bezier(ca, end, cb, i as f32 / last) + Vec2::random_unit(rng);
            *p = [pos, pos, Vec2::random_unit(rng)];
        }
    }

    #[must_use]
    pub fn base(&self) -> Vec2 {
        self.points[0][0]
    }

    #[must_use]
    pub fn tip(&self) -> Vec2 {
        self.points[self.points.len() - 1][0]
    }

    /// `Phase1` and `Phase4`: each point moves by its velocity and is pulled to
    /// `conRad` from the one before, with a straightening push two apart. (The wall
    /// avoidance is off for overseers.)
    pub(crate) fn step(&mut self) {
        let mut next: Vec<[Vec2; 3]> = self
            .points
            .iter()
            .map(|&[pos, _, vel]| [pos + vel, pos, vel * 0.999])
            .collect();
        for i in 1..next.len() {
            let f = next[i][0] - next[i - 1][0];
            let push = f.normalized() * ((self.con_rad - f.length()) * 0.5);
            next[i][0] += push;
            next[i][2] += push;
            next[i - 1][0] -= push;
            next[i - 1][2] -= push;
            if i > 1 {
                let dir = (next[i][0] - next[i - 2][0]).normalized();
                next[i][2] += dir * 0.2;
                next[i - 2][2] -= dir * 0.2;
            }
        }
        self.points = next;
    }

    /// `TriangleMesh.MakeLongMesh(segments, pointyTip: false, …)`'s triangles.
    #[must_use]
    pub fn long_mesh_triangles(segments: usize) -> Vec<[u16; 3]> {
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

    /// `MakeLongMesh`'s `UVvertices`: x alternating 0, 1; y up the mesh in steps of
    /// `1 / ((segments - 1) * 2 + 1)`.
    #[must_use]
    pub fn long_mesh_uvs(segments: usize) -> Vec<Vec2> {
        let step = 1.0 / ((segments - 1) * 2 + 1) as f32;
        (0..segments * 4)
            .map(|k| Vec2::new(if k % 2 == 0 { 0.0 } else { 1.0 }, (k / 2) as f32 * step))
            .collect()
    }

    /// `DrawSprites` and `UpdateColor`: a 1 px wide strip from `from` (the owner's
    /// connection point) through the points at `t`, shading from `color` to a dark
    /// blue-green after `gradient_start`, its last two vertices blue.
    #[must_use]
    pub fn mesh(
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
            uvs: Self::long_mesh_uvs(n),
            triangles: Cow::Owned(Self::long_mesh_triangles(n)),
            color,
            vertex_colors,
            shader: Shader::Basic,
            layer,
        }
    }
}
