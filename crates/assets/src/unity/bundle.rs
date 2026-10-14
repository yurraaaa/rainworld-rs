//! UnityFS asset bundles (`StreamingAssets/AssetBundles/*`): block table, directory
//! and the bundle's `AssetBundle` name table (`rainworld-re/notes/unity-assets.md`
//! sections 4 and 5). Blocks are decompressed on demand, so a clip costs only the
//! blocks it spans.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::fsb5::{Fsb5, PcmAudio};
use super::objects::{AudioClip, StreamedResource};
use super::reader::Reader;
use super::serialized::{ObjectInfo, SerializedFile, UnityObject};
use crate::AssetError;

/// One compressed block; offsets are in the uncompressed stream and in the file.
#[derive(Debug, Clone, Copy)]
struct Block {
    offset: u64,
    size: u32,
    file_offset: u64,
    compressed_size: u32,
    compression: u16,
}

/// A file inside the bundle: a byte range of the uncompressed stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleNode {
    pub offset: u64,
    pub size: u64,
    pub flags: u32,
    pub path: String,
}

/// The bundle's uncompressed stream, read through its block table.
#[derive(Debug)]
struct BlockStream {
    path: PathBuf,
    blocks: Vec<Block>,
}

/// An opened UnityFS bundle with its serialized file and asset names.
#[derive(Debug)]
pub struct AssetBundle {
    stream: BlockStream,
    nodes: Vec<BundleNode>,
    file: SerializedFile,
    /// Lower-case asset path (`assets/loadedsoundeffects/arc.wav`) and path ID.
    container: Vec<(String, i64)>,
}

impl AssetBundle {
    const SIGNATURE: &'static str = "UnityFS";
    const ASSET_BUNDLE: i32 = 142;
    /// Node flag of the serialized file.
    const SERIALIZED_NODE: u32 = 4;

