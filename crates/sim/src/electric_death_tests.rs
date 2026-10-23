//! `ElectricDeath`'s pieces. Expected values from `ElectricDeath.cs` (`Intensity`,
//! `LightFlash`, `SparkFlash`, `Update`), replayed by hand on a second generator.

use rw_random::UnityRandom;

use super::{ElectricDeath, LightFlash, SparkFlash};
use crate::world::RainCycle;
use crate::{FloatExt, Vec2};

fn cycle(timer: i32) -> RainCycle {
    RainCycle {
        cycle_length: 4800,
        timer,
        ..RainCycle::default()
    }
}

/// `Intensity` is `amount * InverseLerp(cycleLength - 400, cycleLength + 1200, timer)^2.2`:
/// 0 until 400 ticks before the rain, `amount` 1200 ticks after it.
#[test]
fn intensity_follows_the_cycle() {
    let d = ElectricDeath::new(1, 1.0);
    assert_eq!(d.intensity(&cycle(0)), 0.0);
    assert_eq!(d.intensity(&cycle(4400)), 0.0);
    assert_eq!(d.intensity(&cycle(6000)), 1.0);
    assert_eq!(d.intensity(&cycle(9000)), 1.0);
    // Halfway (timer 5200): 0.5^2.2.
    let half = d.intensity(&cycle(5200));
    assert!((half - 0.5f32.powf(2.2)).abs() < 1e-6, "{half}");
    // The amount scales it: uw_c05's 0.88.
    let d = ElectricDeath::new(1, 0.88);
    assert!((d.intensity(&cycle(6000)) - 0.88).abs() < 1e-6);
    assert!((d.intensity(&cycle(5200)) - 0.88 * 0.5f32.powf(2.2)).abs() < 1e-6);
}

/// `Reset` draws four: x and y over the screen, the lifetime (3 to `Lerp(34, 12, I)`) and
/// the radius (`Lerp(40, 800, v * Lerp(0.5, 1, I))`).
#[test]
fn a_light_flash_reset_draws_four() {
    for seed in 0..20 {
        let mut rng = UnityRandom::new(seed);
        let mut e = UnityRandom::new(seed);
        let mut f = LightFlash::default();
        f.reset(0.6, &mut rng);
        let (x, y) = (1366.0 * e.value(), 768.0 * e.value());
        let life_time = 3.0f32.lerp(34.0f32.lerp(12.0, 0.6), e.value());
        let rad = 40.0f32.lerp(800.0, e.value() * 0.5f32.lerp(1.0, 0.6));
        assert_eq!(rng.state(), e.state());
        assert_eq!(
            (f.pos, f.life_time, f.rad),
            (Vec2::new(x, y), life_time, rad)
        );
        assert_eq!((f.life, f.last_life), (1.0, 1.0));
    }
}

/// A new flash (life 0, last 0) resets on its first update; one alive runs down by
/// `1 / lifeTime` and, once dead for two ticks, resets again.
#[test]
fn a_light_flash_runs_down_and_resets() {
    let mut rng = UnityRandom::new(3);
    let mut f = LightFlash::default();
    f.update(1.0, &mut rng);
    assert_eq!((f.life, f.last_life), (1.0, 1.0));
    let after_reset = rng.state();
    let life_time = f.life_time;
    f.update(1.0, &mut rng);
    assert_eq!(rng.state(), after_reset, "a live flash draws nothing");
    assert_eq!((f.last_life, f.life), (1.0, 1.0 - 1.0 / life_time));
    let mut ticks = 1;
    while f.life > 0.0 {
        f.update(1.0, &mut rng);
        ticks += 1;
    }
    assert_eq!(f.life, 0.0);
    assert!(ticks as f32 >= life_time - 1.0);
    // Dead, but `lastLife` is still above 0: one more tick without a reset.
    assert!(f.last_life > 0.0);
    f.update(1.0, &mut rng);
    assert_eq!((f.last_life, f.life), (0.0, 0.0));
    assert_eq!(rng.state(), after_reset);
    f.update(1.0, &mut rng);
    assert_eq!(f.life, 1.0);
    assert_ne!(rng.state(), after_reset);
}

