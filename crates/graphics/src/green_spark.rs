//! `GreenSparks.GreenSpark`'s sprite: a `pixel` stretched along its last step, `Items`
//! in the play layer and `Foreground` on a depth; behind the play layer its colour fades
//! toward the sky and, past it, `CustomDepth` shows it against the sky only.
//!
//! Mirrors: GreenSparks.cs (GreenSpark.InitiateSprites, DrawSprites, ApplyPalette, AddToContainer)

use rw_sim::{FloatExt, GreenSpark, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

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
mod tests {
    use rw_random::UnityRandom;
    use rw_sim::{GreenSpark, Vec2};

    use super::GreenSparkSprites;
    use crate::draw::{Color, Layer, Shader};

    /// A play-layer spark is a 1.5 px wide `pixel` in `Items`, `2 + 1.1 *` its last
    /// step long, anchored at its start and aimed along the step; one on a depth is in
    /// `Foreground` under `CustomDepth` with alpha 0, narrower, tinted toward the sky.
    #[test]
    fn spark_sprite_stretches_along_its_step() {
        let mut s = GreenSpark::new(Vec2::new(100.0, 100.0), &mut UnityRandom::new(1));
        s.depth = 0.0;
        s.last_last_pos = Vec2::new(100.0, 100.0);
        s.last_pos = Vec2::new(100.0, 100.0);
        s.pos = Vec2::new(100.0, 110.0);
        let sky = Color::rgb(0.5, 0.5, 1.0);
        let list = s.draw(1.0, sky);
        let sp = list.sprite(0).expect("sprite");
        assert_eq!((sp.layer, sp.shader), (Layer::Items, Shader::Basic));
        assert_eq!(sp.color, Color::rgb(0.0, 1.0, 1.0 / 85.0));
        assert_eq!(sp.scale, Vec2::new(1.5, 2.0 + 1.1 * 10.0));
        s.last_last_pos = Vec2::new(100.0, 100.0);
        s.last_pos = Vec2::new(110.0, 100.0);
        s.pos = Vec2::new(110.0, 100.0);
        let sp = *s.draw(0.0, sky).sprite(0).expect("sprite");
        assert_eq!(sp.scale.y, 2.0 + 1.1 * 10.0);
        assert!((sp.rotation - 90.0).abs() < 1e-4);

        s.depth = 5.0;
        let sp = *s.draw(1.0, sky).sprite(0).expect("sprite");
        assert_eq!(
            (sp.layer, sp.shader),
            (Layer::Foreground, Shader::CustomDepth)
        );
        assert!((sp.scale.x - 0.1).abs() < 1e-6);
        assert_eq!(sp.color.a, 0.0);
        assert_eq!((sp.color.r, sp.color.g), (0.0, 1.0));
    }
}
