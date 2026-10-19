//! Sound playback for the sim's [`SoundEvent`]s and the room's ambient loops, mixed
//! as the game's `VirtualMicrophone` mixes them, on kira (cpal; desktop and Android).
//!
//! Behaviour: `rainworld-re/notes/sound.md` sections 2.2 to 2.5. Divergences:
//! docs/divergences/ D12.
//!
//! Mirrors: - (crate root)

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

/// The game camera's `VirtualMicrophone`: its sound objects, the emitters they follow
/// and the ambient loops.
struct GameSounds {
    mic: Microphone,
    emitters: Emitters,
    voices: Vec<Playing>,
    ambients: Vec<AmbientSource>,
}

impl GameSounds {
    fn new(exponent: f32) -> Self {
        Self {
            mic: Microphone::new(exponent),
            emitters: Emitters::default(),
            voices: Vec::new(),
            ambients: Vec::new(),
        }
    }

    /// `VirtualMicrophone.AllQuiet` at `RainWorldGame.ShutDownProcess`: every sound
    /// object and ambient loop is destroyed; the next game's camera has a new microphone.
    fn all_quiet(&mut self) {
        let sources = self.voices.drain(..).filter_map(|p| p.source);
        for source in sources.chain(self.ambients.drain(..).filter_map(|a| a.source)) {
            source.stop();
        }
        let sfx_volume = self.mic.sfx_volume;
        *self = Self::new(self.mic.exponent());
        self.mic.sfx_volume = sfx_volume;
    }

    /// The camera left the room: `ObjectSound`s no longer refresh their emitters'
    /// `soundStillPlaying`, so the old room's object emitters and their sounds die.
    fn leave_room(&mut self) {
        self.emitters.clear_objects();
    }
}

