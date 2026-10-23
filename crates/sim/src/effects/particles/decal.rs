//! `CustomDecal`: a placed picture mapped over a quad of the room, drawn in `Foreground`
//! by the `Decal` shader. The grid mesh (`UpdateVerts`) is built once from the placed
//! data: its per-vertex noise is drawn from `Random.InitState((int)corner sum)` with
//! the global state restored, so it makes no shared-stream draws (D1); here that is a
//! stream of its own. `Update` only checks that the corners moved (editor), so the
//! decal has no per-tick state. Sprites: `rw-graphics` `decal.rs`.
//!
//! Mirrors: CustomDecal.cs (constructor, GetIdealGridDiv, UpdateVerts), Room.cs (Loaded: CustomDecal)

use rw_assets::CustomDecalData;
use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// One `CustomDecal`.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomDecal {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    pub data: CustomDecalData,
    /// `quad`: the object's position and its three handles' positions.
    pub quad: [Vec2; 4],
    pub grid_div: usize,
    /// `verts`, index `j * (grid_div + 1) + i`.
    pub verts: Vec<Vec2>,
    /// `verticeColors`: `(fromDepth, toDepth, f, f2)` per vertex.
    pub colors: Vec<[f32; 4]>,
}

impl CustomDecal {
    /// The constructor and the first `UpdateVerts`.
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, data: CustomDecalData) -> Self {
        let quad = [
            pos,
            pos + Vec2::from(data.handles[0]),
            pos + Vec2::from(data.handles[1]),
            pos + Vec2::from(data.handles[2]),
        ];
        let mut decal = Self {
            update_order,
            data,
            quad,
            grid_div: Self::ideal_grid_div(&quad),
            verts: Vec::new(),
            colors: Vec::new(),
        };
        decal.update_verts();
        decal
    }

    /// `GetIdealGridDiv`: the longest of the sides `0-1`, `1-2`, `2-3` and `0-3` over
    /// 150 px, rounded half to even, within 1 to 20.
    #[must_use]
    pub fn ideal_grid_div(quad: &[Vec2; 4]) -> usize {
        let mut longest = 0.0_f32;
        for i in 0..3 {
            longest = longest.max(quad[i].distance(quad[i + 1]));
        }
        longest = longest.max(quad[0].distance(quad[3]));
        ((longest / 150.0).round_ties_even() as i32).clamp(1, 20) as usize
    }

    /// `UpdateVerts` after the grid division is chosen: every vertex's position (the
    /// intersection of the grid lines, a perspective warp of the quad) and colour. Four
    /// `Random.value` per vertex, the noise exponents then the lerp targets, whatever
    /// `noise` is.
    fn update_verts(&mut self) {
        let [q0, q1, q2, q3] = self.quad;
        let seed = q0.x + q0.y + q1.x + q1.y + q2.x + q2.y + q3.x + q3.y;
        let mut rng = UnityRandom::new(seed.to_int_cs());
        let div = self.grid_div as f32;
        let v = self.data.vertices;
        let noise = self.data.noise;
        let n = self.grid_div + 1;
        self.verts = vec![Vec2::ZERO; n * n];
        self.colors = vec![[0.0; 4]; n * n];
        for i in 0..=self.grid_div {
            for j in 0..=self.grid_div {
                let (ti, tj) = (i as f32 / div, j as f32 / div);
                let a = q0.lerp(q1, tj);
                let b = q1.lerp(q2, ti);
                let b2 = q3.lerp(q2, tj);
                let a2 = q0.lerp(q3, ti);
                let mut f = v[3][1]
                    .lerp(v[2][1], ti)
                    .lerp(v[0][1].lerp(v[1][1], ti), tj);
                let mut f2 = v[3][0]
                    .lerp(v[2][0], ti)
                    .lerp(v[0][0].lerp(v[1][0], ti), tj);
                f = f.mathf_pow(1.0 + (-0.5_f32).lerp(0.5, rng.value()) * noise);
                f2 = f2.mathf_pow(1.0 + (-0.5_f32).lerp(0.5, rng.value()) * noise);
                f = f.lerp(
                    rng.value(),
                    noise * (1.0 - 2.0 * (f - 0.5).abs()).mathf_pow(2.5),
                );
                f2 = f2.lerp(
                    rng.value(),
                    noise * (1.0 - 2.0 * (f - 0.5).abs()).mathf_pow(2.5),
                );
                let at = j * n + i;
                self.verts[at] = a.line_intersection(b2, a2, b);
                self.colors[at] = [self.data.from_depth, self.data.to_depth, f, f2];
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/decal.rs"]
mod tests;
