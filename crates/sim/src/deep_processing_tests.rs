//! `DeepProcessing` and `DeepProcessingLight`: the constructor, the power's jumps toward
//! the room's electric power with their draws, and the light's follow.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::DeepProcessingData;
use rw_random::UnityRandom;

use super::{DeepProcessing, DeepProcessingLight};
use crate::zap_coil::ZAP_BLUE;
use crate::{FloatExt, LightTie, Vec2};

fn quad_data() -> DeepProcessingData {
    DeepProcessingData::parse("0~100~200~100~200~0~0~0~0.25~0.9~0.5").expect("data")
}

/// The first seed whose draws satisfy `want` (each draw's index in order).
fn seed_with(want: impl Fn(&[f32]) -> bool, draws: usize) -> i32 {
    (1..10_000_i32)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            let v: Vec<f32> = (0..draws).map(|_| r.value()).collect();
            want(&v)
        })
        .expect("a seed")
}

/// The constructor: the quad is the position and the three handles added to it, `power`
/// starts at 1.
#[test]
fn the_constructor_lays_out_the_quad() {
    let d = DeepProcessing::new(4, Vec2::new(10.0, 20.0), quad_data());
    assert_eq!(
        d.quad,
        [
            Vec2::new(10.0, 20.0),
            Vec2::new(10.0, 120.0),
            Vec2::new(210.0, 120.0),
            Vec2::new(210.0, 20.0),
        ]
    );
    assert_eq!(d.power, 1.0);
}

/// A tick with the roll above 1/14 draws once and changes nothing; with the power equal to
/// the electric power a winning roll draws no more.
#[test]
fn a_quiet_tick_draws_once() {
    let mut d = DeepProcessing::new(4, Vec2::ZERO, quad_data());
    let seed = seed_with(|v| v[0] >= 1.0 / 14.0, 1);
    let mut rng = UnityRandom::new(seed);
    d.update(0.5, &mut rng);
    let mut e = UnityRandom::new(seed);
    e.value();
    assert_eq!((d.power, rng.state()), (1.0, e.state()));
    d.power = 1.0;
    let seed = seed_with(|v| v[0] < 1.0 / 14.0, 1);
    let mut rng = UnityRandom::new(seed);
    d.update(1.0, &mut rng);
    let mut e = UnityRandom::new(seed);
    e.value();
    assert_eq!((d.power, rng.state()), (1.0, e.state()));
}

/// Above the electric power, a winning roll draws twice more: with the first below 0.2
/// the target is 0, else the electric power; the power drops by `1 / Lerp(1, 4, v)` but
/// not past the target (`Mathf.Max`).
#[test]
fn power_falls_toward_the_electric_power() {
    let seed = seed_with(|v| v[0] < 1.0 / 14.0 && v[1] >= 0.2, 3);
    let mut e = UnityRandom::new(seed);
    let draws: Vec<f32> = (0..3).map(|_| e.value()).collect();
    let mut d = DeepProcessing::new(4, Vec2::ZERO, quad_data());
    d.power = 0.9;
    let mut rng = UnityRandom::new(seed);
    d.update(0.3, &mut rng);
    let step = 1.0 / 1.0f32.lerp(4.0, draws[2]);
    assert_eq!(d.power, 0.3f32.max(0.9 - step));
    assert_eq!(rng.state(), e.state());

    let seed = seed_with(|v| v[0] < 1.0 / 14.0 && v[1] < 0.2, 3);
    let mut e = UnityRandom::new(seed);
    let draws: Vec<f32> = (0..3).map(|_| e.value()).collect();
    let mut d = DeepProcessing::new(4, Vec2::ZERO, quad_data());
    d.power = 0.9;
    let mut rng = UnityRandom::new(seed);
    d.update(0.3, &mut rng);
    assert_eq!(d.power, 0.0f32.max(0.9 - 1.0 / 1.0f32.lerp(4.0, draws[2])));
    assert_eq!(rng.state(), e.state());
}

/// Below the electric power: the target is 1 on a roll under 0.2, else the electric
/// power; the power rises by `1 / Lerp(1, 4, v)` but not past the target (`Mathf.Min`).
#[test]
fn power_rises_toward_the_electric_power() {
    let seed = seed_with(|v| v[0] < 1.0 / 14.0 && v[1] >= 0.2, 3);
    let mut e = UnityRandom::new(seed);
    let draws: Vec<f32> = (0..3).map(|_| e.value()).collect();
    let mut d = DeepProcessing::new(4, Vec2::ZERO, quad_data());
    d.power = 0.1;
    let mut rng = UnityRandom::new(seed);
    d.update(0.8, &mut rng);
    assert_eq!(d.power, 0.8f32.min(0.1 + 1.0 / 1.0f32.lerp(4.0, draws[2])));
    assert_eq!(rng.state(), e.state());

    let seed = seed_with(|v| v[0] < 1.0 / 14.0 && v[1] < 0.2, 3);
    let mut e = UnityRandom::new(seed);
    let draws: Vec<f32> = (0..3).map(|_| e.value()).collect();
    let mut d = DeepProcessing::new(4, Vec2::ZERO, quad_data());
    d.power = 0.1;
    let mut rng = UnityRandom::new(seed);
    d.update(0.8, &mut rng);
    assert_eq!(d.power, 1.0f32.min(0.1 + 1.0 / 1.0f32.lerp(4.0, draws[2])));
    assert_eq!(rng.state(), e.state());
}

/// The light: blue, tied to the fixture, alpha 1, no palette darkness; an update sets
/// the rad `Lerp(80, 800, seed / 100) * power * Lerp(0.9, 1.1, v)` (one draw), the
/// position and alpha `power / 2`.
#[test]
fn the_light_follows_the_electric_power() {
    let pos = Vec2::new(30.0, 40.0);
    let l = DeepProcessingLight::light_source(3, 0, 4, pos);
    assert_eq!(l.color, ZAP_BLUE);
    assert_eq!(l.tied_to, Some(LightTie::Fixture(4)));
    assert_eq!(l.set_alpha, Some(1.0));
    assert_eq!(l.set_rad, None);
    assert_eq!(l.affected_by_palette_darkness, 0.0);
    assert!(!l.environmental_light);
    let light = DeepProcessingLight {
        update_order: 4,
        pos,
        random_seed: 25,
        light: 3,
    };
    let mut lights = vec![l];
    let mut rng = UnityRandom::new(8);
    let mut e = UnityRandom::new(8);
    let v = e.value();
    light.update(&mut lights, 0.6, &mut rng);
    assert_eq!(
        lights[0].set_rad,
        Some(80.0f32.lerp(800.0, 0.25) * 0.6 * 0.9f32.lerp(1.1, v))
    );
    assert_eq!(lights[0].set_alpha, Some(0.6 * 0.5));
    assert_eq!(lights[0].set_pos, Some(pos));
    assert_eq!(rng.state(), e.state());
}
