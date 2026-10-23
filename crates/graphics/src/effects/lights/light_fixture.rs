//! The sprites of the light fixtures: the `HolyFire`'s embers (`deerEyeB` in `Midground`;
//! the game's `RippleBasic` is texture times colour in the base game, so `Shader::Basic`) and the `SlimeMoldLight`'s five soft discs (`Futile_White` under
//! `LightSource` in `Foreground`). The `Redlight`'s and `HolyFire`'s lights are
//! [`rw_sim::LightSource`]s (`light_source.rs`). No draws.
//!
//! Mirrors: HolyFire.cs (HolyFireSprite.InitiateSprites, DrawSprites), SlimeMoldLight.cs (InitiateSprites, DrawSprites, ApplyPalette, AddToContainer)

use rw_assets::RoomPalette;
use rw_sim::{FloatExt, HolyFireSprite, SlimeMoldLight, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::effects::particles::slime_mold::SlimeMoldPalette;

/// An ember's sprite.
pub trait HolyFireSpriteDraw {
    /// `DrawSprites` at `t` between ticks.
    fn draw(&self, t: f32) -> DrawList;
}

/// The slime mold light's discs.
pub trait SlimeMoldLightSprites {
    /// `DrawSprites` after `ApplyPalette`: the discs sit where the layout put them (they
    /// do not move between ticks).
    fn draw(&self, palette: &RoomPalette) -> DrawList;
}

impl HolyFireSpriteDraw for HolyFireSprite {
    fn draw(&self, t: f32) -> DrawList {
        let life = self.last_life.lerp(self.life, t);
        let color = Color::hsl(
            0.01_f32.lerp(0.08, life),
            1.0,
            0.5_f32.lerp(1.0, life.mathf_pow(3.0)),
        );
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "deerEyeB",
                pos: self.last_pos.lerp(self.pos, t),
                rotation: 0.0,
                scale: Vec2::new(life, life),
                anchor: Vec2::new(0.5, 0.5),
                color,
                shader: Shader::Basic,
                layer: Layer::Midground,
            })],
        }
    }
}

impl SlimeMoldLightSprites for SlimeMoldLight {
    fn draw(&self, palette: &RoomPalette) -> DrawList {
        // `Custom.Saturate(SlimeMoldColorFromPalette(palette), 1)`: stretched to the full
        // range of its channels.
        let c = palette.slime_mold_color();
        let (lo, hi) = (c.r.min(c.g).min(c.b), c.r.max(c.g).max(c.b));
        let color = Color::rgb(
            c.r.inverse_lerp(lo, hi),
            c.g.inverse_lerp(lo, hi),
            c.b.inverse_lerp(lo, hi),
        );
        let items = (0..5)
            .map(|i| {
                let [size, alpha] = self.sizes_and_alphas[i];
                let offset = self.positions[i] * (self.random_seed as f32 * 2.0);
                DrawItem::Sprite(Sprite {
                    index: i,
                    element: "Futile_White",
                    pos: self.pos + offset,
                    rotation: 0.0,
                    scale: Vec2::new(size / 16.0, size / 16.0),
                    anchor: Vec2::new(0.5, 0.5),
                    color: color.with_alpha(alpha * palette.darkness.mathf_pow(0.5)),
                    shader: Shader::LightSource,
                    layer: Layer::Foreground,
                })
            })
            .collect();
        DrawList { items }
    }
}

#[cfg(test)]
#[path = "tests/light_fixture.rs"]
mod tests;
