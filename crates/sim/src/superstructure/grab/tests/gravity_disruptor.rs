//! `GravityDisruptor` and `DisruptorSpeck`: the constructor's draws, the swirl's numbers,
//! the streaks' draws and the speck's orbit.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{DisruptorSpeck, GravityDisruptor, SPECKS, STREAKS};
use crate::{FloatExt, Vec2};

fn disruptor() -> GravityDisruptor {
    GravityDisruptor::new(30, Vec2::ZERO, 1, &mut UnityRandom::new(1))
}

fn speck_at_100() -> DisruptorSpeck {
    DisruptorSpeck::new(
        1,
        30,
        Vec2::new(100.0, 0.0),
        Vec2::ZERO,
        &mut UnityRandom::new(2),
    )
}

/// 98 draws in all: for each of the twenty specks a direction and a distance (2) and the
/// speck's own speed and direction (2), then a drift for each of the sixteen streaks, the
/// pointing direction and `dirFac`. The first speck sits at `pos + RNV * (400 * v)`.
#[test]
fn the_constructor_draws_ninety_eight() {
    let pos = Vec2::new(500.0, 300.0);
    let orders: [u64; SPECKS] = std::array::from_fn(|i| 10 + i as u64);
    let mut rng = UnityRandom::new(3);
    let specks = GravityDisruptor::make_specks(pos, &orders, 30, &mut rng);
    let d = GravityDisruptor::new(30, pos, 2, &mut rng);
    let mut e = UnityRandom::new(3);
    let draws: Vec<f32> = (0..98).map(|_| e.value()).collect();
    assert_eq!(rng.state(), e.state());
    assert_eq!(specks.len(), 20);
    let first = pos + Vec2::from_degrees(draws[0] * 360.0) * (400.0 * draws[1]);
    assert_eq!(specks[0].pos, first);
    assert_eq!((specks[0].update_order, specks[0].disruptor), (10, 30));
    let speed = 2.0f32.lerp(8.0, draws[2]);
    assert_eq!(specks[0].my_float_speed, speed);
    assert_eq!(specks[0].vel, Vec2::from_degrees(draws[3] * 360.0) * speed);
    assert_eq!(specks[0].my_orbit_rad, pos.distance(first));
    assert_eq!(specks[0].new_orbit_rad, specks[0].my_orbit_rad);
    // The second speck's draws start at 4.
    assert_eq!(
        specks[1].pos,
        pos + Vec2::from_degrees(draws[4] * 360.0) * (400.0 * draws[5])
    );
    for (m, l) in d.lights.iter().enumerate() {
        assert_eq!(l[3], draws[80 + m]);
        assert_eq!(l[..3], [0.0; 3]);
    }
    assert_eq!(d.point_dir, Vec2::from_degrees(draws[96] * 360.0));
    assert_eq!(d.get_to_point_dir, d.point_dir);
    assert_eq!((d.dir_fac, d.dir_fac_get_to), (draws[97], draws[97]));
    assert_eq!((d.power, d.last_power, d.depth), (1.0, 1.0, 2));
}

/// A chunk 100 px to the right of the centre, at rest: inside 200 px the weight is 1 and
/// inside 260 the push is `(260 - 100) * 0.05` outward; the side of its line is `Sign(0)
/// = 1`, so the swirl is `0.1` up (the perpendicular of right).
#[test]
fn a_chunk_close_in_is_pushed_out_and_round() {
    let d = disruptor();
    let (push, swirl) = d
        .chunk_push(Vec2::new(100.0, 0.0), Vec2::ZERO)
        .expect("in reach");
    assert_eq!(push, Vec2::new((260.0 - 100.0) * 0.05, 0.0));
    assert_eq!(swirl, Vec2::new(0.0, 0.1));
}

/// 325 px out: the weight is `InverseLerp(450, 200, 325)^2 = 0.25`, the push is inward
/// (`260 - 325`) at half strength outside 260 px.
#[test]
fn a_chunk_far_out_is_pulled_in_gently() {
    let d = disruptor();
    let (push, swirl) = d
        .chunk_push(Vec2::new(325.0, 0.0), Vec2::ZERO)
        .expect("in reach");
    assert_eq!(push, Vec2::new((260.0 - 325.0) * 0.05 * 0.25 * 0.5, 0.0));
    assert_eq!(swirl, Vec2::new(0.0, 0.1 * 0.25));
}

