//! `Bubble.DrawSprites`: an air bubble in the water.
//!
//! Mirrors: Bubble.cs

use rw_sim::{Bubble, Vec2};

use crate::core::draw::{Color, DrawItem, Layer, Shader, Sprite};
use crate::core::math::Mathf;

/// `Bubble.DrawSprites`.
pub(crate) trait AirBubbleSprite {
    fn sprite(&self, t: f32) -> DrawItem;
}

impl AirBubbleSprite for Bubble {
    /// `LizardBubble5` in `Midground`, scaled `size * fullSize * 0.45`, in the
    /// `(0, 1/255, 0)` the water's shader shows as lit water (`backgroundBubble`, set
    /// only by More Slugcats' room scripts, is not ported: D11).
    fn sprite(&self, t: f32) -> DrawItem {
        let scale = self.last_size.lerp_to(self.size, t) * self.full_size * 0.45;
        DrawItem::Sprite(Sprite {
            index: 0,
            element: "LizardBubble5",
            pos: self.last_pos.lerp(self.pos, t),
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::rgb(0.0, 1.0 / 255.0, 0.0),
            shader: Shader::default(),
            layer: Layer::Midground,
        })
    }
}

#[cfg(test)]
#[path = "tests/bubble.rs"]
mod tests;
