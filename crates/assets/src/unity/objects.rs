//! The object classes rainworld-rs reads, in their Unity 2020.3 layouts
//! (`rainworld-re/notes/unity-assets.md` section 4).
//!
//! Mirrors: -

use super::bcn::{Bc1Block, Bc3Block};
use super::reader::Reader;
use super::serialized::UnityObject;
use crate::PngImage;

/// Bytes stored outside the serialized file (`StreamingInfo` / `StreamedResource`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamedResource {
    /// Relative to the serialized file's directory, or `archive:/<cab>/<node>` in a bundle.
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

/// Unity's `TextureFormat`; only the formats the game ships are decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgb24,
    Rgba32,
    Dxt1,
    Dxt5,
    Other(i32),
}

impl TextureFormat {
    #[must_use]
    pub fn from_id(id: i32) -> Self {
        match id {
            3 => Self::Rgb24,
            4 => Self::Rgba32,
            10 => Self::Dxt1,
            12 => Self::Dxt5,
            other => Self::Other(other),
        }
    }

    /// Bytes of one `width` x `height` image, `None` for an unsupported format.
    #[must_use]
    pub fn image_size(self, width: usize, height: usize) -> Option<usize> {
        let blocks = width.div_ceil(4) * height.div_ceil(4);
        match self {
            Self::Rgb24 => Some(width * height * 3),
            Self::Rgba32 => Some(width * height * 4),
            Self::Dxt1 => Some(blocks * 8),
            Self::Dxt5 => Some(blocks * 16),
            Self::Other(_) => None,
        }
    }
}

/// Where a texture's pixels are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
    Inline(Vec<u8>),
    Streamed(StreamedResource),
}

/// `Texture2D` (class 28).
#[derive(Debug, Clone, PartialEq)]
pub struct Texture2D {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub format: TextureFormat,
    pub mip_count: u32,
    /// 0 point, 1 bilinear, 2 trilinear.
    pub filter_mode: i32,
    /// U, V, W; 0 repeat, 1 clamp.
    pub wrap: [i32; 3],
    pub image: ImageData,
}

impl Texture2D {
    /// Decodes mip 0 of `data` (all of the texture's image bytes, mip 0 first) to RGBA8.
    ///
    /// # Errors
    /// For an unsupported format or too little data.
    pub fn decode(&self, data: &[u8]) -> Result<PngImage, String> {
        let (w, h) = (self.width, self.height);
        let size = self.format.image_size(w, h).ok_or_else(|| {
            format!(
                "texture {}: unsupported format {:?}",
                self.name, self.format
            )
        })?;
        let data = data.get(..size).ok_or_else(|| {
            format!(
                "texture {}: {} bytes, mip 0 needs {size}",
                self.name,
                data.len()
            )
        })?;
        // Unity row order: row 0 is the bottom.
        let mut pixels = vec![[0u8; 4]; w * h];
        match self.format {
            TextureFormat::Rgb24 => {
                for (p, &[r, g, b]) in pixels.iter_mut().zip(data.as_chunks::<3>().0) {
                    *p = [r, g, b, 255];
                }
            }
            TextureFormat::Rgba32 => {
                for (p, c) in pixels.iter_mut().zip(data.as_chunks::<4>().0) {
                    *p = *c;
                }
            }
            TextureFormat::Dxt1 => {
                for (i, block) in data.as_chunks::<8>().0.iter().enumerate() {
                    self.put_block(&mut pixels, i, &Bc1Block(*block).decode());
                }
            }
            TextureFormat::Dxt5 => {
                for (i, block) in data.as_chunks::<16>().0.iter().enumerate() {
                    self.put_block(&mut pixels, i, &Bc3Block(*block).decode());
                }
            }
            TextureFormat::Other(_) => unreachable!("rejected by image_size"),
        }
        let top_first = pixels.chunks_exact(w).rev().flatten().copied().collect();
        Ok(PngImage::from_pixels(w, h, top_first))
    }

