//! `LizardVoice` against `LizardVoice.cs` and `CreatureVoice.cs`: the seeded
//! constructor, `MakeSound`'s draw and guards, the articulation, the sound's draws and
//! the visualizer on a synthetic clip.

use std::sync::Arc;

use rw_assets::sound_table::ClipNames;
use rw_assets::unity::PcmAudio;
use rw_assets::{CreatureType, SoundTable};
use rw_random::UnityRandom;

use super::{LizardEmotion, LizardVoice, VoiceArticulation, VoiceCx, VoiceSpectrum};
use crate::color::RandomVariation;
use crate::{FloatExt, SoundEvent, SoundId, Sounds, Vec2};

const TEXT: &str = "Volume: 6\nVolume exponent: 1.8\n\
    Lizard_Voice_Green_A : LizardVoiceGreenA/vol=0.3\n";

fn table() -> SoundTable {
    let names = ClipNames::new(["lizardvoicegreena".to_owned()], None);
    SoundTable::parse(TEXT, &names).unwrap()
}

/// A 2 s full-scale 1 kHz sine at 44.1 kHz, mono.
fn sine_clip() -> PcmAudio {
    let samples = (0..88_200)
        .map(|i| {
            let t = i as f64 / 44_100.0;
            ((std::f64::consts::TAU * 1000.0 * t).sin() * 32767.0) as i16
        })
        .collect();
    PcmAudio {
        channels: 1,
        sample_rate: 44_100,
        samples,
    }
}

struct Rig {
    voice: LizardVoice,
    sounds: Sounds,
    table: Arc<SoundTable>,
    rng: UnityRandom,
    seq: u64,
    tick: u64,
}

impl Rig {
    fn new(with_clip: bool) -> Self {
        let mut t = table();
        if with_clip {
            let asset = t.clips()[0].variations[0].asset.clone();
            t.insert_pcm(&asset, sine_clip());
        }
        let table = Arc::new(t);
        Self {
            voice: LizardVoice::new(CreatureType::GreenLizard, 1234, &table),
            sounds: Sounds::new(Arc::clone(&table)),
            table,
            rng: UnityRandom::new(5),
            seq: 0,
            tick: 0,
        }
    }

    /// One `voice.Update()` in the camera's room (0).
    fn update(&mut self, holding: bool) {
        self.tick += 1;
        let mut cx = VoiceCx {
            room_index: 0,
            camera_room: 0,
            tick: self.tick,
            sounds: &mut self.sounds,
            rng: &mut self.rng,
            table: &self.table,
            update_seq: &mut self.seq,
            pos: Vec2::new(100.0, 100.0),
            holding,
            jaw_forced_shut: 0.0,
        };
        self.voice.update(&mut cx);
    }
}

/// The constructor on the ID's seed: `Range(0, 1)` over the one working green voice,
/// `ClampedRandomVariation(0.5, 0.2, 0.8) * 2` for the pitch, the two factors, the
/// shared 80-tick articulation (6 points), then one per emotion.
#[test]
fn the_constructor_replays_on_the_seed() {
    let table = table();
    let v = LizardVoice::new(CreatureType::GreenLizard, 77, &table);
    let mut e = UnityRandom::new(77);
    assert_eq!(e.range_i32(0, 1), 0);
    let pitch = e.clamped_random_variation(0.5, 0.2, 0.8) * 2.0;
    let length_fac = 0.5f32.lerp(1.5, e.value());
    let volume_fac = 0.7f32.lerp(1.3, e.value());
    let shared = VoiceArticulation {
        length: 80.0,
        max_volume: 1.0,
        modifier: (0..6).map(|_| [e.value(), e.value()]).collect(),
    };
    // SpottedPreyFirstTime: 50 ticks * factor, 2 + length / 20 points.
    let first_points = 2 + (50.0 * length_fac / 20.0) as usize;
    let first: Vec<f32> = (0..first_points * 2).map(|_| e.value()).collect();
    assert_eq!(v.my_voice_trigger, Some(SoundId::LizardVoiceGreenA));
    assert_eq!(v.my_pitch, pitch);
    assert_eq!(v.articulations.len(), 13);
    let a = &v.articulations[0];
    assert_eq!(a.length, 50.0 * length_fac);
    assert_eq!(a.max_volume, volume_fac);
    assert_eq!(a.modifier.len(), first_points);
    // The first point: volume Max(its pitch draw, Sin(0)), pitch Lerp(it, Sin(0), 0.5),
    // then each halfway to the shared curve at 0.
    let vol = first[1].max(0.0).lerp(shared.return_mod(0.0, 0), 0.5);
    let pitch0 = first[1].lerp(0.0, 0.5).lerp(shared.return_mod(0.0, 1), 0.5);
    assert_eq!(a.modifier[0], [vol, pitch0]);
    // ReSpottedPrey (12 ticks: 2 points): the second volume point is set to 1 before
    // the last blend, the pitch runs from 1 to 0 before it.
    let re = &v.articulations[1];
    assert_eq!(re.modifier.len(), 2);
    assert_eq!(
        re.modifier[1][0],
        1.0f32.lerp(shared.return_mod(1.0, 0), 0.5)
    );
    assert_eq!(
        re.modifier[0][1],
        1.0f32.lerp(shared.return_mod(0.0, 1), 0.5)
    );
    assert_eq!(
        re.modifier[1][1],
        0.0f32.lerp(shared.return_mod(1.0, 1), 0.5)
    );
}

