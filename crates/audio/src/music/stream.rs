//! Music sources on kira: clips read from `music_songs` on a loader thread, then
//! streamed (decoded a Vorbis packet at a time on kira's decode thread) as Unity
//! streams them (`m_LoadType` 2).
//!
//! Mirrors: - (kira streaming sources)

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use kira::sound::PlaybackState;
use kira::sound::streaming::{Decoder, StreamingSoundData, StreamingSoundHandle};
use kira::{AudioManager, Frame, Tween};
use rustc_hash::FxHashMap;
use rw_assets::StreamingAssets;
use rw_assets::unity::{Fsb5, Fsb5Stream};

use super::bank::{MusicBundle, SongBank};
use super::sources::{MusicSources, TrackId};
use crate::output::CpalOutput;
use crate::{Audio, Source};

/// A song's FSB5 bank decoded as kira asks for it.
struct SongDecoder {
    name: String,
    stream: Fsb5Stream,
    /// Frames returned since the last seek.
    produced: usize,
    failed: bool,
}

impl SongDecoder {
    /// Frames of silence returned per call once decoding failed.
    const SILENCE: usize = 4096;
}

impl Decoder for SongDecoder {
    type Error = String;

    fn sample_rate(&self) -> u32 {
        self.stream.sample_rate
    }

    fn num_frames(&self) -> usize {
        self.stream.frames as usize
    }

    /// Never errs: a corrupt clip turns silent for its remaining length, as kira
    /// retries a failed chunk forever.
    fn decode(&mut self) -> Result<Vec<Frame>, String> {
        let left = self.num_frames().saturating_sub(self.produced);
        let chunk = if self.failed {
            None
        } else {
            match self.stream.next_chunk() {
                Ok(chunk) => chunk,
                Err(e) => {
                    log::warn!("song {}: {e}", self.name);
                    self.failed = true;
                    None
                }
            }
        };
        let frames: Vec<Frame> = match chunk {
            Some(samples) => {
                let channels = usize::from(self.stream.channels.max(1));
                let scale = 1.0 / 32768.0;
                samples
                    .chunks_exact(channels)
                    .map(|f| {
                        Frame::new(
                            f32::from(f[0]) * scale,
                            f32::from(f[channels.min(2) - 1]) * scale,
                        )
                    })
                    .collect()
            }
            None => vec![Frame::ZERO; left.min(Self::SILENCE)],
        };
        self.produced += frames.len();
        Ok(frames)
    }

    /// Restarts from the first frame (FMOD Vorbis has no seek table).
    fn seek(&mut self, _index: usize) -> Result<usize, String> {
        self.stream.rewind();
        self.produced = 0;
        self.failed = false;
        Ok(0)
    }
}

enum LoaderEvent {
    /// The bundles opened (or failed to).
    Bundle,
    Clip(TrackId, Option<Arc<Vec<u8>>>),
}

/// Reads clip banks from the music bundles in its own thread.
struct SongLoader {
    songs: PathBuf,
    procedural: PathBuf,
    requests: Receiver<(TrackId, MusicBundle, String)>,
    events: Sender<LoaderEvent>,
}

impl SongLoader {
    fn run(self) {
        let [songs, procedural] = [&self.songs, &self.procedural].map(|path| {
            SongBank::open_path(path)
                .inspect_err(|e| {
                    log::warn!("music bundle unavailable ({e}); its clips stay silent")
                })
                .ok()
        });
        if self.events.send(LoaderEvent::Bundle).is_err() {
            return;
        }
        while let Ok((track, bundle, name)) = self.requests.recv() {
            let bank = match bundle {
                MusicBundle::Songs => &songs,
                MusicBundle::Procedural => &procedural,
            };
            let bytes = bank.as_ref().and_then(|b| {
                b.bank(&name)
                    .inspect_err(|e| log::warn!("song {name}: {e}"))
                    .ok()
            });
            if self
                .events
                .send(LoaderEvent::Clip(track, bytes.map(Arc::new)))
                .is_err()
            {
                break;
            }
        }
    }
}

enum ClipState {
    Loading,
    Ready(Arc<Vec<u8>>),
    Missing,
}

struct KiraTrack {
    clip: String,
    state: ClipState,
    volume: f32,
    handle: Option<StreamingSoundHandle<String>>,
    /// Seconds, from the bank's header once it arrived.
    clip_length: Option<f32>,
}

/// The music's sources: the clip loader and a streamed kira sound per playing
/// sub-track, mixed on the main track (under `AudioListener.volume`).
pub struct MusicOutput {
    requests: Sender<(TrackId, MusicBundle, String)>,
    events: Receiver<LoaderEvent>,
    bundles_loaded: bool,
    tracks: FxHashMap<TrackId, KiraTrack>,
}

