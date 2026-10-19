//! Clips decoded on demand on background threads and kept for the session (the game's
//! `CacheAllAudio`). A sound waits for its clip as the game's `SoundObject` waits for
//! its `AssetBundleLoadAssetOperation`.
//!
//! Mirrors: SoundLoader.cs

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use kira::Frame;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use rustc_hash::FxHashMap;
use rw_assets::unity::{AssetBundle, AudioClip, PcmAudio};

/// A clip by bundle and asset name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClipKey {
    /// A variation in `loadedsoundeffects` (`jump2_3`).
    Effect(String),
    /// A sample in `loadedsoundeffects_ambient`, without ".ogg".
    Ambient(String),
}

#[derive(Debug, Clone)]
pub enum ClipSlot {
    Loading,
    Ready(Box<StaticSoundData>),
    /// Not in the bundle, or it failed to decode.
    Missing,
}

/// Decodes the clips of one bundle in its own thread.
struct ClipWorker {
    path: PathBuf,
    requests: Receiver<ClipKey>,
    results: Sender<(ClipKey, Option<StaticSoundData>)>,
}

impl ClipWorker {
    fn run(self) {
        let bundle = match AssetBundle::open(&self.path) {
            Ok(bundle) => Some(bundle),
            Err(e) => {
                log::warn!("audio bundle unavailable ({e}); its sounds stay silent");
                None
            }
        };
        while let Ok(key) = self.requests.recv() {
            let data = bundle.as_ref().and_then(|b| Self::load(b, &key));
            if self.results.send((key, data)).is_err() {
                break;
            }
        }
    }

    fn load(bundle: &AssetBundle, key: &ClipKey) -> Option<StaticSoundData> {
        let name = match key {
            ClipKey::Effect(name) | ClipKey::Ambient(name) => name,
        };
        let decoded = bundle
            .load::<AudioClip>(name)
            .and_then(|clip| bundle.decode_audio(&clip));
        match decoded {
            Ok(pcm) => Some(Self::sound_data(&pcm)),
            Err(e) => {
                log::warn!("clip {name}: {e}");
                None
            }
        }
    }

    fn sound_data(pcm: &PcmAudio) -> StaticSoundData {
        let channels = usize::from(pcm.channels.max(1));
        let scale = 1.0 / 32768.0;
        let frames = pcm
            .samples
            .chunks_exact(channels)
            .map(|f| {
                let left = f32::from(f[0]) * scale;
                let right = f32::from(f[channels.min(2) - 1]) * scale;
                Frame::new(left, right)
            })
            .collect();
        StaticSoundData {
            sample_rate: pcm.sample_rate,
            frames,
            settings: StaticSoundSettings::default(),
            slice: None,
        }
    }
}

/// The clip cache and the two loader threads.
pub struct ClipLoader {
    effects: Sender<ClipKey>,
    ambient: Sender<ClipKey>,
    results: Receiver<(ClipKey, Option<StaticSoundData>)>,
    cache: FxHashMap<ClipKey, ClipSlot>,
}

impl ClipLoader {
    /// Starts the loaders of the two bundles (opened in their threads).
    #[must_use]
    pub fn new(effects: PathBuf, ambient: PathBuf) -> Self {
        let (results_tx, results) = channel();
        let effects = Self::spawn(effects, results_tx.clone());
        let ambient = Self::spawn(ambient, results_tx);
        Self {
            effects,
            ambient,
            results,
            cache: FxHashMap::default(),
        }
    }

    fn spawn(
        path: PathBuf,
        results: Sender<(ClipKey, Option<StaticSoundData>)>,
    ) -> Sender<ClipKey> {
        let (tx, requests) = channel();
        let worker = ClipWorker {
            path,
            requests,
            results,
        };
        thread::Builder::new()
            .name("rw-audio clips".into())
            .spawn(move || worker.run())
            .expect("spawning the clip loader thread");
        tx
    }

    /// Takes in what the threads finished.
    pub fn poll(&mut self) {
        while let Ok((key, data)) = self.results.try_recv() {
            let slot = data.map_or(ClipSlot::Missing, |d| ClipSlot::Ready(Box::new(d)));
            self.cache.insert(key, slot);
        }
    }

    /// The clip, requesting it on first use.
    pub fn get(&mut self, key: &ClipKey) -> &ClipSlot {
        if !self.cache.contains_key(key) {
            let sender = match key {
                ClipKey::Effect(_) => &self.effects,
                ClipKey::Ambient(_) => &self.ambient,
            };
            let slot = if sender.send(key.clone()).is_ok() {
                ClipSlot::Loading
            } else {
                ClipSlot::Missing
            };
            self.cache.insert(key.clone(), slot);
        }
        &self.cache[key]
    }
}
