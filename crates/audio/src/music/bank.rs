//! The `music_songs` bundle: song clips by name (`AssetBundle.LoadAsset<AudioClip>`).

use std::path::Path;

use rw_assets::unity::{AssetBundle, AudioClip, Fsb5};
use rw_assets::{AssetError, StreamingAssets};

/// A clip's length from its FSB5 header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipLength {
    pub channels: u16,
    pub sample_rate: u32,
    /// Samples per channel.
    pub frames: u32,
}

/// The opened `music_songs` bundle.
#[derive(Debug)]
pub struct SongBank {
    bundle: AssetBundle,
}

impl SongBank {
    pub const BUNDLE: &'static str = "music_songs";

    /// # Errors
    /// When the bundle cannot be read.
    pub fn open(assets: &StreamingAssets) -> Result<Self, AssetError> {
        Self::open_path(&assets.asset_bundle(Self::BUNDLE))
    }

    /// # Errors
    /// When the bundle cannot be read.
    pub fn open_path(path: &Path) -> Result<Self, AssetError> {
        AssetBundle::open(path).map(|bundle| Self { bundle })
    }

    /// The FSB5 bank of the song clip `name` (case-insensitive, as `LoadAsset`).
    ///
    /// # Errors
    /// When there is no such clip or it cannot be read.
    pub fn bank(&self, name: &str) -> Result<Vec<u8>, AssetError> {
        let clip: AudioClip = self.bundle.load(name)?;
        self.bundle.audio_data(&clip)
    }

    /// The clip's length, `None` when it is missing or unreadable.
    #[must_use]
    pub fn length(&self, name: &str) -> Option<ClipLength> {
        let bytes = self
            .bank(name)
            .inspect_err(|e| log::warn!("song {name}: {e}"))
            .ok()?;
        let fsb = Fsb5::parse(&bytes)
            .inspect_err(|e| log::warn!("song {name}: {e}"))
            .ok()?;
        Some(ClipLength {
            channels: fsb.channels,
            sample_rate: fsb.sample_rate,
            frames: fsb.frames,
        })
    }

    /// Every clip's `m_Name`.
    ///
    /// # Errors
    /// When a clip cannot be read.
    pub fn clip_names(&self) -> Result<Vec<String>, AssetError> {
        self.bundle
            .asset_paths()
            .map(|path| self.bundle.load::<AudioClip>(path).map(|c| c.name))
            .collect()
    }
}
