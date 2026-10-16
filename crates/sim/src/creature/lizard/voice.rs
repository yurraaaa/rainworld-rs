//! `LizardVoice` with its base `CreatureVoice` (decompile `LizardVoice.cs`,
//! `CreatureVoice.cs`): an articulated one-shot on the head chunk whose volume and pitch
//! follow the emotion's articulation, replayed while the emotion lasts, and the
//! `IntensityVisualizer` the graphics read.
//!
//! The game's visualizer reads `AudioSource.GetSpectrumData` of the playing sound, which
//! the audio thread fills; the port computes it in the sim from the decoded clip at the
//! position the ticks have played to ([`VoiceSpectrum`]; docs/DIVERGENCES.md D12).

use rw_assets::unity::PcmAudio;
use rw_assets::{CreatureType, SoundTable};
use rw_random::UnityRandom;

use crate::color::RandomVariation;
use crate::sound::{LoopEmitter, SoundEvent, SoundSource};
use crate::{FloatExt, SoundId, Sounds, Vec2};

/// `LizardVoice.Emotion` (`Love` is Remix's `MMFEnums.LizardVoiceEmotion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LizardEmotion {
    SpottedPreyFirstTime,
    ReSpottedPrey,
    GeneralSmallNoise,
    Curious,
    Dominance,
    Submission,
    PainImpact,
    PainIdle,
    Frustration,
    Fear,
    Boredom,
    BloodLust,
    OutOfShortcut,
    Love,
}

impl LizardEmotion {
    /// The base game's emotions in registration order (their `Index`).
    pub const BASE: [Self; 13] = [
        Self::SpottedPreyFirstTime,
        Self::ReSpottedPrey,
        Self::GeneralSmallNoise,
        Self::Curious,
        Self::Dominance,
        Self::Submission,
        Self::PainImpact,
        Self::PainIdle,
        Self::Frustration,
        Self::Fear,
        Self::Boredom,
        Self::BloodLust,
        Self::OutOfShortcut,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::SpottedPreyFirstTime => "SpottedPreyFirstTime",
            Self::ReSpottedPrey => "ReSpottedPrey",
            Self::GeneralSmallNoise => "GeneralSmallNoise",
            Self::Curious => "Curious",
            Self::Dominance => "Dominance",
            Self::Submission => "Submission",
            Self::PainImpact => "PainImpact",
            Self::PainIdle => "PainIdle",
            Self::Frustration => "Frustration",
            Self::Fear => "Fear",
            Self::Boredom => "Boredom",
            Self::BloodLust => "BloodLust",
            Self::OutOfShortcut => "OutOfShortcut",
            Self::Love => "Love",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::BASE
            .into_iter()
            .chain([Self::Love])
            .find(|e| e.value() == value)
    }

    /// `Index` with Remix off (Love is not registered then).
    #[must_use]
    pub fn index(self) -> Option<usize> {
        Self::BASE.iter().position(|&e| e == self)
    }

    /// The articulation's base length (ticks) and maximum volume.
    fn articulation(self) -> (f32, f32) {
        match self {
            Self::SpottedPreyFirstTime => (50.0, 1.0),
            Self::GeneralSmallNoise | Self::ReSpottedPrey => (12.0, 0.5),
            Self::Curious => (40.0, 0.4),
            Self::Dominance => (40.0, 0.8),
            Self::Submission => (20.0, 0.5),
            Self::PainImpact => (20.0, 1.0),
            Self::PainIdle => (50.0, 0.1),
            Self::Boredom => (30.0, 0.2),
            Self::BloodLust => (35.0, 0.5),
            Self::OutOfShortcut => (30.0, 0.5),
            Self::Love => (50.0, 0.5),
            Self::Frustration | Self::Fear => (40.0, 1.0),
        }
    }
}

/// `LizardVoice.VoiceArticulation`: volume (0) and pitch (1) curves over the sound.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceArticulation {
    pub length: f32,
    pub max_volume: f32,
    pub modifier: Vec<[f32; 2]>,
}

impl VoiceArticulation {
    /// `2 + length / 20` points, two draws each.
    fn new(length: f32, max_volume: f32, rng: &mut UnityRandom) -> Self {
        let n = 2 + (length / 20.0) as usize;
        let modifier = (0..n).map(|_| [rng.value(), rng.value()]).collect();
        Self {
            length,
            max_volume,
            modifier,
        }
    }