    /// Decodes every mip level of `data` (mip 0 first, each level half the size, to one
    /// texel) to RGBA8; the levels a texture holds only (`mip_count`).
    ///
    /// # Errors
    /// For an unsupported format or too little data.
    pub fn decode_mips(&self, data: &[u8]) -> Result<Vec<PngImage>, String> {
        let mut levels = Vec::new();
        let mut offset = 0;
        for level in 0..self.mip_count.max(1) {
            let (w, h) = ((self.width >> level).max(1), (self.height >> level).max(1));
            let size = self.format.image_size(w, h).ok_or_else(|| {
                format!(
                    "texture {}: unsupported format {:?}",
                    self.name, self.format
                )
            })?;
            let part = Self {
                width: w,
                height: h,
                mip_count: 1,
                image: ImageData::Inline(Vec::new()),
                ..self.clone()
            };
            let bytes = data.get(offset..).unwrap_or(&[]);
            levels.push(part.decode(bytes)?);
            offset += size;
        }
        Ok(levels)
    }

    /// Writes the 4x4 texels of block `index` (blocks in row order), clipped to the image.
    fn put_block(&self, pixels: &mut [[u8; 4]], index: usize, texels: &[[u8; 4]; 16]) {
        let per_row = self.width.div_ceil(4);
        let (bx, by) = (index % per_row * 4, index / per_row * 4);
        for (k, texel) in texels.iter().enumerate() {
            let (x, y) = (bx + k % 4, by + k / 4);
            if x < self.width && y < self.height {
                pixels[y * self.width + x] = *texel;
            }
        }
    }
}

impl UnityObject for Texture2D {
    const CLASS_ID: i32 = 28;

    fn parse(data: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(data);
        let name = r.string()?;
        let _forced_fallback_format = r.i32()?;
        r.skip(4)?; // downscale fallback, alpha channel optional, pad
        let width = r.count()?;
        let height = r.count()?;
        let _complete_image_size = r.u32()?;
        let _mips_stripped = r.i32()?;
        let format = TextureFormat::from_id(r.i32()?);
        let mip_count = r.u32()?;
        r.skip(4)?; // readable, pre-processed, ignore master texture limit, streaming mipmaps
        let _streaming_priority = r.i32()?;
        let _image_count = r.i32()?;
        let _dimension = r.i32()?;
        let filter_mode = r.i32()?;
        let _aniso = r.i32()?;
        let _mip_bias = r.f32()?;
        let wrap = [r.i32()?, r.i32()?, r.i32()?];
        let _lightmap_format = r.i32()?;
        let _color_space = r.i32()?;
        let _platform_blob = r.byte_array()?;
        let inline = r.byte_array()?.to_vec();
        let stream = StreamedResource {
            offset: r.u64()?,
            size: u64::from(r.u32()?),
            path: r.string()?,
        };
        if !r.is_at_end() {
            return Err(format!("texture {name}: data left after m_StreamData"));
        }
        let image = if inline.is_empty() && stream.size > 0 {
            ImageData::Streamed(stream)
        } else {
            ImageData::Inline(inline)
        };
        Ok(Self {
            name,
            width,
            height,
            format,
            mip_count,
            filter_mode,
            wrap,
            image,
        })
    }
}

/// `TextAsset` (class 49): `script` is the asset's bytes (text or binary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAsset {
    pub name: String,
    pub script: Vec<u8>,
}

impl UnityObject for TextAsset {
    const CLASS_ID: i32 = 49;

    fn parse(data: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(data);
        let name = r.string()?;
        let script = r.byte_array()?.to_vec();
        if !r.is_at_end() {
            return Err(format!("text asset {name}: data left after m_Script"));
        }
        Ok(Self { name, script })
    }
}

