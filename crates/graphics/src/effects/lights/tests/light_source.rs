use rw_sim::{LightHardSet, LightSource, Vec2};

use super::{LightSourceSprites, LightSourceView};
use crate::core::draw::{DrawItem, Layer, Shader};

fn view(water: bool) -> LightSourceView {
    LightSourceView {
        cam_pos: Vec2::new(10.25, 20.0),
        darkness: 0.9,
        darken_lights: 0.0,
        water,
        water_lights: false,
        shader_light: 1.0,
    }
}

fn disc_alpha(list: &crate::core::draw::DrawList) -> f32 {
    let DrawItem::Sprite(s) = &list.items[0] else {
        panic!("a sprite");
    };
    s.color.a
}

fn light() -> LightSource {
    let mut l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
    l.hard_set(LightHardSet {
        pos: Vec2::new(100.7, 50.2),
        rad: 160.0,
        alpha: 0.1,
    });
    l
}

/// The guide light's disc: at the floored stage position plus half a pixel, scale
/// rad / 8, alpha `alpha * Lerp(1, darkness, 1) * colorAlpha` in `ForegroundLights`.
#[test]
fn disc_in_foreground_lights() {
    let list = light().draw(&view(false), 0.3);
    assert_eq!(list.items.len(), 1);
    let DrawItem::Sprite(s) = &list.items[0] else {
        panic!("a sprite");
    };
    assert_eq!(s.shader, Shader::LightSource);
    assert_eq!(s.layer, Layer::ForegroundLights);
    assert_eq!(s.pos, Vec2::new(90.5 + 10.25, 30.5 + 20.0));
    assert_eq!(s.scale, Vec2::new(20.0, 20.0));
    assert!((s.color.a - 0.09).abs() < 1e-6);
    assert_eq!((s.color.r, s.color.g, s.color.b), (1.0, 0.8, 0.3));
}

/// A `RedlightLight` (the `LightMask0` element) draws the same disc over that element,
/// whose 128 px make it `16 * rad` wide.
#[test]
fn redlight_light_uses_its_own_element() {
    let mut l = light();
    l.element = rw_sim::LightElement::LightMask0;
    let list = l.draw(&view(false), 1.0);
    let DrawItem::Sprite(s) = &list.items[0] else {
        panic!("a sprite");
    };
    assert_eq!((s.element, s.scale), ("LightMask0", Vec2::new(20.0, 20.0)));
    assert_eq!(
        light().draw(&view(false), 1.0).sprite(0).map(|s| s.element),
        Some("Futile_White")
    );
}

/// `Lerp(lastAlpha, Alpha, t)`: a light fading with the sun at `ShaderLight` -0.25
/// draws `0.1 * 0.5^1.2` at t = 1, its `lastAlpha` at t = 0.
#[test]
fn alpha_fades_with_the_sun() {
    let mut l = light();
    l.fade_with_sun = true;
    let mut v = view(false);
    v.shader_light = -0.25;
    let want = 0.1 * 0.5f32.powf(1.2) * 0.9;
    assert!((disc_alpha(&l.draw(&v, 1.0)) - want).abs() < 1e-6);
    assert!((disc_alpha(&l.draw(&v, 0.0)) - 0.09).abs() < 1e-6);
}

/// A colour of largest component 0.25 has `colorAlpha` 0.25 and `color.a` 4 (Unity's
/// `Color / float`); the sprite alpha's `colorAlpha` and Futile's `color.a *
/// alpha` cancel, so both sprites draw at the light's own alpha.
#[test]
fn color_alpha_cancels_in_the_vertex_alpha() {
    let mut l = light();
    l.set_color([0.25, 0.125, 0.0]);
    l.affected_by_palette_darkness = 0.0;
    l.water_surface_level = 500.0;
    let list = l.draw(&view(true), 1.0);
    for item in &list.items {
        let DrawItem::Sprite(s) = item else {
            panic!("a sprite");
        };
        assert!((s.color.a - 0.1).abs() < 1e-6, "{}", s.color.a);
        assert_eq!((s.color.r, s.color.g, s.color.b), (1.0, 0.5, 0.0));
    }
}

/// With water, the under-water glow: above the surface by more than rad / 4 it has
/// no size or alpha; deep under it, half the rad, at the light, full alpha.
#[test]
fn under_water_glow() {
    let mut l = light();
    l.water_surface_level = 0.0;
    let list = l.draw(&view(true), 1.0);
    let DrawItem::Sprite(s) = &list.items[1] else {
        panic!("a sprite");
    };
    assert_eq!((s.shader, s.layer), (Shader::UnderWaterLight, Layer::Water));
    assert_eq!((s.scale.x, s.color.a), (0.0, 0.0));
    l.water_surface_level = 500.0;
    let list = l.draw(&view(true), 1.0);
    let DrawItem::Sprite(s) = &list.items[1] else {
        panic!("a sprite");
    };
    assert_eq!(s.scale, Vec2::new(10.0, 10.0));
    assert_eq!(s.pos, Vec2::new(90.5 + 10.25, 30.5 + 20.0));
    assert!((s.color.a - 0.09).abs() < 1e-6);
}
