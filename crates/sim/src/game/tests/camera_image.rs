
use std::sync::Arc;

use rw_assets::{PaletteImage, PaletteTexture, PngImage};

use super::CameraImage;
use crate::Vec2;

/// A 4x2 level image and a palette whose runtime texel (x, y) is (8x, 30y, 0) over
/// 255, with (9, 7) red 255 (no fog) unless `fog`.
impl CameraImage {
    fn depth_at(&self, x: f32, y: f32) -> f32 {
        self.depth_at_coordinate(Vec2::new(100.0 + x, 200.0 + y))
    }
}

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

/// `DepthAtCoordinate`: sky white is 1; red 5 is (5 - 1) / 30; red 95 loses its 90 and
/// reads the same; red 34 is 33 % 30 = 3 over 30; red 0 is C#'s `-1 % 30` = -1 over 30.
#[test]
fn depth_at_coordinate_reads_the_layer() {
    let c = image(false);
    assert_eq!(c.depth_at(0.5, 0.5), 1.0);
    assert_eq!(c.depth_at(1.5, 0.0), 4.0 / 30.0);
    assert_eq!(c.depth_at(2.9, 0.9), 4.0 / 30.0);
    assert_eq!(c.depth_at(0.0, 1.0), 3.0 / 30.0);
    assert_eq!(c.depth_at(3.5, 0.5), -1.0 / 30.0);
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
