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

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::mycelium::LongMesh;

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
mod tests {
    use rw_random::UnityRandom;
    use rw_sim::{DandelionStalk, SkyDandelion, Vec2};

    use super::{DandelionSprites, StalkSprites};
    use crate::draw::{Color, DrawItem, Layer, Shader};
    use crate::tests::room;

    /// A play-layer dandelion is one `Items` sprite in `Lerp(fog, white, 0.5)`; a
    /// farther one is in `Foreground` under `CustomDepth` with alpha 0 and scaled
    /// `1 / (0.6 * depth + 1)`; one below 0 adds the unscaled red shadow sprite pushed
    /// along the light angle by `15 * InverseLerp(0, -0.5, depth)`.
    #[test]
    fn dandelion_layers_shaders_and_shadow() {
        let fog = Color::rgb(0.2, 0.4, 0.6);
        let angle = Vec2::new(0.5, 0.8);
        let mut d = SkyDandelion::new(Vec2::new(100.0, 100.0), true, &mut UnityRandom::new(1));
        let list = d.draw(0.5, fog, angle);
        let s = list.sprite(0).expect("body");
        assert_eq!((s.layer, s.shader), (Layer::Items, Shader::Basic));
        assert_eq!(s.color, fog.lerp(Color::WHITE, 0.5));
        assert_eq!(list.items.len(), 1);

        d.depth = 5.0;
        let list = d.draw(0.5, fog, angle);
        let s = list.sprite(0).expect("body");
        assert_eq!(
            (s.layer, s.shader),
            (Layer::Foreground, Shader::CustomDepth)
        );
        assert_eq!((s.color.a, s.scale.x), (0.0, 1.0 / 4.0));

        d.depth = -0.25;
        let list = d.draw(0.5, fog, angle);
        let shadow = list.sprite(1).expect("shadow");
        assert_eq!(shadow.layer, Layer::Shadows);
        assert_eq!(shadow.scale, Vec2::new(1.0, 1.0));
        assert_eq!(shadow.pos, Vec2::new(100.0 + 0.5 * 7.5, 100.0 - 0.8 * 7.5));
        assert_eq!(shadow.color, Color::rgb(1.0 / 255.0, 0.0, 0.0));
        d.slated = true;
        assert!(d.draw(0.5, fog, angle).items.is_empty());
    }

    /// The stalk is one `Midground` long mesh of four vertices a segment, shaded from
    /// the root colour to white, whose last vertices sit by the dandelion.
    #[test]
    fn stalk_mesh_runs_from_root_to_dandelion() {
        let rows = ["..........."; 12];
        let room = room(&rows);
        let mut d = SkyDandelion::new(Vec2::new(110.0, 100.0), true, &mut UnityRandom::new(1));
        d.update_order = 3;
        let mut stalk = DandelionStalk::new(&d, &room, 0, &mut UnityRandom::new(1));
        stalk.dandelion = Some(3);
        let root = Color::rgb(0.1, 0.2, 0.3);
        let list = stalk.draw(0.5, Some(d.pos), root);
        let DrawItem::Mesh(m) = &list.items[0] else {
            panic!("mesh");
        };
        assert_eq!(m.layer, Layer::Midground);
        assert_eq!(m.vertices.len(), stalk.segs * 4);
        assert_eq!(m.vertex_colors[0], root);
        assert_eq!(*m.vertex_colors.last().expect("colours"), Color::WHITE);
        assert!(m.vertices[m.vertices.len() - 1].distance(d.pos) < 5.0);
    }
}
