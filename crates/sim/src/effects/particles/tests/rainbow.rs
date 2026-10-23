//! `Rainbow`'s geometry, fade and spawn condition. Expected values follow `Rainbow.Refresh`,
//! `Update` and `Room.Loaded`.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::RainbowData;

use super::Rainbow;
use crate::Vec2;

fn data() -> RainbowData {
    RainbowData::parse("-30~40~0~0~1,1,1,1,0.5,0.15").unwrap()
}

#[test]
fn position_and_radius_follow_the_handle() {
    let r = Rainbow::new(3, Vec2::new(100.0, 200.0), &data(), false);
    // pos = placed - handle; rad = |(-30, 40)| * 2 = 100.
    assert_eq!(r.pos, Vec2::new(130.0, 160.0));
    assert_eq!(r.rad, 100.0);
    assert_eq!(r.fades, [1.0, 1.0, 1.0, 1.0, 0.5, 0.15]);
    assert_eq!(r.update_order, 3);
    assert!(!r.always_show && !r.slated);
    assert_eq!(r.fade, 0.0);
}

/// `fade = Clamp01(sin(start-up^0.75 * pi))^0.6`.
#[test]
fn fade_follows_the_cycle_start_up() {
    for start_up in [0.0_f32, 0.1, 0.25, 0.5, 0.9] {
        let mut r = Rainbow::new(0, Vec2::ZERO, &data(), false);
        r.update(start_up, false);
        let want = (f64::from(start_up).powf(0.75) * std::f64::consts::PI)
            .sin()
            .clamp(0.0, 1.0)
            .powf(0.6);
        assert!(
            (f64::from(r.fade) - want).abs() < 1e-6,
            "{start_up}: {}",
            r.fade
        );
        assert!(!r.slated);
    }
}

#[test]
fn it_ends_with_the_start_up() {
    let mut r = Rainbow::new(0, Vec2::ZERO, &data(), false);
    r.update(1.0, false);
    assert!(r.slated);
    // sin(pi) is slightly negative in single precision: clamped to 0.
    assert_eq!(r.fade, 0.0);
}

#[test]
fn arena_sessions_and_the_no_fade_variant_show_fully_and_stay() {
    let mut arena = Rainbow::new(0, Vec2::ZERO, &data(), false);
    arena.update(1.0, true);
    assert_eq!((arena.fade, arena.slated), (1.0, false));
    let mut no_fade = Rainbow::new(0, Vec2::ZERO, &data(), true);
    no_fade.update(1.0, false);
    assert_eq!((no_fade.fade, no_fade.slated), (1.0, false));
}

/// `CycleStartUp < 1` and the seeded value under the chance, or an arena session.
#[test]
fn spawn_condition() {
    assert!(Rainbow::spawns(0.5, 0.1, 0.15, false));
    assert!(!Rainbow::spawns(0.5, 0.15, 0.15, false));
    assert!(!Rainbow::spawns(0.5, 0.9, 0.15, false));
    assert!(!Rainbow::spawns(1.0, 0.1, 0.15, false));
    assert!(Rainbow::spawns(1.0, 0.9, 0.15, true));
}
