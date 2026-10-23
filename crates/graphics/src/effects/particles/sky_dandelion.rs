//! `SkyDandelions.SkyDandelion`'s and `DandelionStalk`'s sprites. A dandelion is one
//! `SkyDandelion` sprite, in `Items` in the play layer and in `Foreground` on a depth
//! (shown by `CustomDepth` against the sky only, its alpha 0 being depth 1), with a
//! second, unscaled one in `Shadows` offset along the light for a depth below 0. The
//! stalk is a long mesh of 0.5 px half-width in `Midground`, bent by a Bezier from its
//! root to the dandelion (or its loose tip) and shaded from palette texel (5, 2) to white.
//!
//! Mirrors: SkyDandelions.cs (SkyDandelion.InitiateSprites, DrawSprites, ApplyPalette, AddToContainer; DandelionStalk.InitiateSprites, DrawSprites, ApplyPalette)

use std::borrow::Cow;

use rw_sim::{DandelionStalk, FloatExt, SkyDandelion, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::superstructure::coral::mycelium::LongMesh;

/// The dandelion's sprites.
pub trait DandelionSprites {
    /// `DrawSprites` at `t` between ticks in a palette whose fog colour is `fog` (the
    /// sprite is `Lerp(fog, white, 0.5)`), the room's `lightAngle` for the shadow.
    /// Nothing once destroyed.
    fn draw(&self, t: f32, fog: Color, light_angle: Vec2) -> DrawList;
}

impl DandelionSprites for SkyDandelion {
    fn draw(&self, t: f32, fog: Color, light_angle: Vec2) -> DrawList {
        if self.slated {
            return DrawList::default();
        }
        let at = self.last_pos.lerp(self.pos, t);
        let health = self.last_health.lerp(self.health, t);
        let scale = health.max(0.0).mathf_pow(0.2) / (self.depth * 0.6 + 1.0);
        let mut body = Sprite {
            index: 0,
            element: "SkyDandelion",
            pos: at,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color: fog.lerp(Color::WHITE, 0.5),
            shader: Shader::Basic,
            layer: if self.in_play_layer() {
                Layer::Items
            } else {
                Layer::Foreground
            },
        };
        if self.depth > 0.0 {
            body.shader = Shader::CustomDepth;
            body.color = body.color.with_alpha(0.0);
        }
        let mut items = vec![DrawItem::Sprite(body)];
        if self.depth < 0.0 {
            let k = self.depth.inverse_lerp(0.0, -0.5) * 15.0;
            items.push(DrawItem::Sprite(Sprite {
                index: 1,
                pos: at + Vec2::new(light_angle.x * k, -light_angle.y * k),
                scale: Vec2::new(1.0, 1.0),
                color: Color::rgb(1.0 / 255.0, 0.0, 0.0),
                layer: Layer::Shadows,
                shader: Shader::Basic,
                ..body
            }));
        }
        DrawList { items }
    }
}

/// The stalk's mesh.
pub trait StalkSprites {
    /// `DrawSprites` at `t`: the Bezier from the root to the dandelion (`dandelion`,
    /// its position at `t`) or the loose tip; `root_color` is palette texel (5, 2).
    fn draw(&self, t: f32, dandelion: Option<Vec2>, root_color: Color) -> DrawList;
}

impl StalkSprites for DandelionStalk {
    fn draw(&self, t: f32, dandelion: Option<Vec2>, root_color: Color) -> DrawList {
        let segs = self.segs;
        let head = match (self.dandelion, dandelion) {
            (Some(_), Some(d)) => d,
            _ => self.last_tip.lerp(self.tip, t),
        };
        let straight = 1.0 - self.last_straight_stalk.lerp(self.straight_stalk, t);
        let lean = self.stalk_dir * (self.stalk_length * 0.5 * straight);
        let half = 0.5;
        let mut previous = self.pos;
        let mut vertices = vec![Vec2::ZERO; segs * 4];
        for i in 0..segs {
            let f = i as f32 / (segs - 1) as f32;
            let next = self.pos.bezier(self.pos + lean, head, head - lean, f);
            let dir = (previous - next).normalized();
            let side = dir.perpendicular();
            let overlap = previous.distance(next) / 5.0;
            vertices[i * 4] = previous - dir * overlap - side * half;
            vertices[i * 4 + 1] = previous - dir * overlap + side * half;
            vertices[i * 4 + 2] = next + dir * overlap - side * half;
            vertices[i * 4 + 3] = next + dir * overlap + side * half;
            previous = next;
        }
        let last = (segs * 4 - 1) as f32;
        let vertex_colors = (0..segs * 4)
            .map(|i| root_color.lerp(Color::WHITE, i as f32 / last))
            .collect();
        DrawList {
            items: vec![DrawItem::Mesh(Mesh {
                index: 0,
                vertices,
                uvs: LongMesh::uvs(segs),
                triangles: Cow::Owned(LongMesh::triangles(segs)),
                color: Color::WHITE,
                vertex_colors,
                shader: Shader::Basic,
                layer: Layer::Midground,
            })],
        }
    }
}

#[cfg(test)]
#[path = "tests/sky_dandelion.rs"]
mod tests;
