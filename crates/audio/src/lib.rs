//! Sound playback for the sim's [`SoundEvent`]s and the room's ambient loops, mixed
//! as the game's `VirtualMicrophone` mixes them, on kira (cpal; desktop and Android).
//!
//! Behaviour: `rainworld-re/notes/sound.md` sections 2.2 to 2.5. Divergences:
//! docs/DIVERGENCES.md D12.

mod ambient;
mod clips;
mod menu_mic;
mod mic;
pub mod music;
mod output;
mod voice;

use std::sync::Arc;
use std::time::Instant;

use kira::effect::filter::{FilterBuilder, FilterHandle};
use kira::sound::PlaybackState;
use kira::sound::static_sound::StaticSoundHandle;
use kira::track::{TrackBuilder, TrackHandle};
use kira::{
    AudioManager, AudioManagerSettings, Capacities, Decibels, Mix, Panning, PlaybackRate, Tween,
};
use rw_assets::{AmbientSound, SoundTable, StreamingAssets};
use rw_sim::{BodyChunk, SoundEvent, SoundSource, Water};

use output::CpalOutput;

pub use ambient::AmbientPlayer;
pub use clips::{ClipKey, ClipLoader, ClipSlot};
pub use menu_mic::{MenuPlayback, MenuSoundStart};
pub use mic::{Doppler, Follow, ListenerInput, Microphone, SourceMix};
pub use voice::{Emitter, Emitters, Voice};

/// Unity's real voices (`globalgamemanagers` AudioManager): the quietest sources past
/// this many are silent.
pub const REAL_VOICES: usize = 32;

/// A source playing on its own mixer track, so it can have its own low-pass filter.
struct Source {
    track: TrackHandle,
    filter: FilterHandle,
    sound: StaticSoundHandle,
    /// Whether the filter is currently on.
    filtered: bool,
}

impl Source {
    /// `AudioLowPassFilter.lowpassResonanceQ` 1 on kira's filter (`k = 2 - 1.9 r`).
    const RESONANCE: f64 = 1.0 / 1.9;

    fn amplitude_db(volume: f32) -> Decibels {
        if volume <= 0.0 {
            Decibels::SILENCE
        } else {
            Decibels(20.0 * volume.log10())
        }
    }

    fn start(
        manager: &mut AudioManager<CpalOutput>,
        data: kira::sound::static_sound::StaticSoundData,
        looped: bool,
        start_at: Option<f32>,
        mix: SourceMix,
    ) -> Option<Self> {
        let mut builder = TrackBuilder::new();
        let filter = builder.add_effect(
            FilterBuilder::new()
                .cutoff(f64::from(mix.cutoff.unwrap_or(22000.0)))
                .resonance(Self::RESONANCE)
                .mix(if mix.cutoff.is_some() {
                    Mix::WET
                } else {
                    Mix::DRY
                }),
        );
        let mut track = manager
            .add_sub_track(builder)
            .inspect_err(|e| log::warn!("no mixer track for a sound: {e}"))
            .ok()?;
        let mut data = data
            .volume(Self::amplitude_db(mix.volume))
            .panning(Panning(mix.pan))
            .playback_rate(PlaybackRate(f64::from(mix.pitch)));
        if looped {
            data = data.loop_region(..);
        }
        if let Some(at) = start_at {
            data = data.start_position(data.duration().as_secs_f64() * f64::from(at));
        }
        let sound = track
            .play(data)
            .inspect_err(|e| log::warn!("sound not played: {e}"))
            .ok()?;
        Some(Self {
            track,
            filter,
            sound,
            filtered: mix.cutoff.is_some(),
        })
    }

    fn apply(&mut self, mix: SourceMix) {
        let now = Tween::default();
        self.sound.set_volume(Self::amplitude_db(mix.volume), now);
        self.sound.set_panning(Panning(mix.pan), now);
        self.sound
            .set_playback_rate(PlaybackRate(f64::from(mix.pitch)), now);
        if let Some(cutoff) = mix.cutoff {
            self.filter.set_cutoff(f64::from(cutoff), now);
        }
        if mix.cutoff.is_some() != self.filtered {
            self.filtered = mix.cutoff.is_some();
            let wet = if self.filtered { Mix::WET } else { Mix::DRY };
            self.filter.set_mix(wet, now);
        }
    }

    fn finished(&self) -> bool {
        self.sound.state() == PlaybackState::Stopped
    }

