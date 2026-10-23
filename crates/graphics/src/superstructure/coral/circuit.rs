//! The sprites of a `CoralCircuit`: each `CircuitBit` one `maze` sprite in `Background`
//! (the `CoralCircuit` shader while the bit sits in place, `DeadCoralCircuit` otherwise;
//! the vertex colour is the window of the maze texture it shows, so its pattern stays
//! fixed to the room), each `CircuitConnector` its strands and four `Circle20` dots in
//! `Midground`, each `CircuitCrab` two dots and its legs. The objects themselves are
//! `rw-sim`'s.
//!
//! Mirrors: CoralBrain/CoralCircuit.cs (CircuitBit: InitiateSprites, DrawSprites, AddToContainer), CoralBrain/CircuitConnector.cs (InitiateSprites, DrawSprites), CoralBrain/CircuitCrab.cs (InitiateSprites, DrawSprites), CoralBrain/CrabLeg.cs (InitiateSprites, DrawSprites)

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::{CircuitBit, CircuitCrab, CoralCircuit, MyceliumPool, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::core::math::{CustomVec, Mathf};
use crate::effects::particles::decal::Unorm8;
use crate::superstructure::coral::mycelium::{LongMesh, MyceliumSprites};

/// The `maze` texture's side (`sourcePixelSize` of the `maze` element: the whole 128 x 128
/// texture).
pub const MAZE_SIZE: f32 = 128.0;

/// The `maze` atlas element's name (`LoadAtlasFromTexture("maze", ...)`).
pub const MAZE_ELEMENT: &str = "maze";

/// A circuit's draw.
pub trait CircuitSprites {
    /// `CircuitBit.DrawSprites` of bit `i` at `t` between ticks.
    fn draw_bit(&self, i: usize, t: f32) -> DrawList;

    /// `CircuitConnector.DrawSprites` of connector `k`: the strands that are not culled,
    /// then the four dots (the last one's colour draws from `rng`).
    fn draw_connector(
        &self,
        k: usize,
        pool: &MyceliumPool,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList;

    /// `CircuitCrab.DrawSprites` of crab `k`.
    fn draw_crab(&self, k: usize, t: f32) -> DrawList;
}

impl CircuitSprites for CoralCircuit {
    fn draw_bit(&self, i: usize, t: f32) -> DrawList {
        let bit = &self.bits[i];
        let origin = bit.stuck_pos - bit.size / 2.0;
        // `(float)((int)vector.x % 128) / 128f`: C# remainder, the sign of the dividend; the
        // mesh's colours are 8 bits a channel.
        let window = [
            (origin.x as i32 % 128) as f32 / MAZE_SIZE,
            (origin.y as i32 % 128) as f32 / MAZE_SIZE,
            bit.size.x / MAZE_SIZE,
            bit.size.y / MAZE_SIZE,
        ];
        let flip = bit.last_d_rotat.slerp(bit.d_rotat, t).y;
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: MAZE_ELEMENT,
                pos: bit.last_pos.lerp_clamped(bit.pos, t),
                rotation: bit.last_rotat.slerp(bit.rotat, t).to_degrees(),
                scale: Vec2::new(bit.size.x / MAZE_SIZE * flip, bit.size.y / MAZE_SIZE),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::from(window.unorm8()),
                shader: if bit.shader {
                    Shader::CoralCircuit
                } else {
                    Shader::DeadCoralCircuit
                },
                layer: Layer::Background,
            })],
        }
    }

    fn draw_connector(
        &self,
        k: usize,
        pool: &MyceliumPool,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let x = &self.connectors[k];
        let n = x.connections.len();
        let dot = x.dot_pos(&self.bits, t);
        let bit = &self.bits[x.bit];
        let mut items = Vec::new();
        for (i, &id) in x.connections.iter().enumerate() {
            if let Some(strand) = pool.get(id).filter(|m| !m.culled) {
                items.push(DrawItem::Mesh(strand.mesh(
                    i,
                    dot,
                    t,
                    Color::rgb(0.0, 0.0, 0.1),
                    0.0,
                    Layer::Midground,
                )));
            }
        }
        let flip = bit.last_d_rotat.slerp(bit.d_rotat, t).y;
        let flip = 0.2f32.lerp_to(1.0, flip.abs()) * flip.unity_sign();
        let rotation = bit.last_rotat.slerp(bit.rotat, t).to_degrees();
        let wide = flip.abs();
        let glow = rng.value();
        for (j, (offset, scale, color)) in [
            (Vec2::ZERO, 0.8, Color::hsl(0.963_888_9, 1.0, 0.1)),
            (
                Vec2::new(-1.0 - 2.0 * wide, 1.0 + 2.0 * wide),
                0.4,
                Color::hsl(0.963_888_9, 1.0, 0.25),
            ),
            (Vec2::new(-1.0, 1.0), 0.3, Color::hsl(0.963_888_9, 1.0, 0.1)),
            (Vec2::new(-1.0, 1.0), 0.15, Color::rgb(0.0, 0.0, glow)),
        ]
        .into_iter()
        .enumerate()
        {
            items.push(DrawItem::Sprite(Sprite {
                index: n + j,
                element: "Circle20",
                pos: dot + offset,
                rotation,
                scale: Vec2::new(scale * flip, scale),
                anchor: Vec2::new(0.5, 0.5),
                color,
                shader: Shader::Basic,
                layer: Layer::Midground,
            }));
        }
        DrawList { items }
    }

    fn draw_crab(&self, k: usize, t: f32) -> DrawList {
        let crab = &self.crabs[k];
        let body = crab.body_pos(&self.bits, t);
        let skin = Color::hsl(0.025, 0.5, 0.85);
        let mut items = vec![
            CircuitCrab::dot(0, body, 0.2, skin),
            CircuitCrab::dot(1, body + Vec2::new(-1.0, 1.0), 0.120_000_005, Color::WHITE),
        ];
        items.extend(
            (0..crab.legs.len()).map(|i| DrawItem::Mesh(crab.leg_mesh(i, &self.bits, t, skin))),
        );
        DrawList { items }
    }
}

