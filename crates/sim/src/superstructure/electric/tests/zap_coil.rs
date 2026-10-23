//! `ZapCoil`, `ZapFlash` and `ZapCoilLight`: the constructor, the rectangle, a tick's
//! draws and branches, `TriggerZap`, the flash's life and the light's follow.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;

use super::{CoilEnv, ZAP_BLUE, ZapCoil, ZapCoilLight, ZapFlash};
use crate::{FloatExt, LightTie, SoundId, Sounds, Vec2};

fn env(gravity: f32) -> CoilEnv {
    CoilEnv {
        fully_loaded: true,
        gravity,
        broken_zero_g: false,
        switch_settled_on: false,
    }
}

/// A rectangle wider than high is horizontal; `GetFloatRect` grows the tiles' pixels by 2
/// below and left and by 2 past the next tile's edge above and right; `Vector2Inside` is
/// strict; the constructor leaves the loops on their sounds with the sizzle silent.
#[test]
fn the_constructor_and_the_rectangle() {
    let c = ZapCoil::new(7, [2, 3, 5, 4]);
    assert!(c.horizontal);
    assert!(!ZapCoil::new(7, [2, 3, 3, 8]).horizontal);
    assert!(!ZapCoil::new(7, [2, 3, 4, 5]).horizontal);
    let r = c.float_rect();
    assert_eq!(
        (r.left, r.bottom, r.right, r.top),
        (38.0, 58.0, 122.0, 102.0)
    );
    assert!(c.inside(Vec2::new(60.0, 60.0)));
    assert!(!c.inside(Vec2::new(38.0, 60.0)));
    assert!(!c.inside(Vec2::new(60.0, 102.0)));
    assert_eq!(
        (c.disruption, c.turned_on, c.last_turned_on),
        (-1.0, 1.0, 1.0)
    );
    assert!(c.powered);
    assert_eq!(c.sound_loop.sound, Some(SoundId::ZapperLoop));
    assert_eq!(c.sound_loop.volume(), 1.0);
    assert_eq!(c.disrupted_loop.sound, Some(SoundId::ZapperDisruptedLoop));
    assert_eq!(c.disrupted_loop.volume(), 0.0);
}

/// A quiet tick (no disruption, powered, no spell): the draws are the 0.005 roll, the
/// disruption decay, two for each edge's flicker (wobble, then jitter), the `< disruption`
/// roll (one, short-circuited) and `turnedOn`'s climb; gravity 1 lifts the disruption to 1.
#[test]
fn a_quiet_tick_draws_eight() {
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    let mut rng = UnityRandom::new(5);
    let mut e = UnityRandom::new(5);
    let draws: Vec<f32> = (0..8).map(|_| e.value()).collect();
    assert!(draws[0] >= 0.005, "this seed rolls no disruption");
    c.step(&env(1.0), &mut rng);
    assert_eq!(rng.state(), e.state());
    assert_eq!(c.disruption, 1.0);
    // disruption was 0 after its decay: the smooth value follows 0 and the volumes are
    // the undisrupted ones.
    assert_eq!(c.smooth_disruption, 0.0);
    assert_eq!(c.sound_loop.volume(), 1.0);
    assert_eq!(c.disrupted_loop.volume(), 0.0);
    assert_eq!(c.turned_on, 1.0);
    // Edge 0: the wobble moves by `Lerp(-1, 1, v) / 10`; with no smooth disruption the
    // edge reads 0.5.
    assert_eq!(
        c.flicker[0][3],
        ((-1.0f32).lerp(1.0, draws[2]) / 10.0).clamp(0.0, 1.0)
    );
    assert_eq!(c.flicker[0][0], 0.5);
    assert_eq!(c.flicker[1][2], 1.0 / 70.0f32.lerp(20.0, c.flicker[1][3]));
}

/// The decay of the disruption is `1 / Lerp(70, 300, v)` (the second draw), the smooth
/// value follows it by a fifth.
#[test]
fn the_disruption_decays() {
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    c.disruption = 0.9;
    let mut rng = UnityRandom::new(5);
    let mut e = UnityRandom::new(5);
    let (_, decay) = (e.value(), e.value());
    c.step(&env(0.0), &mut rng);
    assert_eq!(c.disruption, 0.9 - 1.0 / 70.0f32.lerp(300.0, decay));
    assert_eq!(
        c.smooth_disruption,
        0.0f32.lerp(0.9 - 1.0 / 70.0f32.lerp(300.0, decay), 0.2)
    );
}

/// A spell off: the counter counts down, `turnedOn` is 0 (the first roll under a half),
/// `smoothDisruption` is 1; an unpowered coil fades by 0.1.
#[test]
fn a_spell_switches_the_coil_off_and_no_power_fades_it() {
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    c.turned_off_counter = 3;
    c.disruption = 0.0;
    let mut rng = UnityRandom::new(5);
    c.step(&env(0.0), &mut rng);
    assert_eq!(c.turned_off_counter, 2);
    assert_eq!(c.smooth_disruption, 1.0);
    assert_eq!(c.turned_on, 0.0);
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    c.powered = false;
    c.step(&env(0.0), &mut UnityRandom::new(5));
    assert_eq!(c.turned_on, 1.0 - 0.1);
}

