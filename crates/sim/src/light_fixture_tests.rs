//! `Redlight`, `HolyFire`, `HolyFireSprite` and `SlimeMoldLight`: their draws per call and
//! what they set on their lights. Expected values follow `Redlight.cs`, `HolyFire.cs` and
//! `SlimeMoldLight.cs`.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{HolyFire, HolyFireSprite, Redlight, SlimeMoldLight};
use crate::{FloatExt, LightElement, LightSource, LightTie, Sounds, Vec2};

const POS: Vec2 = Vec2::new(100.0, 60.0);

/// A fixture with its flat light (id 1) and its `RedlightLight` (id 2) in a room's list.
fn red_with_lights(seed: i32, broken: f32) -> (Redlight, Vec<LightSource>, UnityRandom) {
    let mut rng = UnityRandom::new(7);
    let mut red = Redlight::new(3, 1, POS, seed, false, broken, &mut rng);
    let lights = vec![
        Redlight::flat_light_source(1, 0, 3, POS),
        Redlight::mask_light_source(2, 0, 3, POS, false),
    ];
    red.light = Some(2);
    (red, lights, rng)
}

/// The constructor draws `sin = value` and `flickerWait = Range(0, 700)`; `powered`
/// is true unless the room's `BrokenZeroG` and a positive seed make it gravity-dependent.
#[test]
fn redlight_constructor_draws_two() {
    for seed in 0..20 {
        let mut rng = UnityRandom::new(seed);
        let mut want = UnityRandom::new(seed);
        let r = Redlight::new(5, 4, POS, 40, false, 0.0, &mut rng);
        assert_eq!(r.sin, want.value());
        assert_eq!(r.flicker_wait, want.range_i32(0, 700));
        assert_eq!(rng.state(), want.state());
        assert!(!r.gravity_dependent && r.powered && r.light.is_none());
        assert_eq!(r.switch_on, 0.4);
    }
    let mut rng = UnityRandom::new(1);
    let r = Redlight::new(5, 4, POS, 40, false, 0.5, &mut rng);
    assert!(r.gravity_dependent && !r.powered);
    // A zero seed is never gravity-dependent; a zero amount neither.
    let r = Redlight::new(5, 4, POS, 0, false, 0.5, &mut rng);
    assert!(!r.gravity_dependent && r.powered);
    let r = Redlight::new(5, 4, POS, 40, true, 0.0, &mut rng);
    assert!(r.submersible && !r.gravity_dependent);
}

/// The lights as the constructors make them: the flat one `(1, 0.05, 0.05)`, the other
/// `(1, 0, 0)` of the `LightMask0` element, both tied to the fixture and not environmental.
#[test]
fn redlight_lights_as_made() {
    let flat = Redlight::flat_light_source(1, 0, 3, POS);
    assert!(flat.flat && !flat.environmental_light);
    assert_eq!(flat.color, [1.0, 0.05, 0.05]);
    assert_eq!(flat.tied_to, Some(LightTie::Fixture(3)));
    assert_eq!(flat.affected_by_palette_darkness, 1.0);
    let main = Redlight::mask_light_source(2, 0, 3, POS, true);
    assert!(!main.flat && main.submersible);
    assert_eq!(main.color, [1.0, 0.0, 0.0]);
    assert_eq!(main.element, LightElement::LightMask0);
}

