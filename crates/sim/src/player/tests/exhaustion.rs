//! Malnourished stats, `aerobicLevel` and exhaustion: `SetMalnourished` (86189),
//! `AerobicIncrease` (86492), the exhaustion block of `Player.Update` (86884-86941),
//! `Player.Stun` (88673) and `SleepUpdate`'s aerobic level (88788).

use rw_random::UnityRandom;

use super::{RIGHT, assert_close, flat_room};
use crate::campaign::SlugcatCampaign;
use crate::{HEAD, HIPS, Input, Player, Sounds, Vec2};

impl Player {
    /// A standing Survivor on the flat room's floor with the malnourished stats.
    fn malnourished_at(hips: Vec2) -> Self {
        let mut p = Self::upright_at(hips);
        p.set_stats(SlugcatCampaign::SURVIVOR.stats.malnourished());
        p
    }
}

/// The first seed whose first two `value()`s pass `ok`.
fn seed_where(ok: fn(f32, f32) -> bool) -> i32 {
    (0..)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            let a = r.value();
            ok(a, r.value())
        })
        .unwrap()
}

#[test]
fn malnourished_chunks_weigh_0_7_times_0_9() {
    let mut p = Player::malnourished_at(Vec2::new(100.0, 48.0));
    // `0.7 * min(1, 0.9) / 2` per chunk.
    assert_close(p.body.chunks[HEAD].mass, 0.315, "head");
    assert_close(p.body.chunks[HIPS].mass, 0.315, "hips");
    assert!(p.malnourished());
    p.set_stats(SlugcatCampaign::SURVIVOR.stats);
    assert_close(p.total_mass(), 0.7, "cured");
}

#[test]
fn malnourished_stun_is_rounded_half_to_even() {
    let mut p = Player::new();
    assert_eq!(p.stun_multiplied(7), 7);
    p.set_stats(SlugcatCampaign::SURVIVOR.stats.malnourished());
    // `Mathf.RoundToInt(7 * 1.5)`: 10.5 rounds to 10, 13.5 (9 * 1.5) to 14.
    assert_eq!(p.stun_multiplied(7), 10);
    assert_eq!(p.stun_multiplied(9), 14);
    p.exhausted = true;
    assert_eq!(p.stun_multiplied(7), 14);
    p.stun(7, &mut UnityRandom::new(0), &mut Sounds::default());
    assert_eq!(p.stun_ticks(), 14);
}

#[test]
fn exhaustion_starts_at_full_aerobic_level_and_draws_twice() {
    let mut p = Player::malnourished_at(Vec2::new(100.0, 48.0));
    p.aerobic_level = 1.0;
    // Neither a stun (value < 0.05) nor a fall (value < 0.1).
    let seed = seed_where(|a, b| a >= 0.05 && b >= 0.1);
    let mut rng = UnityRandom::new(seed);
    p.exhaustion_update(&mut rng, &mut Sounds::default());
    let mut expected = UnityRandom::new(seed);
    expected.value();
    expected.value();
    assert_eq!(rng, expected, "two draws");
    assert!(p.exhausted());
    assert!(p.standing);
    assert_eq!(p.stun_ticks(), 0);
    // `(int)LerpMap(1, 0.7, 0.4, 6, 0)`.
    assert_eq!(p.slow_movement_stun, 6);
    assert_close(p.swim_cycle, 0.05, "swim cycle");
    // Still, malnourished: `1 - 1.2 / (400 * (1 + 3 * InverseLerp(0.9, 1, 1)))`.
    assert_close(p.aerobic_level(), 1.0 - 1.2 / 1600.0, "recovery");
}