    fn stop(mut self) {
        self.sound.stop(Tween::default());
        self.track.set_volume(Decibels::SILENCE, Tween::default());
    }
}

/// A sound object and its source once its clip is loaded.
struct Playing {
    voice: Voice,
    clip: ClipKey,
    source: Option<Source>,
}

/// An ambient loop and its source once its clip is loaded.
struct AmbientSource {
    player: AmbientPlayer,
    clip: ClipKey,
    source: Option<Source>,
}

/// The audio output: the microphone, its sound objects and ambient loops.
pub struct Audio {
    manager: AudioManager<CpalOutput>,
    clips: ClipLoader,
    table: Arc<SoundTable>,
    mic: Microphone,
    emitters: Emitters,
    voices: Vec<Playing>,
    ambients: Vec<AmbientSource>,
    /// `Time.time` zero.
    start: Instant,
}

impl Audio {
    /// `AllowSound`: with more than this many sound objects alive, a sound is refused
    /// when too many of its ID already play.
    const CROWDED: usize = 25;
    /// ... when more than `CAP_BASE - alive` of its ID play.
    const CAP_BASE: usize = 30;

    /// Opens the default output device; clips are read from the game's bundles as
    /// sounds need them.
    ///
    /// # Errors
    /// When there is no output device or it cannot be opened.
    pub fn new(assets: &StreamingAssets, table: Arc<SoundTable>) -> Result<Self, String> {
        let settings = AudioManagerSettings::<CpalOutput> {
            capacities: Capacities {
                sub_track_capacity: 512,
                ..Capacities::default()
            },
            ..AudioManagerSettings::default()
        };
        let mut manager = AudioManager::<CpalOutput>::new(settings).map_err(|e| e.to_string())?;
        // `AudioListener.volume = soundLoader.volume`: a master gain of 6.
        manager
            .main_track()
            .set_volume(Source::amplitude_db(table.volume), Tween::default());
        Ok(Self {
            manager,
            clips: ClipLoader::new(
                assets.asset_bundle("loadedsoundeffects"),
                assets.asset_bundle("loadedsoundeffects_ambient"),
            ),
            mic: Microphone::new(table.volume_exponent),
            table,
            emitters: Emitters::default(),
            voices: Vec::new(),
            ambients: Vec::new(),
            start: Instant::now(),
        })
    }

