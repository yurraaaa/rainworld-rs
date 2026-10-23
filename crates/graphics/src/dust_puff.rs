//! A `DustPuff`'s sprite: a `Futile_White` disc with the `Spores` shader in the
//! `Background` container, in the palette's texel (9, 5), growing and fading with its
//! life.
//!
//! Mirrors: RoofTopView.cs (DustPuff: InitiateSprites, DrawSprites, ApplyPalette)

use rw_sim::{DustPuff, FloatExt, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// The puff's sprite.
pub trait DustPuffSprites {
    /// `DrawSprites` at `time_stacker` between ticks, in `color` (`ApplyPalette`'s
    /// `palette.texture.GetPixel(9, 5)`).
    fn draw(&self, time_stacker: f32, color: Color) -> DrawList;
}

impl DustPuffSprites for DustPuff {
    fn draw(&self, time_stacker: f32, color: Color) -> DrawList {
        let life = FloatExt::lerp(self.last_life, self.life, time_stacker);
        let grow = FloatExt::mathf_pow(1.0 - life, 0.35);
        let scale = 10.0 * grow * FloatExt::lerp(0.5, 1.5, self.size);
        let alpha = color.a * life;
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "Futile_White",
                pos: self.last_pos + (self.pos - self.last_pos) * time_stacker,
                rotation: 0.0,
                scale: Vec2::new(scale, scale),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::from(
                    [color.r, color.g, color.b, alpha.clamp(0.0, 1.0)]
                        .map(|c: f32| (c * 255.0).round_ties_even() / 255.0),
                ),
                shader: Shader::Spores,
                layer: Layer::Background,
            })],
        }
    }
}

#[cfg(test)]
mod tests {
    use rw_random::UnityRandom;

    use super::*;

    #[test]
    fn a_puff_grows_as_it_fades() {
        let mut p = DustPuff::new(Vec2::new(10.0, 20.0), 0.5, &mut UnityRandom::new(1));
        p.update();
        let colour = Color::rgb(0.2, 0.4, 0.6);
        let list = DustPuffSprites::draw(&p, 1.0, colour);
        let s = list.sprite(0).unwrap();
        // pos (10.25, 20.5); scale 10 * (1 - life)^0.35 * Lerp(0.5, 1.5, 0.5); alpha = life.
        assert_eq!(s.pos, Vec2::new(10.25, 20.5));
        let want = 10.0 * (1.0 / f64::from(p.life_time)).powf(0.35) as f32;
        assert!((s.scale.x - want).abs() < 1e-3, "{} {want}", s.scale.x);
        assert_eq!(s.layer, Layer::Background);
        assert_eq!(s.shader, Shader::Spores);
        let alpha = ((1.0 - 1.0 / p.life_time) * 255.0).round_ties_even() / 255.0;
        assert_eq!(s.color.a, alpha);
        // Halfway between ticks.
        let half = DustPuffSprites::draw(&p, 0.5, colour);
        assert_eq!(half.sprite(0).unwrap().pos, Vec2::new(10.125, 20.25));
    }
}
