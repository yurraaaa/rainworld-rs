//! `Splash.DrawSprites`: a splash's two `WaterSplash` quads.
//!
//! Mirrors: SplashWater/Splash.cs (DrawSprites, InitiateSprites, AddToContainer)

use rw_sim::{Splash, Vec2};

use crate::core::draw::{Color, DrawItem, Layer, Shader, Sprite};
use crate::core::math::{CustomVec, Mathf};

/// `Splash.DrawSprites`.
pub(crate) trait SplashSprites {
    fn sprites(&self, t: f32) -> Vec<DrawItem>;
}

impl SplashSprites for Splash {
    /// Two `Futile_White` quads on the `WaterSplash` shader, anchored at their bottom
    /// edge on the splash and stretched to the linger point (`rad / 8` wide, `distance
    /// / 16` long), at half the life's alpha (clamped as `FNode.alpha`). Sprite 0, colour
    /// `(0, 0, 0)`, is in `Water`; sprite 1, `(0, 0, 1)` (the shader's foam), in
    /// `GrabShaders`.
    fn sprites(&self, t: f32) -> Vec<DrawItem> {
        let pos = self.last_pos.lerp(self.pos, t);
        let linger = self.last_linger_pos.lerp(self.linger_pos, t);
        let scale = Vec2::new(
            self.last_rad.lerp_to(self.rad, t) / 8.0,
            pos.distance(linger) / 16.0,
        );
        let alpha = (0.5 * self.last_life.lerp_to(self.life, t)).clamp(0.0, 1.0);
        [Layer::Water, Layer::GrabShaders]
            .into_iter()
            .enumerate()
            .map(|(i, layer)| {
                DrawItem::Sprite(Sprite {
                    index: i,
                    element: "Futile_White",
                    pos,
                    rotation: pos.aim_to(linger),
                    scale,
                    anchor: Vec2::new(0.5, 0.0),
                    color: Color::rgb(0.0, 0.0, i as f32).with_alpha(alpha),
                    shader: Shader::WaterSplash,
                    layer,
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "tests/splash.rs"]
mod tests;
