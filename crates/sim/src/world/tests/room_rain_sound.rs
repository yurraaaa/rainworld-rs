//! `RoomRain`'s sound loops. Expected values from `RoomRain.cs` (constructor and
//! `Update`), `DynamicSoundLoop.cs` and `sounds.txt`.
//!
//! Mirrors: - (port-only: tests)

use std::f32::consts::PI;
use std::sync::Arc;

use rw_assets::{ClipNames, DangerType, SoundTable};
use rw_random::UnityRandom;

use super::{RainLoops, RainSoundInput};
use crate::item::LoopCx;
use crate::{SoundEvent, SoundId, Sounds};

const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\n\
    Normal_Rain_LOOP : a/vol=0.7\nHeavy_Rain_LOOP : a/vol=0.6\nDeath_Rain_LOOP : a/vol=0.6\n\
    Death_Rain_Heard_From_Underground_LOOP : a/vol=0.5\nFlash_Flood_LOOP : a/vol=0.3\n\
    Death_Rain_Approaching_LOOP : a/vol=0.5\n\
    Death_Rain_Approaching_Heard_From_Underground_LOOP : a/vol=0.4\n\
    Death_Rain_Rumble_LOOP : a/vol=0.5\nScreen_Shake_LOOP : a/vol=0.5\n";

struct Rig {
    table: Arc<SoundTable>,
    sounds: Sounds,
    rng: UnityRandom,
    seq: u64,
    tick: u64,
}

impl Rig {
    fn new() -> Self {
        let names = ClipNames::new(["a".to_owned()], None);
        let sounds = Sounds::new(Arc::new(SoundTable::parse(TEXT, &names).unwrap()));
        Self {
            table: Arc::clone(sounds.table()),
            sounds,
            rng: UnityRandom::new(4),
            seq: 10,
            tick: 0,
        }
    }

    fn tick(&mut self, loops: &mut RainLoops, input: &RainSoundInput) -> Vec<SoundEvent> {
        self.tick += 1;
        let table = Arc::clone(&self.table);
        let mut cx = LoopCx {
            room_index: 0,
            sounds: &mut self.sounds,
            rng: &mut self.rng,
            triggers: &table,
            camera_room: 0,
            tick: self.tick,
            update_seq: &mut self.seq,
        };
        loops.update(input, &mut cx);
        self.sounds.take_events()
    }
}

fn input(intensity: f32) -> RainSoundInput {
    RainSoundInput {
        intensity,
        rumble_sound: 0.0,
        rumble_intensity: 1.0,
        rain_intensity: 1.0,
        time_until_rain: 5000,
        flood_speed: 0.0,
        void_sea: false,
        camera_screen_shake: None,
    }
}

fn sounds_of(loops: &RainLoops) -> Vec<(SoundId, usize)> {
    loops
        .each()
        .map(|l| (l.sound.unwrap(), l.volume_group))
        .collect()
}

/// A `Rain` room makes no flooding loop; a `Flood` room (a shelter's too) no normal or
/// heavy loop, and the underground clips and groups 3 for flooding; `FloodAndRain` makes
/// all seven.
#[test]
fn the_loops_by_danger_type() {
    assert_eq!(
        sounds_of(&RainLoops::new(DangerType::Rain)),
        vec![
            (SoundId::NormalRainLoop, 3),
            (SoundId::HeavyRainLoop, 3),
            (SoundId::DeathRainLoop, 3),
            (SoundId::DeathRainRumbleLoop, 3),
            (SoundId::DeathRainApproachingLoop, 0),
            (SoundId::ScreenShakeLoop, 0),
        ]
    );
    assert_eq!(
        sounds_of(&RainLoops::new(DangerType::Flood)),
        vec![
            (SoundId::DeathRainHeardFromUndergroundLoop, 3),
            (SoundId::DeathRainRumbleLoop, 3),
            (SoundId::DeathRainApproachingHeardFromUndergroundLoop, 0),
            (SoundId::FlashFloodLoop, 3),
            (SoundId::ScreenShakeLoop, 0),
        ]
    );
    let both = sounds_of(&RainLoops::new(DangerType::FloodAndRain));
    assert_eq!(both.len(), 7);
    assert_eq!(both[5], (SoundId::FlashFloodLoop, 0));
    assert!(RainLoops::new(DangerType::AerieBlizzard).flooding.is_none());
}

