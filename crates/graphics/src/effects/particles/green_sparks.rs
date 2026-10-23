//! `GreenSparks.GreenSpark`'s sprite: a `pixel` stretched along its last step, `Items`
//! in the play layer and `Foreground` on a depth; behind the play layer its colour fades
//! toward the sky and, past it, `CustomDepth` shows it against the sky only.
//!
//! Mirrors: GreenSparks.cs (GreenSpark.InitiateSprites, DrawSprites, ApplyPalette, AddToContainer)

use rw_sim::{FloatExt, GreenSpark, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// The spark's sprite.
pub trait GreenSparkSprites {
    /// `DrawSprites` at `t` between ticks in a palette with this sky colour. Nothing
    /// once destroyed.
    fn draw(&self, t: f32, sky: Color) -> DrawList;
}

impl GreenSparkSprites for GreenSpark {
    fn draw(&self, t: f32, sky: Color) -> DrawList {
        if self.slated {
            return DrawList::default();
        }
        let [r, g, b] = GreenSpark::COLOR;
        let col = Color::rgb(r, g, b);
        let width = if self.depth < 0.0 {
            self.depth.lerp_map(0.0, -0.5, 1.5, 2.0)
        } else if self.depth > 0.0 {
            self.depth.lerp_map(0.0, 5.0, 1.5, 0.1)
        } else {
            1.5
        };
        let from = self.last_last_pos.lerp(self.last_pos, t);
        let to = self.last_pos.lerp(self.pos, t);
        let (color, shader) = if self.depth > 0.0 {
            (
                sky.lerp(col, self.depth.inverse_lerp(0.0, 5.0))
                    .with_alpha(0.0),
                Shader::CustomDepth,
            )
        } else {
            (col, Shader::Basic)
        };
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "pixel",
                pos: self.last_pos.lerp(self.pos, t),
                rotation: (to - from).to_degrees(),
                scale: Vec2::new(width, (2.0_f32).max(2.0 + 1.1 * from.distance(to))),
                anchor: Vec2::new(0.5, 0.0),
                color,
                shader,
                layer: if self.in_play_layer() {
                    Layer::Items
                } else {
                    Layer::Foreground
                },
            })],
        }
    }
}

#[cfg(test)]
#[path = "tests/green_sparks.rs"]
mod tests;
