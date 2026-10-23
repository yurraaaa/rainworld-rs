//! Draw order from `rainworld-re/notes/sound.md` 2.8 (`VirtualMicrophone.PlaySound`,
//! `SoundTrigger.GetSoundData`, `SoundLoader.GetAudioClip`), replayed by hand on a
//! second generator.

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};
use rw_random::UnityRandom;

use super::{EmitterId, PlayedSound, SoundEvent, SoundId, SoundSource, Sounds, StaticSoundLoop};
use crate::FloatExt;

const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\n\
        Slugcat_Normal_Jump/minVol=0.5/maxVol=1/minPitch=0.9/maxPitch=1.1/rangeFac=0.5 : \
        clipA/minVol=0.2/maxVol=0.4, clipB/pitch=1.5/ignoreEffects=0.5\n\
        Slugcat_Super_Jump/PLAYALL : clipB, clipA\n\
        Player_Tick_Along_In_Shortcut/silentChance=0.25 : clipB\n\
        Slugcat_Wall_Jump : solo\n\
        UI_Slugcat_Die : missing\n";

fn sounds() -> Sounds {
    let names = ["clipa_1", "clipa_2", "clipa_3", "clipb", "solo", "solo_2"];
    let names = ClipNames::new(names.map(str::to_owned), None);
    Sounds::new(Arc::new(SoundTable::parse(TEXT, &names).unwrap()))
}

/// `rng` after `n` raw steps from `seed`.
fn advanced(seed: i32, n: usize) -> UnityRandom {
    let mut rng = UnityRandom::new(seed);
    for _ in 0..n {
        rng.next_u32();
    }
    rng
}

#[test]
fn one_shot_draws_pick_four_values_and_two_variation_picks() {
    let mut s = sounds();
    let mut rng = UnityRandom::new(42);
    let id = s.play_on_chunk(SoundId::SlugcatNormalJump, 0, false, 0.8, 1.2, &mut rng);
    assert_eq!(id, EmitterId(0));

    // By hand: Range(0, 2) sample, value x4 (sample vol, trigger vol, sample pitch,
    // trigger pitch), then Range(0, variations) twice (SoundClipReady, SoundObject).
    let mut e = UnityRandom::new(42);
    let sample = e.range_i32(0, 2) as usize;
    let (r1, r2, _r3, r4) = (e.value(), e.value(), e.value(), e.value());
    let variations = if sample == 0 { 3 } else { 1 };
    let _ready = e.range_i32(0, variations);
    let variation = e.range_i32(0, variations) as usize;
    assert_eq!(rng, e);
    assert_eq!(rng, advanced(42, 7), "seven draws");

    let (vol, pitch, clip, ignore) = if sample == 0 {
        (0.2.lerp(0.4, r1), 1.0, 0, 0.0)
    } else {
        (1.0, 1.5, 1, 0.5)
    };
    let want = PlayedSound {
        id: SoundId::SlugcatNormalJump,
        clip,
        variation,
        volume: vol * 0.5.lerp(1.0, r2),
        pitch: pitch * 0.9.lerp(1.1, r4),
        range: 0.5,
        doppler_fac: 1.0,
        ignore_effects: ignore,
        start_at: None,
        source: SoundSource::Chunk {
            emitter: EmitterId(0),
            chunk: 0,
        },
        looped: false,
        emitter_volume: 0.8,
        emitter_pitch: 1.2,
    };
    assert_eq!(s.take_events(), [SoundEvent::Play(want)]);
}

#[test]
fn a_static_loop_restarts_when_heard_again() {
    let mut heard = sounds();
    let mut other_room = Sounds::default();
    let mut rng = UnityRandom::new(1);
    let mut l = StaticSoundLoop::new(SoundId::SlugcatWallJump, crate::Vec2::ZERO, 1.0, 1.0);
    l.update(&mut heard, &mut rng);
    assert_eq!(l.emitter, Some(EmitterId(0)));
    l.update(&mut other_room, &mut rng);
    assert_eq!(l.emitter, None);
    heard.take_events();
    l.update(&mut heard, &mut rng);
    assert_eq!(l.emitter, Some(EmitterId(1)));
    assert!(matches!(heard.take_events()[..], [SoundEvent::Play(_)]));
}

