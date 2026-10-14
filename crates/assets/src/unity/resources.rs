//! `Resources.Load`: the `ResourceManager` path table of `globalgamemanagers` over the
//! objects of `resources.assets` (`rainworld-re/notes/unity-assets.md` section 3).

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::objects::{AudioClip, ImageData, StreamedResource, TextAsset, Texture2D};
use super::reader::Reader;
use super::serialized::{ObjectInfo, SerializedFile, UnityObject};
use crate::{AssetError, PngImage};

/// The game's `Resources/` assets, read from `RainWorld_Data` (never written).
#[derive(Debug)]
pub struct UnityResources {
    data_dir: PathBuf,
    resources: SerializedFile,
    /// Lower-case resource path to the `resources.assets` objects of that path.
    container: HashMap<String, Vec<i64>>,
}

impl UnityResources {
    const RESOURCE_MANAGER: i32 = 147;
    const RESOURCES_FILE: &'static str = "resources.assets";

    /// Reads the path table and the object table of `resources.assets` in `data_dir`
    /// (`RainWorld_Data`).
    ///
    /// # Errors
    /// When either file cannot be read or parsed.
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Self, AssetError> {
        let data_dir = data_dir.into();
        let managers = SerializedFile::open(&data_dir.join("globalgamemanagers"))?;
        let manager = managers
            .objects()
            .iter()
            .find(|o| o.class_id == Self::RESOURCE_MANAGER)
            .ok_or_else(|| AssetError::format(managers.path(), "no ResourceManager"))?;
        let data = managers.object_data(manager)?;
        let entries = Self::parse_container(&data)
            .map_err(|m| AssetError::format(managers.path(), format!("ResourceManager: {m}")))?;
        let mut container: HashMap<String, Vec<i64>> = HashMap::new();
        for (path, file_id, path_id) in entries {
            let external = usize::try_from(file_id)
                .ok()
                .and_then(|i| managers.externals().get(i.checked_sub(1)?));
            if external.is_some_and(|e| e == Self::RESOURCES_FILE) {
                container
                    .entry(path.to_lowercase())
                    .or_default()
                    .push(path_id);
            }
        }
        let resources = SerializedFile::open(&data_dir.join(Self::RESOURCES_FILE))?;
        Ok(Self {
            data_dir,
            resources,
            container,
        })
    }

    /// `m_Container`: `(path, file ID, path ID)` entries.
    fn parse_container(data: &[u8]) -> Result<Vec<(String, i32, i64)>, String> {
        let mut r = Reader::new(data);
        (0..r.count()?)
            .map(|_| Ok((r.string()?, r.i32()?, r.i64()?)))
            .collect()
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    #[must_use]
    pub fn file(&self) -> &SerializedFile {
        &self.resources
    }

    /// Resource paths (lower case), in no particular order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.container.keys().map(String::as_str)
    }

    /// The object of class `class_id` at a `Resources.Load` path (case-insensitive,
    /// no extension), e.g. `Atlases/rainWorld`.
    #[must_use]
    pub fn find(&self, path: &str, class_id: i32) -> Option<ObjectInfo> {
        self.container
            .get(&path.to_lowercase())?
            .iter()
            .filter_map(|&id| self.resources.object(id))
            .find(|o| o.class_id == class_id)
    }

    /// `Resources.Load<T>(path)`.
    ///
    /// # Errors
    /// [`AssetError::Missing`] when no `T` has that path, else when it cannot be read.
    pub fn load<T: UnityObject>(&self, path: &str) -> Result<T, AssetError> {
        let info = self
            .find(path, T::CLASS_ID)
            .ok_or_else(|| AssetError::Missing {
                what: format!(
                    "resource {path} (class {}) in {}",
                    T::CLASS_ID,
                    self.resources.path().display()
                ),
            })?;
        self.resources.read(&info)
    }

    /// The bytes of the TextAsset at `path`.
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn text(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        self.load::<TextAsset>(path).map(|t| t.script)
    }

    /// The Texture2D at `path`, mip 0 decoded to RGBA8 (PNG row order).
    ///
    /// # Errors
    /// As [`Self::load`], or when its pixels cannot be read or decoded.
    pub fn texture_image(&self, path: &str) -> Result<PngImage, AssetError> {
        let texture: Texture2D = self.load(path)?;
        let data = match &texture.image {
            ImageData::Inline(data) => data.clone(),
            ImageData::Streamed(stream) => self.streamed(stream)?,
        };
        texture
            .decode(&data)
            .map_err(|m| AssetError::format(self.resources.path(), m))
    }

    /// The FSB5 bank of a clip from [`Self::load`].
    ///
    /// # Errors
    /// When the resource file cannot be read.
    pub fn audio_data(&self, clip: &AudioClip) -> Result<Vec<u8>, AssetError> {
        self.streamed(&clip.resource)
    }

    /// Reads bytes stored in a file next to `resources.assets` (`.resS`, `.resource`).
    ///
    /// # Errors
    /// When the path is not a plain file name or the bytes cannot be read.
    pub fn streamed(&self, stream: &StreamedResource) -> Result<Vec<u8>, AssetError> {
        let plain = !stream.path.is_empty()
            && !stream.path.contains(['/', '\\'])
            && stream.path != ".."
            && stream.path != ".";
        if !plain {
            return Err(AssetError::format(
                self.resources.path(),
                format!("unsupported resource path {:?}", stream.path),
            ));
        }
        let path = self.data_dir.join(&stream.path);
        let size = usize::try_from(stream.size)
            .map_err(|_| AssetError::format(&path, "resource too large"))?;
        let mut data = vec![0; size];
        File::open(&path)
            .and_then(|mut f| {
                f.seek(SeekFrom::Start(stream.offset))?;
                f.read_exact(&mut data)
            })
            .map_err(|e| AssetError::io(&path, e))?;
        Ok(data)
    }
}
