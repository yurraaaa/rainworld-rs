
use super::{LightHardSet, LightSource, LightSourceTick, LightTie};
use crate::Vec2;

fn tick(tie_gone: bool) -> LightSourceTick {
    LightSourceTick {
        tie_gone,
        shader_light: 1.0,
        environment_color: None,
        water_level: f32::MIN,
    }
}

/// A placed light as `Room.Loaded` makes it: white, environmental, rad and alpha
/// set on the first update; `lastAlpha` is the `Alpha` before the set, scaled by
/// `InverseLerp(-0.5, 0, ShaderLight)^1.2` while fading with the sun; the colour
/// follows the camera's level pixel.
#[test]
fn placed_light_fades_with_the_sun() {
    let mut l = LightSource::new(4, 0, Vec2::new(10.0, 20.0), true, [1.0; 3], None);
    l.set_rad = Some(50.0);
    l.set_alpha = Some(0.75);
    l.fade_with_sun = true;
    l.color_from_environment = true;
    let mut t = tick(false);
    t.shader_light = -0.25;
    t.environment_color = Some([0.25, 0.5, 0.125, 1.0]);
    l.update(&t);
    assert_eq!((l.rad, l.alpha, l.last_alpha), (50.0, 0.75, 0.0));
    assert_eq!(
        (l.color, l.color_alpha, l.color_a),
        ([0.5, 1.0, 0.25], 0.5, 2.0)
    );
    let want = 0.75 * 0.5f32.powf(1.2);
    assert!((l.alpha_now(-0.25) - want).abs() < 1e-6);
    l.update(&t);
    assert!((l.last_alpha - want).abs() < 1e-6);
    assert_eq!((l.alpha_now(0.5), l.alpha_now(-0.6)), (0.75, 0.0));
    l.fade_with_sun = false;
    assert_eq!(l.alpha_now(-1.0), 0.75);
}

/// `Lightness`: the mean of the normalised colour times colour alpha and alpha;
/// the exposure term is 0 outside `Rad` and `Lightness` at the centre.
#[test]
fn lightness_and_exposure() {
    let mut l = LightSource::new(1, 0, Vec2::ZERO, true, [0.0, 0.25, 0.5], None);
    l.alpha = 0.8;
    l.rad = 100.0;
    assert!((l.lightness() - 0.5 * 0.5 * 0.8).abs() < 1e-6);
    assert_eq!(l.exposure_at(Vec2::new(100.0, 0.0)), None);
    assert!((l.exposure_at(Vec2::ZERO).expect("in") - l.lightness()).abs() < 1e-6);
    let half = l.exposure_at(Vec2::new(50.0, 0.0)).expect("in");
    assert!((half - 0.5 * l.lightness()).abs() < 1e-6);
}

/// The guide light's colour (1, 0.8, 0.3) is already at most 1: kept, colour alpha 1.
#[test]
fn color_over_its_largest_component() {
    let l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
    assert_eq!((l.color, l.color_alpha), ([1.0, 0.8, 0.3], 1.0));
    assert_eq!(l.affected_by_palette_darkness, 1.0);
    let l = LightSource::new(1, 0, Vec2::ZERO, true, [0.0, 0.25, 0.5], None);
    assert_eq!(
        (l.color, l.color_alpha, l.color_a),
        ([0.0, 0.5, 1.0], 0.5, 2.0)
    );
    assert_eq!(l.affected_by_palette_darkness, 0.0);
    let l = LightSource::new(1, 0, Vec2::ZERO, false, [0.0; 3], None);
    assert_eq!((l.color, l.color_alpha, l.color_a), ([1.0; 3], 0.0, 1.0));
}

/// Lit when its tie goes: rad and alpha are set to 0 (alpha this update, rad the
/// next), then it is destroyed on the update that finds it dark.
#[test]
fn fades_then_dies_without_its_tie() {
    let mut l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
    l.tied_to = Some(LightTie::HoloLight(3));
    l.hard_set(LightHardSet {
        pos: Vec2::new(5.0, 6.0),
        rad: 200.0,
        alpha: 0.1,
    });
    l.update(&tick(false));
    assert_eq!((l.rad, l.alpha, l.last_alpha), (200.0, 0.1, 0.1));
    l.update(&tick(true));
    assert!(!l.slated);
    assert_eq!((l.rad, l.alpha, l.last_alpha), (200.0, 0.0, 0.1));
    l.update(&tick(true));
    assert!(l.slated);
    assert_eq!((l.rad, l.last_rad), (0.0, 200.0));
    assert_eq!(l.pos, Vec2::new(5.0, 6.0));
}
