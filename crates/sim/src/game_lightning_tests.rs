//! `Room.Loaded`'s `lightning`, its turn in the update list and the background noise it
//! makes. Expected values from `Room.cs` (`Loaded`, `Update`, `MakeBackgroundNoise`),
//! `Lightning.cs` and `NoiseTracker.HeardNoise`.

use rw_assets::{RoomEffect, RoomEffectType};

use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::Entity;
use crate::{InGameNoise, Input, RainWorldGame, Vec2};

fn effect(kind: RoomEffectType, amount: f32) -> RoomEffect {
    RoomEffect {
        kind: kind.value().to_owned(),
        amount,
    }
}

impl RainWorldGame {
    fn set_effects(&mut self, effects: Vec<RoomEffect>) {
        self.world_mut()
            .realized_mut(0)
            .expect("room")
            .settings
            .effects = effects;
    }
}

/// `Room.Loaded` makes one `Lightning` from the first `Lightning` or `BkgOnlyLightning`
/// effect, whatever its amount, joining the update list there.
#[test]
fn loaded_makes_the_first_lightning_effect() {
    let mut g = game(&HALL, (3, 1));
    g.set_effects(vec![
        effect(RoomEffectType::ExtraLoudThunder, 1.0),
        effect(RoomEffectType::BkgOnlyLightning, 0.0),
        effect(RoomEffectType::Lightning, 0.8),
    ]);
    let before = g.world().update_seq();
    g.add_lightning(0);
    let l = g.world().realized(0).and_then(|r| r.lightning.as_ref());
    let l = l.expect("made");
    assert!(l.bkg_only);
    assert_eq!(l.intensity, 0.0);
    assert_eq!(l.update_order, before + 1);
    g.set_effects(Vec::new());
    g.world_mut().realized_mut(0).unwrap().lightning = None;
    g.add_lightning(0);
    assert!(g.world().realized(0).unwrap().lightning.is_none());
}

/// Each tick the `Lightning` updates: both sources reset on the first, then the wait
/// counts down; the room's `ExtraLoudThunder` is read.
#[test]
fn it_updates_each_tick() {
    let mut g = game(&HALL, (3, 1));
    g.set_effects(vec![effect(RoomEffectType::Lightning, 0.8)]);
    g.add_lightning(0);
    g.tick(Input::NONE, &mut ());
    let l = g.world().realized(0).unwrap().lightning.clone().unwrap();
    let waits = l.sources.clone().map(|s| s.wait);
    assert!(waits.iter().all(|&w| w > 0));
    assert!(l.sources.iter().all(|s| s.thunder_length >= 1));
    g.tick(Input::NONE, &mut ());
    let l = g.world().realized(0).unwrap().lightning.clone().unwrap();
    assert_eq!(l.sources.clone().map(|s| s.wait), waits.map(|w| w - 1));
}

/// `Room.Update` keeps last tick's `backgroundNoise` for hearing and lowers it by 0.05;
/// `MakeBackgroundNoise` only raises it, up to 1.
#[test]
fn background_noise_decays_and_only_rises() {
    let mut g = game(&HALL, (3, 1));
    let r = g.world_mut().realized_mut(0).unwrap();
    r.make_background_noise(0.5);
    assert_eq!(r.last_background_noise(), 0.0);
    r.update_wait();
    assert_eq!(r.last_background_noise(), 0.5);
    r.make_background_noise(0.3);
    r.update_wait();
    assert_eq!(r.last_background_noise(), 0.5 - 0.05);
    r.make_background_noise(2.0);
    assert_eq!(r.last_background_noise(), 0.5 - 0.05);
    r.update_wait();
    assert_eq!(r.last_background_noise(), 1.0);
    r.update_wait();
    assert_eq!(r.last_background_noise(), 1.0 - 0.05);
}

/// `NoiseTracker.HeardNoise` reaches `strength * hearingSkill * (1 - BackgroundNoise)`:
/// a noise just inside earshot is lost under half-strength thunder.
#[test]
fn thunder_dulls_hearing() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    let skill = g.world.lizard_ref(l).ai.noise.hearing_skill;
    let head = g.world.lizard_ref(l).body.chunks[0].pos;
    let at = head - Vec2::new(100.0 * skill - 0.5, 0.0);
    let player = g.player_id;
    let noise = InGameNoise {
        pos: at,
        strength: 100.0,
        source: Some(Entity::Creature(player)),
        interesting: 1.0,
    };
    let r = g.world_mut().realized_mut(0).unwrap();
    r.make_background_noise(0.5);
    r.update_wait();
    let body = g.player_body();
    g.world
        .in_game_noise(0, noise, None, body.as_ref(), &mut g.rng);
    assert!(g.world.lizard_ref(l).ai.noise.sources.is_empty(), "dulled");
    let r = g.world_mut().realized_mut(0).unwrap();
    for _ in 0..11 {
        r.update_wait();
    }
    assert!(r.last_background_noise() < 1e-6);
    g.world
        .in_game_noise(0, noise, None, body.as_ref(), &mut g.rng);
    assert_eq!(
        g.world.lizard_ref(l).ai.noise.sources.len(),
        1,
        "quiet again"
    );
}
