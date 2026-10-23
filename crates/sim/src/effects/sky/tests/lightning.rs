//! `Lightning`. Expected values from `Lightning.cs` (`Reset`, `Update`, `LightIntensity`,
//! `Current*`), replayed by hand against a second stream.

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};
use rw_random::UnityRandom;

use super::{Lightning, LightningEnv, LightningRoom, LightningSource, ThunderSound};
use crate::item::LoopCx;
use crate::{FloatExt, ScreenMovement, SoundEvent, SoundId, Sounds};

const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\nThunder : clip\nThunder_Close : clip\nElectricity_Loop : clip\n";

const CALM: LightningEnv = LightningEnv {
    extra_loud_thunder: 0.0,
    time_until_rain: 4000,
    cycle_end_boost: 0.0,
};

/// A `Room` that records what `Lightning.Update` asks of it.
struct Rig {
    table: Arc<SoundTable>,
    sounds: Sounds,
    rng: UnityRandom,
    seq: u64,
    noise: Vec<f32>,
    movements: Vec<ScreenMovement>,
}

impl Rig {
    fn new(seed: i32) -> Self {
        let names = ClipNames::new(["clip".to_owned()], None);
        let table = Arc::new(SoundTable::parse(TEXT, &names).unwrap());
        Self {
            sounds: Sounds::new(Arc::clone(&table)),
            table,
            rng: UnityRandom::new(seed),
            seq: 0,
            noise: Vec::new(),
            movements: Vec::new(),
        }
    }
}

impl LightningRoom for Rig {
    fn rng(&mut self) -> &mut UnityRandom {
        &mut self.rng
    }

    fn make_background_noise(&mut self, noise: f32) {
        self.noise.push(noise);
    }

    fn screen_movement(&mut self, m: ScreenMovement) {
        self.movements.push(m);
    }

    fn loop_cx(&mut self) -> LoopCx<'_> {
        LoopCx {
            room_index: 0,
            sounds: &mut self.sounds,
            rng: &mut self.rng,
            triggers: &self.table,
            camera_room: 0,
            tick: 1,
            update_seq: &mut self.seq,
        }
    }
}

fn draws(rng: &mut UnityRandom, n: usize) {
    for _ in 0..n {
        rng.value();
    }
}

/// `Reset` of the front source: seven draws, in the game's order and with its formulas.
#[test]
fn reset_front_source() {
    let mut rng = UnityRandom::new(5);
    let mut e = rng.clone();
    let mut s = LightningSource::new(false);
    s.reset(0.8, 0.0, &mut rng);
    let wait = (10.0f32.lerp(440.0, e.value()) * 1.5f32.lerp(1.0, 0.8)) as i32;
    assert_eq!(s.wait, wait);
    assert_eq!(s.front_lightning_show_in_bkg, e.value() < 1.0 / 3.0);
    let distance = 0.25f32
        .lerp(0.0, 0.8)
        .lerp(0.25, e.value().mathf_pow(0.5f32.lerp(2.0, 0.8)));
    assert_eq!(s.distance, distance);
    let power = 0.7f32.lerp(1.0, e.value());
    assert_eq!(s.power, power);
    let pan = (-1.0f32).lerp(1.0, e.value());
    let pan = pan.lerp(0.0, (1.0 - distance) * e.value().mathf_pow(0.5));
    assert_eq!(s.pan, pan);
    assert_eq!(
        s.thunder_length,
        e.range_i32(1, 10.0f32.lerp(32.0, power) as i32)
    );
    assert_eq!(rng, e, "seven draws");
}