    fn now(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    /// `VirtualMicrophone.NewRoom`: keeps `OMNI` loops of a sample the new room also has
    /// as `OMNI` (taking its volume and pitch), replaces everything else.
    pub fn set_ambients(&mut self, sounds: &[AmbientSound]) {
        let mut kept = Vec::new();
        for a in self.ambients.drain(..) {
            if sounds
                .iter()
                .any(|s| AmbientPlayer::carries_over(&a.player.sound, s))
            {
                kept.push(a);
            } else if let Some(source) = a.source {
                source.stop();
            }
        }
        let now = self.now();
        for s in sounds {
            if let Some(a) = kept
                .iter_mut()
                .find(|a| AmbientPlayer::carries_over(s, &a.player.sound))
            {
                a.player.sound.volume = s.volume;
                a.player.sound.pitch = s.pitch;
                continue;
            }
            let player = AmbientPlayer::new(s.clone(), now);
            log::info!("ambient {} ({:?}, volume {})", s.sample, s.kind, s.volume);
            self.ambients.push(AmbientSource {
                clip: ClipKey::Ambient(player.clip_name().to_owned()),
                player,
                source: None,
            });
        }
        self.ambients.extend(kept);
    }

    /// One game tick: the microphone, the chunk emitters following `chunks` (`None`
    /// when the body is not in the room: its sounds end), then the tick's events.
    pub fn tick(
        &mut self,
        events: Vec<SoundEvent>,
        listener: &ListenerInput,
        chunks: Option<&[BodyChunk]>,
        water: Option<&Water>,
    ) {
        self.mic.tick(listener, water);
        match chunks {
            Some(chunks) => self.emitters.follow(chunks),
            None => self.emitters.clear(),
        }
        let now = self.now();
        for event in events {
            match event {
                SoundEvent::Play(sound) => {
                    if let Some(chunks) = chunks {
                        self.emitters.insert_for(&sound, chunks);
                    } else if matches!(sound.source, SoundSource::Chunk { .. }) {
                        continue;
                    }
                    if !self.allow_sound(sound.id) {
                        log::debug!("<CAP> {}", sound.id.name());
                        continue;
                    }
                    let Some(clip) = self.table.clip_at(sound.clip) else {
                        continue;
                    };
                    let Some(variation) = clip.variations.get(sound.variation) else {
                        continue;
                    };
                    log::debug!(
                        "play {} ({}, volume {:.2}, pitch {:.2})",
                        sound.id.name(),
                        variation.asset,
                        sound.volume * sound.emitter_volume,
                        sound.pitch * sound.emitter_pitch
                    );
                    let key = ClipKey::Effect(variation.asset.clone());
                    self.voices.push(Playing {
                        voice: Voice::new(sound, &self.mic, now),
                        clip: key,
                        source: None,
                    });
                }
                SoundEvent::Emitter { id, volume, pitch } => self.emitters.set(id, volume, pitch),
                SoundEvent::Stop(id) => self.emitters.remove(id),
                SoundEvent::OwnerRemoved => self.emitters.clear(),
            }
        }
        self.emitters.end_tick();
    }

    /// `AllowSound`'s cap on sounds of one ID in a crowded room.
    fn allow_sound(&self, id: rw_sim::SoundId) -> bool {
        let total = self.voices.len();
        let same = self
            .voices
            .iter()
            .filter(|p| p.voice.sound.id == id)
            .count();
        Self::allowed(same, total)
    }

    /// `AllowSound` with `same` sounds of the ID among `total` alive.
    #[must_use]
    pub fn allowed(same: usize, total: usize) -> bool {
        total <= Self::CROWDED || same <= Self::CAP_BASE.saturating_sub(total)
    }

    /// One drawn frame, `time_stacker` between the last two ticks: every source's
    /// volume, pan, pitch and filter.
    pub fn frame(&mut self, time_stacker: f32, water: Option<&Water>) {
        self.clips.poll();
        let now = self.now();
        let mut mixes = Vec::with_capacity(self.voices.len());
        let mut i = 0;
        while i < self.voices.len() {
            let p = &mut self.voices[i];
            let mix = p
                .voice
                .mix(&self.mic, &self.emitters, time_stacker, now, water);
            let ended = p.source.as_ref().is_some_and(Source::finished)
                || matches!(self.clips.get(&p.clip), ClipSlot::Missing);
            match mix {
                Some(mix) if !ended => {
                    mixes.push(mix);
                    i += 1;
                }
                _ => {
                    if let Some(source) = self.voices.swap_remove(i).source {
                        source.stop();
                    }
                }
            }
        }
        let mut order: Vec<usize> = (0..mixes.len()).collect();
        order.sort_by(|&a, &b| mixes[b].volume.total_cmp(&mixes[a].volume));
        for &virtual_voice in order.iter().skip(REAL_VOICES) {
            mixes[virtual_voice].volume = 0.0;
        }
        for (p, mix) in self.voices.iter_mut().zip(mixes) {
            match &mut p.source {
                Some(source) => source.apply(mix),
                None => {
                    if let ClipSlot::Ready(data) = self.clips.get(&p.clip) {
                        let s = &p.voice.sound;
                        p.source = Source::start(
                            &mut self.manager,
                            (**data).clone(),
                            s.looped,
                            s.start_at,
                            mix,
                        );
                    }
                }
            }
        }
        let listener = self.mic.listener_at(time_stacker);
        for a in &mut self.ambients {
            let mix = a.player.mix(&self.mic, listener, now);
            match &mut a.source {
                Some(source) => source.apply(mix),
                None => {
                    if let ClipSlot::Ready(data) = self.clips.get(&a.clip) {
                        a.source =
                            Source::start(&mut self.manager, (**data).clone(), true, None, mix);
                        log::info!(
                            "ambient {} playing ({:.1} s loop, volume {:.3}, pan {:.2})",
                            a.player.sound.sample,
                            data.duration().as_secs_f32(),
                            mix.volume,
                            mix.pan
                        );
                    }
                }
            }
        }
        let used: Vec<_> = self
            .voices
            .iter()
            .filter_map(|p| p.voice.emitter())
            .collect();
        self.emitters.prune(&used);
    }
}

#[cfg(test)]
mod tests {
    use super::Audio;

    #[test]
    fn crowded_rooms_cap_sounds_of_one_id() {
        assert!(Audio::allowed(25, 25));
        assert!(Audio::allowed(4, 26));
        assert!(!Audio::allowed(5, 26));
        assert!(Audio::allowed(0, 30));
        assert!(!Audio::allowed(1, 30));
        assert!(!Audio::allowed(1, 40));
    }
}