#[test]
fn the_sample_pick_covers_both_samples() {
    let picks: Vec<usize> = (0..20)
        .filter_map(|seed| {
            let mut s = sounds();
            s.play_at(
                SoundId::SlugcatNormalJump,
                crate::Vec2::ZERO,
                1.0,
                1.0,
                &mut UnityRandom::new(seed),
            );
            match &s.take_events()[..] {
                [SoundEvent::Play(p)] => Some(p.clip),
                _ => None,
            }
        })
        .collect();
    assert_eq!(picks.len(), 20);
    assert!(picks.contains(&0) && picks.contains(&1));
}

#[test]
fn play_all_draws_per_sample_without_a_pick() {
    let mut s = sounds();
    let mut rng = UnityRandom::new(3);
    s.play_disembodied(SoundId::SlugcatSuperJump, -0.5, 1.0, 1.0, &mut rng);
    // Per sample: 4 values + 2 variation picks.
    assert_eq!(rng, advanced(3, 12));
    let clips: Vec<usize> = s
        .take_events()
        .iter()
        .map(|e| match e {
            SoundEvent::Play(p) => {
                assert_eq!(
                    p.source,
                    SoundSource::Disembodied {
                        pan: -0.5,
                        group: 0
                    }
                );
                p.clip
            }
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(clips, [1, 0]);
}

#[test]
fn silent_chance_draws_first_and_may_stop() {
    let (mut played, mut silenced) = (0, 0);
    for seed in 0..40 {
        let mut s = sounds();
        let mut rng = UnityRandom::new(seed);
        let first = UnityRandom::new(seed).value();
        s.play_disembodied(SoundId::PlayerTickAlongInShortcut, 0.0, 1.0, 1.0, &mut rng);
        if first > 0.25 {
            assert_eq!(rng, advanced(seed, 1 + 7), "seed {seed}");
            assert_eq!(s.events().len(), 1);
            played += 1;
        } else {
            assert_eq!(rng, advanced(seed, 1), "seed {seed}");
            assert!(s.events().is_empty());
            silenced += 1;
        }
    }
    assert!(played > 0 && silenced > 0);
}

#[test]
fn dead_and_unknown_ids_draw_nothing() {
    let mut s = sounds();
    let mut rng = UnityRandom::new(9);
    let a = s.play_on_chunk(SoundId::UiSlugcatDie, 0, false, 1.0, 1.0, &mut rng);
    let b = s.play_on_chunk(SoundId::SlugcatStepA, 1, true, 1.0, 1.0, &mut rng);
    assert_eq!(rng, UnityRandom::new(9));
    assert!(s.events().is_empty());
    assert_eq!(
        (a, b),
        (EmitterId(0), EmitterId(1)),
        "emitters exist anyway"
    );
    assert!(Sounds::default().table().ids().next().is_none());
}

#[test]
fn a_missing_variation_creates_no_sound() {
    // `solo` exists bare and as `solo_2`: two variations, `solo_1` and `solo_2`, and
    // only the second loads.
    let (mut played, mut dropped) = (0, 0);
    for seed in 0..40 {
        let mut s = sounds();
        let mut rng = UnityRandom::new(seed);
        let mut e = UnityRandom::new(seed);
        e.range_i32(0, 1);
        for _ in 0..4 {
            e.value();
        }
        let ready = e.range_i32(0, 2);
        s.play_disembodied(SoundId::SlugcatWallJump, 0.0, 1.0, 1.0, &mut rng);
        if ready == 1 {
            let variation = e.range_i32(0, 2) as usize;
            match &s.take_events()[..] {
                [SoundEvent::Play(p)] => assert_eq!(p.variation, variation),
                other => panic!("{other:?}"),
            }
            played += 1;
        } else {
            assert!(s.events().is_empty());
            dropped += 1;
        }
        assert_eq!(rng, e, "seed {seed}");
    }
    assert!(played > 0 && dropped > 0);
}