/// The volume formulas at given intensities.
#[test]
fn the_volume_formulas() {
    assert_eq!(RainLoops::normal_volume(0.0), 0.0);
    let i = 0.35_f32;
    let want = 0.1 + 0.9 * (((i - 0.001) / 0.699) * PI).sin().powf(1.5);
    assert!((RainLoops::normal_volume(i) - want).abs() < 1e-5);
    assert!((RainLoops::death_volume(0.55) - 0.5_f32.powf(0.8)).abs() < 1e-6);
    assert_eq!(RainLoops::death_volume(0.2), 0.0);
    let heavy = RainLoops::heavy_volume(0.31, 0.25);
    let want = (0.19_f32 / 0.38).powf(0.85) * 0.75_f32.powf(0.3);
    assert!((heavy - want).abs() < 1e-6);
}

/// `Update` sets each loop's volume: rumble `RumbleSound * RumbleIntensity`, the distant
/// rumble `InverseLerp(1400, 0, TimeUntilRain) * RainIntensity`, the flood loop
/// `InverseLerp(0.01, 0.5, floodSpeed)` (not with `VoidSea`), the screen shake
/// `ScreenShake * (1 - rumble volume)` only in the camera's room.
#[test]
fn update_sets_the_volumes() {
    let mut rig = Rig::new();
    let mut loops = RainLoops::new(DangerType::FloodAndRain);
    let mut inp = input(0.4);
    inp.rumble_sound = 0.5;
    inp.rumble_intensity = 0.8;
    inp.rain_intensity = 0.5;
    inp.time_until_rain = 700;
    inp.flood_speed = 0.255;
    inp.camera_screen_shake = Some(0.6);
    rig.tick(&mut loops, &inp);
    assert!((loops.rumble.volume() - 0.4).abs() < 1e-6);
    assert!((loops.distant.volume() - 0.25).abs() < 1e-6);
    assert!((loops.flooding.as_ref().unwrap().volume() - 0.5).abs() < 1e-5);
    assert!((loops.screen_shake.volume() - 0.6 * 0.6).abs() < 1e-6);
    let d = RainLoops::death_volume(0.4);
    assert!((loops.death.volume() - d).abs() < 1e-6);
    inp.camera_screen_shake = None;
    inp.void_sea = true;
    inp.flood_speed = 0.5;
    rig.tick(&mut loops, &inp);
    assert_eq!(loops.screen_shake.volume(), 0.0);
    assert!((loops.flooding.as_ref().unwrap().volume() - 0.5).abs() < 1e-5);
}

/// Loops start when their volume first passes 0 and make no draws before: with the
/// intensity 0 no sound plays; at 0.4 the normal, heavy, death, rumble (a nonzero
/// `RumbleSound`) and distant loops start, each with the game's seven draws
/// (`startAtRandomTime` off), and the rain loops are set to volume group 3. Each starts
/// once, where the game's `lastVol == 0` arm starts it a second time (D48).
#[test]
fn loops_start_with_seven_draws_and_their_group() {
    let mut rig = Rig::new();
    let mut loops = RainLoops::new(DangerType::Rain);
    let before = rig.rng.clone();
    let events = rig.tick(&mut loops, &input(0.0));
    assert!(events.iter().all(|e| !matches!(e, SoundEvent::Play(_))));
    assert_eq!(rig.rng, before, "silent loops draw nothing");
    let mut inp = input(0.4);
    inp.rumble_sound = 0.5;
    inp.time_until_rain = 0;
    let events = rig.tick(&mut loops, &inp);
    let plays = events
        .iter()
        .filter(|e| matches!(e, SoundEvent::Play(_)))
        .count();
    assert_eq!(plays, 5);
    let groups: Vec<usize> = events
        .iter()
        .filter_map(|e| match e {
            SoundEvent::EmitterGroup { group, .. } => Some(*group),
            _ => None,
        })
        .collect();
    assert_eq!(groups, vec![3; 4], "normal, heavy, death, rumble");
    let mut expect = before;
    for _ in 0..plays {
        let _ = expect.range_i32(0, 1);
        for _ in 0..4 {
            let _ = expect.value();
        }
        let _ = expect.range_i32(0, 1);
        let _ = expect.range_i32(0, 1);
    }
    assert_eq!(rig.rng, expect, "seven draws a loop");
    let again = rig.rng.clone();
    rig.tick(&mut loops, &inp);
    assert_eq!(rig.rng, again, "no draws while they play");
}