    /// `ReturnMod(f, m)`: curve `m` at `f` (0 to 1), linear between points.
    #[must_use]
    pub fn return_mod(&self, f: f32, m: usize) -> f32 {
        let last = self.modifier.len() - 1;
        let f = f * last as f32;
        let i = (f.floor() as usize).min(last);
        let frac = f - i as f32;
        self.modifier[i][m].lerp(self.modifier[(i + 1).min(last)][m], frac)
    }

    /// The constructor's shaping of emotion `e`'s curves, then each point halfway to
    /// the shared `general` articulation.
    fn shape(&mut self, e: LizardEmotion, general: &Self) {
        let n = self.modifier.len();
        for j in 0..n {
            let f = j as f32 / (n - 1) as f32;
            let sine = (f * std::f32::consts::PI).sin();
            let m = &mut self.modifier;
            match e {
                LizardEmotion::SpottedPreyFirstTime => {
                    m[j][0] = m[j][1].max(sine);
                    m[j][1] = m[j][1].lerp(sine, 0.5);
                }
                LizardEmotion::ReSpottedPrey => {
                    m[1][0] = 1.0;
                    if f == 0.0 {
                        m[j][1] = 1.0;
                    } else if f == 1.0 {
                        m[j][1] = 0.0;
                    }
                }
                LizardEmotion::Curious => {
                    m[j][1] = m[j][1].lerp(f, 0.5);
                    if f == 1.0 {
                        m[j][1] = 1.0;
                    }
                }
                LizardEmotion::Dominance => {
                    m[j][0] = m[j][0].lerp(1.0, (1.0 - f).mathf_pow(0.5));
                    m[j][1] = m[j][1].lerp(sine * 0.5, 0.5);
                }
                LizardEmotion::Submission => {
                    m[j][0] = m[j][0].lerp(0.0, f);
                    m[j][1] = m[j][1].lerp(1.0, (1.0 - f) * 0.5);
                }
                _ => {}
            }
            for (k, v) in m[j].iter_mut().enumerate() {
                *v = v.lerp(general.return_mod(f, k), 0.5);
            }
        }
    }
}

/// The clip a voice's sound plays and how far it has played.
#[derive(Debug, Clone, PartialEq)]
pub struct VoicePlayback {
    /// The variation's asset (the key of [`SoundTable::pcm`]).
    pub asset: String,
    /// `soundData.pitch`.
    pub clip_pitch: f32,
    /// Play position in clip frames.
    pub pos: f64,
    /// The microphone has given the emitter its sound object (`currentSoundObject`): set
    /// on the first update after the sound was made.
    pub started: bool,
}

/// `soundA`: the voice's `ChunkSoundEmitter` (`requireActiveUpkeep`) and, when the
/// camera's microphone made a sound object for it, what that plays.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSound {
    pub id: SoundId,
    pub emitter: LoopEmitter,
    pub playback: Option<VoicePlayback>,
}

/// What the voice's update reads and plays into.
pub struct VoiceCx<'a> {
    pub room_index: i32,
    pub camera_room: i32,
    pub tick: u64,
    pub sounds: &'a mut Sounds,
    pub rng: &'a mut UnityRandom,
    /// The game's sound table (the decoded clips), whichever sink `sounds` is.
    pub table: &'a SoundTable,
    /// The world's `updateList` counter (a new emitter joins the room).
    pub update_seq: &'a mut u64,
    /// `voiceChunk.pos` (the head).
    pub pos: Vec2,
    /// `lizard.grasps[0] != null`.
    pub holding: bool,
    pub jaw_forced_shut: f32,
}

/// A realized lizard's `LizardVoice`.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardVoice {
    pub my_pitch: f32,
    pub current_articulation_progression: f32,
    /// `articulationIndex` (-1 is `None`).
    pub articulation: Option<LizardEmotion>,
    pub current_emotion_intensity: f32,
    /// `myVoiceTrigger` (`None` is `SoundID.None`).
    pub my_voice_trigger: Option<SoundId>,
    pub articulations: Vec<VoiceArticulation>,
    /// `soundID`.
    pub sound_id: Option<SoundId>,
    last_sound_id: Option<SoundId>,
    /// `vol` (`Volume`).
    pub volume: f32,
    /// `ptch` (`Pitch`).
    pub pitch: f32,
    pub sound_a: Option<VoiceSound>,
    /// The visualizer's `savedIntensity`.
    pub saved_intensity: f32,
}

