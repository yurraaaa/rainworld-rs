//! The level shader's `_NoiseTex`: Unity resource `Palettes/noise`, a 64x64 DXT1
//! texture with bilinear filter and repeat wrap inside `resources.assets`
//! (`rainworld-re/notes/room-rendering.md` section 5). It is a DXT1-compressed copy of
//! `StreamingAssets/palettes/noise.png`, so it differs from that file by up to 20/255.
//!
//! Only the red channel is kept (the shaders read `.x`), with the whole mip chain (7 levels,
//! 64 down to 1): the level shader always magnifies and reads mip 0, but `DeepProcessing`'s
//! taps at quantised coordinates minify in places, and Unity picks the level per pixel
//! quad from the coordinate's derivatives.
//!
//! Mirrors: -

use crate::{AssetError, PngImage, UnityResources};

/// Red channel of mip 0, row 0 = Unity bottom row (texture `v` = 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseTexture {
    /// The red channel of every mip level, level 0 first (64x64, 32x32, ... 1x1), each
    /// with its bottom row first.
    levels: Vec<Vec<u8>>,
}

impl NoiseTexture {
    pub const SIZE: usize = 64;

    /// The Unity resource `Palettes/noise`, decoded from its DXT1 blocks.
    ///
    /// # Errors
    /// When it cannot be read or decoded, or is not 64x64.
    pub fn load(resources: &UnityResources) -> Result<Self, AssetError> {
        let (_, mips) = resources.texture_mips("Palettes/noise")?;
        Self::from_mips(&mips).map_err(|m| AssetError::format(resources.file().path(), m))
    }

    /// The red channel of a 64x64 image (PNG rows top first).
    ///
    /// # Errors
    /// When the image is not 64x64.
    pub fn from_image(image: &PngImage) -> Result<Self, String> {
        let mut levels = vec![Self::red_of(image, Self::SIZE)?];
        while levels.len() < 7 {
            let (above, side) = (&levels[levels.len() - 1], Self::SIZE >> levels.len());
            let level = (0..side * side)
                .map(|i| {
                    let (x, y) = (i % side * 2, i / side * 2);
                    let (row, first) = (side * 2, y * side * 2 + x);
                    let sum = u32::from(above[first])
                        + u32::from(above[first + 1])
                        + u32::from(above[first + row])
                        + u32::from(above[first + row + 1]);
                    ((sum + 2) / 4) as u8
                })
                .collect();
            levels.push(level);
        }
        Ok(Self { levels })
    }

    /// The mip chain of the texture as Unity decoded it, 64x64 down to 1x1.
    ///
    /// # Errors
    /// When there are not 7 levels or level 0 is not 64x64.
    pub fn from_mips(mips: &[PngImage]) -> Result<Self, String> {
        if mips.len() != 7 {
            return Err(format!(
                "noise texture has {} mip levels, not 7",
                mips.len()
            ));
        }
        let levels = mips
            .iter()
            .enumerate()
            .map(|(k, image)| Self::red_of(image, Self::SIZE >> k))
            .collect::<Result<_, _>>()?;
        Ok(Self { levels })
    }

    fn red_of(image: &PngImage, side: usize) -> Result<Vec<u8>, String> {
        if (image.width(), image.height()) != (side, side) {
            return Err(format!(
                "noise texture level is {}x{}, not {side}x{side}",
                image.width(),
                image.height()
            ));
        }
        Ok((0..side)
            .flat_map(|y| (0..side).map(move |x| (x, y)))
            .map(|(x, y)| image.unity_pixel(x, y)[0])
            .collect())
    }

    /// Red value at texel `(x, y)`, `y` = 0 the bottom row.
    #[must_use]
    pub fn red(&self, x: usize, y: usize) -> u8 {
        self.levels[0][y * Self::SIZE + x]
    }

    /// Rows bottom first, for upload as an R8 texture.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.levels[0]
    }

    /// Every mip level's bytes, level 0 first.
    pub fn mip_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.levels.iter().map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_flipped_to_unity_order() {
        let mut pixels = vec![[0, 0, 0, 255]; 64 * 64];
        pixels[3] = [200, 1, 2, 255];
        let noise = NoiseTexture::from_image(&PngImage::from_pixels(64, 64, pixels)).unwrap();
        assert_eq!(noise.red(3, 63), 200);
        assert_eq!(noise.red(3, 0), 0);
        assert!(NoiseTexture::from_image(&PngImage::from_pixels(1, 1, vec![[0; 4]])).is_err());
    }

    /// `from_image` builds seven levels, each a 2x2 average of the one above; `from_mips`
    /// keeps the given levels.
    #[test]
    fn the_mip_chain_has_seven_levels() {
        let mut pixels = vec![[0, 0, 0, 255]; 64 * 64];
        for p in pixels.iter_mut().take(2) {
            p[0] = 100;
        }
        let noise = NoiseTexture::from_image(&PngImage::from_pixels(64, 64, pixels)).unwrap();
        let sizes: Vec<usize> = noise.mip_bytes().map(<[u8]>::len).collect();
        assert_eq!(sizes, [4096, 1024, 256, 64, 16, 4, 1]);
        // The two top-row texels fall in one 2x2 block of level 1: (100 + 100 + 0 + 0 + 2) / 4.
        assert_eq!(noise.mip_bytes().nth(1).unwrap()[31 * 32], 50);
        let levels: Vec<PngImage> = (0..7)
            .map(|k| {
                let s = 64 >> k;
                PngImage::from_pixels(s, s, vec![[k as u8, 0, 0, 255]; s * s])
            })
            .collect();
        let noise = NoiseTexture::from_mips(&levels).unwrap();
        assert_eq!(
            noise.mip_bytes().map(|l| l[0]).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5, 6]
        );
        assert!(NoiseTexture::from_mips(&levels[..6]).is_err());
    }
}