/// A powered, steady light: `flickerWait--`, `sin += 1 / Lerp(60, 80, value)`, then past
/// the flicker the alpha `Lerp(0.9, 1, 0.5 + sin(2 pi sin) * 0.5 * value)` and the flat
/// light's rad `Lerp(28, 32, value)`: three draws; the rad is `0.16 * Lerp(290, 310, ..)`.
#[test]
fn redlight_steady_update_draws_three() {
    let (mut red, mut lights, mut rng) = red_with_lights(0, 0.0);
    red.flicker_wait = 500;
    let sin0 = red.sin;
    let mut want = rng.clone();
    let mut sounds = Sounds::default();
    red.update(&mut lights, 1.0, &mut sounds, &mut rng);
    let sin = sin0 + 1.0 / 60.0_f32.lerp(80.0, want.value());
    let wave = f64::from(sin * std::f32::consts::PI * 2.0).sin() as f32 * 0.5;
    let alpha = 0.9_f32.lerp(1.0, 0.5 + wave * want.value());
    let flat_rad = 28.0_f32.lerp(32.0, want.value());
    assert_eq!(rng.state(), want.state());
    assert_eq!((red.flicker_wait, red.sin), (499, sin));
    let (flat, main) = (&lights[0], &lights[1]);
    assert_eq!(main.set_alpha, Some(alpha));
    assert_eq!(main.set_rad, Some(290.0_f32.lerp(310.0, 0.5 + wave) * 0.16));
    assert_eq!((flat.set_alpha, flat.set_rad), (Some(0.25), Some(flat_rad)));
    assert_eq!((main.set_pos, flat.set_pos), (Some(POS), Some(POS)));
}

/// `flickerWait < 1` redraws it (`Range(0, 700)`) and starts a flicker (`Range(1, 15)`);
/// a flicker tick is `value < 1/3` and only then `Pow(value, 0.5)` for the alphas and the
/// flat rad `30 * that`.
#[test]
fn redlight_flicker_draws() {
    for seed in 0..30 {
        let (mut red, mut lights, _) = red_with_lights(0, 0.0);
        let mut rng = UnityRandom::new(seed);
        red.flicker_wait = 1;
        let mut want = rng.clone();
        red.update(&mut lights, 1.0, &mut Sounds::default(), &mut rng);
        want.value();
        let wait = want.range_i32(0, 700);
        let flicker = want.range_i32(1, 15);
        assert_eq!((red.flicker_wait, red.flicker), (wait, flicker - 1));
        let lit = want.value() < 1.0 / 3.0;
        let n = if lit {
            Some(want.value().mathf_pow(0.5))
        } else {
            None
        };
        assert_eq!(rng.state(), want.state(), "seed {seed}");
        assert_eq!(lights[1].set_alpha, n);
        assert_eq!(lights[0].set_alpha, n.map(|n| n * 0.25));
        assert_eq!(lights[0].set_rad, n.map(|n| n * 30.0));
    }
}

/// A gravity-dependent light without power sets both alphas to 0 and draws nothing
/// until the room's lack of power passes `Lerp(0.65, 0.95, switchOn)`; then one value
/// against `1 / Lerp(20, 80, switchOn)`; on success it powers up (`Range(1, 15)`) and goes
/// on with the ordinary update.
#[test]
fn unpowered_gravity_dependent_light() {
    let (mut red, mut lights, mut rng) = red_with_lights(50, 0.5);
    assert!(red.gravity_dependent && !red.powered);
    let before = rng.state();
    // Full power: nothing to draw, both alphas 0.
    red.update(&mut lights, 1.0, &mut Sounds::default(), &mut rng);
    assert_eq!(rng.state(), before);
    assert_eq!(
        (lights[0].set_alpha, lights[1].set_alpha),
        (Some(0.0), Some(0.0))
    );
    // The threshold at seed 50 is Lerp(0.65, 0.95, 0.5) = 0.8: 0.75 lacking is not enough.
    red.update(&mut lights, 0.25, &mut Sounds::default(), &mut rng);
    assert_eq!(rng.state(), before);
    // Past it, the chance is 1 / 50; look for the seeds where it comes up and where not.
    let (mut hit, mut miss) = (false, false);
    for seed in 0..400 {
        let (mut red, mut lights, _) = red_with_lights(50, 0.5);
        red.flicker_wait = 500;
        let mut rng = UnityRandom::new(seed);
        let mut want = rng.clone();
        red.update(&mut lights, 0.1, &mut Sounds::default(), &mut rng);
        let on = want.value() < 1.0 / 50.0;
        assert_eq!(red.powered, on);
        if on {
            hit = true;
            let flicker = want.range_i32(1, 15);
            // The ordinary update follows: `sin`, then the flicker's `value < 1/3` and, if
            // so, its `Pow(value, 0.5)`.
            assert_eq!(red.flicker, flicker - 1);
            want.value();
            if want.value() < 1.0 / 3.0 {
                want.value();
            }
            assert_eq!(rng.state(), want.state());
        } else {
            miss = true;
            assert_eq!(rng.state(), want.state());
            assert_eq!(
                (lights[0].set_alpha, lights[1].set_alpha),
                (Some(0.0), Some(0.0))
            );
        }
    }
    assert!(hit && miss);
}

