//! Room palettes: `palettes/palette<N>.png` (32x16) with the effect colours from
//! `palettes/effectcolors.png` written in, blended into the 32x8 texture the level shader
//! samples. Mirrors `RoomCamera.LoadPalette`, `ApplyEffectColorsToPaletteTexture` and
//! `ApplyFade`; see `rainworld-re/notes/room-rendering.md` section 2.
//!
//! Coordinates here are Unity's: `y` = 0 is the bottom row of the PNG.

use crate::{AssetError, PngImage, StreamingAssets};

/// A palette file with effect colours applied (`fadeTexA` / `fadeTexB` in the game).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteImage {
    /// `WIDTH * HEIGHT` texels, row `y` (Unity, 0 = bottom) at `y * WIDTH`.
    texels: Vec<[u8; 4]>,
}

/// `palettes/effectcolors.png`: colour `k` is the 2x2 block at x = 2k; Unity rows 2-3
/// (the PNG's top rows) are the normal half, rows 0-1 the rain-dark half.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectColors(PngImage);

/// The 32x8 palette the level shader samples (`paletteTexture`), row 0 = Unity bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteTexture {
    texels: Vec<[u8; 4]>,
}

/// `RoomPalette`: named colours read from the palette texture's top row (`GetPixel(x,
/// 7)`), RGBA 0..1 (`rainworld-re/notes/water-rendering.md` section 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomPalette {
    pub sky_color: [f32; 4],
    pub fog_color: [f32; 4],
    pub black_color: [f32; 4],
    pub water_color1: [f32; 4],
    pub water_color2: [f32; 4],
    pub water_surface_color1: [f32; 4],
    pub water_surface_color2: [f32; 4],
    pub water_shine_color: [f32; 4],
    /// `1 - R` of texel (9, 7) (no blue special case, unlike the shader's `_fogAmount`).
    pub fog_amount: f32,
    /// `1 - R` of texel (30, 7).
    pub darkness: f32,
}

impl PaletteImage {
    pub const WIDTH: usize = 32;
    pub const HEIGHT: usize = 16;

    /// Loads `palette<index>.png`, or `palette-1.png` when that file is missing.
    ///
    /// # Errors
    /// When neither file can be read, or the image is not 32x16.
    pub fn load(assets: &StreamingAssets, index: i32) -> Result<Self, AssetError> {
        let path = assets.file(&format!("palettes/palette{index}.png"));
        let path = if path.is_file() {
            path
        } else {
            assets.file("palettes/palette-1.png")
        };
        let image = PngImage::load(&path)?;
        Self::from_image(&image).map_err(|m| AssetError::format(&path, m))
    }

    /// # Errors
    /// When the image is not 32x16 (the game throws).
    pub fn from_image(image: &PngImage) -> Result<Self, String> {
        if (image.width(), image.height()) != (Self::WIDTH, Self::HEIGHT) {
            return Err(format!(
                "palette is {}x{}, must be 32x16",
                image.width(),
                image.height()
            ));
        }
        let texels = (0..Self::HEIGHT)
            .flat_map(|y| (0..Self::WIDTH).map(move |x| image.unity_pixel(x, y)))
            .collect();
        Ok(Self { texels })
    }

    #[must_use]
    pub fn texel(&self, x: usize, y: usize) -> [u8; 4] {
        self.texels[y * Self::WIDTH + x]
    }

    /// Runtime row `y` (0..8): the top-half texel blended toward the dark half by `dark`.
    fn runtime_color(&self, x: usize, y: usize, dark: f32) -> [f32; 4] {
        self.texel(x, y + 8)
            .to_color()
            .lerp(self.texel(x, y).to_color(), dark)
    }

    /// Writes effect colour `a` into texels (30..32, 12..14) (normal) and (30..32, 4..6)
    /// (dark) and `b` into (30..32, 10..12) and (30..32, 2..4). -1 leaves the file's own.
    ///
    /// # Errors
    /// When a colour index lies outside `effectcolors.png` (the game throws).
    pub fn apply_effect_colors(
        &mut self,
        colors: &EffectColors,
        a: i32,
        b: i32,
    ) -> Result<(), String> {
        for (color, normal_y, dark_y) in [(a, 12, 4), (b, 10, 2)] {
            if color > -1 {
                self.copy_block(colors, color, 2, normal_y)?;
                self.copy_block(colors, color, 0, dark_y)?;
            }
        }
        Ok(())
    }

    /// `SetPixels(30, to_y, 2, 2, effectColors.GetPixels(2 * color, from_y, 2, 2))`.
    fn copy_block(
        &mut self,
        colors: &EffectColors,
        color: i32,
        from_y: usize,
        to_y: usize,
    ) -> Result<(), String> {
        let image = &colors.0;
        let x0 = usize::try_from(color)
            .ok()
            .map(|c| 2 * c)
            .filter(|x| x + 2 <= image.width() && from_y + 2 <= image.height())
            .ok_or_else(|| format!("effect colour {color} is outside effectcolors.png"))?;
        for j in 0..2 {
            for i in 0..2 {
                self.texels[(to_y + j) * Self::WIDTH + 30 + i] =
                    image.unity_pixel(x0 + i, from_y + j);
            }
        }
        Ok(())
    }
}

