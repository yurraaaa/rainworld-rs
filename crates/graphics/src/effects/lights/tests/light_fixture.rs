use rw_assets::RoomPalette;
use rw_random::UnityRandom;
use rw_sim::{FloatExt, HolyFireSprite, SlimeMoldLight, Vec2};

use super::{HolyFireSpriteDraw, SlimeMoldLightSprites};
use crate::core::draw::{Color, Layer, Shader};

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