impl LizardVoice {
    /// The constructor, under `Random.InitState(ID.RandomSeed)` with the global state
    /// restored after: the voice trigger (a `Range` over the breed's working ones), the
    /// pitch, the length and volume factors, a shared articulation, then one per emotion.
    #[must_use]
    pub fn new(kind: CreatureType, seed: i32, table: &SoundTable) -> Self {
        let mut rng = UnityRandom::new(seed);
        let my_voice_trigger = Self::voice_trigger(kind, table, &mut rng);
        let my_pitch = rng.clamped_random_variation(0.5, 0.2, 0.8) * 2.0;
        let length_fac = 0.5f32.lerp(1.5, rng.value());
        let volume_fac = 0.7f32.lerp(1.3, rng.value());
        let general = VoiceArticulation::new(80.0, 1.0, &mut rng);
        let articulations = LizardEmotion::BASE
            .into_iter()
            .map(|e| {
                let (length, volume) = e.articulation();
                let mut a =
                    VoiceArticulation::new(length * length_fac, volume * volume_fac, &mut rng);
                a.shape(e, &general);
                a
            })
            .collect();
        Self {
            my_pitch,
            current_articulation_progression: 0.0,
            articulation: None,
            current_emotion_intensity: 0.0,
            my_voice_trigger,
            articulations,
            sound_id: None,
            last_sound_id: None,
            volume: 1.0,
            pitch: 1.0,
            sound_a: None,
            saved_intensity: 0.0,
        }
    }

    /// `GetMyVoiceTrigger` with Remix off and no rot: the breed's `Lizard_Voice_*` IDs
    /// with a working trigger, one picked at random.
    fn voice_trigger(
        kind: CreatureType,
        table: &SoundTable,
        rng: &mut UnityRandom,
    ) -> Option<SoundId> {
        let ids: &[SoundId] = match kind {
            CreatureType::PinkLizard => &[
                SoundId::LizardVoicePinkA,
                SoundId::LizardVoicePinkB,
                SoundId::LizardVoicePinkC,
                SoundId::LizardVoicePinkD,
                SoundId::LizardVoicePinkE,
            ],
            CreatureType::GreenLizard => &[SoundId::LizardVoiceGreenA],
            CreatureType::BlueLizard => &[SoundId::LizardVoiceBlueA],
            _ => &[],
        };
        let working: Vec<SoundId> = ids
            .iter()
            .copied()
            .filter(|id| table.trigger(id.name()).is_some())
            .collect();
        if working.is_empty() {
            return None;
        }
        let i = rng.range_i32(0, working.len() as i32) as usize;
        Some(working[i])
    }

    /// `MakingASound`.
    #[must_use]
    pub fn making_a_sound(&self) -> bool {
        self.sound_id.is_some() && self.volume > 0.0
    }

    /// `VisualizedIntensity`.
    #[must_use]
    pub fn visualized_intensity(&self) -> f32 {
        self.saved_intensity * self.volume
    }

    /// `MakeSound(emotion)`: the intensity drawn (`Lerp(0.8, 1.2, value)`), then the
    /// sound.
    pub fn make_sound(
        &mut self,
        emotion: LizardEmotion,
        conscious: bool,
        holding: bool,
        rng: &mut UnityRandom,
    ) {
        let intensity = 0.8f32.lerp(1.2, rng.value());
        self.make_sound_with(emotion, intensity, conscious, holding);
    }

    /// `MakeSound(emotion, intensity)`: a conscious lizard with nothing in its jaws
    /// starts the emotion's articulation over unless it is under half as intense as
    /// the current one.
    pub fn make_sound_with(
        &mut self,
        emotion: LizardEmotion,
        intensity: f32,
        conscious: bool,
        holding: bool,
    ) {
        if !conscious
            || intensity < self.current_emotion_intensity / 2.0
            || holding
            || emotion.index().is_none()
        {
            return;
        }
        self.current_articulation_progression = 0.0;
        self.articulation = Some(emotion);
        self.sound_id = self.my_voice_trigger;
        self.current_emotion_intensity = intensity;
    }

