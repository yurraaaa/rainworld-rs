//! A rendered frame read back from the GPU.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Opaque 8-bit RGBA pixels, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl FrameImage {
    /// # Panics
    /// When `rgba` is not `width * height * 4` bytes.
    #[must_use]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        Self {
            width,
            height,
            rgba,
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Writes the frame as a binary PPM (P6).
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn save_ppm(&self, path: &Path) -> io::Result<()> {
        let mut out = BufWriter::new(File::create(path)?);
        write!(out, "P6\n{} {}\n255\n", self.width, self.height)?;
        for px in self.rgba.as_chunks::<4>().0 {
            out.write_all(&px[..3])?;
        }
        out.flush()
    }

    /// The frame as an RGB PNG file.
    ///
    /// # Errors
    /// When encoding fails.
    pub fn encode_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        let rgb: Vec<u8> = self
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| [px[0], px[1], px[2]])
            .collect();
        writer.write_image_data(&rgb)?;
        writer.finish()?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trips() {
        let image = FrameImage::new(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]);
        let png = image.encode_png().expect("encodes");
        let decoder = png::Decoder::new(std::io::Cursor::new(png));
        let mut reader = decoder.read_info().expect("decodes");
        let mut buf = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut buf).expect("frame");
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(&buf[..info.buffer_size()], &[255, 0, 0, 0, 0, 255]);
    }
}