/// `Reset` of the background source: a wider wait and distance, `power` the bare draw.
#[test]
fn reset_background_source() {
    let mut rng = UnityRandom::new(6);
    let mut e = rng.clone();
    let mut s = LightningSource::new(true);
    s.reset(0.5, 0.25, &mut rng);
    let wait = (10.0f32.lerp(120.0, e.value()) * 1.5f32.lerp(1.0, 0.5) * 0.75) as i32;
    assert_eq!(s.wait, wait);
    e.value();
    let distance = 1.0f32
        .lerp(0.5, 0.5)
        .lerp(1.0, e.value().mathf_pow(0.5f32.lerp(2.0, 0.5)));
    assert_eq!(s.distance, distance);
    assert_eq!(s.power, e.value());
    draws(&mut e, 2);
    e.range_i32(1, 10.0f32.lerp(32.0, s.power) as i32);
    assert_eq!(rng, e, "seven draws");
}

/// A new `Lightning`'s first tick: each source's `randomLevelChange` expires (a `Range`
/// and a `value`) and its `thunder` runs out, so `Reset` (seven) follows: 9 each.
#[test]
fn the_first_tick_draws_nine_per_source() {
    let mut r = Rig::new(3);
    let mut l = Lightning::new(1, 0.8, false);
    l.update(CALM, &mut r);
    let mut e = UnityRandom::new(3);
    draws(&mut e, 18);
    assert_eq!(r.rng, e);
    assert!(
        r.sounds.events().is_empty(),
        "the loop is made, not started"
    );
    assert!(l.sources.iter().all(|s| s.sound_loop.is_some()));
    assert!(
        l.sources
            .iter()
            .all(|s| (1..=5).contains(&s.random_level_change))
    );
}

/// In a background-only room there are no loops.
#[test]
fn a_background_only_room_makes_no_loops() {
    let mut r = Rig::new(3);
    let mut l = Lightning::new(1, 0.5, true);
    l.update(CALM, &mut r);
    assert!(l.sources.iter().all(|s| s.sound_loop.is_none()));
}

/// `CycleEndIntensityBoost` adds one draw per source at the end of the tick (and shrinks
/// `wait`).
#[test]
fn the_cycle_end_boost_draws_once_per_source() {
    let mut r = Rig::new(3);
    let mut l = Lightning::new(1, 0.8, false);
    l.update(
        LightningEnv {
            cycle_end_boost: 0.5,
            ..CALM
        },
        &mut r,
    );
    let mut e = UnityRandom::new(3);
    draws(&mut e, 20);
    assert_eq!(r.rng, e);
    assert!(l.sources[0].intensity.is_finite());
}

#[test]
fn the_boost_runs_from_400_before_the_end_to_800_after() {
    assert_eq!(LightningEnv::boost(4800, 4400), 0.0);
    assert_eq!(LightningEnv::boost(4800, 5600), 1.0);
    assert_eq!(LightningEnv::boost(4800, 5000), 0.5);
}

/// The wait running out sets `thunder` and queues a sound at `(int)(distance * 100)`;
/// only the background source draws first (`value * 0.5 < power * (1 - distance)`).
#[test]
fn a_flash_queues_thunder() {
    let mut front = LightningSource::new(false);
    front.wait = 1;
    front.thunder_length = 6;
    front.distance = 0.2;
    front.power = 0.9;
    front.pan = -0.3;
    front.random_level_change = 3;
    let mut r = Rig::new(8);
    front.update(0.8, false, CALM, &mut r);
    assert_eq!(front.thunder, 6);
    assert_eq!(
        front.thunder_sounds,
        vec![ThunderSound {
            count_down: 19,
            distance: 0.2,
            power: 0.9,
            pan: -0.3
        }]
    );
    let mut e = UnityRandom::new(8);
    assert_eq!(r.rng, e, "the front source draws nothing");
    // It counted down in the same tick (20 to 19), and the nearest moved the loop.
    assert!(front.loop_vol > 0.0);

    let mut back = LightningSource::new(true);
    back.wait = 1;
    back.thunder_length = 6;
    back.distance = 0.2;
    back.power = 0.9;
    back.random_level_change = 3;
    let mut r = Rig::new(8);
    back.update(0.8, false, CALM, &mut r);
    let roll = e.value() * 0.5;
    assert_eq!(back.thunder_sounds.len(), usize::from(roll < 0.9 * 0.8));
    assert_eq!(r.rng, e, "one draw");

    let mut bkg_only = LightningSource::new(true);
    bkg_only.wait = 1;
    bkg_only.thunder_length = 6;
    bkg_only.distance = 0.2;
    bkg_only.power = 0.9;
    bkg_only.random_level_change = 3;
    let mut r = Rig::new(8);
    bkg_only.update(0.8, true, CALM, &mut r);
    assert!(bkg_only.thunder_sounds.is_empty());
    let mut one = UnityRandom::new(8);
    one.value();
    assert_eq!(r.rng, one, "the draw is made first");
}

