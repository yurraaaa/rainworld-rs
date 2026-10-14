//! The level shader's `_NoiseTex`: Unity resource `Palettes/noise`, a 64x64 DXT1
//! texture with 7 mips, bilinear filter, repeat wrap, stored in `resources.assets`
//! (`rainworld-re/notes/room-rendering.md` section 5). It is a DXT1-compressed copy of
//! `StreamingAssets/palettes/noise.png`, so it differs from that file by up to 20/255.
//!
//! Only the red channel and mip 0 are kept: the shader reads `.x` and always magnifies.

use crate::{AssetError, StreamingAssets};
use std::fs;

/// Red channel of mip 0, row 0 = Unity bottom row (texture `v` = 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseTexture {
    red: Vec<u8>,
}

/// Little-endian reader over a serialized Unity object.
struct Fields<'a> {
    bytes: &'a [u8],
    pos: usize,
}

/// One 4x4 DXT1 (BC1) block.
struct Bc1Block([u8; 8]);

impl NoiseTexture {
    pub const SIZE: usize = 64;

    /// DXT1 is format 10 in Unity's `TextureFormat`.
    const DXT1: u32 = 10;

    /// Serialized name field of the asset: length 5, "noise", padded to 4 bytes.
    const NAME: &'static [u8] = b"\x05\x00\x00\x00noise\x00\x00\x00";

    /// Reads `RainWorld_Data/resources.assets`. Never writes.
    ///
    /// # Errors
    /// When the file cannot be read or holds no texture `noise` with the expected
    /// layout (64x64 DXT1, bilinear, repeat).
    pub fn load(assets: &StreamingAssets) -> Result<Self, AssetError> {
        let path = assets.data_dir().join("resources.assets");
        let bytes = fs::read(&path).map_err(|e| AssetError::io(&path, e))?;
        Self::from_resources(&bytes).map_err(|m| AssetError::format(&path, m))
    }

    /// Finds the serialized `Texture2D` named `noise` (Unity 2020.3 field layout) and
    /// decodes its first mip. Every field is checked, so a match is unambiguous.
    ///
    /// # Errors
    /// When no or more than one candidate matches.
    pub fn from_resources(bytes: &[u8]) -> Result<Self, String> {
        let found: Vec<Self> = bytes
            .windows(Self::NAME.len())
            .enumerate()
            .filter(|(_, w)| *w == Self::NAME)
            .filter_map(|(i, _)| {
                Fields {
                    bytes,
                    pos: i + Self::NAME.len(),
                }
                .noise_texture()
            })
            .collect();
        match <[Self; 1]>::try_from(found) {
            Ok([noise]) => Ok(noise),
            Err(found) => Err(format!(
                "expected one 64x64 DXT1 texture \"noise\", found {}",
                found.len()
            )),
        }
    }

    /// Decodes mip 0 of a 64x64 DXT1 image (blocks in Unity row order).
    #[must_use]
    pub fn from_dxt1(data: &[u8]) -> Option<Self> {
        let blocks_per_row = Self::SIZE / 4;
        if data.len() < blocks_per_row * blocks_per_row * 8 {
            return None;
        }
        let mut red = vec![0; Self::SIZE * Self::SIZE];
        for (b, block) in data
            .as_chunks::<8>()
            .0
            .iter()
            .take(blocks_per_row * blocks_per_row)
            .enumerate()
        {
            let block = Bc1Block(*block);
            let (bx, by) = (b % blocks_per_row * 4, b / blocks_per_row * 4);
            for j in 0..4 {
                for i in 0..4 {
                    red[(by + j) * Self::SIZE + bx + i] = block.texel(i, j)[0];
                }
            }
        }
        Some(Self { red })
    }

    /// Red value at texel `(x, y)`, `y` = 0 the bottom row.
    #[must_use]
    pub fn red(&self, x: usize, y: usize) -> u8 {
        self.red[y * Self::SIZE + x]
    }

    /// Rows bottom first, for upload as an R8 texture.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.red
    }
}

impl Fields<'_> {
    fn u32(&mut self) -> Option<u32> {
        let v = self.bytes.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes(v.try_into().ok()?))
    }

    /// Reads the `Texture2D` fields after `m_Name` and returns the texture when they
    /// describe the noise texture as shipped.
    fn noise_texture(&mut self) -> Option<NoiseTexture> {
        let _forced_fallback = self.u32()?;
        let _downscale_alpha_flags = self.u32()?;
        let (width, height) = (self.u32()?, self.u32()?);
        let complete_size = self.u32()?;
        let _mips_stripped = self.u32()?;
        let format = self.u32()?;
        let _mip_count = self.u32()?;
        let _readable_flags = self.u32()?;
        let _streaming_priority = self.u32()?;
        let image_count = self.u32()?;
        let _dimension = self.u32()?;
        let filter = self.u32()?;
        let _aniso = self.u32()?;
        let _mip_bias = self.u32()?;
        let wrap = [self.u32()?, self.u32()?, self.u32()?];
        let _lightmap_format = self.u32()?;
        let _color_space = self.u32()?;
        let blob = self.u32()? as usize;
        self.pos += blob.next_multiple_of(4);
        let size = self.u32()?;
        let shipped = width == 64
            && height == 64
            && format == NoiseTexture::DXT1
            && image_count == 1
            && filter == 1
            && wrap == [0, 0, 0]
            && size == complete_size;
        if !shipped {
            return None;
        }
        let data = self.bytes.get(self.pos..self.pos + size as usize)?;
        NoiseTexture::from_dxt1(data)
    }
}