impl EffectColors {
    /// # Errors
    /// When `palettes/effectcolors.png` cannot be read.
    pub fn load(assets: &StreamingAssets) -> Result<Self, AssetError> {
        PngImage::load(&assets.file("palettes/effectcolors.png")).map(Self)
    }

    #[must_use]
    pub fn from_image(image: PngImage) -> Self {
        Self(image)
    }
}

/// Unity `Color.Lerp` on 8-bit channels: `a + (b - a) * t` in float, `t` clamped.
trait ColorLerp {
    fn lerp(self, to: Self, t: f32) -> Self;
}

impl ColorLerp for [f32; 4] {
    fn lerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        std::array::from_fn(|i| self[i] + (to[i] - self[i]) * t)
    }
}

/// 8-bit texel to Unity `Color` (channel / 255).
trait UnityColor {
    fn to_color(self) -> [f32; 4];
}

impl UnityColor for [u8; 4] {
    fn to_color(self) -> [f32; 4] {
        self.map(|c| f32::from(c) / 255.0)
    }
}

/// Stores a `Color` in an ARGB32 texel: clamped and rounded to the nearest step.
trait Quantise {
    fn quantise(self) -> [u8; 4];
}

impl Quantise for [f32; 4] {
    fn quantise(self) -> [u8; 4] {
        self.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

impl PaletteTexture {
    pub const WIDTH: usize = 32;
    pub const HEIGHT: usize = 8;

    /// `ApplyFade`: each texel of the top half (Unity rows 8..16) blended toward the
    /// bottom (rain-dark) half by `dark`, then toward `fade`'s palette by its amount.
    #[must_use]
    pub fn blend(a: &PaletteImage, fade: Option<(&PaletteImage, f32)>, dark: f32) -> Self {
        let mut texels = Vec::with_capacity(Self::WIDTH * Self::HEIGHT);
        for y in 0..Self::HEIGHT {
            for x in 0..Self::WIDTH {
                let c = match fade {
                    Some((b, amount)) => a
                        .runtime_color(x, y, dark)
                        .lerp(b.runtime_color(x, y, dark), amount),
                    None => a.runtime_color(x, y, dark),
                };
                texels.push(c.quantise());
            }
        }
        Self { texels }
    }

    /// Texel at column `x`, row `y` (0 = bottom; the shader's row index).
    #[must_use]
    pub fn texel(&self, x: usize, y: usize) -> [u8; 4] {
        self.texels[y * Self::WIDTH + x]
    }

    /// `RoomPalette.blackColor`: texel (2, 7), the top row's third texel (the colour of
    /// the slugcat's eyes, among others).
    #[must_use]
    pub fn black_color(&self) -> [u8; 4] {
        self.texel(2, 7)
    }

    /// `_fogAmount` (`ApplyPalette`): `1 - R` of texel (9, 7), or `1 + B` when R and G
    /// are 0 and B is not.
    #[must_use]
    pub fn fog_amount(&self) -> f32 {
        let [r, g, b, _] = self.texel(9, 7);
        if r == 0 && g == 0 && b > 0 {
            1.0 + f32::from(b) / 255.0
        } else {
            1.0 - f32::from(r) / 255.0
        }
    }

    /// The `RoomPalette` the camera builds from this texture (`RoomCamera.ApplyFade`).
    #[must_use]
    pub fn room_palette(&self) -> RoomPalette {
        RoomPalette {
            sky_color: self.top_color(0),
            fog_color: self.top_color(1),
            black_color: self.top_color(2),
            water_color1: self.top_color(4),
            water_color2: self.top_color(5),
            water_surface_color1: self.top_color(6),
            water_surface_color2: self.top_color(7),
            water_shine_color: self.top_color(8),
            fog_amount: 1.0 - self.top_color(9)[0],
            darkness: 1.0 - self.top_color(30)[0],
        }
    }

    /// `GetPixel(x, 7)` as 0..1 RGBA.
    fn top_color(&self, x: usize) -> [f32; 4] {
        self.texel(x, 7).to_color()
    }

    /// RGBA bytes, row 0 (bottom) first, for upload.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.texels.iter().flatten().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_palette_reads_the_top_row() {
        // PNG row 0 (runtime row 7): texel x is (10x, 20, 30); x9 has red 51, x30 red 102.
        let pixels = (0..PaletteImage::HEIGHT)
            .flat_map(|row| (0..PaletteImage::WIDTH).map(move |x| (x, row)))
            .map(|(x, row)| match (x, row) {
                (9, 0) => [51, 0, 0, 255],
                (30, 0) => [102, 0, 0, 255],
                (x, 0) => [(10 * x) as u8, 20, 30, 255],
                _ => [1, 2, 3, 255],
            })
            .collect();
        let image = PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap();
        let p = PaletteTexture::blend(&image, None, 0.0).room_palette();
        for (x, got) in [
            (0.0, p.sky_color),
            (1.0, p.fog_color),
            (2.0, p.black_color),
            (4.0, p.water_color1),
            (5.0, p.water_color2),
            (6.0, p.water_surface_color1),
            (7.0, p.water_surface_color2),
            (8.0, p.water_shine_color),
        ] {
            assert_eq!(
                got,
                [x * 10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0],
                "x {x}"
            );
        }
        assert!((p.fog_amount - 0.8).abs() < 1e-6);
        assert!((p.darkness - 0.6).abs() < 1e-6);
    }
}
