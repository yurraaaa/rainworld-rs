//! Futile atlas frame data: the TexturePacker JSON next to each atlas texture.
//!
//! The atlases are Unity resources `Atlases/<name>` in `resources.assets` (a TextAsset
//! and a Texture2D of the same path), read with [`UnityResources`]
//! (`rainworld-re/notes/sprite-atlases.md`, `unity-assets.md`).

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use rw_assets::{AssetError, UnityResources};

use serde::Deserialize;

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
    /// The atlas resource could not be read.
    Asset(AssetError),
    /// A frame Futile would reject.
    Rotated(String),
}

impl fmt::Display for AtlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Json(e) => write!(f, "bad atlas JSON: {e}"),
            Self::Asset(e) => write!(f, "cannot load atlas: {e}"),
            Self::Rotated(name) => write!(f, "atlas frame {name} is rotated"),
        }
    }
}

impl std::error::Error for AtlasError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(e) => Some(e),
            Self::Asset(e) => Some(e),
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
        Self::parse_bytes(json.as_bytes())
    }

    /// Parses TexturePacker JSON bytes (a TextAsset's script).
    ///
    /// # Errors
    /// As [`Atlas::parse`].
    pub fn parse_bytes(json: &[u8]) -> Result<Self, AtlasError> {
        let file: AtlasFile = serde_json::from_slice(json).map_err(AtlasError::Json)?;
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

    /// Atlas `name`'s frame data: the TextAsset `Atlases/<name>`, as Futile's
    /// `FAtlas` loads it.
    ///
    /// # Errors
    /// [`AtlasError::Asset`] when it cannot be read, otherwise as [`Atlas::parse`].
    pub fn from_resources(resources: &UnityResources, name: &str) -> Result<Self, AtlasError> {
        let json = resources
            .text(&format!("Atlases/{name}"))
            .map_err(AtlasError::Asset)?;
        Self::parse_bytes(&json)
    }

    /// `LoadAtlasFromTexture(name, texture)`: one element `name` covering a `w` x `h`
    /// texture.
    #[must_use]
    pub fn single(name: &str, w: u32, h: u32) -> Self {
        let rect = Rect { x: 0, y: 0, w, h };
        let frame = AtlasFrame {
            frame: rect,
            rotated: false,
            trimmed: false,
            sprite_source_size: rect,
            source_size: Size { w, h },
        };
        Self {
            image: name.to_owned(),
            size: Size { w, h },
            frames: HashMap::from([(name.to_owned(), frame)]),
        }
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AtlasFrame> {
        self.frames.get(name)
    }
}