impl Bc1Block {
    /// RGB565 to 8 bits per channel, replicating the high bits.
    fn expand(c: u16) -> [i32; 3] {
        let (r, g, b) = (
            i32::from(c >> 11),
            i32::from((c >> 5) & 63),
            i32::from(c & 31),
        );
        [
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
        ]
    }

    /// Texel `(i, j)` of the block (`j` = row in data order) as RGBA; colours 2 and 3
    /// are the rounded 2:1 / 1:2 mixes, or the 1:1 mix and transparent black when
    /// colour 0 <= colour 1.
    fn texel(&self, i: usize, j: usize) -> [u8; 4] {
        let c0 = u16::from_le_bytes([self.0[0], self.0[1]]);
        let c1 = u16::from_le_bytes([self.0[2], self.0[3]]);
        let bits = u32::from_le_bytes([self.0[4], self.0[5], self.0[6], self.0[7]]);
        let index = (bits >> (2 * (4 * j + i))) & 3;
        let (a, b) = (Self::expand(c0), Self::expand(c1));
        let mix: [i32; 3] = match (index, c0 > c1) {
            (0, _) => a,
            (1, _) => b,
            (2, true) => std::array::from_fn(|k| (2 * a[k] + b[k] + 1) / 3),
            (3, true) => std::array::from_fn(|k| (a[k] + 2 * b[k] + 1) / 3),
            (2, false) => std::array::from_fn(|k| (a[k] + b[k] + 1) / 2),
            _ => return [0, 0, 0, 0],
        };
        [mix[0] as u8, mix[1] as u8, mix[2] as u8, 255]
    }
}

impl NoiseTexture {
    /// For tests and tools: `noise.png` from StreamingAssets in the same layout.
    ///
    /// # Errors
    /// When the file cannot be read or is not 64x64.
    pub fn load_streaming_png(assets: &StreamingAssets) -> Result<Self, AssetError> {
        let path = assets.file("palettes/noise.png");
        let image = crate::PngImage::load(&path)?;
        if (image.width(), image.height()) != (Self::SIZE, Self::SIZE) {
            return Err(AssetError::format(&path, "noise.png is not 64x64"));
        }
        let red = (0..Self::SIZE)
            .flat_map(|y| (0..Self::SIZE).map(move |x| (x, y)))
            .map(|(x, y)| image.unity_pixel(x, y)[0])
            .collect();
        Ok(Self { red })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc1_four_colour_block() {
        // c0 = pure red (0xF800), c1 = black; indices: row 0 = 0,1,2,3.
        let block = Bc1Block([0x00, 0xF8, 0x00, 0x00, 0b1110_0100, 0, 0, 0]);
        assert_eq!(block.texel(0, 0), [255, 0, 0, 255]);
        assert_eq!(block.texel(1, 0), [0, 0, 0, 255]);
        assert_eq!(block.texel(2, 0), [170, 0, 0, 255]);
        assert_eq!(block.texel(3, 0), [85, 0, 0, 255]);
        assert_eq!(block.texel(0, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn bc1_three_colour_block() {
        // c0 = black <= c1 = red: index 2 is the half mix, 3 transparent black.
        let block = Bc1Block([0x00, 0x00, 0x00, 0xF8, 0b1110_0100, 0, 0, 0]);
        assert_eq!(block.texel(2, 0), [128, 0, 0, 255]);
        assert_eq!(block.texel(3, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn finds_the_texture_by_its_fields() {
        let mut bytes = vec![0xAB; 7];
        bytes.extend_from_slice(NoiseTexture::NAME);
        let size = 64 * 64 / 2;
        for v in [
            4, 0x100, 64, 64, size, 0, 10, 1, 1, 0, 1, 2, 1, 1, 0, 0, 0, 0, 0, 1, 0, size,
        ] {
            bytes.extend_from_slice(&u32::to_le_bytes(v));
        }
        bytes.extend(std::iter::repeat_n(0, size as usize));
        let noise = NoiseTexture::from_resources(&bytes).expect("found");
        assert_eq!(noise.red(5, 7), 0);
        // A wrong filter mode is not the shipped texture.
        let filter_at = 7 + NoiseTexture::NAME.len() + 12 * 4;
        bytes[filter_at] = 0;
        assert!(NoiseTexture::from_resources(&bytes).is_err());
    }
}
