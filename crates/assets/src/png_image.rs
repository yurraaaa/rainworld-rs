//! PNG files decoded to raw 8-bit RGBA, as Unity's `Texture2D.LoadImage` gives them to
//! an ARGB32 texture: no gamma or sRGB conversion, grey copied to R, G and B, alpha 255
//! when the file has none.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use crate::AssetError;

/// An 8-bit RGBA image, rows in file order (row 0 is the top row of the PNG).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngImage {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>,
}

impl PngImage {
    /// Reads and decodes a PNG file. Never writes.
    ///
    /// # Errors
    /// [`AssetError::Io`] if the file cannot be read, [`AssetError::Png`] if it is not a
    /// decodable PNG.
    pub fn load(path: &Path) -> Result<Self, AssetError> {
        let bytes = fs::read(path).map_err(|e| AssetError::io(path, e))?;
        Self::decode(&bytes).map_err(|source| AssetError::Png {
            path: path.to_owned(),
            source,
        })
    }

    /// Decodes PNG bytes; palette, low bit depths and 16-bit channels become 8-bit.
    ///
    /// # Errors
    /// When the bytes are not a decodable PNG.
    pub fn decode(bytes: &[u8]) -> Result<Self, png::DecodingError> {
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info()?;
        let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let info = reader.next_frame(&mut buf)?;
        let (width, height) = (info.width as usize, info.height as usize);
        let channels = info.color_type.samples();
        let pixels = buf
            .chunks_exact(info.line_size)
            .take(height)
            .flat_map(|line| line[..width * channels].chunks_exact(channels))
            .map(|p| match *p {
                [v] => [v, v, v, 255],
                [v, a] => [v, v, v, a],
                [r, g, b] => [r, g, b, 255],
                [r, g, b, a] => [r, g, b, a],
                _ => unreachable!("8-bit PNG output has 1 to 4 channels"),
            })
            .collect();
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// An image from RGBA pixels in PNG row order.
    ///
    /// # Panics
    /// When `pixels` does not hold `width * height` entries.
    #[must_use]
    pub fn from_pixels(width: usize, height: usize, pixels: Vec<[u8; 4]>) -> Self {
        assert_eq!(pixels.len(), width * height, "pixel count");
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Pixel at column `x`, PNG row `row` (0 = top).
    ///
    /// # Panics
    /// When `(x, row)` is outside the image.
    #[must_use]
    pub fn pixel(&self, x: usize, row: usize) -> [u8; 4] {
        assert!(x < self.width && row < self.height, "pixel out of range");
        self.pixels[row * self.width + x]
    }

    /// Pixel in Unity texture coordinates (`y` = 0 is the bottom row), as `GetPixel`.
    #[must_use]
    pub fn unity_pixel(&self, x: usize, y: usize) -> [u8; 4] {
        self.pixel(x, self.height - 1 - y)
    }

    /// An 8-bit RGBA PNG of the image (`Texture2D.EncodeToPNG` of an RGBA32 texture;
    /// the compressed bytes need not match Unity's).
    ///
    /// # Errors
    /// When the encoder fails.
    pub fn encode(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(self.pixels.as_flattened())?;
        writer.finish()?;
        Ok(out)
    }

    /// RGBA bytes with the bottom row first: Unity's texture row order, so a texture
    /// coordinate `v` (0 = bottom) addresses the same texel in wgpu as in Unity.
    #[must_use]
    pub fn rgba_bottom_up(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for row in self.pixels.chunks_exact(self.width).rev() {
            out.extend_from_slice(row.as_flattened());
        }
        out
    }
}
