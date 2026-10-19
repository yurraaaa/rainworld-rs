//! The level shader's `_NoiseTex`: Unity resource `Palettes/noise`, a 64x64 DXT1
//! texture with bilinear filter and repeat wrap inside `resources.assets`
//! (`rainworld-re/notes/room-rendering.md` section 5). It is a DXT1-compressed copy of
//! `StreamingAssets/palettes/noise.png`, so it differs from that file by up to 20/255.
//!
//! Only the red channel of mip 0 is kept: the shader reads `.x` and always magnifies.
//!
//! Mirrors: -

use crate::{AssetError, PngImage, UnityResources};

/// Red channel of mip 0, row 0 = Unity bottom row (texture `v` = 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseTexture {
    red: Vec<u8>,
}

impl NoiseTexture {
    pub const SIZE: usize = 64;

    /// The Unity resource `Palettes/noise`, decoded from its DXT1 blocks.
    ///
    /// # Errors
    /// When it cannot be read or decoded, or is not 64x64.
    pub fn load(resources: &UnityResources) -> Result<Self, AssetError> {
        let image = resources.texture_image("Palettes/noise")?;
        Self::from_image(&image).map_err(|m| AssetError::format(resources.file().path(), m))
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
