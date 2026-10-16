//! `Spark.DrawSprites` (40745) and `ExplosionSpikes.DrawSprites` (34935): a white
//! streak in `Midground`, and a ring of fading spikes in `Foreground`. No draws.

use std::borrow::Cow;

use rw_assets::Room;
use rw_sim::{Cosmetic, CosmeticKind, ExplosionSpikes, RoomExt, Spark};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader};
use crate::math::Mathf;

const STREAK: [[u16; 3]; 1] = [[0, 1, 2]];

/// The sprites of a spark or a ring of spikes.
pub trait CosmeticSprites {
    /// `DrawSprites` at `t` between ticks in `room`.
    fn draw(&self, room: &Room, t: f32) -> DrawList;
}

impl CosmeticSprites for Cosmetic {
    fn draw(&self, room: &Room, t: f32) -> DrawList {
        let item = match &self.kind {
            CosmeticKind::Spark(s) => s.mesh(room, t),
            CosmeticKind::Spikes(s) => s.mesh(room, t),
        };
        DrawList {
            items: vec![DrawItem::Mesh(item)],
        }
    }
}

/// A sprite's colour from the sim's RGBA.
trait Rgba {
    fn color(self) -> Color;
}

impl Rgba for [f32; 4] {
    fn color(self) -> Color {
        Color {
            r: self[0],
            g: self[1],
            b: self[2],
            a: self[3],
        }
    }
}

/// A spark's or the spikes' triangles.
trait CosmeticMesh {
    fn mesh(&self, room: &Room, t: f32) -> Mesh;
}

impl CosmeticMesh for Spark {
    /// From the lerped position back toward the position three ticks ago (at least
    /// 9 px), shrinking to a point over the last tenth of its life; 2 px wide.
    fn mesh(&self, _room: &Room, t: f32) -> Mesh {
        let head = self.last_pos.lerp(self.pos, t);
        let mut tail = self.last_last_last_pos.lerp(self.last_last_pos, t);
        if head.dist_less(tail, 9.0) {
            tail = head + head.direction_to(tail) * 9.0;
        }
        tail = head.lerp(tail, self.life.inverse_lerp(0.0, 0.1));
        let side = (head - tail).normalized().perpendicular();
        Mesh {
            index: 0,
            vertices: vec![head + side, head - side, tail],
            uvs: Vec::new(),
            triangles: Cow::Borrowed(&STREAK),
            color: self.color.color(),
            vertex_colors: Vec::new(),
            shader: Shader::Basic,
            layer: Layer::Midground,
        }
    }
}

impl CosmeticMesh for ExplosionSpikes {
    /// Each spike grows from a tenth of its length and narrows as it fades over its
    /// lifetime, white at first; a tip inside a Solid tile fades twice as fast.
    fn mesh(&self, room: &Room, t: f32) -> Mesh {
        let value = self.time as f32 + t;
        let mut vertices = Vec::new();
        let mut colors = Vec::new();
        let mut triangles = Vec::new();
        let white = Color::WHITE;
        let base = self.color.color();
        for (i, (v, dir)) in self.spikes.iter().enumerate() {
            let num = value.inverse_lerp(0.0, v[2]);
            let mut num2 = if self.time == 0 {
                t
            } else {
                value.inverse_lerp(v[2], 0.0)
            };
            let num3 = (v[0] * 0.1).lerp_to(v[0], num.powf(0.45));
            let num4 =
                v[1] * (0.5 + 0.5 * (num * std::f32::consts::PI).mathf_sin()) * num2.powf(0.3);
            let col = base.lerp(white, num2.powf(0.2f32.lerp_to(1.5, num)));
            let tip = self.pos + *dir * (self.inner_rad + num3);
            if room.tile_at_pos(tip).is_solid() {
                num2 *= 0.5;
            }
            let root = self.pos + *dir * (self.inner_rad + num3 * 0.1);
            let side = (tip - root).perpendicular();
            vertices.extend([tip, root - side * (num4 * 0.5), root + side * (num4 * 0.5)]);
            colors.extend([
                col.with_alpha(num2.powf(0.75)),
                col.with_alpha(0.0),
                col.with_alpha(0.0),
            ]);
            let k = u16::try_from(i * 3).unwrap_or(0);
            triangles.push([k, k + 1, k + 2]);
        }
        Mesh {
            index: 0,
            vertices,
            uvs: Vec::new(),
            triangles: Cow::Owned(triangles),
            color: white,
            vertex_colors: colors,
            shader: Shader::Basic,
            layer: Layer::Foreground,
        }
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;
    use rw_sim::{Spark, Vec2};

    use super::CosmeticMesh;

    #[test]
    fn a_new_spark_streaks_at_least_nine_pixels() {
        let mut rng = UnityRandom::new(1);
        let s = Spark::new(
            Vec2::new(50.0, 50.0),
            Vec2::new(5.0, 0.0),
            [1.0; 4],
            6,
            18,
            &mut rng,
        );
        let room = crate::tests::room(&["...."]);
        let m = s.mesh(&room, 0.0);
        assert_eq!(m.vertices.len(), 3);
        assert!((m.vertices[2] - Vec2::new(50.0, 50.0)).length() > 8.99);
    }
}
