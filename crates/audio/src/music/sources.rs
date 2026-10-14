//! The audio sources the music player drives (one Unity `AudioSource` per sub-track),
//! and a silent, tick-timed implementation for headless runs and tests.

use std::collections::HashMap;

use super::bank::{ClipLength, SongBank};

/// A sub-track's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackId(pub u64);

/// What `MusicPiece.SubTrack` does with its `AudioSource` and clip.
pub trait MusicSources {
    /// `music_songs` and `music_procedural` are loaded (`AssetBundleManager`).
    fn bundles_loaded(&self) -> bool;
    /// Starts loading the clip named `clip` for `track` (`LoadAsset`, `LoadAudioData`).
    fn load(&mut self, track: TrackId, clip: &str);
    /// The clip's audio data is loaded (`AudioDataLoadState.Loaded`).
    fn loaded(&self, track: TrackId) -> bool;
    /// `AudioSource.isPlaying`.
    fn is_playing(&self, track: TrackId) -> bool;
    /// `AudioSource.Play()` from the start.
    fn play(&mut self, track: TrackId, looped: bool);
    /// `AudioSource.volume`, an amplitude factor in 0..=1.
    fn set_volume(&mut self, track: TrackId, volume: f32);
    /// `Stop()` and destroy the source.
    fn destroy(&mut self, track: TrackId);
}

/// A silent source that plays for its clip's length in ticks.
#[derive(Debug, Clone, Default)]
pub struct TimedTrack {
    pub clip: String,
    pub length: Option<ClipLength>,
    pub looped: bool,
    pub playing: bool,
    /// Ticks played since `play`.
    pub ticks: u64,
    pub volume: f32,
    /// Times `play` was called.
    pub plays: u32,
}

/// Sources without audio output: clips load at once and play for their FSB5 length,
/// advanced by [`TimedSources::advance`] once per 40 Hz tick. Deterministic, so a
/// headless run's music state does not depend on an audio device.
#[derive(Debug, Default)]
pub struct TimedSources {
    bank: Option<SongBank>,
    lengths: HashMap<String, Option<ClipLength>>,
    tracks: HashMap<TrackId, TimedTrack>,
}

impl TimedSources {
    /// Clip lengths from the bank's FSB5 headers; with none, every clip is missing
    /// unless given by [`Self::with_length`].
    #[must_use]
    pub fn new(bank: Option<SongBank>) -> Self {
        Self {
            bank,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_length(mut self, clip: &str, length: ClipLength) -> Self {
        self.lengths.insert(clip.to_owned(), Some(length));
        self
    }

    #[must_use]
    pub fn track(&self, track: TrackId) -> Option<&TimedTrack> {
        self.tracks.get(&track)
    }

    /// One tick of playback (1/40 s).
    pub fn advance(&mut self) {
        for t in self.tracks.values_mut() {
            let Some(length) = t.length.filter(|_| t.playing) else {
                continue;
            };
            t.ticks += 1;
            if !t.looped && t.ticks * u64::from(length.sample_rate) / 40 >= u64::from(length.frames)
            {
                t.playing = false;
            }
        }
    }
}

impl MusicSources for TimedSources {
    fn bundles_loaded(&self) -> bool {
        true
    }

    fn load(&mut self, track: TrackId, clip: &str) {
        let length = match self.lengths.get(clip) {
            Some(length) => *length,
            None => {
                let length = self.bank.as_ref().and_then(|b| b.length(clip));
                self.lengths.insert(clip.to_owned(), length);
                length
            }
        };
        let t = self.tracks.entry(track).or_default();
        t.clip = clip.to_owned();
        t.length = length;
    }

    fn loaded(&self, track: TrackId) -> bool {
        self.tracks.get(&track).is_some_and(|t| t.length.is_some())
    }

    fn is_playing(&self, track: TrackId) -> bool {
        self.tracks.get(&track).is_some_and(|t| t.playing)
    }

    fn play(&mut self, track: TrackId, looped: bool) {
        let t = self.tracks.entry(track).or_default();
        t.playing = t.length.is_some();
        t.looped = looped;
        t.ticks = 0;
        t.plays += 1;
    }

    fn set_volume(&mut self, track: TrackId, volume: f32) {
        self.tracks.entry(track).or_default().volume = volume;
    }

    fn destroy(&mut self, track: TrackId) {
        self.tracks.remove(&track);
    }
}
