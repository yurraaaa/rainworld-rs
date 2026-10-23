//! `CoralNeuron`'s sprites in `Items`: a long mesh with the neuron's colour ramp, a second
//! over it under the `CoralNeuron` shader (the scrolling bright lines), the strands as long
//! thin meshes, and per bump a coloured disc, a white highlight and a ping dot (moved to
//! `Water` in a room with `WaterLights`). The strands themselves are `rw-sim`'s.
//!
//! Mirrors: CoralBrain/CoralNeuron.cs (MeshColor, InitiateSprites, DrawSprites, AddToContainer, Rad, Pos, Direction, DirAtSegment)

use std::borrow::Cow;

use rw_sim::{CoralNeuron, MyceliumPool, NeuronGeometry, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::math::{CustomVec, Mathf};
use crate::mycelium::{LongMesh, MyceliumSprites};

/// The neuron's colours and widths along its length.
pub struct NeuronLook;

impl NeuronLook {
    /// `MeshColor(f)`: an HSL ramp, dark and thin in the middle.
    #[must_use]
    pub fn mesh_color(f: f32) -> Color {
        let f = (f - 0.5).abs() * 2.0;
        let hue = 1.025f32.lerp_to(0.963_888_9, 0.5 + 0.5 * rw_sim::FloatExt::mathf_pow(f, 3.0));
        Color::hsl(
            hue - hue.floor(),
            rw_sim::FloatExt::lerp_map(f, 0.8, 1.0, 1.0, 0.5),
            rw_sim::FloatExt::lerp_map(f, 0.7, 1.0, 0.5, 0.15),
        )
    }

    /// `Rad(float f)`: the body's half width along its length.
    #[must_use]
    pub fn rad(f: f32) -> f32 {
        let sine = Mathf::mathf_sin(f * std::f32::consts::PI).clamp(0.0, 1.0);
        0.2f32.lerp_to(1.0, rw_sim::FloatExt::mathf_pow(sine, 0.5))
    }

    /// One of a bump's `deerEyeB` discs.
    fn disc(index: usize, pos: Vec2, scale: f32, color: Color, layer: Layer) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "deerEyeB",
            pos,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader: Shader::Basic,
            layer,
        })
    }
}

/// The curve of the segments between ticks.
trait NeuronCurve {
    /// `DirAtSegment(seg, timeStacker)`.
    fn dir_at_segment(&self, seg: i32, t: f32) -> Vec2;
    /// `Direction(f, timeStacker)`: the segments' directions slerped by `f`.
    fn direction(&self, f: f32, t: f32) -> Vec2;
    /// `Pos(f, timeStacker)`.
    fn pos_along(&self, f: f32, t: f32) -> Vec2;
    /// A segment's place between ticks.
    fn lerped(&self, i: usize, t: f32) -> Vec2;
}

impl NeuronCurve for NeuronGeometry {
    fn lerped(&self, i: usize, t: f32) -> Vec2 {
        self.segments[i][1].lerp_clamped(self.segments[i][0], t)
    }

    fn dir_at_segment(&self, seg: i32, t: f32) -> Vec2 {
        let len = self.segments.len();
        if seg >= len as i32 - 1 {
            return self.segments[len - 2][0].direction_to(self.segments[len - 1][0]);
        }
        let seg = seg as usize;
        self.lerped(seg, t).direction_to(self.lerped(seg + 1, t))
    }

    fn direction(&self, f: f32, t: f32) -> Vec2 {
        let len = self.segments.len() as i32;
        let num = (f * (len - 1) as f32).floor() as i32;
        // The ints divide as ints: both fractions are 0 but the last segment's second.
        let amount = f.inverse_lerp((num / len) as f32, ((num + 1) / len) as f32);
        self.dir_at_segment(num, t)
            .slerp(self.dir_at_segment(num + 1, t), amount)
    }

    fn pos_along(&self, f: f32, t: f32) -> Vec2 {
        let last = self.segments.len() - 1;
        let at = f * last as f32;
        let num = (at.floor() as i32).clamp(0, last as i32) as usize;
        let next = (num + 1).min(last);
        self.lerped(num, t).lerp_clamped(
            self.lerped(next, t),
            at.inverse_lerp(num as f32, num as f32 + 1.0),
        )
    }
}

