//! The audio sources the music player drives (one Unity `AudioSource` per sub-track),
//! and a silent, tick-timed implementation for headless runs and tests.

use rustc_hash::FxHashMap;

use super::bank::{ClipLength, MusicBundle, SongBank};

/// A sub-track's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackId(pub u64);

/// Hands out the music player's source ids.
#[derive(Debug, Clone, Default)]
pub struct TrackIds(u64);

impl TrackIds {
    pub fn next_id(&mut self) -> TrackId {
        self.0 += 1;
        TrackId(self.0)
    }
}

/// What `MusicPiece.SubTrack` does with its `AudioSource` and clip.
pub trait MusicSources {
    /// `music_songs` and `music_procedural` are loaded (`AssetBundleManager`).
    fn bundles_loaded(&self) -> bool;
    /// Starts loading the clip named `clip` of `bundle` for `track` (`LoadAsset`,
    /// `LoadAudioData`).
    fn load(&mut self, track: TrackId, bundle: MusicBundle, clip: &str);
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

/// What a sub-track's `AudioSource` reports of its playback, read between updates
/// (`SlideShow.RawUpdate` follows it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourcePlayback {
    /// `isPlaying`.
    pub is_playing: bool,
    /// `time`: seconds into the clip.
    pub time: f32,
    /// `clip.length` in seconds; `None` while no clip is loaded (`clip == null`).
    pub clip_length: Option<f32>,
}

/// Sources whose playback position can be read.
pub trait MusicClock {
    /// The source of `track`, if it exists.
    fn playback(&self, track: TrackId) -> Option<SourcePlayback>;
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
    procedural: Option<SongBank>,
    lengths: FxHashMap<(MusicBundle, String), Option<ClipLength>>,
    tracks: FxHashMap<TrackId, TimedTrack>,
}

impl TimedSources {
    /// Song lengths from the `music_songs` bank's FSB5 headers; with none, every song
    /// is missing unless given by [`Self::with_length`].
    #[must_use]
    pub fn new(bank: Option<SongBank>) -> Self {
        Self {
            bank,
            ..Self::default()
        }
    }

    /// Lengths from the installed game's `music_songs` and `music_procedural`; a missing
    /// bundle leaves its clips missing.
    #[must_use]
    pub fn installed(assets: &rw_assets::StreamingAssets) -> Self {
        let [songs, procedural] = [MusicBundle::Songs, MusicBundle::Procedural].map(|b| {
            SongBank::open_bundle(assets, b)
                .inspect_err(|e| log::warn!("no {} bundle ({e}); its clips are missing", b.value()))
                .ok()
        });
        Self::new(songs).with_procedural(procedural)
    }

    /// Stem lengths from the `music_procedural` bank.
    #[must_use]
    pub fn with_procedural(mut self, bank: Option<SongBank>) -> Self {
        self.procedural = bank;
        self
    }

    /// The song `clip`'s length.
    #[must_use]
    pub fn with_length(self, clip: &str, length: ClipLength) -> Self {
        self.with_bundle_length(MusicBundle::Songs, clip, length)
    }

    #[must_use]
    pub fn with_bundle_length(
        mut self,
        bundle: MusicBundle,
        clip: &str,
        length: ClipLength,
    ) -> Self {
        self.lengths.insert((bundle, clip.to_owned()), Some(length));
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

impl MusicClock for TimedSources {
    /// The position is the ticks played, at 40 per second.
    fn playback(&self, track: TrackId) -> Option<SourcePlayback> {
        let t = self.tracks.get(&track)?;
        Some(SourcePlayback {
            is_playing: t.playing,
            time: t.ticks as f32 / 40.0,
            clip_length: t.length.map(ClipLength::seconds),
        })
    }
}

impl MusicSources for TimedSources {
    fn bundles_loaded(&self) -> bool {
        true
    }

    fn load(&mut self, track: TrackId, bundle: MusicBundle, clip: &str) {
        let key = (bundle, clip.to_owned());
        let length = match self.lengths.get(&key) {
            Some(length) => *length,
            None => {
                let bank = match bundle {
                    MusicBundle::Songs => self.bank.as_ref(),
                    MusicBundle::Procedural => self.procedural.as_ref(),
                };
                let length = bank.and_then(|b| b.length(clip));
                self.lengths.insert(key, length);
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