/// The nine flashes: the first is never updated (`for i = 1`), so eight resets of four
/// draws on the first tick.
#[test]
fn eight_flashes_update() {
    let mut d = ElectricDeath::new(1, 1.0);
    let mut rng = UnityRandom::new(8);
    let mut e = UnityRandom::new(8);
    d.update_flashes(0.5, &mut rng);
    for _ in 0..32 {
        e.value();
    }
    assert_eq!(rng.state(), e.state());
    assert_eq!(d.flashes[0], LightFlash::default());
    assert!(d.flashes[1..].iter().all(|f| f.life == 1.0));
}

/// The colour block: `Lerp(color, getToColor, v * 0.3)` (one draw), then the first
/// `v < 1/3` redraws x (one more), else the second redraws y, else the third z.
#[test]
fn the_colour_chain_draws() {
    for seed in 0..60 {
        let mut d = ElectricDeath::new(1, 1.0);
        d.get_to_color = crate::Vec3::new(0.9, 0.8, 0.7);
        d.color = crate::Vec3::new(0.1, 0.2, 0.3);
        let mut rng = UnityRandom::new(seed);
        let mut e = UnityRandom::new(seed);
        d.update_color(&mut rng);
        let t = e.value() * 0.3;
        let want = crate::Vec3::new(
            0.1 + (0.9 - 0.1) * t,
            0.2 + (0.8 - 0.2) * t,
            0.3 + (0.7 - 0.3) * t,
        );
        let mut to = crate::Vec3::new(0.9, 0.8, 0.7);
        if e.value() < 1.0 / 3.0 {
            to.x = e.value();
        } else if e.value() < 1.0 / 3.0 {
            to.y = e.value();
        } else if e.value() < 1.0 / 3.0 {
            to.z = e.value();
        }
        assert_eq!(rng.state(), e.state(), "seed {seed}");
        assert_eq!(d.color, want);
        assert_eq!(d.last_color, crate::Vec3::new(0.1, 0.2, 0.3));
        assert_eq!(d.get_to_color, to);
    }
}

/// The spontaneous spark: no draw without wall tiles or when the room is not viewed; one
/// draw when the gate fails; on passing, the tile (`Range`), the spot (two) and the size
/// (one) after the gate's: 1 + 4. The gate is `InverseLerp(1000, 9120, W*H) * Intensity`.
#[test]
fn the_wall_spark_gate_draws_one_then_four() {
    let mut d = ElectricDeath::new(1, 1.0);
    let mut rng = UnityRandom::new(1);
    assert!(d.roll_wall_spark(1.0, true, 9120, &mut rng).is_none());
    d.close_to_wall_tiles = Some(Vec::new());
    assert!(d.roll_wall_spark(1.0, true, 9120, &mut rng).is_none());
    d.close_to_wall_tiles = Some(vec![(3, 4), (5, 6), (7, 8)]);
    assert!(d.roll_wall_spark(1.0, false, 9120, &mut rng).is_none());
    assert_eq!(rng.state(), UnityRandom::new(1).state(), "no draws yet");
    let area = 5060;
    let chance = 0.5; // InverseLerp(1000, 9120, 5060) = 0.5, times the intensity 1
    assert!((5060.0f32.inverse_lerp(1000.0, 9120.0) - chance).abs() < 1e-6);
    let (mut passed, mut failed) = (0, 0);
    for seed in 0..40 {
        let mut rng = UnityRandom::new(seed);
        let mut e = UnityRandom::new(seed);
        let spark = d.roll_wall_spark(1.0, true, area, &mut rng);
        if e.value() < chance {
            passed += 1;
            let tile = [(3, 4), (5, 6), (7, 8)][e.range_i32(0, 3) as usize];
            let x = (-10.0f32).lerp(10.0, e.value());
            let y = (-10.0f32).lerp(10.0, e.value());
            let size = e.value() * 1.0;
            let spark = spark.expect("gate passed");
            let middle = Vec2::new(tile.0 as f32 * 20.0 + 10.0, tile.1 as f32 * 20.0 + 10.0);
            assert_eq!(spark.pos, middle + Vec2::new(x, y));
            assert_eq!(spark.size, size);
        } else {
            failed += 1;
            assert!(spark.is_none());
        }
        assert_eq!(rng.state(), e.state(), "seed {seed}");
    }
    assert!(passed > 5 && failed > 5, "{passed} {failed}");
}

