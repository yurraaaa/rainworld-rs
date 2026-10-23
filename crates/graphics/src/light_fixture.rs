//! The sprites of the light fixtures: the `HolyFire`'s embers (`deerEyeB` in `Midground`;
//! the game's `RippleBasic` is texture times colour in the base game, so `Shader::Basic`) and the `SlimeMoldLight`'s five soft discs (`Futile_White` under
//! `LightSource` in `Foreground`). The `Redlight`'s and `HolyFire`'s lights are
//! [`rw_sim::LightSource`]s (`light_source.rs`). No draws.
//!
//! Mirrors: HolyFire.cs (HolyFireSprite.InitiateSprites, DrawSprites), SlimeMoldLight.cs (InitiateSprites, DrawSprites, ApplyPalette, AddToContainer)

use rw_assets::RoomPalette;
use rw_sim::{FloatExt, HolyFireSprite, SlimeMoldLight, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::slime_mold::SlimeMoldPalette;

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
mod tests {
    use rw_assets::RoomPalette;
    use rw_random::UnityRandom;
    use rw_sim::{FloatExt, HolyFireSprite, SlimeMoldLight, Vec2};

    use super::{HolyFireSpriteDraw, SlimeMoldLightSprites};
    use crate::draw::{Color, Layer, Shader};

    /// A fresh ember is a full-size `deerEyeB` of `HSL2RGB(0.08, 1, 1)` (white), halfway
    /// through its life half-size and hotter-red than orange; sprites in `Midground`
    /// under the plain shader, between ticks interpolated.
    #[test]
    fn ember_shrinks_and_reddens() {
        let mut s = HolyFireSprite::new(Vec2::new(10.0, 20.0), &mut UnityRandom::new(1));
        s.vel = Vec2::new(4.0, 0.0);
        let sp = *s.draw(1.0).sprite(0).expect("sprite");
        assert_eq!(
            (sp.element, sp.layer, sp.shader),
            ("deerEyeB", Layer::Midground, Shader::Basic)
        );
        assert_eq!(
            (sp.scale, sp.pos),
            (Vec2::new(1.0, 1.0), Vec2::new(10.0, 20.0))
        );
        assert_eq!(sp.color, Color::hsl(0.08, 1.0, 1.0));
        s.last_life = 1.0;
        s.life = 0.5;
        s.last_pos = Vec2::new(0.0, 0.0);
        s.pos = Vec2::new(4.0, 0.0);
        let sp = *s.draw(0.5).sprite(0).expect("sprite");
        assert_eq!((sp.scale.x, sp.pos), (0.75, Vec2::new(2.0, 0.0)));
        let want = Color::hsl(
            0.01_f32.lerp(0.08, 0.75),
            1.0,
            0.5_f32.lerp(1.0, 0.75_f32.powi(3)),
        );
        assert_eq!(sp.color, want);
    }

    /// Five `Futile_White` discs in `Foreground` under `LightSource`, each `random_seed * 2`
    /// times its unit offset from the fixture, sized `size / 16`, alpha scaled by the
    /// palette darkness' square root, the slime colour stretched to its full range.
    #[test]
    fn slime_light_discs() {
        let white = [1.0, 1.0, 1.0, 1.0];
        let p = RoomPalette {
            sky_color: white,
            fog_color: white,
            black_color: [0.0, 0.0, 0.0, 1.0],
            water_color1: white,
            water_color2: white,
            water_surface_color1: white,
            water_surface_color2: white,
            water_shine_color: white,
            fog_amount: 0.0,
            darkness: 0.25,
            shortcut_colors: [white; 3],
            shortcut_symbol: white,
        };
        let mut light = SlimeMoldLight::new(2, Vec2::new(100.0, 50.0), 30);
        light.positions[3] = Vec2::new(0.5, -0.25);
        light.sizes_and_alphas[3] = [80.0, 0.3];
        let list = light.draw(&p);
        assert_eq!(list.items.len(), 5);
        let sp = *list.sprite(3).expect("sprite");
        assert_eq!(
            (sp.layer, sp.shader, sp.element),
            (Layer::Foreground, Shader::LightSource, "Futile_White")
        );
        assert_eq!(sp.pos, Vec2::new(100.0 + 0.5 * 60.0, 50.0 - 0.25 * 60.0));
        assert_eq!(sp.scale, Vec2::new(5.0, 5.0));
        assert!((sp.color.a - 0.3 * 0.5).abs() < 1e-6);
        let hi = sp.color.r.max(sp.color.g).max(sp.color.b);
        let lo = sp.color.r.min(sp.color.g).min(sp.color.b);
        assert_eq!((hi, lo), (1.0, 0.0));
    }
}