/// The pieces of a crab's draw.
trait CrabParts {
    /// A `Circle20` of the crab's body.
    fn dot(index: usize, pos: Vec2, scale: f32, color: Color) -> DrawItem;

    /// `CrabLeg.DrawSprites` of leg `i`: a 1 px strip from the body through the leg's
    /// points, one colour.
    fn leg_mesh(&self, i: usize, bits: &[CircuitBit], t: f32, color: Color) -> Mesh;
}

impl CrabParts for CircuitCrab {
    fn dot(index: usize, pos: Vec2, scale: f32, color: Color) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "Circle20",
            pos,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader: Shader::Basic,
            layer: Layer::Midground,
        })
    }

    fn leg_mesh(&self, i: usize, bits: &[CircuitBit], t: f32, color: Color) -> Mesh {
        let leg = &self.legs[i];
        let n = leg.points.len();
        let mut vertices = vec![Vec2::ZERO; n * 4];
        let mut prev = self.body_pos(bits, t);
        for (j, p) in leg.points.iter().enumerate() {
            let pos = p[1].lerp_clamped(p[0], t);
            let dir = (prev - pos).normalized();
            let perp = dir.perpendicular() * 0.5;
            let overlap = prev.distance(pos) / 5.0;
            vertices[j * 4] = prev - dir * overlap - perp;
            vertices[j * 4 + 1] = prev - dir * overlap + perp;
            vertices[j * 4 + 2] = pos + dir * overlap - perp;
            vertices[j * 4 + 3] = pos + dir * overlap + perp;
            prev = pos;
        }
        Mesh {
            index: i + 2,
            vertices,
            uvs: LongMesh::uvs(n),
            triangles: Cow::Owned(LongMesh::triangles(n)),
            color,
            vertex_colors: Vec::new(),
            shader: Shader::Basic,
            layer: Layer::Midground,
        }
    }
}

#[cfg(test)]
#[path = "tests/circuit.rs"]
mod tests;