/// The strike gate draws once, and only above intensity 0.5; its chance is
/// `LerpMap(I, 0.5, 1, 0, 0.5)`.
#[test]
fn the_strike_gate() {
    let mut rng = UnityRandom::new(5);
    assert_eq!(ElectricDeath::strike_chance(0.5, &mut rng), None);
    assert_eq!(rng.state(), UnityRandom::new(5).state(), "no draw at 0.5");
    let mut hits = 0;
    for seed in 0..60 {
        let mut rng = UnityRandom::new(seed);
        let mut e = UnityRandom::new(seed);
        let got = ElectricDeath::strike_chance(0.75, &mut rng);
        let pass = e.value() < 0.25;
        assert_eq!(rng.state(), e.state());
        assert_eq!(got.is_some(), pass);
        if let Some(c) = got {
            hits += 1;
            assert!((c - 0.25).abs() < 1e-6);
        }
    }
    assert!(hits > 3);
    let mut rng = UnityRandom::new(2);
    let p = ElectricDeath::strike_power(1.0, &mut rng);
    let mut e = UnityRandom::new(2);
    assert_eq!(p, e.value().powf(0.9));
    let p = ElectricDeath::strike_power(0.75, &mut UnityRandom::new(2));
    assert!((p - UnityRandom::new(2).value().powf(0.9) * 0.5).abs() < 1e-6);
}

/// `SparkFlash`'s constructor draws one (`Lerp(2, 16, size * v)`); each update sheds a
/// green spark (a direction, a speed, then the spark's own 3 or 4 draws), and the flash
/// is destroyed once its life is out for two ticks.
#[test]
fn a_spark_flash_sheds_sparks_and_dies() {
    let mut rng = UnityRandom::new(7);
    let mut e = UnityRandom::new(7);
    let mut f = SparkFlash::new(Vec2::new(100.0, 60.0), 0.5, &mut rng);
    assert_eq!(f.life_time, 2.0f32.lerp(16.0, 0.5 * e.value()));
    assert_eq!(rng.state(), e.state());
    assert_eq!((f.life, f.last_life), (1.0, 1.0));
    let spark = f.spark(&mut rng);
    let dir = Vec2::from_degrees(e.value() * 360.0);
    let vel = dir * (60.0 * e.value());
    let ahead = vel.normalized() * (30.0 * e.value());
    let gravity = 0.4f32.lerp(0.9, e.value());
    let mut life = e.range_i32(0, 4);
    if e.value() < 0.1 {
        life = e.range_i32(4, 50);
    }
    assert_eq!(rng.state(), e.state());
    assert_eq!(spark.pos, Vec2::new(100.0, 60.0) + ahead);
    assert_eq!(
        (spark.vel, spark.gravity, spark.life_time),
        (vel, gravity, life)
    );
    assert_eq!(spark.color, [0.0, 1.0, 0.0, 1.0]);
    let life_time = f.life_time;
    let mut updates = 0;
    while !f.update() {
        updates += 1;
        assert!(updates < 40);
    }
    // `life` reaches 0 after ceil(lifeTime) updates; one more with `lastLife` 0 ends it.
    let want = life_time.ceil() as i32 + 1;
    assert!((want - 1..=want).contains(&updates), "{updates} {want}");
}
