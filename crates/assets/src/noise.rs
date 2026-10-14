//! The level shader's `_NoiseTex`: Unity resource `Palettes/noise`, a 64x64 DXT1
//! texture with bilinear filter and repeat wrap inside `resources.assets`
//! (`rainworld-re/notes/room-rendering.md` section 5), read from AssetRipper's export
//! of it in the [`AssetCache`]. It is a DXT1-compressed copy of
//! `StreamingAssets/palettes/noise.png`, so it differs from that file by up to 20/255.
//!
//! Only the red channel of mip 0 is kept: the shader reads `.x` and always magnifies.

use crate::{AssetCache, AssetError, PngImage};

/// Red channel of mip 0, row 0 = Unity bottom row (texture `v` = 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseTexture {
    red: Vec<u8>,
}

impl NoiseTexture {
    pub const SIZE: usize = 64;

    /// Reads `palettes/noise.png` from the asset cache.
    ///
    /// # Errors
    /// [`AssetError::Missing`] when the cache has no noise texture (with a hint on how
    /// to extract it), else when it cannot be decoded or is not 64x64.
    pub fn load(cache: &AssetCache) -> Result<Self, AssetError> {
        let path = cache.noise_png();
        if !path.is_file() {
            return Err(AssetError::Missing {
                what: format!("noise texture {}; {}", path.display(), cache.fill_hint()),
            });
        }
        let image = PngImage::load(&path)?;
        Self::from_image(&image).map_err(|m| AssetError::format(&path, m))
    }

    /// The red channel of a 64x64 image (PNG rows top first).
    ///
    /// # Errors
    /// When the image is not 64x64.
    pub fn from_image(image: &PngImage) -> Result<Self, String> {
        if (image.width(), image.height()) != (Self::SIZE, Self::SIZE) {
            return Err(format!(
                "noise texture is {}x{}, not 64x64",
                image.width(),
                image.height()
            ));
        }
        let red = (0..Self::SIZE)
            .flat_map(|y| (0..Self::SIZE).map(move |x| (x, y)))
            .map(|(x, y)| image.unity_pixel(x, y)[0])
            .collect();
        Ok(Self { red })
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
}
