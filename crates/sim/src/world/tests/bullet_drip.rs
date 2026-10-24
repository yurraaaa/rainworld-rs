//! `BulletDrip.Update` and `Strike`'s setup: the draw order and counts, the formulas, and
//! the early returns that leave `delay` alone. Expected values from `BulletDrip.cs`,
//! replayed by hand on a second generator.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::*;
use crate::tests::room;

const ROWS: [&str; 5] = [
    "#...#.", //
    "#.....", //
    "#...#.", //
    "#..#..", //
    "######",
];

/// A `rainReach` (`GenerateShelterTex` makes it from the tiles; here it is just given).
fn reach() -> Vec<i32> {
    vec![4, 3, 0, 1, 4, 0]
}

/// A seed whose first `Range(0, 6)` draw is `column`.
fn seed_for(column: i32) -> i32 {
    (0..10_000)
        .find(|&s| UnityRandom::new(s).range_i32(0, 6) == column)
        .unwrap()
}

/// `Update`: `falling` moves by `fallSpeed` (capped at 1) and `lastFalling` follows; only
/// from `lastFalling >= 1` does the delay count down and the strike run at `delay < 1`.
#[test]
fn update_counts_the_delay_down_only_once_landed() {
    let mut d = BulletDrip::new(1);
    d.delay = 2;
    assert!(!d.update());
    assert_eq!(d.delay, 1);
    assert!(d.update());
    assert_eq!(d.delay, 0);
    assert!(d.update(), "a failed strike retries every tick");
    assert_eq!(d.delay, -1);

    d.falling = 0.0;
    d.last_falling = 0.0;
    d.fall_speed = 0.75;
    d.move_tip = true;
    d.delay = 0;
    assert!(!d.update());
    assert_eq!((d.last_falling, d.falling, d.move_tip), (0.0, 0.75, false));
    assert_eq!(d.delay, 0, "not landed: no countdown");
    assert!(!d.update());
    assert_eq!((d.last_falling, d.falling), (0.75, 1.0));
    assert!(d.update(), "landed now, delay 0 -> -1");
}

/// A column with a solid top tile (column 0) returns after its one draw and leaves
/// `delay` and the position untouched.
#[test]
fn a_solid_top_tile_returns_after_one_draw() {
    let r = room(&ROWS);
    let seed = seed_for(0);
    let mut d = BulletDrip::new(1);
    d.delay = -3;
    let mut rng = UnityRandom::new(seed);
    assert!(!d.strike_setup(&r, &reach(), 0.0, 0.5, &mut rng));
    let mut e = UnityRandom::new(seed);
    e.range_i32(0, 6);
    assert_eq!(rng.state(), e.state());
    assert_eq!((d.delay, d.falling, d.pos), (-3, 1.0, Vec2::ZERO));
}

/// A column whose `rainReach` is the room's height returns after its one draw too.
#[test]
fn a_groundless_column_returns_after_one_draw() {
    let r = room(&ROWS);
    let seed = seed_for(1);
    let mut d = BulletDrip::new(1);
    let mut rng = UnityRandom::new(seed);
    assert!(!d.strike_setup(&r, &[5; 6], 0.0, 0.5, &mut rng));
    let mut e = UnityRandom::new(seed);
    e.range_i32(0, 6);
    assert_eq!(rng.state(), e.state());
    assert_eq!(d.delay, 0);
}

/// A strike in column 1 (reach 3): `pos` at the middle of tile (1, 3) plus
/// `(Lerp(-10, 10, v), 10)`, `skyPos` at the middle of tile (1, 3 + 5 + 50) plus
/// `(Lerp(-30, 30, v) - 30 * rainDirection, 0)`, `fallSpeed = 1 / Lerp(0.2, 1.8, v)`,
/// `delay = Range(0, 60 - (int)(density * 60))`: 1 + 3 + 1 draws.
#[test]
fn a_strike_places_the_drop_and_draws_in_order() {
    let r = room(&ROWS);
    let seed = seed_for(1);
    let mut d = BulletDrip::new(1);
    let mut rng = UnityRandom::new(seed);
    assert!(d.strike_setup(&r, &reach(), 0.5, 0.25, &mut rng));
    let mut e = UnityRandom::new(seed);
    e.range_i32(0, 6);
    let pos = Vec2::new(30.0 + (-10.0_f32).lerp(10.0, e.value()), 70.0 + 10.0);
    let sky = Vec2::new(
        30.0 + (-30.0_f32).lerp(30.0, e.value()) - 15.0,
        10.0 + 20.0 * 58.0,
    );
    let speed = 1.0 / 0.2_f32.lerp(1.8, e.value());
    let delay = e.range_i32(0, 60 - 15);
    assert_eq!(rng.state(), e.state());
    assert_eq!((d.pos, d.sky_pos), (pos, sky));
    assert_eq!(d.fall_speed, speed);
    assert_eq!(d.delay, delay);
    assert_eq!((d.falling, d.last_falling, d.move_tip), (0.0, 0.0, true));
}

/// A column without a solid tile (`rainReach` 0) is struck at row -10; at density 1 the
/// delay is `Range(0, 0)`, which draws nothing.
#[test]
fn a_column_without_ground_strikes_at_row_minus_ten() {
    let r = room(&ROWS);
    let seed = seed_for(2);
    let mut d = BulletDrip::new(1);
    let mut rng = UnityRandom::new(seed);
    assert!(d.strike_setup(&r, &reach(), 0.0, 1.0, &mut rng));
    let mut e = UnityRandom::new(seed);
    e.range_i32(0, 6);
    let x = (-10.0_f32).lerp(10.0, e.value());
    assert_eq!(d.pos, Vec2::new(50.0 + x, -190.0 + 10.0));
    e.value();
    e.value();
    assert_eq!(rng.state(), e.state(), "Range(0, 0) makes no draw");
    assert_eq!(d.delay, 0);
}

/// `DrawSprites`: the tip starts at `skyPos` on the strike's tick, and the head follows
/// `Lerp(lastFalling, falling, timeStacker)`.
#[test]
fn the_tip_and_head_follow_the_fall() {
    let mut d = BulletDrip::new(1);
    d.pos = Vec2::new(0.0, 0.0);
    d.sky_pos = Vec2::new(100.0, 1000.0);
    d.last_falling = 0.25;
    d.falling = 0.75;
    d.move_tip = true;
    assert_eq!(d.tip(0.5), Vec2::new(50.0, 500.0));
    assert_eq!(d.head(0.5), d.sky_pos.lerp(d.pos, 0.5));
    d.move_tip = false;
    assert_eq!(d.tip(0.5), d.pos);
}