/// A powered gravity-dependent light draws `value < 0.05` against going dark while the
/// lack of power is under 0.6, and scales its alphas by the lack of power.
#[test]
fn powered_gravity_dependent_light() {
    let (mut red, mut lights, mut rng) = red_with_lights(50, 0.5);
    red.powered = true;
    red.flicker_wait = 500;
    let mut want = rng.clone();
    // Lack of power 0.75 is past 0.6: no draw for going dark, then the 3 of the update.
    red.update(&mut lights, 0.25, &mut Sounds::default(), &mut rng);
    for _ in 0..3 {
        want.value();
    }
    assert_eq!(rng.state(), want.state());
    assert!(red.powered);
    assert_eq!(lights[0].set_alpha, Some(0.25 * 0.75));
    // Lack of power 0.5: one more draw, which switches it off when under 0.05.
    let mut seen = [false; 2];
    for seed in 0..300 {
        let (mut red, mut lights, _) = red_with_lights(50, 0.5);
        red.powered = true;
        red.flicker_wait = 500;
        let mut rng = UnityRandom::new(seed);
        let mut want = rng.clone();
        red.update(&mut lights, 0.5, &mut Sounds::default(), &mut rng);
        let off = want.value() < 0.05;
        assert_eq!(red.powered, !off);
        seen[usize::from(off)] = true;
        for _ in 0..3 {
            want.value();
        }
        assert_eq!(rng.state(), want.state());
    }
    assert_eq!(seen, [true, true]);
}

fn fire() -> (HolyFire, Vec<LightSource>) {
    let mut lights: Vec<LightSource> = (0..3)
        .map(|i| HolyFire::light_source(i, 10 + i as u64, 0, 9, POS))
        .collect();
    lights.push(HolyFire::flat_light_source(13, 0, 9, POS));
    (HolyFire::new(9, POS, [10, 11, 12], 13), lights)
}

/// The three lights run from red to orange by index, start with an alpha of 1 set, and
/// the flat one is white.
#[test]
fn holy_fire_lights_as_made() {
    let (_, lights) = fire();
    assert_eq!(lights[0].set_alpha, Some(1.0));
    assert!(lights[3].flat && lights[3].color == [1.0; 3] && lights[3].set_alpha.is_none());
    // `HSL2RGB(Lerp(0.01, 0.07, i / 2), 1, 0.5)`: red-orange, rising in green with `i`.
    assert!(lights[0].color[1] < lights[1].color[1] && lights[1].color[1] < lights[2].color[1]);
    assert_eq!(lights[0].color[0], 1.0);
    assert!(
        lights
            .iter()
            .all(|l| l.tied_to == Some(LightTie::Fixture(9)))
    );
}