/// The sign is that of the chunk's next position against the line through the centre
/// (`DistanceToLine`): moving up the right side is positive, moving down negative. The power scales
/// both, and no power, or 450 px away, gives none.
#[test]
fn the_swirl_follows_the_chunks_side_and_the_power() {
    let mut d = disruptor();
    let (_, up) = d
        .chunk_push(Vec2::new(100.0, 0.0), Vec2::new(0.0, 5.0))
        .expect("in reach");
    let (_, down) = d
        .chunk_push(Vec2::new(100.0, 0.0), Vec2::new(0.0, -5.0))
        .expect("in reach");
    assert_eq!((up.y, down.y), (0.1, -0.1));
    d.power = 0.5;
    let (push, swirl) = d
        .chunk_push(Vec2::new(100.0, 0.0), Vec2::ZERO)
        .expect("in reach");
    assert_eq!(push, Vec2::new((260.0 - 100.0) * 0.05 * 0.5, 0.0));
    assert_eq!(swirl, Vec2::new(0.0, 0.1 * 0.5));
    assert!(d.chunk_push(Vec2::new(450.0, 0.0), Vec2::ZERO).is_none());
    d.power = 0.0;
    assert!(d.chunk_push(Vec2::new(100.0, 0.0), Vec2::ZERO).is_none());
}

/// A speck is only swirled round: `0.5 * weight * power` along the perpendicular.
#[test]
fn a_speck_is_swirled_round() {
    let d = disruptor();
    assert_eq!(
        d.speck_push(Vec2::new(100.0, 0.0), Vec2::ZERO),
        Some(Vec2::new(0.0, 0.5))
    );
    assert_eq!(
        d.speck_push(Vec2::new(325.0, 0.0), Vec2::ZERO),
        Some(Vec2::new(0.0, 0.5 * 0.25))
    );
    assert!(d.speck_push(Vec2::new(500.0, 0.0), Vec2::ZERO).is_none());
}

/// `Update` after the swirl draws 2 (the two 0.0125 rolls) and 2 for each streak (the
/// drift, the 0.005 roll) when none hits: 34, and a hit adds the new target's draws.
#[test]
fn an_update_draws_thirty_four_without_hits() {
    let seed = (1..10_000_i32)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            let v: Vec<f32> = (0..34).map(|_| r.value()).collect();
            v[0] >= 0.0125 && v[1] >= 0.0125 && (0..STREAKS).all(|m| v[3 + 2 * m] >= 0.005)
        })
        .expect("a seed");
    let mut d = disruptor();
    let mut rng = UnityRandom::new(seed);
    d.update(None, &mut rng);
    let mut e = UnityRandom::new(seed);
    for _ in 0..34 {
        e.value();
    }
    assert_eq!(rng.state(), e.state());
    assert!(d.lights.iter().all(|l| (0.0..=1.0).contains(&l[0])));
    assert_eq!(d.last_power, 1.0);
}

/// A streak's brightness: the old values shift back, the new one is the clamped drift
/// (`Lerp(-1, 1, v) / 120 + Lerp(-1, 1, drift) / 60`), pulled toward the pointing
/// direction by `|dot|^8 * 0.3 * dirFac * InverseLerp(0.5, 0, |0.5 - drift|)` and toward
/// its neighbours' two-tick-old values by 5%; two draws when the re-roll misses.
#[test]
fn a_streak_drifts_and_is_pulled() {
    let mut d = disruptor();
    d.lights[3] = [0.5, 0.25, 0.1, 0.8];
    d.lights[2][2] = 0.4;
    d.lights[4][2] = 0.2;
    d.point_dir = Vec2::from_degrees(3.0 / 16.0 * 360.0);
    d.dir_fac = 0.6;
    let seed = (1..10_000_i32)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value();
            r.value() >= 0.005
        })
        .expect("a seed");
    let mut e = UnityRandom::new(seed);
    let drift = e.value();
    e.value();
    let mut rng = UnityRandom::new(seed);
    d.update_streak(3, &mut rng);
    assert_eq!(rng.state(), e.state());
    assert_eq!((d.lights[3][1], d.lights[3][2]), (0.5, 0.25));
    let facing = Vec2::from_degrees(3.0 / 16.0 * 360.0).dot(d.point_dir);
    let mut expected = (0.5 + (-1.0f32).lerp(1.0, drift) / 120.0 + (-1.0f32).lerp(1.0, 0.8) / 60.0)
        .clamp(0.0, 1.0);
    expected = expected.lerp(
        facing.inverse_lerp(-1.0, 1.0).mathf_pow(1.5),
        facing.abs().mathf_pow(8.0) * 0.3 * 0.6 * (0.5f32 - 0.8).abs().inverse_lerp(0.5, 0.0),
    );
    expected = expected.lerp((0.2 + 0.4) / 2.0, 0.05);
    assert_eq!(d.lights[3][0], expected);
    assert_eq!(d.lights[3][3], 0.8);
}