/// In a `BrokenZeroG` room: while the switch is not settled on the disruption is 1 and a
/// powered coil loses power on a roll under 0.2; settled on, an unpowered coil regains it
/// on a roll under 0.025.
#[test]
fn the_broken_gravity_switch_powers_the_coil() {
    // The roll deciding comes after the 8 draws of a quiet tick (7 when unpowered).
    let mut e = UnityRandom::new(5);
    let draws: Vec<f32> = (0..9).map(|_| e.value()).collect();
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    let mut rng = UnityRandom::new(5);
    c.step(&CoilEnv::broken(false), &mut rng);
    assert_eq!(c.disruption, 1.0);
    assert_eq!(c.powered, draws[8] >= 0.2);
    assert_eq!(rng.state(), UnityRandom::new(5).tap(9).state());
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    c.powered = false;
    let mut rng = UnityRandom::new(5);
    c.step(&CoilEnv::broken(true), &mut rng);
    assert_eq!(c.powered, draws[7] < 0.025);
    assert_eq!(rng.state(), UnityRandom::new(5).tap(8).state());
    // Powered and settled on: no roll.
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    let mut rng = UnityRandom::new(5);
    c.step(&CoilEnv::broken(true), &mut rng);
    assert_eq!(rng.state(), UnityRandom::new(5).tap(8).state());
}

impl CoilEnv {
    fn broken(settled: bool) -> Self {
        Self {
            broken_zero_g: true,
            switch_settled_on: settled,
            ..env(0.0)
        }
    }
}

trait Tap {
    fn tap(self, n: usize) -> Self;
}

impl Tap for UnityRandom {
    fn tap(mut self, n: usize) -> Self {
        for _ in 0..n {
            self.value();
        }
        self
    }
}

/// `TriggerZap`: the flash's draw, the disruption roll, the two rolls for a spell (the
/// second only under the first), `Zapper_Zap`'s draws, `zapLit` 1; the flash's size is
/// `InverseLerp(-0.05, 15, rad)` and its life time `Lerp(1, 4, v) + 2 size`.
#[test]
fn trigger_zap_draws_in_order() {
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    let mut rng = UnityRandom::new(8);
    let mut e = UnityRandom::new(8);
    let mut sounds = Sounds::default();
    let flash = c.trigger_zap(Vec2::new(60.0, 50.0), 4.0, 12, &mut rng, &mut sounds);
    let life = e.value();
    let size = 4.0f32.inverse_lerp(-0.05, 15.0);
    assert_eq!(flash.update_order, 12);
    assert_eq!(flash.size, size);
    assert_eq!(flash.life_time, 1.0f32.lerp(4.0, life) + 2.0 * size);
    let roll = e.value();
    let disruption = (-1.0f32).max(4.0f32.inverse_lerp(-0.05, 9.0) + roll * 0.5);
    assert_eq!(c.disruption, disruption);
    assert_eq!(c.smooth_disruption, disruption);
    let first = e.value();
    let mut spell = None;
    if first < disruption && e.value() < 0.5 {
        spell = Some(e.range_i32(2, 15));
    }
    assert_eq!(c.turned_off_counter, spell.unwrap_or(0));
    let mut s2 = Sounds::default();
    s2.play_at(SoundId::ZapperZap, Vec2::new(60.0, 50.0), 1.0, 1.0, &mut e);
    assert_eq!(rng.state(), e.state());
    assert_eq!(c.zap_lit, 1.0);
}

/// The flash lives until `lastLife` is below 0: with a life time of 2, the fourth update.
#[test]
fn a_flash_lives_its_life_time() {
    let mut f = ZapFlash::new(3, Vec2::new(10.0, 20.0), 0.5, &mut UnityRandom::new(1));
    f.life_time = 2.0;
    let mut ticks = 0;
    while !f.update() {
        ticks += 1;
        assert!(ticks < 10);
    }
    assert_eq!(ticks, 3);
    assert!(f.slated);
    let l = f.light_source(9, 0);
    assert_eq!((l.color, l.tied_to), (ZAP_BLUE, Some(LightTie::Fixture(3))));
    assert!(!l.environmental_light);
}

/// The light: rad from the seed, alpha 1, half affected by the palette's darkness; it
/// finds the first coil whose rectangle holds it; with a coil, one draw gives alpha
/// `num^0.8` and rad `Lerp(100, 2000, seed / 100) * Lerp(0.5, 1, num)` for `num =
/// Lerp(1, 0.85, smooth * v) * turnedOn`; without a coil it only follows the fixture.
#[test]
fn the_coil_light_follows_its_coil() {
    let pos = Vec2::new(60.0, 70.0);
    let l = ZapCoilLight::light_source(3, 0, 4, pos, 25);
    assert_eq!(l.set_rad, Some(100.0f32.lerp(2000.0, 0.25)));
    assert_eq!(l.set_alpha, Some(1.0));
    assert_eq!(l.affected_by_palette_darkness, 0.5);
    assert_eq!(l.color, ZAP_BLUE);
    let mut light = ZapCoilLight {
        update_order: 4,
        pos,
        random_seed: 25,
        light: 3,
        coil: None,
    };
    let coils = [
        ZapCoil::new(1, [20, 20, 25, 21]),
        ZapCoil::new(2, [2, 3, 5, 4]),
    ];
    light.find_coil(&coils);
    assert_eq!(light.coil, Some(2));
    let mut lights = vec![l];
    let mut rng = UnityRandom::new(2);
    light.update(&mut lights, None, &mut rng);
    assert_eq!(lights[0].set_pos, Some(pos));
    assert_eq!(rng.state(), UnityRandom::new(2).state());
    let mut e = UnityRandom::new(2);
    light.update(&mut lights, Some((0.5, 0.4)), &mut rng);
    let num = 1.0f32.lerp(0.85, 0.4 * e.value()) * 0.5;
    assert_eq!(lights[0].set_alpha, Some(num.mathf_pow(0.8)));
    assert_eq!(
        lights[0].set_rad,
        Some(100.0f32.lerp(2000.0, 0.25) * 0.5f32.lerp(1.0, num))
    );
    assert_eq!(rng.state(), e.state());
}
