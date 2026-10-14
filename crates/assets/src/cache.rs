//! The local cache of game assets that live inside the game's Unity files (sprite
//! atlases, the noise texture), extracted with AssetRipper by
//! `rainworld-re/tools/extract_atlases.sh` (`rainworld-re/notes/sprite-atlases.md`).
//! Never inside the repository or the game directory.

use std::path::PathBuf;

/// Environment variable naming the asset cache directory.
pub const CACHE_ENV: &str = "RW_ASSET_CACHE";

/// The local directory holding extracted game assets.
///
/// Layout: `<root>/atlases/<name>.json|png` and `<root>/palettes/noise.png`, which is
/// AssetRipper's `ExportedProject/Assets/Resources` directory as exported (so the root
/// can point straight at an export).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCache {
    pub root: PathBuf,
}

impl AssetCache {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$RW_ASSET_CACHE`, else `$XDG_CACHE_HOME/rainworld-rs`, else
    /// `$HOME/.cache/rainworld-rs`; `None` if none of them is set.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        if let Some(dir) = std::env::var_os(CACHE_ENV) {
            return Some(Self::new(dir));
        }
        if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
            return Some(Self::new(PathBuf::from(dir).join("rainworld-rs")));
        }
        std::env::var_os("HOME")
            .map(|home| Self::new(PathBuf::from(home).join(".cache/rainworld-rs")))
    }

    /// Path of atlas `name`'s frame data (e.g. `rainWorld`).
    #[must_use]
    pub fn atlas_json(&self, name: &str) -> PathBuf {
        self.root.join("atlases").join(format!("{name}.json"))
    }

    /// Path of atlas `name`'s image.
    #[must_use]
    pub fn atlas_png(&self, name: &str) -> PathBuf {
        self.root.join("atlases").join(format!("{name}.png"))
    }

    /// Path of the level shader's noise texture (Unity resource `Palettes/noise`).
    #[must_use]
    pub fn noise_png(&self) -> PathBuf {
        self.root.join("palettes").join("noise.png")
    }

    /// Hint for error messages: how to fill the cache.
    #[must_use]
    pub fn fill_hint(&self) -> String {
        format!(
            "run rainworld-re/tools/extract_atlases.sh OUTDIR PORT {} to extract the game's \
             Unity assets into the cache (or set {CACHE_ENV})",
            self.root.display()
        )
    }
}
