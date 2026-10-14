//! Locating game files under `StreamingAssets`, and the error type of the visual-asset
//! loaders (images, palettes, settings, noise).

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// The installed game's `RainWorld_Data/StreamingAssets` directory (read only).
///
/// File and directory names on disk are lower case; every lookup lowercases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingAssets(PathBuf);

impl StreamingAssets {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// `<StreamingAssets>/<relative>` with `relative` lowercased.
    #[must_use]
    pub fn file(&self, relative: &str) -> PathBuf {
        self.0.join(relative.to_lowercase())
    }

    /// The game's room file lookup (`WorldLoader.FindRoomFile`): `<room><suffix>` in
    /// `world/<prefix>-rooms` (prefix = text before the first `_`), then `world/gates`
    /// (only for `GATE_` rooms), then `world/gates/gate_shelters`, then `levels`.
    /// Each suffix is looked up on its own, as in the game.
    #[must_use]
    pub fn find_room_file(&self, room: &str, suffix: &str) -> Option<PathBuf> {
        let name = format!("{room}{suffix}");
        let prefix = room.split('_').next().unwrap_or(room);
        let mut candidates = vec![format!("world/{prefix}-rooms/{name}")];
        if prefix.eq_ignore_ascii_case("gate") {
            candidates.push(format!("world/gates/{name}"));
        }
        candidates.push(format!("world/gates/gate_shelters/{name}"));
        candidates.push(format!("levels/{name}"));
        candidates
            .iter()
            .map(|c| self.file(c))
            .find(|p| p.is_file())
    }

    /// The region whose settings templates a room inherits from: the prefix of a room
    /// found in `world/<prefix>-rooms` when `world/<prefix>` exists. Arena levels and
    /// gates (whose region is the world being loaded) give `None`.
    #[must_use]
    pub fn region_of_room(&self, room: &str) -> Option<String> {
        let prefix = room.split('_').next()?.to_lowercase();
        let in_region = self
            .find_room_file(room, ".txt")?
            .starts_with(self.file(&format!("world/{prefix}-rooms")));
        (in_region && self.file(&format!("world/{prefix}")).is_dir()).then_some(prefix)
    }
}

/// Error from loading a visual asset (image, palette, settings, noise texture).
#[derive(Debug)]
#[non_exhaustive]
pub enum AssetError {
    /// Reading the file failed.
    Io { path: PathBuf, source: io::Error },
    /// The PNG could not be decoded.
    Png {
        path: PathBuf,
        source: png::DecodingError,
    },
    /// The file was read but its contents are not what the game expects.
    Format { path: PathBuf, message: String },
    /// No file found for a room.
    Missing { what: String },
}

impl AssetError {
    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }

    pub(crate) fn format(path: &Path, message: impl Into<String>) -> Self {
        Self::Format {
            path: path.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } => write!(f, "failed to read {}", path.display()),
            Self::Png { path, .. } => write!(f, "failed to decode {}", path.display()),
            Self::Format { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Missing { what } => write!(f, "not found: {what}"),
        }
    }
}

impl Error for AssetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Png { source, .. } => Some(source),
            _ => None,
        }
    }
}