/// `AudioClip` (class 83). The sound itself is an FMOD bank (`FSB5`) in `resource`.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClip {
    pub name: String,
    /// 0 decompress on load, 1 compressed in memory, 2 streaming.
    pub load_type: i32,
    pub channels: u32,
    /// Samples per second.
    pub frequency: u32,
    pub bits_per_sample: u32,
    /// Seconds.
    pub length: f32,
    /// 0 PCM, 1 Vorbis, 2 ADPCM, 3 MP3.
    pub compression_format: i32,
    pub resource: StreamedResource,
}

impl UnityObject for AudioClip {
    const CLASS_ID: i32 = 83;

    fn parse(data: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(data);
        let name = r.string()?;
        let load_type = r.i32()?;
        let channels = r.u32()?;
        let frequency = r.u32()?;
        let bits_per_sample = r.u32()?;
        let length = r.f32()?;
        r.skip(2)?; // tracker format, ambisonic
        r.align();
        let _subsound_index = r.i32()?;
        r.skip(3)?; // preload, load in background, legacy 3D
        r.align();
        let path = r.string()?;
        let resource = StreamedResource {
            path,
            offset: r.u64()?,
            size: r.u64()?,
        };
        let compression_format = r.i32()?;
        if !r.is_at_end() {
            return Err(format!(
                "audio clip {name}: data left after m_CompressionFormat"
            ));
        }
        Ok(Self {
            name,
            load_type,
            channels,
            frequency,
            bits_per_sample,
            length,
            compression_format,
            resource,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_rgb24_bottom_row_first() {
        let texture = Texture2D {
            name: "t".into(),
            width: 2,
            height: 2,
            format: TextureFormat::Rgb24,
            mip_count: 1,
            filter_mode: 0,
            wrap: [0; 3],
            image: ImageData::Inline(Vec::new()),
        };
        let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let image = texture.decode(&data).unwrap();
        assert_eq!(image.unity_pixel(0, 0), [1, 2, 3, 255]);
        assert_eq!(image.unity_pixel(1, 0), [4, 5, 6, 255]);
        assert_eq!(image.pixel(0, 0), [7, 8, 9, 255]);
        assert!(texture.decode(&data[..11]).is_err());
    }

    #[test]
    fn dxt_blocks_fill_rows_from_the_bottom() {
        let texture = Texture2D {
            name: "t".into(),
            width: 8,
            height: 4,
            format: TextureFormat::Dxt1,
            mip_count: 1,
            filter_mode: 0,
            wrap: [0; 3],
            image: ImageData::Inline(Vec::new()),
        };
        // Block 0 all colour 0 = red, block 1 all colour 0 = blue.
        let data = [0x00, 0xF8, 0, 0, 0, 0, 0, 0, 0x1F, 0x00, 0, 0, 0, 0, 0, 0];
        let image = texture.decode(&data).unwrap();
        assert_eq!(image.unity_pixel(3, 3), [255, 0, 0, 255]);
        assert_eq!(image.unity_pixel(4, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn decodes_every_mip_level_after_the_first() {
        let texture = Texture2D {
            name: "t".into(),
            width: 4,
            height: 2,
            format: TextureFormat::Rgba32,
            mip_count: 3,
            filter_mode: 1,
            wrap: [0; 3],
            image: ImageData::Inline(Vec::new()),
        };
        // 4x2 (32 bytes), 2x1 (8), 1x1 (4), each pixel's four bytes the level's number.
        let mut data = vec![10; 32];
        data.extend([11; 8]);
        data.extend([12; 4]);
        let levels = texture.decode_mips(&data).unwrap();
        let sizes: Vec<(usize, usize)> = levels.iter().map(|l| (l.width(), l.height())).collect();
        assert_eq!(sizes, [(4, 2), (2, 1), (1, 1)]);
        assert_eq!(levels[1].unity_pixel(1, 0), [11; 4]);
        assert_eq!(levels[2].unity_pixel(0, 0), [12; 4]);
        assert!(texture.decode_mips(&data[..43]).is_err());
    }
}