/// A neuron's draw.
pub trait CoralNeuronSprites {
    /// `DrawSprites` at `t` between ticks; `water_lights` is the room's `WaterLights` effect
    /// being above 0.
    fn draw(&self, pool: &MyceliumPool, t: f32, water_lights: bool) -> DrawList;
}

impl CoralNeuronSprites for CoralNeuron {
    fn draw(&self, pool: &MyceliumPool, t: f32, water_lights: bool) -> DrawList {
        let g = &self.geometry;
        let n = g.segments.len();
        let mut vertices = vec![Vec2::ZERO; n * 4];
        let mut vector = g.lerped(0, t);
        for i in 0..n {
            let vector2 = g.lerped(i, t);
            let normalized = (vector - vector2).normalized();
            let across = normalized.perpendicular();
            let overlap = vector.distance(vector2) / 5.0;
            let width = NeuronLook::rad(i as f32 / (n - 1) as f32);
            if i == 0 {
                vertices[0] = vector - across;
                vertices[1] = vector + across;
            } else {
                let wide = across * (1.5 * width);
                vertices[i * 4] = vector - normalized * overlap - wide;
                vertices[i * 4 + 1] = vector - normalized * overlap + wide;
            }
            if i == n - 1 {
                vertices[i * 4 + 2] = vector2 - across;
                vertices[i * 4 + 3] = vector2 + across;
            } else {
                let wide = across * (3.5 * width);
                vertices[i * 4 + 2] = vector2 + normalized * overlap - wide;
                vertices[i * 4 + 3] = vector2 + normalized * overlap + wide;
            }
            vector = vector2;
        }
        let count = n * 4;
        let ramp: Vec<Color> = (0..count)
            .map(|i| NeuronLook::mesh_color(i as f32 / (count - 1) as f32))
            .collect();
        let triangles: Cow<'static, [[u16; 3]]> = Cow::Owned(LongMesh::triangles(n));
        let uvs = LongMesh::uvs(n);
        let mut items = vec![
            DrawItem::Mesh(Mesh {
                index: 0,
                vertices: vertices.clone(),
                uvs: uvs.clone(),
                triangles: triangles.clone(),
                color: Color::WHITE,
                vertex_colors: ramp,
                shader: Shader::Basic,
                layer: Layer::Items,
            }),
            DrawItem::Mesh(Mesh {
                index: 1,
                vertices,
                uvs,
                triangles,
                color: Color::WHITE.with_alpha(1.0 / n as f32),
                vertex_colors: Vec::new(),
                shader: Shader::CoralNeuron,
                layer: Layer::Items,
            }),
        ];
        let mut sprite = 2;
        for (i, &id) in self.mycelia.iter().enumerate() {
            if let Some(strand) = pool.get(id).filter(|m| !m.culled) {
                let hung = g.segment_of_mycelium(i) as f32 / (n - 1) as f32;
                items.push(DrawItem::Mesh(strand.mesh(
                    sprite,
                    g.connection_pos_at(i, t),
                    t,
                    NeuronLook::mesh_color(hung),
                    0.0,
                    Layer::Items,
                )));
            }
            sprite += 1;
        }
        for (m, bump) in self.bumps.iter().enumerate() {
            let rad = NeuronLook::rad(bump.y);
            let at = g.pos_along(bump.y, t)
                + g.direction(bump.y, t).perpendicular() * (bump.x * 4.0 * rad);
            let scale = rad.inverse_lerp(0.0, 0.6);
            let ping = self.bump_pings[m][1].lerp_to(self.bump_pings[m][0], t);
            items.push(NeuronLook::disc(
                sprite,
                at,
                scale,
                NeuronLook::mesh_color(bump.y).lerp(Color::rgb(0.0, 0.0, 0.2), 0.25),
                Layer::Items,
            ));
            items.push(NeuronLook::disc(
                sprite + 1,
                at + Vec2::new(-rad * 1.5, rad * 1.5),
                scale * 0.5,
                Color::WHITE.with_alpha(0.3),
                Layer::Items,
            ));
            items.push(NeuronLook::disc(
                sprite + 2,
                at,
                scale * 0.5,
                Color::hsl(2.0 / 3.0, 1.0, 0.1 + 0.9 * ping),
                if water_lights {
                    Layer::Water
                } else {
                    Layer::Items
                },
            ));
            sprite += 3;
        }
        DrawList { items }
    }
}
