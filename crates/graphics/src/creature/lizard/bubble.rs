//! `LizardBubble.DrawSprites` and `StationaryEffect.DrawSprites`: a lizard's bubble and
//! its head shield's flash, in `Midground`. The flash draws on the cosmetic stream (D1).
//!
//! Mirrors: LizardBubble.cs, StationaryEffect.cs

use rw_random::UnityRandom;
use rw_sim::creature::lizard::LizardBubble;
use rw_sim::{StationaryEffect, Vec2};

use crate::core::draw::{Color, DrawItem, Layer, Shader, Sprite};
use crate::core::math::{CustomVec, Mathf};
use crate::effects::cosmetic::LizardColors;

/// `LizardBubble0` to `7`, by hollowness.
const BUBBLES: [&str; 8] = [
    "LizardBubble0",
    "LizardBubble1",
    "LizardBubble2",
    "LizardBubble3",
    "LizardBubble4",
    "LizardBubble5",
    "LizardBubble6",
    "LizardBubble7",
];

/// A lizard bubble's or flash's sprite.
trait LizardCosmeticSprite {
    fn sprite_at(
        element: &'static str,
        pos: Vec2,
        scale: Vec2,
        rotation: f32,
        anchor_y: f32,
        color: Color,
    ) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index: 0,
            element,
            pos,
            rotation,
            scale,
            anchor: Vec2::new(0.5, anchor_y),
            color,
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }
}

impl LizardCosmeticSprite for LizardBubble {}
impl LizardCosmeticSprite for StationaryEffect {}

/// `LizardBubble.DrawSprites`.
pub(crate) trait BubbleSprite {
    fn sprite(&self, t: f32, lizards: &dyn LizardColors) -> DrawItem;
}

impl BubbleSprite for LizardBubble {
    /// Fading from the module's head colour to its black over the second half of its
    /// life, hollow rings once free, stretched toward its spot on the head while stuck
    /// to it. Without its module (a room nobody views) the camera's black and the head
    /// as the bubble last read it stand in.
    fn sprite(&self, t: f32, lizards: &dyn LizardColors) -> DrawItem {
        let last = (std::f32::consts::PI * self.last_life)
            .mathf_sin()
            .lerp_to(self.last_life, 0.5);
        let now = (std::f32::consts::PI * self.life)
            .mathf_sin()
            .lerp_to(self.life, 0.5);
        let size = 0.625 * last.lerp_to(now, t);
        let pos = self.last_pos.lerp(self.pos, t);
        let module = lizards.lizard_module(self.lizard, self.module, t);
        let black = module.map_or(lizards.black(), |m| m.black);
        let head = module.map_or(black, |m| m.head_color);
        let life = self.last_life.lerp_to(self.life, t);
        let color = head.lerp(black, 1.0 - (life * 2.0).clamp(0.0, 1.0));
        let mut frame = 0;
        if self.hollow {
            let h = self.life.inverse_lerp(self.life_time_when_free, 0.0);
            frame = ((rw_sim::FloatExt::mathf_pow(h, self.hollow_ness) * 7.0) as i32).clamp(1, 7);
        }
        let element = BUBBLES[usize::try_from(frame).unwrap_or(0)];
        if self.stuck_to_origin || !self.free_floating {
            let anchor = if self.stuck_to_origin {
                let (head, rotation) = module.map_or((self.head.pos, self.head.rotation), |m| {
                    (m.head, m.rotation)
                });
                head + self.origin_point.rotate_around_origo(rotation)
            } else {
                self.last_liberated_origin.lerp(self.liberated_origin, t)
            };
            let stretch = pos.distance(anchor) / 16.0;
            let scale = Vec2::new(
                size.min(size / stretch.lerp_to(1.0, 0.35)),
                size.max(stretch - 0.3125),
            );
            let anchor_y = stretch.inverse_lerp(1.25, 0.3125) * 0.5;
            Self::sprite_at(element, pos, scale, pos.aim_to(anchor), anchor_y, color)
        } else {
            Self::sprite_at(element, pos, Vec2::new(size, size), 0.0, 0.5, color)
        }
    }
}

/// `StationaryEffect.DrawSprites` of a `FlashingOrb`.
pub(crate) trait FlashSprite {
    fn sprite(&self, t: f32, lizards: &dyn LizardColors, rng: &mut UnityRandom) -> DrawItem;
}

impl FlashSprite for StationaryEffect {
    /// A circle jittering by up to 10 px, its size random, fading with its life; in
    /// the head colour of the module it was given (grey to white without one). Three
    /// or four draws. White stands in for a module this view lacks (a room nobody
    /// views).
    fn sprite(&self, t: f32, lizards: &dyn LizardColors, rng: &mut UnityRandom) -> DrawItem {
        let life = self.last_life.lerp_to(self.life, t);
        let deg = rng.value() * 360.0;
        let pos =
            self.last_pos.lerp(self.pos, t) + Vec2::from_degrees(deg) * (rng.value() * 10.0 * life);
        let scale = rw_sim::FloatExt::mathf_pow(rng.value() * life, 0.1);
        let color = match self.lizard {
            Some((id, module)) => lizards
                .lizard_module(id, module, t)
                .map_or(Color::WHITE, |m| m.head_color),
            None => Color::GRAY.lerp(Color::WHITE, rng.value()),
        };
        Self::sprite_at("Circle20", pos, Vec2::new(scale, scale), 0.0, 0.5, color)
    }
}

#[cfg(test)]
#[path = "tests/bubble.rs"]
mod tests;