/// Streak 15 takes its neighbours from streak 0 and 14, and streak 0 from 1 and 15.
#[test]
fn the_streaks_form_a_ring() {
    let mut d = disruptor();
    d.lights = [[0.0; 4]; STREAKS];
    d.lights[0][2] = 0.8;
    d.lights[14][2] = 0.2;
    d.dir_fac = 0.0;
    let seed = (1..10_000_i32)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value();
            r.value() >= 0.005
        })
        .expect("a seed");
    let drift = UnityRandom::new(seed).value();
    d.update_streak(15, &mut UnityRandom::new(seed));
    // No facing pull (dirFac 0); the drift keeps the base 0 + Lerp(-1, 1, v) / 120 + -1 / 60
    // clamped to 0.
    let base =
        ((-1.0f32).lerp(1.0, drift) / 120.0 + (-1.0f32).lerp(1.0, 0.0) / 60.0).clamp(0.0, 1.0);
    assert_eq!(d.lights[15][0], base.lerp((0.8 + 0.2) / 2.0, 0.05));
}

/// A roll under 0.0125 picks a new target direction (one draw).
#[test]
fn the_rolls_pick_new_targets() {
    let seed = (1..100_000_i32)
        .find(|&s| UnityRandom::new(s).value() < 0.0125)
        .expect("a seed");
    let mut d = disruptor();
    let before = d.get_to_point_dir;
    let mut rng = UnityRandom::new(seed);
    let mut e = UnityRandom::new(seed);
    e.value();
    let target = Vec2::from_degrees(e.value() * 360.0);
    d.update(None, &mut rng);
    assert_ne!(d.get_to_point_dir, before);
    assert_eq!(d.get_to_point_dir, target);
}

/// In a `BrokenZeroG` room the power becomes the world's `CurrentAntiGravity`, the last
/// power what it was.
#[test]
fn the_power_follows_the_anti_gravity() {
    let mut d = disruptor();
    d.update(Some(0.3), &mut UnityRandom::new(5));
    assert_eq!((d.last_power, d.power), (1.0, 0.3));
    d.update(Some(0.7), &mut UnityRandom::new(5));
    assert_eq!((d.last_power, d.power), (0.3, 0.7));
    d.update(None, &mut UnityRandom::new(5));
    assert_eq!((d.last_power, d.power), (0.7, 0.7));
}

/// A moved placed object shows the debug colours for 80 ticks.
#[test]
fn a_move_sets_the_debug_mode() {
    let mut d = disruptor();
    d.follow();
    assert_eq!(d.debug_mode, 0);
    d.placed_pos = Vec2::new(5.0, 5.0);
    d.follow();
    assert_eq!(
        (d.pos, d.last_pos, d.debug_mode),
        (Vec2::new(5.0, 5.0), Vec2::ZERO, 79)
    );
}

/// A speck at rest on its orbit 100 px out: the base class moves it by its velocity (0),
/// the radius pull is 0, the tangent kick is `LerpMap(100, 400, 100, 0.01, 0.15) = 0.15`
/// (side `Sign(0) = 1`), then the velocity eases 1% toward `speed / (Lerp(100, 100, .5) *
/// .01)` = 4 along its direction, times the power. Its one draw is above 0.01.
#[test]
fn a_speck_orbits() {
    let mut speck = speck_at_100();
    speck.vel = Vec2::ZERO;
    speck.my_float_speed = 4.0;
    let seed = (1..10_000_i32)
        .find(|&s| UnityRandom::new(s).value() >= 0.01)
        .expect("a seed");
    let mut rng = UnityRandom::new(seed);
    speck.update(Vec2::ZERO, 1.0, &mut rng);
    let mut e = UnityRandom::new(seed);
    e.value();
    assert_eq!(rng.state(), e.state());
    assert_eq!(speck.pos, Vec2::new(100.0, 0.0));
    assert_eq!(speck.last_pos, Vec2::new(100.0, 0.0));
    assert_eq!(speck.vel, Vec2::new(0.0, 0.15f32.lerp(4.0, 0.01)));
    // No power freezes it.
    speck.update(Vec2::ZERO, 0.0, &mut UnityRandom::new(seed));
    assert_eq!(speck.vel, Vec2::ZERO);
}

/// A roll under 0.01 picks `Lerp(50, 400, v)` for the radius it settles on; the radius
/// closes 1% of the way.
#[test]
fn a_speck_picks_a_new_orbit() {
    let mut speck = speck_at_100();
    let seed = (1..100_000_i32)
        .find(|&s| UnityRandom::new(s).value() < 0.01)
        .expect("a seed");
    let mut e = UnityRandom::new(seed);
    e.value();
    let radius = 50.0f32.lerp(400.0, e.value());
    let mut rng = UnityRandom::new(seed);
    speck.update(Vec2::ZERO, 1.0, &mut rng);
    assert_eq!(speck.new_orbit_rad, radius);
    assert_eq!(speck.my_orbit_rad, 100.0f32.lerp(radius, 0.01));
    assert_eq!(rng.state(), e.state());
}