/// No working trigger: no `Range` draw and no sound.
#[test]
fn without_a_working_trigger_the_voice_is_silent() {
    let empty = SoundTable::default();
    let v = LizardVoice::new(CreatureType::GreenLizard, 77, &empty);
    assert_eq!(v.my_voice_trigger, None);
    let mut e = UnityRandom::new(77);
    let pitch = e.clamped_random_variation(0.5, 0.2, 0.8) * 2.0;
    assert_eq!(v.my_pitch, pitch);
}

/// `ReturnMod` interpolates between the points.
#[test]
fn return_mod_is_linear_between_points() {
    let a = VoiceArticulation {
        length: 40.0,
        max_volume: 1.0,
        modifier: vec![[0.0, 1.0], [1.0, 0.0], [0.5, 0.5]],
    };
    assert_eq!(a.return_mod(0.25, 0), 0.5);
    assert_eq!(a.return_mod(0.75, 0), 0.75);
    assert_eq!(a.return_mod(1.0, 1), 0.5);
}

/// `MakeSound(emotion)` makes one draw whether or not it sounds; unconscious, holding
/// or half as intense as the current sound, nothing changes.
#[test]
fn make_sound_draws_once_and_respects_its_guards() {
    let mut r = Rig::new(false);
    let mut e = r.rng.clone();
    r.voice
        .make_sound(LizardEmotion::Fear, false, false, &mut r.rng);
    e.value();
    assert_eq!(r.rng, e);
    assert_eq!(r.voice.sound_id, None);
    r.voice
        .make_sound(LizardEmotion::Fear, true, true, &mut r.rng);
    assert_eq!(r.voice.sound_id, None);
    r.voice
        .make_sound_with(LizardEmotion::Fear, 1.0, true, false);
    assert_eq!(r.voice.articulation, Some(LizardEmotion::Fear));
    assert_eq!(r.voice.sound_id, Some(SoundId::LizardVoiceGreenA));
    r.voice
        .make_sound_with(LizardEmotion::Curious, 0.4, true, false);
    assert_eq!(r.voice.articulation, Some(LizardEmotion::Fear));
    r.voice
        .make_sound_with(LizardEmotion::Curious, 0.5, true, false);
    assert_eq!(r.voice.articulation, Some(LizardEmotion::Curious));
}

/// The update after `MakeSound` plays the trigger on the head (7 microphone draws: the
/// sample, four values, two variation picks), then sets the articulation's volume and
/// pitch; later updates keep the emitter up without draws until the articulation ends.
#[test]
fn the_sound_plays_once_and_follows_the_articulation() {
    let mut r = Rig::new(false);
    r.voice
        .make_sound_with(LizardEmotion::PainImpact, 1.0, true, false);
    let mut e = r.rng.clone();
    r.update(false);
    for _ in 0..7 {
        e.next_u32();
    }
    assert_eq!(r.rng, e, "seven draws");
    let events = r.sounds.take_events();
    assert!(matches!(&events[0], SoundEvent::Play(p) if p.id == SoundId::LizardVoiceGreenA));
    let art = r.voice.articulations[LizardEmotion::PainImpact.index().unwrap()].clone();
    let speed = 1.0f32.lerp_map(0.0, 2.0, 0.5, 1.5);
    let p = 1.0 / (art.length * speed);
    let loud = 1.0f32.lerp_map(0.0, 2.0, 0.4, 3.0);
    assert_eq!(r.voice.volume, art.return_mod(p, 0) * art.max_volume * loud);
    assert_eq!(
        r.voice.pitch,
        0.5f32.lerp(1.5, art.return_mod(p, 1)) * r.voice.my_pitch
    );
    assert!(r.voice.making_a_sound());
    let ticks = (art.length * speed).ceil() as usize;
    for _ in 1..ticks {
        r.update(false);
    }
    assert_eq!(r.rng, e, "no draws while it lasts");
    assert_eq!(r.voice.sound_id, None);
    assert!(!r.voice.making_a_sound());
    r.update(false);
    assert!(r.voice.sound_a.is_none());
}