    /// `LizardVoice.Update`: the base's sound upkeep, then the articulation's volume
    /// and pitch, and the sound let go while the jaws hold something.
    pub fn update(&mut self, cx: &mut VoiceCx<'_>) {
        self.creature_voice_update(cx);
        if let Some(e) = self.articulation
            && let Some(art) = e.index().map(|i| &self.articulations[i])
        {
            let speed = self.current_emotion_intensity.lerp_map(0.0, 2.0, 0.5, 1.5);
            self.current_articulation_progression += 1.0 / (art.length * speed);
            if self.current_articulation_progression >= 1.0 {
                self.sound_id = None;
                self.articulation = None;
            } else {
                let p = self.current_articulation_progression;
                let loud = self.current_emotion_intensity.lerp_map(0.0, 2.0, 0.4, 3.0);
                self.volume =
                    art.return_mod(p, 0) * art.max_volume * loud * (1.0 - cx.jaw_forced_shut);
                self.pitch = 0.5f32.lerp(1.5, art.return_mod(p, 1)) * self.my_pitch;
            }
        }
        if cx.holding {
            self.drop_sound(cx.sounds);
        }
        if let Some(s) = &self.sound_a {
            cx.sounds
                .update_emitter(s.emitter.sound, self.volume, self.pitch);
        }
    }

    /// `CreatureVoice.Update`: the sound is let go when there is none, the volume is 0,
    /// the ID changed or its emitter was destroyed; kept alive, or made again (`Room.
    /// PlaySound(soundID, voiceChunk, false, Volume, Pitch)`); then the visualizer.
    fn creature_voice_update(&mut self, cx: &mut VoiceCx<'_>) {
        if self.sound_id.is_none()
            || self.volume == 0.0
            || self.sound_id != self.last_sound_id
            || self.sound_a.as_ref().is_some_and(|s| s.emitter.slated)
        {
            self.drop_sound(cx.sounds);
        }
        self.last_sound_id = self.sound_id;
        if let Some(s) = &mut self.sound_a {
            s.emitter.alive = true;
        } else if let Some(id) = self.sound_id
            && self.volume > 0.0
        {
            self.sound_a = Some(Self::play(id, self.volume, self.pitch, cx));
        }
        self.update_visualizer(cx);
    }

    /// Lets the emitter go; the port stops its sound at once (the game's dies on its
    /// next update without upkeep).
    fn drop_sound(&mut self, sounds: &mut Sounds) {
        if let Some(s) = self.sound_a.take()
            && !s.emitter.slated
        {
            sounds.stop_emitter(s.emitter.sound);
        }
    }

    /// `Room.PlaySound` on the head chunk: the microphone's draws, and the emitter
    /// joining the room's update list.
    fn play(id: SoundId, volume: f32, pitch: f32, cx: &mut VoiceCx<'_>) -> VoiceSound {
        let emitter = cx
            .sounds
            .play_on_object(id, cx.pos, false, volume, pitch, cx.rng);
        let played = cx.sounds.events().iter().rev().find_map(|e| match e {
            SoundEvent::Play(p)
                if matches!(p.source, SoundSource::Object { emitter: e, .. } if e == emitter) =>
            {
                Some(p)
            }
            _ => None,
        });
        let heard = cx.room_index == cx.camera_room;
        let playback = played.filter(|_| heard).and_then(|p| {
            let clip = cx.table.clip_at(p.clip)?;
            Some(VoicePlayback {
                asset: clip.variations.get(p.variation)?.asset.clone(),
                clip_pitch: p.pitch,
                pos: 0.0,
                started: false,
            })
        });
        *cx.update_seq += 1;
        VoiceSound {
            id,
            emitter: LoopEmitter {
                sound: emitter,
                room: cx.room_index,
                update_order: *cx.update_seq,
                alive: true,
                slated: false,
                still_playing: true,
                last_playing_tick: cx.tick,
                playing: playback.is_some(),
            },
            playback,
        }
    }

