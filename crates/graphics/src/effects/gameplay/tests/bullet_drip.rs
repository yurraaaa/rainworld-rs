//! A `BulletDrip`'s sprites. Expected values from `BulletDrip.DrawSprites`.
//!
//! Mirrors: - (port-only: tests)

use super::*;
use crate::tests::sprites;

fn falling_drip() -> BulletDrip {
    let mut d = BulletDrip::new(1);
    d.pos = Vec2::new(100.0, 100.0);
    d.sky_pos = Vec2::new(100.0, 900.0);
    d.last_falling = 0.25;
    d.falling = 0.75;
    d.move_tip = false;
    d
}

/// The streak runs from the tip (`pos`) toward the head at `Lerp(skyPos, pos, Lerp(
/// lastFalling, falling, t))`: rotation `AimFromOneVectorToAnother`, `scaleY` the length
/// over 16; the splash is `Sin(falling * pi) * 0.4` large with one rotation draw.
#[test]
fn the_streak_and_splash_follow_the_fall() {
    let d = falling_drip();
    let mut rng = UnityRandom::new(3);
    let list = d.draw(0.5, &mut rng);
    let drawn = sprites(&list.items);
    assert_eq!(drawn.len(), 2);
    let (streak, splash) = (drawn[0], drawn[1]);
    // falling 0.5: the head half way from the sky down to the tip, at (100, 500).
    assert_eq!(streak.pos, Vec2::new(100.0, 100.0));
    assert_eq!(streak.rotation, 0.0);
    assert_eq!(streak.scale, Vec2::new(0.125, 400.0 / 16.0));
    assert_eq!(
        (streak.shader, streak.layer),
        (Shader::BulletRain, Layer::GrabShaders)
    );
    assert_eq!(streak.anchor, Vec2::new(0.5, 0.0));
    assert_eq!(splash.element, "RainSplash");
    assert_eq!(splash.layer, Layer::Items);
    let size = (0.5_f32 * std::f32::consts::PI).sin() * 0.4;
    assert_eq!(splash.scale, Vec2::new(size, size));
    let mut e = UnityRandom::new(3);
    assert_eq!(splash.rotation, e.value() * 360.0);
    assert_eq!(rng.state(), e.state(), "one draw");
}

/// On the strike's tick the tip itself is drawn sliding from `skyPos`: at `t` 0.25 it is a
/// quarter of the way down, the head (falling 0) still at the sky.
#[test]
fn the_tip_starts_at_the_sky_on_the_strike_tick() {
    let mut d = falling_drip();
    d.last_falling = 0.0;
    d.falling = 0.0;
    d.move_tip = true;
    let list = d.draw(0.25, &mut UnityRandom::new(1));
    let drawn = sprites(&list.items);
    assert_eq!(drawn[0].pos, Vec2::new(100.0, 700.0));
    assert_eq!(drawn[0].scale.y, 200.0 / 16.0);
    assert_eq!(drawn[0].rotation, 0.0, "the head is above the tip");
}