/// A sound under 10 ticks off shakes the screen by `((1 - d) * p * intensity)^3 *
/// LerpMap(countDown, 10, 0, 0, 0.9 * InverseLerp(0.5, 1, intensity))`, with no source.
#[test]
fn queued_thunder_shakes_the_screen() {
    let mut s = LightningSource::new(false);
    s.thunder = 3;
    s.thunder_length = 8;
    s.random_level_change = 3;
    s.thunder_sounds.push(ThunderSound {
        count_down: 6,
        distance: 0.1,
        power: 0.8,
        pan: 0.0,
    });
    let mut r = Rig::new(2);
    s.update(0.9, false, CALM, &mut r);
    assert_eq!(r.movements.len(), 1);
    let m = r.movements[0];
    assert_eq!(m.source, None);
    assert_eq!(m.bump, crate::Vec2::ZERO);
    let want =
        ((1.0f32 - 0.1) * 0.8 * 0.9).mathf_pow(3.0) * 5.0f32.lerp_map(10.0, 0.0, 0.0, 0.9 * 0.8);
    assert!((m.shake - want).abs() < 1e-6, "{} vs {want}", m.shake);
    assert_eq!(s.thunder_sounds[0].count_down, 5);
}

/// A sound at 0 plays `Thunder` at `(1 - d) * p * InverseLerp(0.2, 1, intensity)^1.3`,
/// `Thunder_Close` scaled by `ExtraLoudThunder` after it, raises the noise counter and
/// leaves the queue; the next tick makes the noise.
#[test]
fn thunder_sounds_and_makes_noise() {
    let mut s = LightningSource::new(false);
    s.thunder = 3;
    s.thunder_length = 8;
    s.random_level_change = 3;
    s.thunder_sounds.push(ThunderSound {
        count_down: 1,
        distance: 0.1,
        power: 0.8,
        pan: 0.4,
    });
    // The source's own `pan` (a `Reset` may have redrawn it), not the queued sound's.
    s.pan = -0.7;
    let mut r = Rig::new(2);
    let env = LightningEnv {
        extra_loud_thunder: 0.5,
        ..CALM
    };
    s.update(0.9, false, env, &mut r);
    let vol = (1.0f32 - 0.1) * 0.8 * 0.9f32.inverse_lerp(0.2, 1.0).mathf_pow(1.3);
    let played: Vec<_> = r
        .sounds
        .events()
        .iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) => Some((p.id, p.source, p.emitter_volume)),
            _ => None,
        })
        .collect();
    assert_eq!(played.len(), 2);
    assert_eq!(played[0].0, SoundId::Thunder);
    assert_eq!(played[1].0, SoundId::ThunderClose);
    for p in &played {
        assert!(matches!(p.1, crate::SoundSource::Disembodied { pan, .. } if pan == -0.7));
    }
    assert!(s.thunder_sounds.is_empty());
    assert_eq!(s.noise_counter, 1.0);
    assert_eq!(s.noise_level, vol);
    // Both go through `VirtualMicrophone.PlaySound`'s draws; nothing else draws.
    let mut e = UnityRandom::new(2);
    let mut other = Rig::new(2);
    for id in [SoundId::Thunder, SoundId::ThunderClose] {
        other.sounds.play_disembodied(id, 0.4, 1.0, 1.0, &mut e);
    }
    assert_eq!(r.rng, e);
    // Next tick: the noise, then it decays by 1/120.
    s.update(0.9, false, CALM, &mut r);
    let want = 1.0f32.mathf_pow(1.4) * vol.inverse_lerp(0.1, 0.7).mathf_pow(0.5);
    assert_eq!(r.noise, vec![0.0, want]);
    assert_eq!(s.noise_counter, 1.0 - 1.0 / 120.0);
}