    /// `IntensityVisualizer.Update`: 0 without a playing sound object in the camera's
    /// room, else the spectrum's mean through the cutoff curve.
    fn update_visualizer(&mut self, cx: &VoiceCx<'_>) {
        self.saved_intensity = 0.0;
        let pitch = self.pitch;
        let Some(s) = &mut self.sound_a else {
            return;
        };
        if !s.emitter.playing || s.emitter.room != cx.camera_room {
            return;
        }
        let Some(p) = &mut s.playback else {
            return;
        };
        let Some(pcm) = cx.table.pcm(&p.asset) else {
            return;
        };
        if !p.started {
            p.started = true;
            return;
        }
        let rate = f64::from(pcm.sample_rate) * f64::from(p.clip_pitch * pitch);
        p.pos += rate / 40.0;
        if p.pos >= pcm.frames() as f64 {
            s.emitter.playing = false;
            return;
        }
        let spectrum = VoiceSpectrum::of(pcm, p.pos, rate / VoiceSpectrum::OUTPUT_RATE);
        self.saved_intensity = spectrum.intensity();
    }
}

/// `AudioSource.GetSpectrumData(64 samples, channel 0, BlackmanHarris)` of a clip
/// playing at a position, as the port models Unity's FMOD FFT (INFERRED: a 128-point
/// FFT of the last 128 mixer-rate samples of channel 0 before volume, a periodic
/// 4-term Blackman-Harris window, magnitudes scaled by `2 / N`).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSpectrum {
    pub bins: [f32; 64],
}

impl VoiceSpectrum {
    pub const SIZE: usize = 128;
    /// The mixer's output rate (INFERRED: Unity's default on desktop).
    pub const OUTPUT_RATE: f64 = 48_000.0;
    const BLACKMAN_HARRIS: [f64; 4] = [0.35875, 0.48829, 0.14128, 0.01168];

    /// The spectrum of the 128 output samples ending at clip frame `end`, `step` clip
    /// frames apart (linear between frames, 0 before the start).
    #[must_use]
    pub fn of(pcm: &PcmAudio, end: f64, step: f64) -> Self {
        let samples: Vec<f64> = (0..Self::SIZE)
            .map(|n| Self::sample_at(pcm, end - (Self::SIZE - 1 - n) as f64 * step))
            .collect();
        Self::of_samples(&samples)
    }

    /// The spectrum of 128 samples (-1 to 1).
    #[must_use]
    pub fn of_samples(samples: &[f64]) -> Self {
        let n = Self::SIZE as f64;
        let tau = std::f64::consts::TAU;
        let windowed: Vec<f64> = samples
            .iter()
            .enumerate()
            .map(|(i, x)| {
                let a = Self::BLACKMAN_HARRIS;
                let t = tau * i as f64 / n;
                let w = a[0] - a[1] * t.cos() + a[2] * (2.0 * t).cos() - a[3] * (3.0 * t).cos();
                x * w
            })
            .collect();
        let mut bins = [0.0f32; 64];
        for (k, bin) in bins.iter_mut().enumerate() {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, x) in windowed.iter().enumerate() {
                let a = tau * (k * i) as f64 / n;
                re += x * a.cos();
                im -= x * a.sin();
            }
            *bin = (2.0 * (re * re + im * im).sqrt() / n) as f32;
        }
        Self { bins }
    }

    /// Channel 0 at fractional frame `t`.
    fn sample_at(pcm: &PcmAudio, t: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        let channels = usize::from(pcm.channels.max(1));
        let frames = pcm.frames();
        let i = t.floor() as usize;
        if i >= frames {
            return 0.0;
        }
        let a = f64::from(pcm.samples[i * channels]);
        let b = if i + 1 < frames {
            f64::from(pcm.samples[(i + 1) * channels])
        } else {
            0.0
        };
        (a + (b - a) * (t - i as f64)) / 32768.0
    }

    #[must_use]
    pub fn mean(&self) -> f32 {
        self.bins.iter().sum::<f32>() / self.bins.len() as f32
    }

    /// `savedIntensity`: `Pow(InverseLerp(0.0002, 0.003, mean), 0.5)`.
    #[must_use]
    pub fn intensity(&self) -> f32 {
        self.mean().inverse_lerp(0.0002, 0.003).mathf_pow(0.5)
    }
}

#[cfg(test)]
#[path = "voice_tests.rs"]
mod tests;