/// Something in the jaws lets the sound go.
#[test]
fn holding_drops_the_sound() {
    let mut r = Rig::new(false);
    r.voice
        .make_sound_with(LizardEmotion::Fear, 1.0, true, false);
    r.update(false);
    assert!(r.voice.sound_a.is_some());
    r.update(true);
    assert!(r.voice.sound_a.is_none());
    assert!(
        r.sounds
            .take_events()
            .iter()
            .any(|e| matches!(e, SoundEvent::Stop(_)))
    );
}

/// A constant signal: with a periodic Blackman-Harris window only bins 0 to 3 are
/// non-zero, `|X_k| = A N c_k` with `c = (a0, a1/2, a2/2, a3/2)`, scaled by `2 / N`.
#[test]
fn the_spectrum_of_a_constant() {
    let s = VoiceSpectrum::of_samples(&[0.5; 128]);
    let c = [0.35875, 0.48829 / 2.0, 0.14128 / 2.0, 0.01168 / 2.0];
    for (k, want) in c.iter().enumerate() {
        assert!(
            (f64::from(s.bins[k]) - 2.0 * 0.5 * want).abs() < 1e-6,
            "bin {k}"
        );
    }
    assert!(s.bins[4..].iter().all(|b| b.abs() < 1e-6));
    let mean = 2.0 * 0.5 * c.iter().sum::<f64>() / 64.0;
    assert!((f64::from(s.mean()) - mean).abs() < 1e-6);
}

/// A sine on bin 10: the window spreads it over bins 7 to 13, summing to
/// `A (a0 + a1 + a2 + a3)`; past the ceiling (0.003) the intensity is 1.
#[test]
fn the_spectrum_of_a_bin_centred_sine() {
    let samples: Vec<f64> = (0..128)
        .map(|n| 0.25 * (std::f64::consts::TAU * 10.0 * n as f64 / 128.0).sin())
        .collect();
    let s = VoiceSpectrum::of_samples(&samples);
    assert!((f64::from(s.bins[10]) - 0.25 * 0.35875).abs() < 1e-6);
    let mean = 0.25 * (0.35875 + 0.48829 + 0.14128 + 0.01168) / 64.0;
    assert!((f64::from(s.mean()) - mean).abs() < 1e-6);
    assert_eq!(s.intensity(), 1.0);
    let quiet = VoiceSpectrum::of_samples(&vec![0.0; 128]);
    assert_eq!(quiet.intensity(), 0.0);
}

/// The visualizer is 0 on the tick the sound is made (no sound object yet), then reads
/// the clip where the ticks have played it to; `VisualizedIntensity` scales it by the
/// volume, and the clip running out ends it.
#[test]
fn voice_visualization_over_a_sound() {
    let mut r = Rig::new(true);
    r.voice
        .make_sound_with(LizardEmotion::Fear, 1.0, true, false);
    r.update(false);
    assert_eq!(r.voice.saved_intensity, 0.0);
    let pitch = r.voice.pitch;
    let clip_pitch = r
        .voice
        .sound_a
        .as_ref()
        .unwrap()
        .playback
        .as_ref()
        .unwrap()
        .clip_pitch;
    r.update(false);
    let p = r.voice.sound_a.as_ref().unwrap().playback.as_ref().unwrap();
    let rate = 44_100.0 * f64::from(clip_pitch * pitch);
    assert!((p.pos - rate / 40.0).abs() < 1e-6);
    let want = VoiceSpectrum::of(&sine_clip(), p.pos, rate / VoiceSpectrum::OUTPUT_RATE);
    assert_eq!(r.voice.saved_intensity, want.intensity());
    assert!(r.voice.saved_intensity > 0.0);
    assert_eq!(
        r.voice.visualized_intensity(),
        r.voice.saved_intensity * r.voice.volume
    );
}

/// In a room the camera is not in, nothing is heard and nothing shows.
#[test]
fn an_unheard_voice_shows_nothing() {
    let mut r = Rig::new(true);
    r.voice
        .make_sound_with(LizardEmotion::Fear, 1.0, true, false);
    for _ in 0..3 {
        r.tick += 1;
        let mut cx = VoiceCx {
            room_index: 1,
            camera_room: 0,
            tick: r.tick,
            sounds: &mut r.sounds,
            rng: &mut r.rng,
            table: &r.table,
            update_seq: &mut r.seq,
            pos: Vec2::ZERO,
            holding: false,
            jaw_forced_shut: 0.0,
        };
        r.voice.update(&mut cx);
        assert_eq!(r.voice.saved_intensity, 0.0);
    }
}