/// Background-only rooms shake no screens and play no thunder.
#[test]
fn a_background_only_room_is_silent() {
    let mut s = LightningSource::new(true);
    s.thunder = 3;
    s.thunder_length = 8;
    s.random_level_change = 3;
    s.thunder_sounds.push(ThunderSound {
        count_down: 1,
        distance: 0.1,
        power: 0.8,
        pan: 0.4,
    });
    let mut r = Rig::new(2);
    s.update(0.5, true, CALM, &mut r);
    assert!(r.movements.is_empty());
    assert!(r.sounds.events().is_empty());
    assert_eq!(s.thunder_sounds.len(), 1);
}

/// While a flash lasts, `intensity` is `randomLevel ^ Lerp(3, 0.1, Sin(tF * pi))`; the
/// last tick's becomes `lastIntensity`.
#[test]
fn the_flash_follows_its_curve() {
    let mut s = LightningSource::new(false);
    s.wait = 0;
    s.thunder = 5;
    s.thunder_length = 8;
    s.random_level = 0.6;
    s.random_level_change = 3;
    s.intensity = 0.25;
    let mut r = Rig::new(2);
    s.update(0.7, false, CALM, &mut r);
    assert_eq!(s.thunder, 4);
    assert_eq!(s.last_intensity, 0.25);
    let tf = 1.0 - 4.0 / 8.0;
    let want = 0.6f32.mathf_pow(3.0f32.lerp(0.1, (tf * std::f32::consts::PI).sin()));
    assert_eq!(s.intensity, want);
}

/// `LightIntensity`: dark without a draw for the front source of a background-only
/// room; else one draw, and two more (`< 0.5`, `value`) when it is below a third.
#[test]
fn light_intensity_draws_flicker() {
    let mut s = LightningSource::new(false);
    s.last_intensity = 0.2;
    s.intensity = 0.6;
    let mut rng = UnityRandom::new(1);
    let before = rng.clone();
    assert_eq!(s.light_intensity(true, 0.5, &mut rng), 0.0);
    assert_eq!(rng, before);
    for seed in 0..12 {
        let mut rng = UnityRandom::new(seed);
        let mut e = rng.clone();
        let v = s.light_intensity(false, 0.5, &mut rng);
        let mut num = 0.2f32.lerp(0.6, 0.5);
        if e.value() < 1.0 / 3.0 {
            let target = if e.value() < 0.5 { 1.0 } else { 0.0 };
            num = num.lerp(target, e.value() * num);
        }
        let x = num * 2.0 - 1.0;
        let curve = if x < 0.0 {
            let x = (1.0 + x).abs();
            0.5 * x / (0.5 - x + 1.0) * 0.5
        } else {
            0.5 + (-1.5) * x / (-1.5 - x + 1.0) * 0.5
        };
        assert!((v - curve).abs() < 1e-6, "seed {seed}");
        assert_eq!(rng, e, "seed {seed}");
    }
}

