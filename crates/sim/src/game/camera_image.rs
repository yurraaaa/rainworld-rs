//! What the sim reads of camera 0's picture: the shown camera's level image and the
//! palette texture (after `ApplyFade`), which the app hands over before each tick.
//! `PixelColorAtCoordinate` colours the placed lights; `ApplyPalette` gives the
//! effect-coloured ones their colour (D24). No draws.
//!
//! Mirrors: RoomCamera.cs (PixelColorAtCoordinate, DepthAtCoordinate)

use std::sync::Arc;

use rw_assets::{PaletteTexture, PngImage};

use crate::Vec2;

/// Camera 0's level image and palette.
#[derive(Debug, Clone)]
pub struct CameraImage {
    /// The global index of the room the camera shows.
    pub room: i32,
    /// `CamPos(currentCameraPosition)`: the image's bottom-left in room pixels.
    pub cam_pos: Vec2,
    /// `levelTexture`.
    pub image: Arc<PngImage>,
    /// `paletteTexture`.
    pub palette: PaletteTexture,
}

impl CameraImage {
    /// `levelTexture.GetPixel(x, y)` (Unity rows, bottom up; wrap mode `Clamp`).
    fn level_texel(&self, x: i32, y: i32) -> [f32; 4] {
        let w = i32::try_from(self.image.width()).unwrap_or(i32::MAX);
        let h = i32::try_from(self.image.height()).unwrap_or(i32::MAX);
        let x = usize::try_from(x.clamp(0, w - 1)).unwrap_or(0);
        let y = usize::try_from(y.clamp(0, h - 1)).unwrap_or(0);
        self.image.unity_pixel(x, y).map(|c| f32::from(c) / 255.0)
    }

    /// `paletteTexture.GetPixel(x, y)` (32x8, wrap mode `Clamp`).
    #[must_use]
    pub fn palette_texel(&self, x: i32, y: i32) -> [f32; 4] {
        let x = usize::try_from(x.clamp(0, 31)).unwrap_or(0);
        let y = usize::try_from(y.clamp(0, 7)).unwrap_or(0);
        self.palette.texel(x, y).map(|c| f32::from(c) / 255.0)
    }

    /// `RoomCamera.PixelColorAtCoordinate(coord)`: sky white is the palette's (0, 7);
    /// else the level pixel's red picks a layer row and depth column (above 90 the
    /// unlit half, `t` 0), fogged toward (1, 7) with depth.
    #[must_use]
    pub fn pixel_color_at_coordinate(&self, coord: Vec2) -> [f32; 4] {
        let v = coord - self.cam_pos;
        let pixel = self.level_texel(v.x.floor() as i32, v.y.floor() as i32);
        if pixel[0] == 1.0 && pixel[1] == 1.0 && pixel[2] == 1.0 {
            return self.palette_texel(0, 7);
        }
        let mut n = (pixel[0] * 255.0).floor() as i32;
        let mut t = 0.0;
        if n > 90 {
            n -= 90;
        } else {
            t = 1.0;
        }
        let row = n.div_euclid(30);
        // C#'s `%` keeps the dividend's sign: `(0 - 1) % 30` is -1.
        let depth = (n - 1) % 30;
        let lit = Self::lerp(
            self.palette_texel(depth, row + 3),
            self.palette_texel(depth, row),
            t,
        );
        let fog = depth as f32 * (1.0 - self.palette_texel(9, 7)[0]) / 30.0;
        Self::lerp(lit, self.palette_texel(1, 7), fog)
    }

    /// `RoomCamera.DepthAtCoordinate(coord)`: 1 for sky white, else the pixel's layer
    /// (red, above 90 the unlit half) over 30.
    #[must_use]
    pub fn depth_at_coordinate(&self, coord: Vec2) -> f32 {
        let v = coord - self.cam_pos;
        let pixel = self.level_texel(v.x.floor() as i32, v.y.floor() as i32);
        if pixel[0] == 1.0 && pixel[1] == 1.0 && pixel[2] == 1.0 {
            return 1.0;
        }
        let mut n = (pixel[0] * 255.0).floor() as i32;
        if n > 90 {
            n -= 90;
        }
        ((n - 1) % 30) as f32 / 30.0
    }

    /// `ApplyPalette` of an effect-coloured light: `palette.texture.GetPixel(30, 5 -
    /// effectColor * 2)`.
    #[must_use]
    pub fn effect_color(&self, effect_color: i32) -> [f32; 4] {
        self.palette_texel(30, 5 - effect_color * 2)
    }

    /// `Color.Lerp` (t clamped).
    fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
        let t = t.clamp(0.0, 1.0);
        [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
    }
}

#[cfg(test)]
#[path = "tests/camera_image.rs"]
mod tests;
