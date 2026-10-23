//! `DrippingSound`. Expected values from `DrippingSound.cs`, `DynamicSoundLoop.cs`,
//! `Room.PlayDisembodiedLoop` and `VirtualMicrophone.PlayDisembodiedLoop` (the draws of
//! `rainworld-re/notes/sound.md` 2.8), `SoundEmitter.Update` and `Room.Loaded`.

use std::sync::Arc;

use rw_assets::{ClipNames, DangerType, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use super::DrippingSound;
use crate::item::LoopCx;
use crate::{FloatExt, SoundEvent, SoundId, SoundSource, Sounds};

/// `sounds.txt`'s line: one sample, one variation.
const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\nCycle_Start_Drips : rainDrips1/vol=0.5\n";

fn sounds() -> Sounds {
    let names = ClipNames::new(["raindrips1".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(TEXT, &names).unwrap()))
}

struct Rig {
    /// The camera's table (`workingTriggers`).
    table: Arc<SoundTable>,
    /// The room's sink: the camera's, or a silent one.
    sounds: Sounds,
    rng: UnityRandom,
    seq: u64,
    tick: u64,
    camera_room: i32,
}

impl Rig {
    fn new(camera_room: i32) -> Self {
        let sounds = sounds();
        Self {
            table: Arc::clone(sounds.table()),
            sounds,
            rng: UnityRandom::new(4),
            seq: 10,
            tick: 0,
            camera_room,
        }
    }

    /// One tick of room 0's update list: the emitters (older), then the sound.
    fn tick(&mut self, d: &mut DrippingSound, start_up: f32) -> Vec<SoundEvent> {
        self.tick += 1;
        let table = Arc::clone(&self.table);
        let orders: Vec<u64> = d.emitters().map(|e| e.update_order).collect();
        let mut cx = LoopCx {
            room_index: 0,
            sounds: &mut self.sounds,
            rng: &mut self.rng,
            triggers: &table,
            camera_room: self.camera_room,
            tick: self.tick,
            update_seq: &mut self.seq,
        };
        for o in orders {
            d.update_emitter(o, &mut cx);
        }
        if !d.slated {
            d.update(start_up, &mut cx);
        }
        self.sounds.take_events()
    }
}

/// `Room.Loaded` adds one while `CycleStartUp < 1`, in a room with `CeilingDrips > 0`
/// and a danger type other than `None`, unless it is a shelter.
#[test]
fn loaded_adds_it_while_the_cycle_starts() {
    let rain = RoomSettings {
        ceiling_drips: 0.3,
        danger_type: Some(DangerType::Rain),
        ..RoomSettings::ANCESTOR
    };
    assert!(DrippingSound::wanted(&rain, false, 0.99));
    assert!(!DrippingSound::wanted(&rain, false, 1.0));
    assert!(!DrippingSound::wanted(&rain, true, 0.0));
    let dry = RoomSettings {
        ceiling_drips: 0.0,
        ..rain.clone()
    };
    assert!(!DrippingSound::wanted(&dry, false, 0.0));
    let safe = RoomSettings {
        danger_type: Some(DangerType::None),
        ..rain
    };
    assert!(!DrippingSound::wanted(&safe, false, 0.0));
}

/// The first update starts the loop (`sound` changed from `None`) at the loop's initial
/// volume 1: `Range(0, 1)` sample, four values, two variation picks; then `Volume` is
/// `InverseLerp(1, 0.75, CycleStartUp) ^ 0.5`. Later updates make no draws.
#[test]
fn the_loop_starts_once_with_seven_draws() {
    let mut rig = Rig::new(0);
    let mut d = DrippingSound::new(5);
    let mut expect = rig.rng.clone();
    let _ = expect.range_i32(0, 1);
    for _ in 0..4 {
        let _ = expect.value();
    }
    let _ = expect.range_i32(0, 1);
    let _ = expect.range_i32(0, 1);
    let events = rig.tick(&mut d, 0.875);
    assert_eq!(rig.rng, expect, "seven draws");
    let SoundEvent::Play(p) = &events[0] else {
        panic!("{events:?}");
    };
    assert_eq!(p.id, SoundId::CycleStartDrips);
    assert!(p.looped);
    assert_eq!(p.emitter_volume, 1.0);
    let emitter = d.sound_loop.emitter.expect("emitter").sound;
    assert_eq!(p.source, SoundSource::DisembodiedLoop { emitter, pan: 0.0 });
    let volume = 0.5_f32.mathf_pow(0.5);
    assert!((d.sound_loop.volume() - volume).abs() < 1e-6);
    assert!(matches!(
        events.last(),
        Some(SoundEvent::Emitter { volume: v, .. }) if (*v - volume).abs() < 1e-6
    ));
    assert_eq!(d.sound_loop.emitter.map(|e| e.update_order), Some(11));

    let before = rig.rng.clone();
    for _ in 0..5 {
        let events = rig.tick(&mut d, 0.5);
        assert!(events.iter().all(|e| !matches!(e, SoundEvent::Play(_))));
    }
    assert_eq!(rig.rng, before, "no draws while it plays");
    assert_eq!(d.sound_loop.volume(), 1.0);
}

/// At `CycleStartUp` 1 it is destroyed with the volume at 0; its emitter, kept alive
/// that tick, dies unkept at its second update after (`alive` cleared, then `Destroy`)
/// and its sound is cut.
#[test]
fn it_ends_with_the_start_up() {
    let mut rig = Rig::new(0);
    let mut d = DrippingSound::new(5);
    rig.tick(&mut d, 0.0);
    rig.tick(&mut d, 1.0);
    assert!(d.slated);
    assert_eq!(d.sound_loop.volume(), 0.0);
    assert!(!d.finished());
    let id = d.sound_loop.emitter.expect("emitter").sound;
    assert!(rig.tick(&mut d, 1.0).is_empty(), "first unkept update");
    assert_eq!(rig.tick(&mut d, 1.0), vec![SoundEvent::Stop(id)]);
    assert!(d.finished());
}

/// Outside the camera's room nothing plays and nothing is drawn (`Room.PlaySound`
/// reaches no microphone); the unheard emitter dies and the loop restarts it.
#[test]
fn unheard_it_draws_nothing() {
    let mut rig = Rig::new(1);
    rig.sounds = Sounds::default();
    let mut d = DrippingSound::new(5);
    let before = rig.rng.clone();
    for _ in 0..6 {
        rig.tick(&mut d, 0.2);
    }
    assert_eq!(rig.rng, before);
}
