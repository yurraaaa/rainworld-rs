//! Unity serialized files (format version 22): header, object table and external
//! references (`rainworld-re/notes/unity-assets.md` section 2).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::reader::Reader;
use crate::AssetError;

/// One object of a serialized file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectInfo {
    pub path_id: i64,
    pub class_id: i32,
    /// Byte offset of the object's data in the file.
    pub offset: u64,
    pub size: u32,
}

/// A Unity object class whose 2020.3 layout is known.
pub trait UnityObject: Sized {
    const CLASS_ID: i32;

    /// Parses the object's data, which must be consumed exactly.
    ///
    /// # Errors
    /// A message when the data does not fit the layout.
    fn parse(data: &[u8]) -> Result<Self, String>;
}

/// Where the object data lives: the file on disk (read per object) or bytes already in
/// memory (a bundle node).
#[derive(Debug)]
enum Source {
    File,
    Memory(Vec<u8>),
}

/// A parsed serialized file: its objects and externals; object data is read on demand.
#[derive(Debug)]
pub struct SerializedFile {
    path: PathBuf,
    source: Source,
    unity_version: String,
    objects: Vec<ObjectInfo>,
    externals: Vec<String>,
}

impl SerializedFile {
    pub const VERSION: u32 = 22;
    const HEADER_SIZE: usize = 48;

