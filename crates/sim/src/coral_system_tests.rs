//! `CoralNeuronSystem` against `CoralNeuronSystem.cs`: the wind's draws, `Update`'s
//! counters and `Frozen`.

use rw_random::UnityRandom;

use super::CoralNeuronSystem;
use crate::Vec2;

/// `wind = RNV() * value`: the direction's draw, then the length's.
#[test]
fn the_constructor_draws_two_for_the_wind() {
    let mut rng = UnityRandom::new(3);
    let mut expect = UnityRandom::new(3);
    let s = CoralNeuronSystem::new(5, &mut rng);
    let direction = Vec2::random_unit(&mut expect);
    let length = expect.value();
    assert_eq!(rng.state(), expect.state());
    assert_eq!(s.wind, direction * length);
    assert_eq!((s.update_order, s.age, s.not_viewed_counter), (5, 0, 0));
    assert!(!s.built && s.wall_mycelia.is_empty());
}

/// `wind += RNV() * (0.2 * value)` clamped to length 1: two draws a tick.
#[test]
fn update_draws_two_and_keeps_the_wind_within_one() {
    let mut rng = UnityRandom::new(3);
    let mut s = CoralNeuronSystem::new(0, &mut rng);
    let mut expect = rng.clone();
    let wind = s.wind;
    s.update(true, &mut rng);
    let direction = Vec2::random_unit(&mut expect);
    let step = direction * (0.2 * expect.value());
    assert_eq!(rng.state(), expect.state());
    assert_eq!(s.wind, (wind + step).clamp_magnitude(1.0));
    // A wind already at 1 stays at most 1 after any number of ticks.
    s.wind = Vec2::new(1.0, 0.0);
    for _ in 0..200 {
        s.update(true, &mut rng);
        assert!(s.wind.length() <= 1.0 + 1e-6);
    }
}

/// `age++`; `notViewedCounter` is 0 while the room is viewed and counts otherwise.
#[test]
fn the_counters_follow_the_camera() {
    let mut rng = UnityRandom::new(1);
    let mut s = CoralNeuronSystem::new(0, &mut rng);
    for _ in 0..3 {
        s.update(false, &mut rng);
    }
    assert_eq!((s.age, s.not_viewed_counter), (3, 3));
    s.update(true, &mut rng);
    assert_eq!((s.age, s.not_viewed_counter), (4, 0));
}

/// `Frozen` is `age > 300 && notViewedCounter > 40`.
#[test]
fn frozen_needs_age_and_time_out_of_sight() {
    let mut rng = UnityRandom::new(1);
    let mut s = CoralNeuronSystem::new(0, &mut rng);
    for (age, away, frozen) in [
        (300, 41, false),
        (301, 40, false),
        (301, 41, true),
        (5000, 0, false),
        (5000, 1000, true),
    ] {
        s.age = age;
        s.not_viewed_counter = away;
        assert_eq!(s.frozen(), frozen, "{age} {away}");
    }
}