/// The audio output: the game's microphone, its sound objects and ambient loops.
pub struct Audio {
    manager: AudioManager<CpalOutput>,
    clips: ClipLoader,
    table: Arc<SoundTable>,
    game: GameSounds,
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
            game: GameSounds::new(table.volume_exponent),
            table,
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
        for a in self.game.ambients.drain(..) {
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
            self.game.ambients.push(AmbientSource {
                clip: ClipKey::Ambient(player.clip_name().to_owned()),
                player,
                source: None,
            });
        }
        self.game.ambients.extend(kept);
    }

    /// The camera moved to another room: the old room's object sounds end and the
    /// ambient loops change.
    pub fn new_room(&mut self, sounds: &[AmbientSound]) {
        self.game.leave_room();
        self.set_ambients(sounds);
    }

    /// The game process shuts down: every game sound stops (menu sounds and music
    /// play on).
    pub fn all_quiet(&mut self) {
        self.game.all_quiet();
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
        self.game.mic.tick(listener, water);
        match chunks {
            Some(chunks) => self.game.emitters.follow(chunks),
            None => self.game.emitters.clear_chunks(),
        }
        self.play_events(events, chunks);
        self.game.emitters.end_tick();
    }

    /// A tick's sound calls without the microphone's update or the emitters' moves: a
    /// paused game's (the HUD's and the pause menu's).
    pub fn play_events(&mut self, events: Vec<SoundEvent>, chunks: Option<&[BodyChunk]>) {
        let now = self.now();
        for event in events {
            match event {
                SoundEvent::Play(sound) => {
                    if chunks.is_none() && matches!(sound.source, SoundSource::Chunk { .. }) {
                        continue;
                    }
                    self.game.emitters.insert_for(&sound, chunks);
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
                    self.game.voices.push(Playing {
                        voice: Voice::new(sound, &self.game.mic, now),
                        clip: key,
                        source: None,
                    });
                }
                SoundEvent::Emitter { id, volume, pitch } => {
                    self.game.emitters.set(id, volume, pitch)
                }
                SoundEvent::Stop(id) => self.game.emitters.remove(id),
                SoundEvent::Moved { id, pos } => self.game.emitters.moved(id, pos),
                SoundEvent::OwnerRemoved => self.game.emitters.clear_chunks(),
            }
        }
    }

    /// `AllowSound`'s cap on sounds of one ID in a crowded room.
    fn allow_sound(&self, id: rw_sim::SoundId) -> bool {
        let total = self.game.voices.len();
        let same = self
            .game
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

    /// `options.soundEffectsVolume`, for game and menu sounds alike.
    pub fn set_sound_effects_volume(&mut self, volume: f32) {
        self.game.mic.sfx_volume = volume;
    }

    /// The pause menu's hold on the microphone: the volume of every group but 1 and the
    /// `timeSpeed` of positioned and ambient sounds' pitch (both 1 without it).
    pub fn set_pause_mix(&mut self, volume: f32, time_speed: f32) {
        self.game.mic.pause_volume = volume;
        self.game.mic.time_speed = time_speed;
    }

    /// One drawn frame, `time_stacker` between the last two ticks: every source's
    /// volume, pan, pitch and filter.
    pub fn frame(&mut self, time_stacker: f32, water: Option<&Water>) {
        self.clips.poll();
        let now = self.now();
        let mut mixes = Vec::with_capacity(self.game.voices.len());
        let mut i = 0;
        while i < self.game.voices.len() {
            let p = &mut self.game.voices[i];
            let mix = p.voice.mix(
                &self.game.mic,
                &self.game.emitters,
                time_stacker,
                now,
                water,
            );
            let ended = p.source.as_ref().is_some_and(Source::finished)
                || matches!(self.clips.get(&p.clip), ClipSlot::Missing);
            match mix {
                Some(mix) if !ended => {
                    mixes.push(mix);
                    i += 1;
                }
                _ => {
                    if let Some(source) = self.game.voices.swap_remove(i).source {
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
        for (p, mix) in self.game.voices.iter_mut().zip(mixes) {
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
        let listener = self.game.mic.listener_at(time_stacker);
        for a in &mut self.game.ambients {
            let mix = a.player.mix(&self.game.mic, listener, now);
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
            .game
            .voices
            .iter()
            .filter_map(|p| p.voice.emitter())
            .collect();
        self.game.emitters.prune(&used);
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::AmbientSound;
    use rw_sim::{EmitterId, PlayedSound, SoundId, SoundSource, Vec2};

    use super::*;

    impl GameSounds {
        /// Starts a looped sound on a new object emitter, as the shelter door's working
        /// loop does, with an ambient loop beside it.
        fn with_door_loop() -> Self {
            let mut game = Self::new(1.8);
            let sound = PlayedSound {
                id: SoundId::ShelterWorkingBackgroundLoop,
                clip: 0,
                variation: 0,
                volume: 1.0,
                pitch: 1.0,
                range: 1.0,
                doppler_fac: 0.0,
                ignore_effects: 0.0,
                start_at: None,
                source: SoundSource::Object {
                    emitter: EmitterId(0),
                    pos: Vec2::new(100.0, 100.0),
                },
                looped: true,
                emitter_volume: 1.0,
                emitter_pitch: 1.0,
            };
            game.emitters.insert_for(&sound, None);
            game.voices.push(Playing {
                voice: Voice::new(sound, &game.mic, 0.0),
                clip: ClipKey::Effect("SH_Working".to_owned()),
                source: None,
            });
            let ambient = AmbientSound::parse_list("OMNI><AM_ENV-Forest.ogg><0><0><0.5><1.2")
                .expect("parses")
                .remove(0);
            game.ambients.push(AmbientSource {
                clip: ClipKey::Ambient("AM_ENV-Forest".to_owned()),
                player: AmbientPlayer::new(ambient, 0.0),
                source: None,
            });
            game
        }
    }

    #[test]
    fn shutting_down_the_game_silences_its_loops() {
        let mut game = GameSounds::with_door_loop();
        game.mic.pause_volume = 0.2;
        game.all_quiet();
        assert!(game.voices.is_empty());
        assert!(game.ambients.is_empty());
        assert!(game.emitters.is_empty());
        assert_eq!(game.mic.pause_volume, 1.0);
    }

    #[test]
    fn leaving_the_room_ends_its_object_sounds() {
        let mut game = GameSounds::with_door_loop();
        game.leave_room();
        let emitters = &game.emitters;
        let mic = &game.mic;
        let voice = &mut game.voices[0].voice;
        assert!(voice.mix(mic, emitters, 1.0, 0.0, None).is_none());
    }

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