impl MusicOutput {
    /// Starts the loader thread on `music_songs` and `music_procedural`.
    #[must_use]
    pub fn new(assets: &StreamingAssets) -> Self {
        let (requests, loader_requests) = channel();
        let (loader_events, events) = channel();
        let loader = SongLoader {
            songs: assets.asset_bundle(MusicBundle::Songs.value()),
            procedural: assets.asset_bundle(MusicBundle::Procedural.value()),
            requests: loader_requests,
            events: loader_events,
        };
        thread::Builder::new()
            .name("rw-audio music".into())
            .spawn(move || loader.run())
            .expect("spawning the music loader thread");
        Self {
            requests,
            events,
            bundles_loaded: false,
            tracks: FxHashMap::default(),
        }
    }

    /// The sources for one [`super::MusicPlayer::update`], playing through `audio`.
    pub fn sources<'a>(&'a mut self, audio: &'a mut Audio) -> KiraMusic<'a> {
        self.poll();
        KiraMusic {
            output: self,
            manager: &mut audio.manager,
        }
    }

    fn poll(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                LoaderEvent::Bundle => self.bundles_loaded = true,
                LoaderEvent::Clip(id, bytes) => {
                    if let Some(track) = self.tracks.get_mut(&id) {
                        track.clip_length = bytes
                            .as_ref()
                            .and_then(|b| Fsb5::parse(b).ok())
                            .map(|f| f.frames as f32 / f.sample_rate as f32);
                        track.state = bytes.map_or(ClipState::Missing, ClipState::Ready);
                    }
                }
            }
        }
    }
}

impl super::sources::MusicClock for MusicOutput {
    /// kira's position of the streamed sound.
    fn playback(&self, track: TrackId) -> Option<super::sources::SourcePlayback> {
        let t = self.tracks.get(&track)?;
        Some(super::sources::SourcePlayback {
            is_playing: t
                .handle
                .as_ref()
                .is_some_and(|h| h.state() != PlaybackState::Stopped),
            time: t.handle.as_ref().map_or(0.0, |h| h.position() as f32),
            clip_length: t.clip_length,
        })
    }
}

/// [`MusicOutput`] bound to the audio manager for an update.
pub struct KiraMusic<'a> {
    output: &'a mut MusicOutput,
    manager: &'a mut AudioManager<CpalOutput>,
}

impl KiraMusic<'_> {
    fn start(&mut self, id: TrackId, looped: bool) -> Option<StreamingSoundHandle<String>> {
        let track = self.output.tracks.get(&id)?;
        let ClipState::Ready(bytes) = &track.state else {
            return None;
        };
        let stream = Fsb5Stream::new(bytes.to_vec())
            .inspect_err(|e| log::warn!("song {}: {e}", track.clip))
            .ok()?;
        let decoder = SongDecoder {
            name: track.clip.clone(),
            stream,
            produced: 0,
            failed: false,
        };
        let mut data =
            StreamingSoundData::from_decoder(decoder).volume(Source::amplitude_db(track.volume));
        if looped {
            data = data.loop_region(..);
        }
        log::info!("music {} playing (looped {looped})", track.clip);
        self.manager
            .play(data)
            .inspect_err(|e| log::warn!("song {} not played: {e}", track.clip))
            .ok()
    }
}

impl MusicSources for KiraMusic<'_> {
    fn bundles_loaded(&self) -> bool {
        self.output.bundles_loaded
    }

    fn load(&mut self, track: TrackId, bundle: MusicBundle, clip: &str) {
        let state = if self
            .output
            .requests
            .send((track, bundle, clip.to_owned()))
            .is_ok()
        {
            ClipState::Loading
        } else {
            ClipState::Missing
        };
        self.output.tracks.insert(
            track,
            KiraTrack {
                clip: clip.to_owned(),
                state,
                volume: 0.0,
                handle: None,
                clip_length: None,
            },
        );
    }

    fn loaded(&self, track: TrackId) -> bool {
        self.output
            .tracks
            .get(&track)
            .is_some_and(|t| matches!(t.state, ClipState::Ready(_)))
    }

    fn is_playing(&self, track: TrackId) -> bool {
        self.output
            .tracks
            .get(&track)
            .and_then(|t| t.handle.as_ref())
            .is_some_and(|h| h.state() != PlaybackState::Stopped)
    }

    fn play(&mut self, track: TrackId, looped: bool) {
        if let Some(mut old) = self
            .output
            .tracks
            .get_mut(&track)
            .and_then(|t| t.handle.take())
        {
            old.stop(Tween::default());
        }
        let handle = self.start(track, looped);
        if let Some(t) = self.output.tracks.get_mut(&track) {
            t.handle = handle;
        }
    }

    fn set_volume(&mut self, track: TrackId, volume: f32) {
        let Some(t) = self.output.tracks.get_mut(&track) else {
            return;
        };
        t.volume = volume;
        if let Some(handle) = &mut t.handle {
            handle.set_volume(Source::amplitude_db(volume), Tween::default());
        }
    }

    fn destroy(&mut self, track: TrackId) {
        if let Some(mut handle) = self.output.tracks.remove(&track).and_then(|t| t.handle) {
            handle.stop(Tween::default());
        }
    }
}
