//! `Water.waterSounds`. Expected values from the `Water` constructor and `Update`,
//! `DynamicSoundLoop.cs`, `RectangularDynamicSoundLoop.InitSound`, `Room.PlayRectSound`
//! and `VirtualMicrophone.PlaySound` (the draws of `rainworld-re/notes/sound.md` 2.8),
//! `SoundEmitter.Update` and `Custom.RestrictInRect`.

use std::sync::Arc;

use rw_assets::{ClipNames, RoomEffect, RoomEffectType, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use super::WaterSound;
use crate::item::LoopCx;
use crate::tests::{room, room_with_water};
use crate::{SoundEvent, SoundId, SoundRect, SoundSource, Sounds, Vec2};

/// The installed `sounds.txt` line: one sample, one variation.
const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\nWater_Surface_Calm_LOOP/rangeFac=0.8 : waterGurgleLOOP1/vol=0.6\n";

fn sounds() -> Sounds {
    let names = ClipNames::new(["watergurgleloop1".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(TEXT, &names).unwrap()))
}

/// 30 x 12 open tiles, water row 4.
fn pool() -> WaterSound {
    let row = ".".repeat(30);
    WaterSound::new(&room_with_water(&[row.as_str(); 12], 4)).expect("water")
}

fn silenced(amount: f32) -> RoomSettings {
    RoomSettings {
        effects: vec![RoomEffect {
            kind: RoomEffectType::SilenceWater.value().to_owned(),
            amount,
        }],
        ..RoomSettings::ANCESTOR
    }
}

struct Rig {
    table: Arc<SoundTable>,
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
            rng: UnityRandom::new(9),
            seq: 10,
            tick: 0,
            camera_room,
        }
    }

    /// One tick of room 0: `Room.Update` runs the water (the loop) before its update
    /// list (the emitters).
    fn tick(&mut self, w: &mut WaterSound, settings: &RoomSettings) -> Vec<SoundEvent> {
        self.tick += 1;
        let table = Arc::clone(&self.table);
        let mut cx = LoopCx {
            room_index: 0,
            sounds: &mut self.sounds,
            rng: &mut self.rng,
            triggers: &table,
            camera_room: self.camera_room,
            tick: self.tick,
            update_seq: &mut self.seq,
        };
        w.update(settings, &mut cx);
        let orders: Vec<u64> = w.emitters().map(|e| e.update_order).collect();
        for o in orders {
            w.update_emitter(o, &mut cx);
        }
        self.sounds.take_events()
    }
}

/// The band is `FloatRect(0, (level - 1) * 20, PixelWidth, level * 20)`; a room without
/// water has none.
#[test]
fn the_band_is_one_tile_under_the_surface_line() {
    let w = pool();
    assert_eq!(
        w.rect,
        SoundRect {
            left: 0.0,
            bottom: 60.0,
            right: 600.0,
            top: 80.0,
        }
    );
    assert_eq!(w.sound_loop.sound, Some(SoundId::WaterSurfaceCalmLoop));
    assert!(WaterSound::new(&room(&["..", ".."])).is_none());
}

/// `RestrictInRect` clamps each axis into the band.
#[test]
fn restrict_clamps_into_the_rect() {
    let r = pool().rect;
    assert_eq!(r.restrict(Vec2::new(-5.0, 300.0)), Vec2::new(0.0, 80.0));
    assert_eq!(r.restrict(Vec2::new(250.0, 70.0)), Vec2::new(250.0, 70.0));
    assert_eq!(r.restrict(Vec2::new(900.0, -40.0)), Vec2::new(600.0, 60.0));
}

