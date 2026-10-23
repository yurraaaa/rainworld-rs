//! `AntiGravity`'s formula and `BrokenAntiGravity`'s draws and curves, with the numbers
//! written from `AntiGravity.cs`.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::*;

fn steady(on: bool, progress: f32) -> BrokenAntiGravity {
    let to = if on { 1.0 } else { 0.0 };
    BrokenAntiGravity {
        counter: 500,
        on,
        from: 1.0 - to,
        to,
        progress,
        lights: to,
        lights_get_to: to,
        cycle_min: 8,
        cycle_max: 18,
    }
}

/// `1 - ZeroG`, with amounts below 1 lerped to at most 0.85: only exactly 1 gives 0.
#[test]
fn zero_g_amount_maps_to_gravity() {
    assert_eq!(AntiGravity::gravity(1.0, 0.0, None), 0.0);
    assert!((AntiGravity::gravity(0.5, 0.0, None) - (1.0 - 0.425)).abs() < 1e-6);
    assert!((AntiGravity::gravity(0.0, 0.0, None) - 1.0).abs() < 1e-6);
    assert!((AntiGravity::gravity(0.9999, 0.0, None) - 0.15).abs() < 1e-3);
}

/// A `BrokenZeroG` room follows the switch; a ZeroG amount of 1 wins over it.
#[test]
fn broken_zero_g_follows_the_switch() {
    let on = steady(true, 1.0);
    assert_eq!(AntiGravity::gravity(0.0, 1.0, Some(&on)), 0.0);
    assert!((AntiGravity::gravity(0.0, 0.489, Some(&on)) - 0.511).abs() < 1e-6);
    let off = steady(false, 1.0);
    assert_eq!(AntiGravity::gravity(0.0, 1.0, Some(&off)), 1.0);
    assert_eq!(AntiGravity::gravity(1.0, 1.0, Some(&off)), 0.0);
}

/// `CurrentAntiGravity` does not start to change before progress 0.35.
#[test]
fn current_anti_gravity_waits_for_progress_035() {
    let mut b = steady(true, 0.0);
    assert_eq!(b.current_anti_gravity(), 0.0);
    b.progress = 0.35;
    assert_eq!(b.current_anti_gravity(), 0.0);
    b.progress = 1.0;
    assert_eq!(b.current_anti_gravity(), 1.0);
    // The S-curve is symmetric about the middle of 0.35..1.
    b.progress = 0.675;
    assert!((b.current_anti_gravity() - 0.5).abs() < 1e-5);
}

#[test]
fn constructor_draws_once_and_starts_steady() {
    let mut rng = UnityRandom::new(7);
    let mut mirror = rng.clone();
    let b = BrokenAntiGravity::new(8, 18, &mut rng);
    let on = mirror.value() < 0.5;
    assert_eq!(rng, mirror);
    assert_eq!((b.on, b.counter, b.progress), (on, 0, 0.0));
    assert_eq!(
        (b.from, b.to, b.lights),
        (on as i32 as f32, on as i32 as f32, on as i32 as f32)
    );
}

/// The first update flips at once (the counter starts at 0): `Range(8*40, 18*40)`, then the
/// transition's draws: `value < 0.125` and, when it passes, a second `value`.
#[test]
fn first_update_flips_and_draws_in_order() {
    let mut rng = UnityRandom::new(11);
    let mut b = BrokenAntiGravity::new(8, 18, &mut rng);
    let was_on = b.on;
    let mut mirror = rng.clone();
    let tick = b.update(&mut rng);
    assert_eq!(tick.switched, Some(!was_on));
    let counter = mirror.range_i32(320, 720);
    assert_eq!(b.counter, counter);
    assert!((b.progress - 1.0 / 120.0).abs() < 1e-7);
    if mirror.value() < 0.125 {
        mirror.value();
    }
    assert_eq!(rng, mirror);
    assert!(tick.shaking);
}

/// Every transition tick draws once (twice on a passed `< 0.125`); none
/// after, and the counter alone runs between switches.
#[test]
fn transition_draws_per_tick() {
    let mut rng = UnityRandom::new(3);
    let mut b = BrokenAntiGravity::new(8, 18, &mut rng);
    b.update(&mut rng);
    // 120 additions of 1/120 in f32 stop at 0.9999994, so a 121st tick finishes it.
    let mut ticks = 1;
    while b.progress < 1.0 {
        ticks += 1;
        let mut mirror = rng.clone();
        let before = b.counter;
        b.update(&mut rng);
        assert_eq!(b.counter, before - 1);
        if mirror.value() < 0.125 {
            mirror.value();
        }
        assert_eq!(rng, mirror);
    }
    assert_eq!(ticks, 121);
    assert_eq!(b.progress, 1.0);
    let mirror = rng.clone();
    let tick = b.update(&mut rng);
    assert_eq!(rng, mirror);
    assert!(!tick.shaking && tick.switched.is_none());
    assert_eq!(b.lights_get_to, b.to);
}

/// The screen shakes by `amount * 0.5 * sin(progress * PI)`.
#[test]
fn shake_follows_a_sine() {
    let b = steady(true, 0.5);
    assert!((b.shake(0.8) - 0.4).abs() < 1e-6);
}
