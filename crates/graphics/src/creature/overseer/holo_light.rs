//! `HologramLight.DrawSprites`: the guide's light, a `HoloGrid` glow and nine
//! flickering lines from the overseer's eye to the terrain around it.
//!
//! Mirrors: HologramLight.cs (DrawSprites)

use rw_assets::Room;
use rw_assets::shortcut::FOUR_DIRECTIONS;
use rw_random::UnityRandom;
use rw_sim::creature::overseer::HologramLight;
use rw_sim::{FloatExt, LightHardSet, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::core::math::CustomVec;

/// The light's sprites, drawn by the camera of its room.
pub trait HologramLightSprites {
    /// `DrawSprites` at `t` (draws on the cosmetic stream): `eye` is the overseer's eye
    /// (`OverseerEyePos(t)`) while it is in the light's room, else `None`. Also what it
    /// hard-sets on its `LightSource`.
    fn draw(
        &self,
        room: &Room,
        eye: Option<Vec2>,
        t: f32,
        rng: &mut UnityRandom,
    ) -> (DrawList, LightHardSet);
}

/// The lines' colour.
const LINE_COLOR: Color = Color::rgb(0.87, 0.72, 0.35);

/// `Custom.fourDirections[i]` as a vector.
struct Direction(Vec2);

impl Direction {
    fn of(i: i32) -> Self {
        let (x, y) = FOUR_DIRECTIONS[usize::try_from(i).unwrap_or(0)];
        Self(Vec2::new(x as f32, y as f32))
    }
}

impl HologramLightSprites for HologramLight {
    fn draw(
        &self,
        room: &Room,
        eye: Option<Vec2>,
        t: f32,
        rng: &mut UnityRandom,
    ) -> (DrawList, LightHardSet) {
        let flicker = self.last_flicker.lerp(self.flicker, t);
        let power = self.last_power.lerp(self.power, t);
        let mut num2 = power * (1.0 - flicker.mathf_pow(2.0 + rng.value() * 2.0));
        let num3 = power * (1.0 - 0.05 * flicker);
        let eye = eye.unwrap_or_else(|| self.last_proj_pos.lerp(self.proj_pos, t));
        let mut b = self.last_pos.lerp(self.pos, t) + self.last_displace.lerp(self.displace, t);
        b = eye.lerp(b, num3) + self.last_push_around_pos.lerp(self.push_around_pos, t);
        num2 *= b.distance(eye).lerp_map_pow(500.0, 1200.0, 1.0, 0.75, 0.6);
        let to_eye = (eye - b).clamp_magnitude(240.0) / 240.0;
        let grow = num2 * num3;
        let mut items = vec![DrawItem::Sprite(Sprite {
            index: 0,
            element: "Futile_White",
            pos: b,
            rotation: 0.0,
            scale: Vec2::new(
                15.0.lerp(20.0, grow) + (grow * std::f32::consts::PI).sin() * 30.0,
                8.0.lerp(20.0, grow),
            ),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::rgb(
                to_eye.x.inverse_lerp(-1.0, 1.0),
                to_eye.y.inverse_lerp(-1.0, 1.0),
                num3,
            )
            .with_alpha(num2),
            shader: Shader::HoloGrid,
            layer: Layer::Water,
        })];
        let reach = 8.0 * 6.0.lerp(16.0, grow);
        let light = LightHardSet {
            pos: b,
            rad: reach * 2.0,
            alpha: num2.inverse_lerp(0.75, 1.0) * 0.1,
        };
        for i in 1..10 {
            if num2 < 0.5 {
                continue;
            }
            if let Some(line) = Self::line(room, eye, b, reach, num2, num3, rng) {
                items.push(DrawItem::Sprite(Sprite { index: i, ..line }));
            }
        }
        (DrawList { items }, light)
    }
}

/// The light's lines (sprites 1 to 9).
trait LightLine {
    /// A line from a point near `b` (snapped to a pole or a wall within `reach` of the
    /// eye) toward the eye; `None` where it is hidden.
    fn line(
        room: &Room,
        eye: Vec2,
        b: Vec2,
        reach: f32,
        num2: f32,
        num3: f32,
        rng: &mut UnityRandom,
    ) -> Option<Sprite>;
}

impl LightLine for HologramLight {
    fn line(
        room: &Room,
        eye: Vec2,
        b: Vec2,
        reach: f32,
        num2: f32,
        num3: f32,
        rng: &mut UnityRandom,
    ) -> Option<Sprite> {
        let dir = Vec2::random_unit(rng);
        let mut p = b + dir * (rng.value().mathf_pow(0.65) * reach);
        let tile = room.tile_at(p.to_array());
        let mut alpha = 0.75;
        if tile.is_solid() {
            alpha = 0.0;
        } else if rng.value() < num3.lerp_map(0.5, 1.0, 0.85, 0.25) {
            alpha = 0.75;
        } else if rng.value() < 0.5 && tile.vertical_pole() {
            let side = if rng.value() < 0.5 { -2.0 } else { 2.0 };
            p.x = Vec2::middle_of_tile(p.tile()).x + side;
            alpha = 1.0;
        } else if rng.value() < 0.5 && tile.horizontal_pole() {
            let side = if rng.value() < 0.5 { -2.0 } else { 2.0 };
            p.y = Vec2::middle_of_tile(p.tile()).y + side;
            alpha = 1.0;
        } else {
            // The game checks the one drawn direction four times.
            let d = Direction::of(rng.range_i32(0, 4)).0 * 20.0;
            if !eye.dist_less(p + d, reach) {
                alpha = 0.0;
            } else if room.tile_at((p + d).to_array()).is_solid() {
                p = Vec2::middle_of_tile((p + d).tile()) - d / 2.0;
                alpha = 1.0;
            }
        }
        if alpha <= 0.0 {
            return None;
        }
        let from = p.lerp(eye, rng.value().mathf_pow(3.0 - 1.5 * num2));
        Some(Sprite {
            index: 0,
            element: "pixel",
            pos: from,
            rotation: from.aim_to(p),
            scale: Vec2::new(1.0, from.distance(p)),
            anchor: Vec2::new(0.5, 0.0),
            color: LINE_COLOR
                .with_alpha(alpha * rng.value().mathf_pow(0.2) * num2.inverse_lerp(0.5, 0.6)),
            shader: Shader::Hologram,
            layer: Layer::Water,
        })
    }
}

#[cfg(test)]
#[path = "tests/holo_light.rs"]
mod tests;