#[test]
fn exhausted_stun_then_fall_draw_in_order() {
    let mut p = Player::malnourished_at(Vec2::new(100.0, 48.0));
    p.aerobic_level = 1.0;
    let seed = seed_where(|a, _| a < 0.05);
    let mut rng = UnityRandom::new(seed);
    let mut reference = p.clone();
    p.exhaustion_update(&mut rng, &mut Sounds::default());
    // value < 0.05: `Stun(7)` (exhausted: 14 ticks) with its own draws, then the fall's
    // value.
    let mut expected = UnityRandom::new(seed);
    expected.value();
    reference.exhausted = true;
    reference.stun(7, &mut expected, &mut Sounds::default());
    expected.value();
    assert_eq!(rng, expected);
    assert_eq!(p.stun_ticks(), 14);
}

#[test]
fn no_draws_at_0_9_or_when_fed() {
    let mut p = Player::malnourished_at(Vec2::new(100.0, 48.0));
    p.exhausted = true;
    p.aerobic_level = 0.9;
    let mut rng = UnityRandom::new(3);
    p.exhaustion_update(&mut rng, &mut Sounds::default());
    assert_eq!(rng, UnityRandom::new(3));
    assert!(p.exhausted());
    // `(int)LerpMap(0.9, 0.7, 0.4, 6, 0)`: above 0.7 the clamped map gives 6.
    assert_eq!(p.slow_movement_stun, 6);

    let mut fed = Player::upright_at(Vec2::new(100.0, 48.0));
    fed.aerobic_level = 1.0;
    fed.exhausted = true;
    fed.exhaustion_update(&mut rng, &mut Sounds::default());
    assert_eq!(rng, UnityRandom::new(3));
    assert!(!fed.exhausted());
    // Not malnourished: `1 - 1 / 1600`.
    assert_close(fed.aerobic_level(), 1.0 - 1.0 / 1600.0, "fed recovery");
}

#[test]
fn rested_malnourished_slowness_follows_the_squared_map() {
    let mut p = Player::malnourished_at(Vec2::new(100.0, 48.0));
    p.aerobic_level = 0.5;
    p.input[0] = RIGHT;
    p.exhaustion_update(&mut UnityRandom::new(0), &mut Sounds::default());
    // `(int)LerpMap(0.5, 1, 0.4, 2, 0, 2)`: 2 * (1 - (5/6)^2) = 0.61 -> 0.
    assert_eq!(p.slow_movement_stun, 0);
    // Moving: `0.5 - 1.2 / 1100`.
    assert_close(p.aerobic_level(), 0.5 - 1.2 / 1100.0, "moving recovery");
    p.aerobic_level = 0.95;
    p.exhaustion_update(&mut UnityRandom::new(0), &mut Sounds::default());
    // 2 * (1 - (1/12)^2) = 1.986 -> 1.
    assert_eq!(p.slow_movement_stun, 1);
}

#[test]
fn lungs_exhausted_sets_the_aerobic_level_to_1() {
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.lungs_exhausted = true;
    p.exhaustion_update(&mut UnityRandom::new(0), &mut Sounds::default());
    assert_eq!(p.aerobic_level(), 1.0);
}

#[test]
fn a_jump_raises_the_aerobic_level_by_a_ninth() {
    let room = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.jump(&room);
    assert_close(p.aerobic_level(), 1.0 / 9.0, "jump");
    p.aerobic_increase(20.0);
    assert_eq!(p.aerobic_level(), 1.0);
}

#[test]
fn malnourished_waking_breathes_hard() {
    let room = flat_room();
    let mut p = Player::malnourished_at(Vec2::new(100.0, 48.0));
    // Uncurling: `sleepCounter` 40 -> 39, `sleepCurlUp = InverseLerp(10, 40, 39)`.
    p.sleep_counter = 40;
    p.sleep_update(&room, Input::NONE, (5, 2), &mut UnityRandom::new(0));
    let curl = (39.0 - 10.0) / 30.0;
    assert_close(p.sleep_curl_up(), curl, "curl");
    assert_close(p.aerobic_level(), 0.75 * (1.0 - curl), "aerobic");
}
