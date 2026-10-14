//! Futile atlas frame data: the TexturePacker JSON next to each atlas PNG.
//!
//! The atlases live inside the game's Unity files and are extracted with AssetRipper
//! into a local cache (`rainworld-re/tools/extract_atlases.sh`, see
//! `rainworld-re/notes/sprite-atlases.md`); [`AssetCache`] finds them there.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Environment variable naming the asset cache directory.
pub const CACHE_ENV: &str = "RW_ASSET_CACHE";

/// The local directory holding extracted game assets.
///
/// Layout: `<root>/atlases/<name>.json` and `<root>/atlases/<name>.png`, which is
/// AssetRipper's `ExportedProject/Assets/Resources` directory as exported (so the
/// root can point straight at an export).
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

    /// Loads atlas `name`'s frame data.
    ///
    /// # Errors
    /// As [`Atlas::load`].
    pub fn load_atlas(&self, name: &str) -> Result<Atlas, AtlasError> {
        Atlas::load(self.atlas_json(name))
    }
}

/// A rectangle in atlas pixels, `y` from the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A width and height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

/// One element (`FAtlasElement`).
///
/// Futile positions a sprite by its untrimmed `source_size` box and the anchor, and
/// draws only the trimmed `frame`, placed at `sprite_source_size.x/y` from the box's
/// top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlasFrame {
    /// Pixels in the atlas image.
    pub frame: Rect,
    /// Always false in the game's atlases (Futile rejects rotated frames).
    pub rotated: bool,
    pub trimmed: bool,
    /// Where `frame` sits in the untrimmed image, from its top-left.
    pub sprite_source_size: Rect,
    /// The untrimmed image size.
    pub source_size: Size,
}

/// One atlas: its image size and elements by name (file extension removed, as
/// Futile's `shouldRemoveAtlasElementFileExtensions`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atlas {
    /// The image file named in the metadata.
    pub image: String,
    pub size: Size,
    pub frames: HashMap<String, AtlasFrame>,
}

/// Error from reading atlas frame data.
#[derive(Debug)]
#[non_exhaustive]
pub enum AtlasError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json(serde_json::Error),
    /// A frame Futile would reject.
    Rotated(String),
}

impl fmt::Display for AtlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Json(e) => write!(f, "bad atlas JSON: {e}"),
            Self::Rotated(name) => write!(f, "atlas frame {name} is rotated"),
        }
    }
}

impl std::error::Error for AtlasError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(e) => Some(e),
            Self::Rotated(_) => None,
        }
    }
}

/// The file layout of TexturePacker's "Unity3D" JSON.
#[derive(Deserialize)]
struct AtlasFile {
    frames: HashMap<String, AtlasFrame>,
    meta: AtlasMeta,
}

#[derive(Deserialize)]
struct AtlasMeta {
    image: String,
    size: Size,
}

impl Atlas {
    /// Parses TexturePacker JSON.
    ///
    /// # Errors
    /// [`AtlasError::Json`] for malformed data, [`AtlasError::Rotated`] for a rotated frame.
    pub fn parse(json: &str) -> Result<Self, AtlasError> {
        let file: AtlasFile = serde_json::from_str(json).map_err(AtlasError::Json)?;
        let mut frames = HashMap::with_capacity(file.frames.len());
        for (key, frame) in file.frames {
            // Futile cuts at the last '.'.
            let name = key
                .rsplit_once('.')
                .map_or(key.as_str(), |(stem, _)| stem)
                .to_owned();
            if frame.rotated {
                return Err(AtlasError::Rotated(name));
            }
            frames.insert(name, frame);
        }
        Ok(Self {
            image: file.meta.image,
            size: file.meta.size,
            frames,
        })
    }

    /// Reads and parses a JSON file.
    ///
    /// # Errors
    /// [`AtlasError::Io`] if it cannot be read, otherwise as [`Atlas::parse`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AtlasError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| AtlasError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AtlasFrame> {
        self.frames.get(name)
    }
}
