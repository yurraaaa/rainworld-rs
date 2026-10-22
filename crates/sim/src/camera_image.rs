//! What the sim reads of camera 0's picture: the shown camera's level image and the
//! palette texture (after `ApplyFade`), which the app hands over before each tick.
//! `PixelColorAtCoordinate` colours the placed lights; `ApplyPalette` gives the
//! effect-coloured ones their colour (D24). No draws.
//!
//! Mirrors: RoomCamera.cs (PixelColorAtCoordinate), LightSource.cs (ApplyPalette)

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
mod tests {
    use std::sync::Arc;

    use rw_assets::{PaletteImage, PaletteTexture, PngImage};

    use super::CameraImage;
    use crate::Vec2;

    /// A 4x2 level image and a palette whose runtime texel (x, y) is (8x, 30y, 0) over
    /// 255, with (9, 7) red 255 (no fog) unless `fog`.
    fn image(fog: bool) -> CameraImage {
        let white = [255, 255, 255, 255];
        // PNG rows top first: Unity row 1, then row 0.
        let mut level = vec![[34, 0, 0, 255]; 4];
        level.extend([white, [5, 0, 0, 255], [95, 0, 0, 255], [0, 0, 0, 255]]);
        let mut pal = Vec::with_capacity(32 * 16);
        for png_row in 0..16u8 {
            let y = (15 - png_row).saturating_sub(8);
            for x in 0..32u8 {
                let red = if x == 9 && y == 7 && !fog { 255 } else { x * 8 };
                pal.push([red, y * 30, 0, 255]);
            }
        }
        let pal = PaletteImage::from_image(&PngImage::from_pixels(32, 16, pal)).expect("palette");
        CameraImage {
            room: 0,
            cam_pos: Vec2::new(100.0, 200.0),
            image: Arc::new(PngImage::from_pixels(4, 2, level)),
            palette: PaletteTexture::blend(&pal, None, 0.0),
        }
    }

    fn at(c: &CameraImage, x: f32, y: f32) -> [u8; 3] {
        let p = c.pixel_color_at_coordinate(Vec2::new(100.0 + x, 200.0 + y));
        [p[0], p[1], p[2]].map(|v| (v * 255.0).round() as u8)
    }

    /// White sky is (0, 7); red 5 (lit, layer 0, depth 4) lerps fully to row 0;
    /// red 95 (unlit 5) stays on row 3; red 34 (layer 1, depth 3) reads row 1.
    #[test]
    fn level_pixels_pick_palette_texels() {
        let c = image(false);
        assert_eq!(at(&c, 0.5, 0.5), [0, 210, 0]);
        assert_eq!(at(&c, 1.5, 0.0), [32, 0, 0]);
        assert_eq!(at(&c, 2.9, 0.9), [32, 90, 0]);
        assert_eq!(at(&c, 0.0, 1.0), [24, 30, 0]);
        // Outside the image the edge texel is read (`Clamp`).
        assert_eq!(at(&c, -50.0, -50.0), [0, 210, 0]);
    }

    /// Fog lerps toward (1, 7) by depth * (1 - red of (9, 7)) / 30: here depth 4 and
    /// red 72/255.
    #[test]
    fn depth_fogs_toward_the_fog_texel() {
        let c = image(true);
        let got = c.pixel_color_at_coordinate(Vec2::new(101.5, 200.0));
        let t = 4.0 * (1.0 - 72.0 / 255.0) / 30.0;
        let want = [32.0 + (8.0 - 32.0) * t, 210.0 * t, 0.0];
        for i in 0..3 {
            assert!((got[i] * 255.0 - want[i]).abs() < 1e-3, "{got:?} {want:?}");
        }
    }
}