/// The first update starts it (`sound` changed from `None`) at volume and pitch 1 on
/// the band: `Range(0, 1)` sample, four values, two variation picks. It then plays on
/// without draws, its emitter kept every tick.
#[test]
fn the_loop_starts_once_with_seven_draws() {
    let mut rig = Rig::new(0);
    let mut w = pool();
    let mut expect = rig.rng.clone();
    let _ = expect.range_i32(0, 1);
    for _ in 0..4 {
        let _ = expect.value();
    }
    let _ = expect.range_i32(0, 1);
    let _ = expect.range_i32(0, 1);
    let events = rig.tick(&mut w, &RoomSettings::ANCESTOR);
    assert_eq!(rig.rng, expect, "seven draws");
    let SoundEvent::Play(p) = &events[0] else {
        panic!("{events:?}");
    };
    let emitter = w.sound_loop.emitter.expect("emitter");
    assert_eq!(p.id, SoundId::WaterSurfaceCalmLoop);
    assert!(p.looped);
    assert_eq!(p.start_at, None);
    assert_eq!((p.emitter_volume, p.emitter_pitch), (1.0, 1.0));
    assert!((p.volume - 0.6).abs() < 1e-6);
    assert!((p.range - 0.8).abs() < 1e-6);
    assert_eq!(
        p.source,
        SoundSource::Rect {
            emitter: emitter.sound,
            rect: w.rect,
        }
    );
    assert_eq!(emitter.update_order, 11);
    assert!(w.playing());

    let before = rig.rng.clone();
    for _ in 0..100 {
        let events = rig.tick(&mut w, &RoomSettings::ANCESTOR);
        assert_eq!(
            events,
            vec![SoundEvent::Emitter {
                id: emitter.sound,
                volume: 1.0,
                pitch: 1.0,
            }]
        );
    }
    assert_eq!(rig.rng, before, "no draws while it plays");
}

/// `SilenceWater` sets `Volume = 1 - amount` after every update: the emitter starts at
/// 1 and takes the lowered volume the same tick.
#[test]
fn silence_water_lowers_the_volume() {
    let mut rig = Rig::new(0);
    let mut w = pool();
    let settings = silenced(0.75);
    let events = rig.tick(&mut w, &settings);
    let SoundEvent::Play(p) = &events[0] else {
        panic!("{events:?}");
    };
    assert_eq!(p.emitter_volume, 1.0);
    let id = w.sound_loop.emitter.expect("emitter").sound;
    assert!(events.contains(&SoundEvent::Emitter {
        id,
        volume: 0.25,
        pitch: 1.0,
    }));
    assert_eq!(w.sound_loop.volume(), 0.25);
    let events = rig.tick(&mut w, &settings);
    assert!(events.iter().all(|e| !matches!(e, SoundEvent::Play(_))));
    assert_eq!(w.sound_loop.volume(), 0.25);
}

/// Fully silenced, the first update still starts the loop at volume 1 (no draws are
/// skipped); the next sees `v == 0` after `lastVol` 1 and lets it go, and it never
/// starts again (no more draws or events).
#[test]
fn full_silence_lets_it_go_after_one_tick() {
    let mut rig = Rig::new(0);
    let mut w = pool();
    let settings = silenced(1.0);
    let first = rig.rng.clone();
    let events = rig.tick(&mut w, &settings);
    assert!(matches!(events[0], SoundEvent::Play(_)));
    assert_ne!(rig.rng, first);
    let id = w.sound_loop.emitter.expect("emitter").sound;
    // Let go, the emitter (after the water in this rig's order) dies the same tick.
    assert_eq!(rig.tick(&mut w, &settings), vec![SoundEvent::Stop(id)]);
    assert!(w.sound_loop.emitter.is_none());
    let before = rig.rng.clone();
    for _ in 0..10 {
        assert!(rig.tick(&mut w, &settings).is_empty());
    }
    assert_eq!(rig.rng, before);
    assert_eq!(w.emitters().count(), 0);
}

/// Unheard (camera elsewhere; the room's sink is silent: no trigger, no draws) the
/// emitter dies two updates on (`soundStillPlaying`) and the loop makes a new one.
#[test]
fn unheard_it_makes_new_emitters_without_draws() {
    let mut rig = Rig::new(1);
    rig.sounds = Sounds::default();
    let mut w = pool();
    let before = rig.rng.clone();
    rig.tick(&mut w, &RoomSettings::ANCESTOR);
    let first = w.sound_loop.emitter.expect("emitter");
    assert!(!first.playing);
    for _ in 0..6 {
        rig.tick(&mut w, &RoomSettings::ANCESTOR);
    }
    assert_ne!(
        w.sound_loop.emitter.map(|e| e.update_order),
        Some(first.update_order)
    );
    assert_eq!(rig.rng, before);
}
