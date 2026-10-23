//! `NeuronSpark` against `NeuronSpark.cs`: the constructor's one draw, the sound on the
//! first update (the microphone's own draws), and the fade that destroys it.

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};
use rw_random::UnityRandom;

use super::NeuronSpark;
use crate::{FloatExt, SoundEvent, SoundId, SoundSource, Sounds, Vec2};

/// The installed `sounds.txt` line of `SS_Mycelia_Spark`, one clip.
fn sounds() -> Sounds {
    let text = "Volume: 6\nVolume exponent: 1.8\n\
                SS_Mycelia_Spark/DopplerFac=0 : MyceliaSparkA/vol=0.25/RangeFac=0.5\n";
    let names = ClipNames::new(["MyceliaSparkA".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()))
}

/// `lifeTime = Lerp(1, 4, value)`: one draw; life starts at 1.
#[test]
fn the_constructor_draws_one() {
    let mut rng = UnityRandom::new(7);
    let mut expect = UnityRandom::new(7);
    let s = NeuronSpark::new(Vec2::new(3.0, 4.0), &mut rng);
    let v = expect.value();
    assert_eq!(rng.state(), expect.state());
    assert_eq!(s.life_time, 1.0f32.lerp(4.0, v));
    assert_eq!((s.life, s.last_life), (1.0, 1.0));
    assert_eq!(
        (s.pos, s.last_pos),
        (Vec2::new(3.0, 4.0), Vec2::new(3.0, 4.0))
    );
}

/// `SS_Mycelia_Spark` plays at the spark's place once, on the update that sees `life == 1`.
#[test]
fn the_sound_plays_on_the_first_update_only() {
    let mut s = NeuronSpark::new(Vec2::new(30.0, 40.0), &mut UnityRandom::new(1));
    s.life_time = 2.0;
    let mut sounds = sounds();
    let mut rng = UnityRandom::new(3);
    let mut expect = UnityRandom::new(3);
    let mut twin = sounds.clone();
    twin.play_at(
        SoundId::SsMyceliaSpark,
        Vec2::new(30.0, 40.0),
        1.0,
        1.0,
        &mut expect,
    );
    let after = expect.state();
    assert_ne!(after, UnityRandom::new(3).state());
    s.update(&mut sounds, &mut rng);
    assert_eq!(rng.state(), after);
    let events = sounds.take_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        SoundEvent::Play(p) => {
            assert_eq!(p.id, SoundId::SsMyceliaSpark);
            assert_eq!(p.source, SoundSource::Position(Vec2::new(30.0, 40.0)));
        }
        other => panic!("{other:?}"),
    }
    // Life is 0.5 now: no more sound, no draws.
    let state = rng.state();
    s.update(&mut sounds, &mut rng);
    assert_eq!(rng.state(), state);
    assert!(sounds.take_events().is_empty());
}

/// `life -= 1 / lifeTime`; it is destroyed once `lastLife < 0`: with `lifeTime` 2 the
/// life runs 1, 0.5, 0, -0.5, so on the fourth update.
#[test]
fn it_fades_and_is_destroyed() {
    let mut s = NeuronSpark::new(Vec2::ZERO, &mut UnityRandom::new(1));
    s.life_time = 2.0;
    let mut sounds = sounds();
    let mut rng = UnityRandom::new(1);
    let gone: Vec<bool> = (0..4).map(|_| s.update(&mut sounds, &mut rng)).collect();
    assert_eq!(gone, [false, false, false, true]);
    assert_eq!((s.last_life, s.life), (-0.5, -1.0));
}