    /// Reads the header and metadata of the file at `path`. Never writes.
    ///
    /// # Errors
    /// When the file cannot be read or is not a version-22 serialized file.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        let mut file = File::open(path).map_err(|e| AssetError::io(path, e))?;
        let mut head = vec![0; Self::HEADER_SIZE];
        file.read_exact(&mut head)
            .map_err(|e| AssetError::io(path, e))?;
        let metadata_size = Self::metadata_size(&head).map_err(|m| AssetError::format(path, m))?;
        head.resize(Self::HEADER_SIZE + metadata_size, 0);
        file.read_exact(&mut head[Self::HEADER_SIZE..])
            .map_err(|e| AssetError::io(path, e))?;
        Self::parse(path, &head, Source::File).map_err(|m| AssetError::format(path, m))
    }

    /// A serialized file held in memory; `path` names it in errors.
    ///
    /// # Errors
    /// When the bytes are not a version-22 serialized file.
    pub fn from_bytes(path: &Path, bytes: Vec<u8>) -> Result<Self, AssetError> {
        let mut file =
            Self::parse(path, &bytes, Source::File).map_err(|m| AssetError::format(path, m))?;
        file.source = Source::Memory(bytes);
        Ok(file)
    }

    fn metadata_size(head: &[u8]) -> Result<usize, String> {
        let mut r = Reader::big_endian(head);
        r.skip(8)?;
        let version = r.u32()?;
        if version != Self::VERSION {
            return Err(format!(
                "serialized file version {version}, only {} is supported",
                Self::VERSION
            ));
        }
        r.skip(8)?;
        Ok(r.u32()? as usize)
    }

    /// Parses header and metadata from `bytes` (which may stop after the metadata).
    fn parse(path: &Path, bytes: &[u8], source: Source) -> Result<Self, String> {
        let metadata_size = Self::metadata_size(bytes)?;
        let mut r = Reader::big_endian(bytes);
        r.seek(16);
        let big_endian = r.u8()? != 0;
        r.skip(3 + 4)?;
        let _file_size = r.u64()?;
        let data_offset = r.u64()?;
        r.skip(8)?;
        let end = Self::HEADER_SIZE + metadata_size;
        let mut r = Reader::new(
            bytes
                .get(..end)
                .ok_or("metadata runs past the end of the file")?,
        );
        r.seek(Self::HEADER_SIZE);
        r.set_big_endian(big_endian);
        let unity_version = r.cstr()?.to_owned();
        let _platform = r.i32()?;
        let type_trees = r.bool()?;
        let class_ids = (0..r.count()?)
            .map(|_| Self::read_type(&mut r, type_trees))
            .collect::<Result<Vec<_>, _>>()?;
        let object_count = r.count()?;
        r.align();
        let objects = (0..object_count)
            .map(|_| Self::read_object(&mut r, &class_ids, data_offset))
            .collect::<Result<Vec<_>, _>>()?;
        for _ in 0..r.count()? {
            r.i32()?;
            r.align();
            r.i64()?;
        }
        let externals = (0..r.count()?)
            .map(|_| Self::read_external(&mut r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            path: path.to_owned(),
            source,
            unity_version,
            objects,
            externals,
        })
    }

    /// One type entry; returns its class ID (type trees are skipped).
    fn read_type(r: &mut Reader, type_trees: bool) -> Result<i32, String> {
        let class_id = r.i32()?;
        let _stripped = r.bool()?;
        let _script_index = r.i16()?;
        if class_id == 114 {
            r.skip(16)?;
        }
        r.skip(16)?;
        if type_trees {
            let nodes = r.count()?;
            let strings = r.count()?;
            r.skip(nodes * 32 + strings)?;
            let dependencies = r.count()?;
            r.skip(dependencies * 4)?;
        }
        Ok(class_id)
    }

    fn read_object(
        r: &mut Reader,
        class_ids: &[i32],
        data_offset: u64,
    ) -> Result<ObjectInfo, String> {
        let path_id = r.i64()?;
        let start = r.u64()?;
        let size = r.u32()?;
        let type_index = r.i32()?;
        let class_id = usize::try_from(type_index)
            .ok()
            .and_then(|i| class_ids.get(i))
            .copied()
            .ok_or_else(|| format!("object {path_id} has bad type index {type_index}"))?;
        Ok(ObjectInfo {
            path_id,
            class_id,
            offset: data_offset + start,
            size,
        })
    }

    fn read_external(r: &mut Reader) -> Result<String, String> {
        r.cstr()?;
        r.skip(16 + 4)?;
        Ok(r.cstr()?.to_owned())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn unity_version(&self) -> &str {
        &self.unity_version
    }

    #[must_use]
    pub fn objects(&self) -> &[ObjectInfo] {
        &self.objects
    }

    /// External file paths; a `PPtr` file ID `n > 0` names `externals()[n - 1]`.
    #[must_use]
    pub fn externals(&self) -> &[String] {
        &self.externals
    }

    #[must_use]
    pub fn object(&self, path_id: i64) -> Option<ObjectInfo> {
        self.objects.iter().copied().find(|o| o.path_id == path_id)
    }

    /// The raw data of an object.
    ///
    /// # Errors
    /// When it cannot be read.
    pub fn object_data(&self, info: &ObjectInfo) -> Result<Vec<u8>, AssetError> {
        let size = info.size as usize;
        match &self.source {
            Source::Memory(bytes) => usize::try_from(info.offset)
                .ok()
                .and_then(|start| bytes.get(start..start.checked_add(size)?))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| {
                    AssetError::format(&self.path, format!("object {} out of range", info.path_id))
                }),
            Source::File => {
                let mut file = File::open(&self.path).map_err(|e| AssetError::io(&self.path, e))?;
                let mut data = vec![0; size];
                file.seek(SeekFrom::Start(info.offset))
                    .and_then(|_| file.read_exact(&mut data))
                    .map_err(|e| AssetError::io(&self.path, e))?;
                Ok(data)
            }
        }
    }

    /// Parses object `info` as `T`.
    ///
    /// # Errors
    /// When the object is of another class, cannot be read or does not fit `T`'s layout.
    pub fn read<T: UnityObject>(&self, info: &ObjectInfo) -> Result<T, AssetError> {
        if info.class_id != T::CLASS_ID {
            return Err(AssetError::format(
                &self.path,
                format!(
                    "object {} is class {}, not {}",
                    info.path_id,
                    info.class_id,
                    T::CLASS_ID
                ),
            ));
        }
        T::parse(&self.object_data(info)?)
            .map_err(|m| AssetError::format(&self.path, format!("object {}: {m}", info.path_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl SerializedFile {
        /// A little-endian serialized file with one TextAsset (path ID 7) and one external.
        pub(crate) fn sample() -> Vec<u8> {
            let mut meta = Vec::new();
            meta.extend_from_slice(b"2020.3.45f1\0");
            meta.extend_from_slice(&19i32.to_le_bytes());
            meta.push(0);
            meta.extend_from_slice(&1i32.to_le_bytes());
            meta.extend_from_slice(&49i32.to_le_bytes());
            meta.push(0);
            meta.extend_from_slice(&(-1i16).to_le_bytes());
            meta.extend_from_slice(&[0; 16]);
            meta.extend_from_slice(&1i32.to_le_bytes());
            while (48 + meta.len()) % 4 != 0 {
                meta.push(0);
            }
            meta.extend_from_slice(&7i64.to_le_bytes());
            meta.extend_from_slice(&0u64.to_le_bytes());
            meta.extend_from_slice(&16u32.to_le_bytes());
            meta.extend_from_slice(&0i32.to_le_bytes());
            meta.extend_from_slice(&0i32.to_le_bytes());
            meta.extend_from_slice(&1i32.to_le_bytes());
            meta.push(0);
            meta.extend_from_slice(&[0; 20]);
            meta.extend_from_slice(b"resources.assets\0");
            let data_offset = (48 + meta.len()).next_multiple_of(16);
            let mut bytes = vec![0; 8];
            bytes.extend_from_slice(&22u32.to_be_bytes());
            bytes.extend_from_slice(&[0; 8]);
            bytes.extend_from_slice(&(meta.len() as u32).to_be_bytes());
            bytes.extend_from_slice(&((data_offset + 16) as u64).to_be_bytes());
            bytes.extend_from_slice(&(data_offset as u64).to_be_bytes());
            bytes.extend_from_slice(&[0; 8]);
            bytes.extend(meta);
            bytes.resize(data_offset, 0);
            bytes.extend_from_slice(&1i32.to_le_bytes());
            bytes.extend_from_slice(b"a\0\0\0");
            bytes.extend_from_slice(&2i32.to_le_bytes());
            bytes.extend_from_slice(b"hi\0\0");
            bytes
        }
    }

    #[test]
    fn parses_objects_and_externals() {
        let file = SerializedFile::from_bytes(Path::new("t"), SerializedFile::sample()).unwrap();
        assert_eq!(file.unity_version(), "2020.3.45f1");
        assert_eq!(file.externals(), ["resources.assets"]);
        let info = file.object(7).unwrap();
        assert_eq!((info.class_id, info.size), (49, 16));
        let text: super::super::TextAsset = file.read(&info).unwrap();
        assert_eq!(
            (text.name.as_str(), text.script.as_slice()),
            ("a", &b"hi"[..])
        );
    }

    #[test]
    fn rejects_other_versions() {
        let mut bytes = SerializedFile::sample();
        bytes[11] = 21;
        assert!(SerializedFile::from_bytes(Path::new("t"), bytes).is_err());
    }
}
