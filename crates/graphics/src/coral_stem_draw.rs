//! A `StemSegment`'s sprites in `Items`: its strands, a disc bridging it to the next segment,
//! two meshes along a curve through its chunks (base colour and highlight, drawn with the
//! neighbours' ends) and per dot three `Circle20` (base, highlight and a glow that blinks
//! blue, moved to `Water` in a room with `WaterLights`). The segments themselves are
//! `rw-sim`'s.
//!
//! Mirrors: CoralBrain/StemSegment.cs (BaseColor, HighLightColor, InitiateSprites, DrawSprites, ApplyPalette, AddToContainer, ConnectionPos)

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::{CoralStem, MyceliumPool, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::math::{CustomVec, Mathf};
use crate::mycelium::{LongMesh, MyceliumSprites};

/// What a segment's draw reads of the frame.
#[derive(Debug, Clone, Copy)]
pub struct StemFrame {
    /// Between ticks.
    pub t: f32,
    /// `room.Darkness(firstChunk.pos)`.
    pub darkness: f32,
    /// The room's `WaterLights` effect is above 0.
    pub water_lights: bool,
    /// The frame's camera position: a sprite the draw never places stays at the screen's
    /// corner.
    pub cam_pos: Vec2,
}

impl StemFrame {
    /// `BaseColor`.
    #[must_use]
    pub fn base_color(&self, size: f32) -> Color {
        let darkness = self.darkness;
        Color::hsl(
            0.025,
            0.4f32.lerp_to(0.1, rw_sim::FloatExt::mathf_pow(size, 0.5)),
            0.05f32.lerp_to(
                0.7 - 0.5 * darkness,
                rw_sim::FloatExt::mathf_pow(size, 0.45),
            ),
        )
        .lerp(
            Color::rgb(0.0, 0.0, 0.1),
            rw_sim::FloatExt::mathf_pow(size.inverse_lerp(0.45, -0.05), 0.9) * 0.5,
        )
    }

    /// `HighLightColor`.
    #[must_use]
    pub fn highlight_color(&self, size: f32) -> Color {
        let darkness = self.darkness;
        Color::hsl(
            0.025,
            0.5f32.lerp_to(0.1, rw_sim::FloatExt::mathf_pow(size, 0.5)),
            0.15f32.lerp_to(
                0.85 - 0.65 * darkness,
                rw_sim::FloatExt::mathf_pow(size, 0.45),
            ),
        )
        .lerp(
            Color::rgb(0.0, 0.0, 0.15),
            rw_sim::FloatExt::mathf_pow(size.inverse_lerp(0.45, -0.05), 0.9) * 0.4,
        )
    }

    fn disc(
        index: usize,
        pos: Vec2,
        scale: Vec2,
        color: Color,
        layer: Layer,
        rotation: f32,
    ) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "Circle20",
            pos,
            rotation,
            scale,
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader: Shader::Basic,
            layer,
        })
    }
}

/// A stem segment's draw.
pub trait StemSegmentSprites {
    /// `DrawSprites` of segment `k` of the stem at `frame.t` (its dots' blinks draw from
    /// `rng`).
    fn draw_segment(
        &self,
        k: usize,
        pool: &MyceliumPool,
        frame: &StemFrame,
        rng: &mut UnityRandom,
    ) -> DrawList;
}