/// One `CurrentLightLevel` is one `LightIntensity` of the front source; the background
/// colour one of the background source plus the front's when it shows in the
/// background; the fog colour four. The colours stay at the palette's with no flash.
#[test]
fn the_camera_frame_makes_the_games_calls() {
    let sky = [0.1, 0.2, 0.3, 1.0];
    let fog = [0.5, 0.4, 0.3, 1.0];
    for show in [false, true] {
        let mut l = Lightning::new(1, 0.8, false);
        l.sources[0].front_lightning_show_in_bkg = show;
        let mut rng = UnityRandom::new(4);
        let mut e = rng.clone();
        let f = l.frame(1.0, sky, fog, &mut rng);
        for _ in 0..1 + 1 + usize::from(show) + 4 {
            if e.value() < 1.0 / 3.0 {
                draws(&mut e, 2);
            }
        }
        assert_eq!(rng, e, "show {show}");
        assert_eq!(f.light_level, Some(-1.0));
        assert_eq!(f.background, sky);
        assert_eq!(f.fog, fog);
    }
    let mut l = Lightning::new(1, 0.8, true);
    let mut rng = UnityRandom::new(4);
    let f = l.frame(1.0, sky, fog, &mut rng);
    assert_eq!(f.light_level, None);
    l.sources[0].intensity = 1.0;
    // A background-only room's front source stays dark.
    let f = l.frame(1.0, sky, fog, &mut rng);
    assert_eq!(f.background, sky);
}

/// A flash at full strength turns the sky toward `bkgGradient` (blue below an index of
/// 1, green above), and the fog by `pow(front^1.5 * 0.5 + back^1.2 * 0.5) * intensity`.
#[test]
fn a_flash_tints_the_sky_and_fog() {
    let sky = [0.1, 0.2, 0.3, 1.0];
    let fog = [0.5, 0.4, 0.3, 1.0];
    let mut l = Lightning::new(1, 1.0, false);
    for s in &mut l.sources {
        s.last_intensity = 1.0;
        s.intensity = 1.0;
        s.distance = 0.0;
        s.power = 1.0;
    }
    // Find a draw with no flicker so that `LightIntensity` is `SCurve(1)` = 1.
    let mut rng = (0..64)
        .map(UnityRandom::new)
        .find(|r| {
            let mut r = r.clone();
            (0..6).all(|_| r.value() >= 1.0 / 3.0)
        })
        .expect("a calm seed");
    let f = l.frame(1.0, sky, fog, &mut rng);
    assert!((f.light_level.unwrap() - 1.0).abs() < 1e-5);
    // Green gradient (index 1) at full strength: `Lerp(sky, green, 1)`.
    let green = [54.0 / 255.0, 1.0, 57.0 / 255.0, 1.0];
    for (got, want) in f.background.into_iter().zip(green) {
        assert!((got - want).abs() < 1e-6);
    }
    for (got, want) in f.fog.into_iter().zip(green) {
        assert!((got - want).abs() < 1e-6);
    }
}

/// The `Electricity_Loop` is made on a source's first tick without starting; with a
/// volume it starts as a disembodied loop at `loopPan`, and the `Pan` setter then
/// sets the emitter's pan; at volume 0 nothing starts.
#[test]
fn the_electricity_loop_starts_with_volume() {
    let mut s = LightningSource::new(false);
    s.random_level_change = 3;
    s.wait = 100;
    let mut r = Rig::new(2);
    s.update(0.8, false, CALM, &mut r);
    assert!(s.sound_loop.as_ref().is_some_and(|l| l.emitter.is_none()));
    s.update(0.8, false, CALM, &mut r);
    assert!(r.sounds.events().is_empty(), "no volume, no sound");
    s.loop_vol = 0.5;
    s.loop_pan = -0.25;
    s.update(0.8, false, CALM, &mut r);
    let events = r.sounds.take_events();
    let emitter = s
        .sound_loop
        .as_ref()
        .and_then(|l| l.emitter)
        .expect("started");
    assert!(events.iter().any(|e| matches!(e,
        SoundEvent::Play(p) if p.id == SoundId::ElectricityLoop
            && p.source == crate::SoundSource::DisembodiedLoop { emitter: emitter.sound, pan: -0.25 })));
    assert!(events.contains(&SoundEvent::EmitterPan {
        id: emitter.sound,
        pan: -0.25
    }));
    // `DynamicSoundLoop.Update` starts twice when the sound was unset and the volume
    // rose from 0 (`lastSound` is cleared while there is no emitter): two emitters.
    assert_eq!(r.seq, 2);
}
