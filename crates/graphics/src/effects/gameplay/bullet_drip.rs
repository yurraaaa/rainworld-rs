//! A `BulletDrip`'s two sprites: a thin streak (`Futile_White`, `scaleX` 0.125, anchored at
//! its foot) on the `BulletRain` shader in `GrabShaders`, from the tip to the falling head,
//! and the `RainSplash` at the tip in `Items`, sized by `Sin(falling * pi) * 0.4` and
//! turned by a draw of the cosmetic stream (D1) each drawn frame.
//!
//! Mirrors: BulletDrip.cs (InitiateSprites, DrawSprites, AddToContainer)

use rw_random::UnityRandom;
use rw_sim::world::BulletDrip;
use rw_sim::{FloatExt, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

pub trait BulletDripSprites {
    /// `DrawSprites` at `time_stacker` between ticks.
    fn draw(&self, time_stacker: f32, rng: &mut UnityRandom) -> DrawList;
}

impl BulletDripSprites for BulletDrip {
    fn draw(&self, time_stacker: f32, rng: &mut UnityRandom) -> DrawList {
        let tip = self.tip(time_stacker);
        let head = self.head(time_stacker);
        let falling = self.last_falling.lerp(self.falling, time_stacker);
        let streak = DrawItem::Sprite(Sprite {
            index: 0,
            element: "Futile_White",
            pos: tip,
            rotation: tip.to_aim(head),
            scale: Vec2::new(0.125, tip.distance(head) / 16.0),
            anchor: Vec2::new(0.5, 0.0),
            color: Color::WHITE,
            shader: Shader::BulletRain,
            layer: Layer::GrabShaders,
        });
        let size = (falling * std::f32::consts::PI).sin() * 0.4;
        let splash = DrawItem::Sprite(Sprite {
            index: 1,
            element: "RainSplash",
            pos: tip,
            rotation: rng.value() * 360.0,
            scale: Vec2::new(size, size),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::WHITE,
            shader: Shader::Basic,
            layer: Layer::Items,
        });
        DrawList {
            items: vec![streak, splash],
        }
    }
}

#[cfg(test)]
#[path = "tests/bullet_drip.rs"]
mod tests;