/// One update: per light a `value < 0.2` (then `RNV` and a value) for the target position
/// and a `value < 0.2` (then `Pow(value, 0.5)`) for the target rad; the sprite's 3 draws;
/// the flat light's 2. The lights ease 0.2 of the way to their targets.
#[test]
fn holy_fire_update_draws() {
    for seed in 0..40 {
        let (mut fire, mut lights) = fire();
        let mut rng = UnityRandom::new(seed);
        let mut want = rng.clone();
        let sprite = fire.update(&mut lights, &mut rng);
        let mut to_pos = [Vec2::ZERO; 3];
        let mut to_rad = [0.0_f32; 3];
        for i in 0..3 {
            if want.value() < 0.2 {
                let dir = Vec2::from_degrees(want.value() * 360.0);
                to_pos[i] = dir * (50.0 * want.value());
            }
            if want.value() < 0.2 {
                let far = 400.0_f32.lerp(200.0, i as f32 / 2.0);
                to_rad[i] = 50.0_f32.lerp(far, want.value().mathf_pow(0.5));
            }
            assert_eq!(lights[i].set_pos, Some(POS.lerp(POS + to_pos[i], 0.2)));
            assert_eq!(lights[i].set_rad, Some(0.0_f32.lerp(to_rad[i], 0.2)));
        }
        // The sprite: `RNV() * (1.5 * value)`, `Lerp(10, 40, value)`.
        let dir = Vec2::from_degrees(want.value() * 360.0);
        assert_eq!(sprite.vel, dir * (1.5 * want.value()));
        assert_eq!(sprite.life_time, 10.0_f32.lerp(40.0, want.value()));
        let flat = &lights[3];
        assert_eq!(flat.set_alpha, Some(0.2_f32.lerp(0.4, want.value())));
        assert_eq!(flat.set_rad, Some(24.0_f32.lerp(33.0, want.value())));
        assert_eq!(rng.state(), want.state(), "seed {seed}");
        assert_eq!(sprite.pos, POS);
    }
}

/// The ember moves by its velocity first, then slows (`* 0.8`), rises (`+ 0.4`) and takes
/// `RNV() * (value * 0.5)` (two draws); its life falls by `1 / lifeTime` and it is
/// destroyed once under 0.
#[test]
fn holy_fire_sprite_update() {
    let mut rng = UnityRandom::new(3);
    let mut s = HolyFireSprite::new(POS, &mut rng);
    s.life_time = 2.0;
    s.vel = Vec2::new(10.0, 0.0);
    let mut want = rng.clone();
    s.update(&mut rng);
    let dir = Vec2::from_degrees(want.value() * 360.0);
    let vel = Vec2::new(8.0, 0.4) + dir * (want.value() * 0.5);
    assert_eq!(rng.state(), want.state());
    assert_eq!((s.last_pos, s.pos), (POS, POS + Vec2::new(10.0, 0.0)));
    assert_eq!(s.vel, vel);
    assert_eq!((s.last_life, s.life, s.slated), (1.0, 0.5, false));
    s.update(&mut rng);
    assert_eq!((s.life, s.slated), (0.0, false));
    s.update(&mut rng);
    assert!(s.life < 0.0 && s.slated);
}

/// The first update lays the discs out from `InitState((int)(x + y))` (four draws a disc,
/// none on the shared stream, whose state is restored); an update at the same position
/// does nothing.
#[test]
fn slime_mold_light_lays_out_once() {
    let mut light = SlimeMoldLight::new(4, Vec2::new(100.7, 60.6), 30);
    let mut rng = UnityRandom::new(11);
    let before = rng.state();
    light.update(&mut rng);
    assert_eq!(rng.state(), before);
    let mut seeded = UnityRandom::new(161);
    for i in 0..5 {
        let dir = Vec2::from_degrees(seeded.value() * 360.0);
        assert_eq!(light.positions[i], dir * seeded.value());
        assert_eq!(
            light.sizes_and_alphas[i][0],
            70.0_f32.lerp(200.0, seeded.value())
        );
        assert_eq!(
            light.sizes_and_alphas[i][1],
            0.2_f32.lerp(0.35, seeded.value())
        );
    }
    let laid = light.clone();
    light.positions[0] = Vec2::ZERO;
    light.update(&mut rng);
    assert_eq!(light.positions[0], Vec2::ZERO);
    assert_eq!(rng.state(), before);
    light.pos = Vec2::new(10.0, 10.0);
    light.update(&mut rng);
    assert_ne!(light.positions, laid.positions);
    assert_eq!(rng.state(), before);
}
