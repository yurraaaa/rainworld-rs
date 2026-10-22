//! The player's `room.PlaySound` calls (`rainworld-re/notes/sound.md` 2.7) against a
//! small `sounds.txt` whose clips all exist.

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};
use rw_random::UnityRandom;

use super::{JUMP, flat_room};
use crate::tests::room;
use crate::{HEAD, HIPS, Input, Player, SoundEvent, SoundId, SoundSource, Sounds, Vec2};

impl Player {
    fn step_with(&mut self, room: &rw_assets::Room, input: Input, sounds: &mut Sounds) {
        self.update(room, None, input, &mut UnityRandom::new(5), sounds);
    }
}

/// Every ported ID with one existing clip.
fn all_sounds() -> Sounds {
    let mut text = "Volume: 6\nVolume exponent: 1.8\n".to_owned();
    for id in SoundId::ALL {
        text.push_str(&format!("{} : clip\n", id.name()));
    }
    let names = ClipNames::new(["clip".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(&text, &names).unwrap()))
}

/// The IDs played this tick with their chunk and emitter volume.
fn played(sounds: &mut Sounds) -> Vec<(SoundId, usize, f32)> {
    sounds
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) => match p.source {
                SoundSource::Chunk { chunk, .. } => Some((p.id, chunk, p.emitter_volume)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn standing_jump_plays_the_normal_jump() {
    let r = flat_room();
    let mut s = all_sounds();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.step_with(&r, JUMP, &mut s);
    assert!(played(&mut s).is_empty());
    p.step_with(&r, JUMP, &mut s);
    assert_eq!(played(&mut s), [(SoundId::SlugcatNormalJump, HEAD, 1.0)]);
}

#[test]
fn fall_above_35_plays_the_hard_impact_and_starts_the_stunned_loop() {
    // The drop of `long_drop_in_a_tall_room_stuns`: both chunks hit the floor at 46.4
    // in one tick, head first; each impact plays the hard sound (and stuns).
    let mut rows = vec![".".repeat(30); 68];
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let mut s = all_sounds();
    let mut p = Player::lying_at(Vec2::new(100.0, 1300.0));
    for _ in 0..52 {
        p.step_with(&r, Input::NONE, &mut s);
    }
    assert!(played(&mut s).is_empty());
    p.step_with(&r, Input::NONE, &mut s);
    assert_eq!(
        played(&mut s),
        [
            (SoundId::SlugcatTerrainImpactHard, HEAD, 1.0),
            (SoundId::UiSlugcatStunnedInit, HEAD, 1.0),
            (SoundId::SlugcatTerrainImpactHard, HEAD, 1.0),
        ]
    );
    // Next tick the slide loop turns into the stunned loop, refreshed at once.
    p.step_with(&r, Input::NONE, &mut s);
    let events = s.take_events();
    let SoundEvent::Play(start) = &events[0] else {
        panic!("{events:?}")
    };
    assert_eq!(
        (start.id, start.looped),
        (SoundId::UiSlugcatStunnedLoop, true)
    );
    // The loop is chosen before the stun counts down: stun 54 of last 54, pitch
    // 0.5 + inverseLerp(11, 54, 54) = 1.5, volume inverseLerp(8, 27, 54)^0.7 = 1.
    assert_eq!(
        events[1],
        SoundEvent::Emitter {
            id: crate::EmitterId(3),
            volume: 1.0,
            pitch: 1.5
        }
    );
}

#[test]
fn landing_softly_while_standing_plays_the_standard_floor_impact() {
    // Standing hips (chunk 1) land at 9.5: volume inverseLerp(8, 11, 9.5) = 0.5.
    let mut s = all_sounds();
    let mut p = Player::upright_at(Vec2::new(100.0, 52.0));
    p.receive_impact_with(
        crate::TerrainImpact {
            chunk: HIPS,
            direction: crate::Contact { x: 0, y: -1 },
            speed: 9.5,
            first_contact: true,
        },
        &mut s,
    );
    assert_eq!(
        played(&mut s),
        [(SoundId::SlugcatFloorImpactStandard, HEAD, 0.5)]
    );
}

/// The noises a player made, with the random state each was made at.
#[derive(Default)]
struct NoiseLog(Vec<(Vec2, f32, f32, UnityRandom)>);

impl crate::NoiseSink for NoiseLog {
    fn in_game_noise(&mut self, pos: Vec2, strength: f32, interesting: f32, rng: &mut UnityRandom) {
        self.0.push((pos, strength, interesting, rng.clone()));
    }

    fn screen_movement(&mut self, _: crate::ScreenMovement, _: &mut UnityRandom) {}
}

/// `UpdateBodyMode`'s `Stand`: each footstep (every seventh frame walking) plays its
/// sound and makes a noise at the hips, strength 150, interesting 1.
#[test]
fn footsteps_make_noises() {
    let room = flat_room();
    let mut p = Player::upright_at(Vec2::new(200.0, 50.0));
    let mut sounds = all_sounds();
    let mut rng = UnityRandom::new(5);
    let mut log = NoiseLog::default();
    let mut steps = 0;
    for _ in 0..40 {
        if p.update_before_catch(&room, None, Input::new(1, 0, false), &mut rng, &mut sounds) {
            p.update_after_catch(&room, None, &mut rng, &mut sounds, &mut log);
        }
        steps += played(&mut sounds)
            .iter()
            .filter(|(id, ..)| matches!(id, SoundId::SlugcatStepA | SoundId::SlugcatStepB))
            .count();
    }
    // The first noise is the landing's `TerrainImpact`, below the chunk that hit.
    let landing = log.0.remove(0);
    assert!(landing.1 > 150.0 && landing.0.y < 45.0);
    assert!(steps >= 4, "{steps} steps");
    assert_eq!(log.0.len(), steps);
    for (pos, strength, interesting, _) in &log.0 {
        assert_eq!((*strength, *interesting), (150.0, 1.0));
        assert!((pos.y - 50.0).abs() < 10.0);
    }
}

/// Queued noises are made where they were called among the queued sounds: one made
/// after a sound hears the sound's draws done, one made before it none.
#[test]
fn noises_keep_their_place_among_sounds() {
    let mut sounds = all_sounds();
    let mut p = Player::new();
    p.play_sound(SoundId::SlugcatStepA, HEAD, 1.0, 1.0);
    let mut alone = UnityRandom::new(3);
    p.flush_sounds(&mut alone, &mut sounds);

    let mut rng = UnityRandom::new(3);
    let mut log = NoiseLog::default();
    p.make_noise(Vec2::ZERO, 10.0, 1.0);
    p.play_sound(SoundId::SlugcatStepA, HEAD, 1.0, 1.0);
    p.make_noise(Vec2::ZERO, 20.0, 1.0);
    p.flush_sounds_and_noises(&mut rng, &mut sounds, &mut log);
    assert_eq!(log.0.len(), 2);
    assert_eq!(log.0[0].3, UnityRandom::new(3));
    assert_eq!(log.0[1].3, alone);
    assert_ne!(alone, UnityRandom::new(3));
}

/// `privSneak` rises 0.1 a tick low and falls 0.04 otherwise; the chunks' loudness is
/// `1.5` (head) and `0.7` (hips) times `(1 - Sneak) * loudnessFac`, `Sneak` being
/// `Lerp(Pow(InverseLerp(0, 0.5, privSneak), 0.5), 0.5, submersion)`.
#[test]
fn sneaking_quietens_the_chunks() {
    let mut p = Player::upright_at(Vec2::new(200.0, 50.0));
    p.priv_sneak = 0.3;
    p.update_sneak(None);
    assert!((p.priv_sneak - 0.26).abs() < 1e-6);
    let quiet = 1.0 - 0.52f32.sqrt();
    assert!((p.sneak(None) - 0.52f32.sqrt()).abs() < 1e-6);
    assert!((p.body.chunks[HEAD].loudness - 1.5 * quiet).abs() < 1e-6);
    assert!((p.body.chunks[HIPS].loudness - 0.7 * quiet).abs() < 1e-6);
}