    /// Reads the block table, the directory, the serialized file and its names.
    ///
    /// # Errors
    /// When the file cannot be read or is not a UnityFS bundle this reader supports.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        let mut file = File::open(path).map_err(|e| AssetError::io(path, e))?;
        let mut head = Vec::with_capacity(128);
        (&mut file)
            .take(128)
            .read_to_end(&mut head)
            .map_err(|e| AssetError::io(path, e))?;
        let (blocks, nodes) =
            Self::read_tables(&mut file, &head).map_err(|m| AssetError::format(path, m))?;
        let stream = BlockStream {
            path: path.to_owned(),
            blocks,
        };
        let node = nodes
            .iter()
            .find(|n| n.flags & Self::SERIALIZED_NODE != 0)
            .ok_or_else(|| AssetError::format(path, "no serialized file in the bundle"))?;
        let bytes = stream.read(node.offset, node.size)?;
        let file = SerializedFile::from_bytes(&path.join(&node.path), bytes)?;
        let container = Self::read_container(&file)?;
        Ok(Self {
            stream,
            nodes,
            file,
            container,
        })
    }

    /// Parses the header and the (compressed) blocks info.
    fn read_tables(file: &mut File, head: &[u8]) -> Result<(Vec<Block>, Vec<BundleNode>), String> {
        let mut r = Reader::big_endian(head);
        let signature = r.cstr()?;
        if signature != Self::SIGNATURE {
            return Err(format!("signature {signature:?}, not UnityFS"));
        }
        let version = r.u32()?;
        let _player_version = r.cstr()?;
        let _engine_version = r.cstr()?;
        let _size = r.i64()?;
        let compressed_info = r.u32()?;
        let info_size = r.u32()?;
        let flags = r.u32()?;
        if flags & 0x80 != 0 {
            return Err("blocks info at the end of the file is not supported".into());
        }
        let mut pos = r.pos() as u64;
        if version >= 7 {
            pos = pos.next_multiple_of(16);
        }
        let mut raw = vec![0; compressed_info as usize];
        file.seek(SeekFrom::Start(pos))
            .and_then(|_| file.read_exact(&mut raw))
            .map_err(|e| format!("blocks info: {e}"))?;
        let info = BlockStream::decompress(&raw, (flags & 0x3f) as u16, info_size as usize)?;
        let mut data_start = pos + u64::from(compressed_info);
        if flags & 0x200 != 0 {
            data_start = data_start.next_multiple_of(16);
        }
        let mut r = Reader::big_endian(&info);
        r.skip(16)?;
        let (mut offset, mut file_offset) = (0u64, data_start);
        let mut blocks = Vec::new();
        for _ in 0..r.count()? {
            let block = Block {
                offset,
                size: r.u32()?,
                compressed_size: r.u32()?,
                compression: r.u16()? & 0x3f,
                file_offset,
            };
            offset += u64::from(block.size);
            file_offset += u64::from(block.compressed_size);
            blocks.push(block);
        }
        let nodes = (0..r.count()?)
            .map(|_| {
                Ok(BundleNode {
                    offset: r.u64()?,
                    size: r.u64()?,
                    flags: r.u32()?,
                    path: r.cstr()?.to_owned(),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok((blocks, nodes))
    }

    /// The `AssetBundle` object's `m_Container`.
    fn read_container(file: &SerializedFile) -> Result<Vec<(String, i64)>, AssetError> {
        let info = file
            .objects()
            .iter()
            .find(|o| o.class_id == Self::ASSET_BUNDLE)
            .ok_or_else(|| AssetError::format(file.path(), "no AssetBundle object"))?;
        let data = file.object_data(info)?;
        Self::parse_container(&data)
            .map_err(|m| AssetError::format(file.path(), format!("AssetBundle: {m}")))
    }

    fn parse_container(data: &[u8]) -> Result<Vec<(String, i64)>, String> {
        let mut r = Reader::new(data);
        let _name = r.string()?;
        let preload = r.count()?;
        r.skip(preload * 12)?;
        (0..r.count()?)
            .map(|_| {
                let path = r.string()?.to_lowercase();
                let _preload_index = r.i32()?;
                let _preload_size = r.i32()?;
                let _file_id = r.i32()?;
                Ok((path, r.i64()?))
            })
            .collect()
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.stream.path
    }

    #[must_use]
    pub fn nodes(&self) -> &[BundleNode] {
        &self.nodes
    }

    #[must_use]
    pub fn file(&self) -> &SerializedFile {
        &self.file
    }

    /// Asset paths (lower case) in the bundle.
    pub fn asset_paths(&self) -> impl Iterator<Item = &str> {
        self.container.iter().map(|(p, _)| p.as_str())
    }

    /// `size` bytes at `offset` within `node`.
    ///
    /// # Errors
    /// When the range is outside the node or the blocks cannot be read.
    pub fn read_node(
        &self,
        node: &BundleNode,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, AssetError> {
        if offset.checked_add(size).is_none_or(|end| end > node.size) {
            return Err(AssetError::format(
                self.path(),
                format!("range {offset}+{size} outside node {}", node.path),
            ));
        }
        self.stream.read(node.offset + offset, size)
    }

    /// The object of class `class_id` named `name`, as `AssetBundle.LoadAsset` finds it:
    /// the full asset path, else the file name without extension; case-insensitive.
    #[must_use]
    pub fn find(&self, name: &str, class_id: i32) -> Option<ObjectInfo> {
        let name = name.to_lowercase();
        let by_path = self.container.iter().filter(|(p, _)| *p == name);
        let by_stem = self.container.iter().filter(|(p, _)| Self::stem(p) == name);
        by_path
            .chain(by_stem)
            .filter_map(|&(_, id)| self.file.object(id))
            .find(|o| o.class_id == class_id)
    }

    /// Whether an asset of any class has that name (`AssetBundle.Contains`).
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.container
            .iter()
            .any(|(p, _)| *p == name || Self::stem(p) == name)
    }

    /// File name without directory and extension.
    fn stem(path: &str) -> &str {
        let file = path.rsplit('/').next().unwrap_or(path);
        file.rsplit_once('.').map_or(file, |(stem, _)| stem)
    }

    /// `AssetBundle.LoadAsset<T>(name)`.
    ///
    /// # Errors
    /// [`AssetError::Missing`] when no `T` has that name, else when it cannot be read.
    pub fn load<T: UnityObject>(&self, name: &str) -> Result<T, AssetError> {
        let info = self
            .find(name, T::CLASS_ID)
            .ok_or_else(|| AssetError::Missing {
                what: format!(
                    "asset {name} (class {}) in {}",
                    T::CLASS_ID,
                    self.path().display()
                ),
            })?;
        self.file.read(&info)
    }

    /// The FSB5 bank of a clip of this bundle.
    ///
    /// # Errors
    /// When its resource is not a node of this bundle or cannot be read.
    pub fn audio_data(&self, clip: &AudioClip) -> Result<Vec<u8>, AssetError> {
        self.streamed(&clip.resource)
    }

    /// A clip of this bundle decoded to 16-bit PCM.
    ///
    /// # Errors
    /// When its bank cannot be read or decoded.
    pub fn decode_audio(&self, clip: &AudioClip) -> Result<PcmAudio, AssetError> {
        let data = self.audio_data(clip)?;
        Fsb5::parse(&data)
            .and_then(|fsb| fsb.decode())
            .map_err(|m| AssetError::format(self.path(), format!("clip {}: {m}", clip.name)))
    }

    /// Bytes of an `archive:/<cab>/<node>` resource.
    ///
    /// # Errors
    /// When the node is not in this bundle or the bytes cannot be read.
    pub fn streamed(&self, stream: &StreamedResource) -> Result<Vec<u8>, AssetError> {
        let name = stream
            .path
            .strip_prefix("archive:/")
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or(&stream.path);
        let node = self.nodes.iter().find(|n| n.path == name).ok_or_else(|| {
            AssetError::format(
                self.path(),
                format!("no node {:?} in the bundle", stream.path),
            )
        })?;
        self.read_node(node, stream.offset, stream.size)
    }
}

impl BlockStream {
    const NONE: u16 = 0;
    const LZMA: u16 = 1;
    const LZ4: u16 = 2;
    const LZ4HC: u16 = 3;

    /// `size` bytes at `start` of the uncompressed stream, decompressing only the
    /// blocks they span.
    fn read(&self, start: u64, size: u64) -> Result<Vec<u8>, AssetError> {
        let end = start + size;
        let len = usize::try_from(size).map_err(|_| AssetError::format(&self.path, "too large"))?;
        let mut out = Vec::with_capacity(len);
        let first = self
            .blocks
            .partition_point(|b| b.offset + u64::from(b.size) <= start);
        let mut file = File::open(&self.path).map_err(|e| AssetError::io(&self.path, e))?;
        for block in self.blocks[first..].iter().take_while(|b| b.offset < end) {
            let mut raw = vec![0; block.compressed_size as usize];
            file.seek(SeekFrom::Start(block.file_offset))
                .and_then(|_| file.read_exact(&mut raw))
                .map_err(|e| AssetError::io(&self.path, e))?;
            let data = Self::decompress(&raw, block.compression, block.size as usize)
                .map_err(|m| AssetError::format(&self.path, m))?;
            let from = start.saturating_sub(block.offset) as usize;
            let to = (end - block.offset).min(u64::from(block.size)) as usize;
            out.extend_from_slice(&data[from..to]);
        }
        if out.len() != len {
            return Err(AssetError::format(
                &self.path,
                format!("stream range {start}+{size} runs past the blocks"),
            ));
        }
        Ok(out)
    }

    fn decompress(data: &[u8], compression: u16, size: usize) -> Result<Vec<u8>, String> {
        match compression {
            Self::NONE => Ok(data.to_vec()),
            Self::LZ4 | Self::LZ4HC => lz4_flex::block::decompress(data, size)
                .map_err(|e| format!("LZ4 block: {e}"))
                .and_then(|d| {
                    if d.len() == size {
                        Ok(d)
                    } else {
                        Err(format!("LZ4 block gave {} bytes, not {size}", d.len()))
                    }
                }),
            Self::LZMA => Err("LZMA bundle blocks are not supported".into()),
            other => Err(format!("unknown block compression {other}")),
        }
    }
}