impl StemSegmentSprites for CoralStem {
    fn draw_segment(
        &self,
        k: usize,
        pool: &MyceliumPool,
        frame: &StemFrame,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let t = frame.t;
        let s = &self.segments[k];
        let (c0, c1) = (&s.body.chunks[0], &s.body.chunks[1]);
        let distance = s.body.connections[0].distance;
        let m = s.mycelia.len();
        let mut items = Vec::new();
        let hang = s.hang().last_pos.lerp_clamped(s.hang().pos, t);
        for (i, &id) in s.mycelia.iter().enumerate() {
            if let Some(strand) = pool.get(id).filter(|x| !x.culled) {
                items.push(DrawItem::Mesh(strand.mesh(
                    i,
                    hang,
                    t,
                    Color::rgb(0.0, 0.0, 0.1),
                    0.0,
                    Layer::Items,
                )));
            }
        }
        let mut vector = c0.last_pos.lerp_clamped(c0.pos, t);
        let mut vector2 = c1.last_pos.lerp_clamped(c1.pos, t);
        let (mut c_a, mut c_b) = (vector, vector2);
        let vector3;
        if k > 0 {
            let p = &self.segments[k - 1].body.chunks[1];
            let prev = p.last_pos.lerp_clamped(p.pos, t);
            vector3 = prev.direction_to(vector);
            vector = vector.lerp_clamped(prev, 0.35);
            c_a = vector + vector3 * (distance * 0.45);
        } else {
            vector3 = s.last_direction.lerp_clamped(s.direction, t).normalized();
            vector -= vector3 * (distance * 0.15);
        }
        let vector5;
        let mut bridge_pos = frame.cam_pos;
        let mut bridge_scale = Vec2::new(c1.rad / 20.0, 1.0);
        let mut bridge_rotation = 0.0;
        if let Some(next) = self.segments.get(k + 1) {
            let n = &next.body.chunks[0];
            let vector4 = n.last_pos.lerp_clamped(n.pos, t);
            vector5 = vector2.direction_to(vector4);
            vector2 = vector2.lerp_clamped(vector4, 0.35);
            c_b = vector2 - vector5 * (distance * 0.45);
            bridge_pos = vector2.lerp_clamped(vector4, 0.5);
            bridge_scale.y = vector2.distance(vector4) / 20.0;
            bridge_rotation = vector2.aim_to(vector4);
        } else {
            vector5 = s.last_direction.lerp_clamped(s.direction, t).normalized();
            vector2 += vector5 * (distance * 0.15);
        }
        items.push(StemFrame::disc(
            m,
            bridge_pos,
            bridge_scale,
            Color::hsl(0.025, 1.0, 0.1),
            Layer::Items,
            bridge_rotation,
        ));
        let segs = s.mesh_segs;
        let mut base = vec![Vec2::ZERO; segs * 4];
        let mut high = vec![Vec2::ZERO; segs * 4];
        let mut vector6 = vector;
        let mut v = vector3;
        let mut num = 0.5f32;
        let b = vector.bezier(c_a, vector2, c_b, 0.5);
        for j in 1..=segs {
            let num2 = j as f32 / (segs - 1) as f32;
            let pi = std::f32::consts::PI;
            let (rise, fall) = (num2.inverse_lerp(0.0, 0.3), num2.inverse_lerp(0.7, 1.0));
            let mut num3 = 0.6
                + 0.5 * (1.0 - (num2 * pi).mathf_sin())
                + 0.3 * (rise * pi).mathf_sin().max((fall * pi).mathf_sin());
            if num2 == 1.0 {
                num3 = 0.5;
            }
            let vector7 = vector.bezier(c_a, vector2, c_b, num2);
            let vector8 = if num2 < 1.0 {
                vector7.direction_to(vector.bezier(
                    c_a,
                    vector2,
                    c_b,
                    (j + 1) as f32 / (segs - 1) as f32,
                ))
            } else {
                vector5
            };
            let vector9 = v.perpendicular() * (c0.rad * (num3 + num) * 0.5);
            let vector10 = vector8.perpendicular() * (c0.rad * num3);
            base[j * 4 - 4] = (vector6 + vector7) / 2.0 + vector9;
            base[j * 4 - 3] = (vector6 + vector7) / 2.0 - vector9;
            base[j * 4 - 2] = vector7 + vector10;
            base[j * 4 - 1] = vector7 - vector10;
            for kk in 1..5 {
                high[j * 4 - kk] = if kk < 2 {
                    base[j * 4 - kk].lerp_clamped(vector7.lerp_clamped(b, 0.5), 0.5)
                        + Vec2::new(-num3 * 0.25, num3 * 0.25) * c0.rad
                } else {
                    base[j * 4 - kk].lerp_clamped(vector6.lerp_clamped(b, 0.5), 0.5)
                        + Vec2::new(-num * 0.25, num * 0.25) * c0.rad
                };
            }
            vector6 = vector7;
            v = vector8;
            num = num3;
        }
        let triangles: Cow<'static, [[u16; 3]]> = Cow::Owned(LongMesh::triangles(segs));
        let uvs = LongMesh::uvs(segs);
        for (index, vertices, color) in [
            (m + 1, base, frame.base_color(s.size)),
            (m + 2, high, frame.highlight_color(s.size)),
        ] {
            items.push(DrawItem::Mesh(Mesh {
                index,
                vertices,
                uvs: uvs.clone(),
                triangles: triangles.clone(),
                color,
                vertex_colors: Vec::new(),
                shader: Shader::Basic,
                layer: Layer::Items,
            }));
        }
        let mut sprite = m + 3;
        let dots = s.dots;
        let dot_scale = 1.2f32.lerp_to(7.0, s.size) / 20.0;
        let offset = 1.0f32.lerp_to(1.5, s.size);
        for l in 0..dots {
            let at = if dots == 1 {
                0.5
            } else {
                (l as f32 / (dots - 1) as f32).lerp_to(
                    0.5,
                    rw_sim::FloatExt::lerp_map(dots as f32, 1.0, 5.0, 0.6, 0.35),
                )
            };
            let p = vector.bezier(c_a, vector2, c_b, at);
            let glow = if rng.value() < 0.5 {
                Color::hsl(2.0 / 3.0, 1.0, 0.1 + 0.4 * rng.value())
            } else {
                Color::rgb(0.0, 0.0, 0.1)
            };
            for (n, color) in [
                (0, frame.base_color(s.size)),
                (1, frame.highlight_color(s.size)),
                (2, glow),
            ] {
                let (dx, dy) = match n {
                    0 => (-offset, offset),
                    1 => (offset, -offset),
                    _ => (0.0, 0.0),
                };
                let scale = dot_scale * if n == 2 { 0.9 } else { 1.0 };
                let layer = if n == 2 && frame.water_lights {
                    Layer::Water
                } else {
                    Layer::Items
                };
                items.push(StemFrame::disc(
                    sprite + n,
                    p + Vec2::new(dx, dy),
                    Vec2::new(scale, scale),
                    color,
                    layer,
                    0.0,
                ));
            }
            sprite += 3;
        }
        DrawList { items }
    }
}
