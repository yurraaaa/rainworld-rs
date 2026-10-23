//! Headless GPU checks of the background scene shaders (`background_scene.wgsl`) on a
//! 64x64 frame, with values worked out from `Background.shader`,
//! `DistantBkgObject.shader`, `CloudDistant.shader` and `_TerrainMask.cginc`
//! (`BackgroundClipVanilla`). Skipped without a Vulkan adapter.

use rw_assets::{NoiseTexture, PaletteImage, PaletteTexture};
use rw_graphics::{Color as SpriteColor, DrawItem, Shader};

use super::tests::{frame_quad, pixel, renderer};
use super::*;
use crate::{LevelParams, SceneGlobals};

/// The level: columns 0..21 a layer-10 pixel, 21..42 sky, 42..64 a pixel with a 255
/// channel (an effect colour; `BackgroundClipVanilla` does not cut it).
fn level_image() -> PngImage {
    PngImage::from_pixels(
        64,
        64,
        (0..64 * 64)
            .map(|i| match i % 64 {
                0..21 => [11, 0, 0, 255],
                21..42 => [255, 255, 255, 255],
                _ => [255, 10, 10, 255],
            })
            .collect(),
    )
}

/// Draws the level, then `behind` before it in `Items` and `over` after it in `Water`.
fn frame(r: &mut WgpuRenderer, behind: Vec<DrawItem>, over: Vec<DrawItem>) -> FrameImage {
    let palette = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, vec![[0, 0, 90, 255]; 512]))
            .unwrap(),
        None,
        0.0,
    );
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &level_image(),
        palette: &palette,
        noise: &noise,
        params: LevelParams::DEFAULT,
        rain_mask: None,
    });
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], (64, 64)));
    r.set_background(Color::rgb(0.0, 0.0, 0.0));
    r.draw_sprites(&DrawList { items: behind }, Layer::Items, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    r.draw_sprites(&DrawList { items: over }, Layer::Water, [0.0, 0.0]);
    r.render_image().unwrap()
}

fn full(shader: Shader, color: SpriteColor) -> DrawItem {
    frame_quad([0.0, 0.0, 64.0, 64.0], shader, color, Layer::Water)
}

/// `Background`: the colour where the level has a 255 channel (sky or an effect colour),
/// nothing over a layer-10 pixel.
#[test]
fn background_shows_only_where_the_level_has_a_full_channel() {
    let Some(mut r) = renderer() else { return };
    let without = frame(&mut r, Vec::new(), Vec::new());
    let image = frame(
        &mut r,
        Vec::new(),
        vec![full(Shader::Background, SpriteColor::rgb(1.0, 0.0, 0.0))],
    );
    assert_eq!(pixel(&image, 10, 32), pixel(&without, 10, 32), "layer 10");
    assert_eq!(pixel(&image, 30, 32), [255, 0, 0], "sky");
    assert_eq!(pixel(&image, 50, 32), [255, 0, 0], "effect colour");
}

/// Nothing drawn behind the level (the `_GrabTexture`) is covered: a pixel there is cut
/// even over sky.
#[test]
fn background_is_cut_where_something_was_drawn_behind_the_level() {
    let Some(mut r) = renderer() else { return };
    let behind = vec![frame_quad(
        [24.0, 0.0, 28.0, 64.0],
        Shader::Basic,
        SpriteColor::rgb(0.0, 0.0, 1.0),
        Layer::Items,
    )];
    let image = frame(
        &mut r,
        behind,
        vec![full(Shader::Background, SpriteColor::rgb(1.0, 0.0, 0.0))],
    );
    assert_eq!(
        pixel(&image, 26, 32),
        [0, 0, 255],
        "behind: the creature shows"
    );
    assert_eq!(pixel(&image, 35, 32), [255, 0, 0], "sky beside it");
}

/// `DistantBkgObject`: the texel's colour toward `_AboveCloudsAtmosphereColor` by the
/// vertex colour's red, times `_MultiplyColor`.
#[test]
fn distant_bkg_object_fades_toward_the_atmosphere_colour() {
    let Some(mut r) = renderer() else { return };
    r.set_scene_globals(SceneGlobals {
        atmosphere: [40.0 / 255.0, 60.0 / 255.0, 80.0 / 255.0],
        multiply: [1.0, 0.5, 1.0],
        world_cam_pos: [0.0; 2],
    });
    let image = frame(
        &mut r,
        Vec::new(),
        vec![full(
            Shader::DistantBkgObject,
            SpriteColor::rgb(1.0, 0.0, 0.0),
        )],
    );
    // The white texel is entirely the atmosphere colour; green is halved.
    assert_eq!(pixel(&image, 30, 32), [40, 30, 80]);
    let half = frame(
        &mut r,
        Vec::new(),
        vec![full(
            Shader::DistantBkgObject,
            SpriteColor::rgb(0.5, 0.0, 0.0),
        )],
    );
    // lerp(white, atmosphere, 128 / 255) (the colour's 8-bit red), green times 0.5.
    let t = 128.0 / 255.0;
    let want = [
        toward(t, 40.0 / 255.0).round() as u8,
        (toward(t, 60.0 / 255.0) * 0.5).round() as u8,
        toward(t, 80.0 / 255.0).round() as u8,
    ];
    let got = pixel(&half, 30, 32);
    for k in 0..3 {
        assert!(got[k].abs_diff(want[k]) <= 1, "{got:?} {want:?}");
    }
}

/// White (255) moved toward `a` (0 to 1) by `t`.
fn toward(t: f32, a: f32) -> f32 {
    ((1.0 - t) + a * t) * 255.0
}

/// The cloud shaders paint over sky and leave a layer-10 pixel alone.
#[test]
fn cloud_shaders_paint_over_sky_only() {
    let Some(mut r) = renderer() else { return };
    r.set_scene_globals(SceneGlobals::default());
    let without = frame(&mut r, Vec::new(), Vec::new());
    for shader in [Shader::Cloud, Shader::CloudDistant, Shader::Dust] {
        let colour = SpriteColor {
            r: 0.5,
            g: 0.2,
            b: 1.0,
            a: 1.0,
        };
        let image = frame(&mut r, Vec::new(), vec![full(shader, colour)]);
        assert_eq!(
            pixel(&image, 10, 32),
            pixel(&without, 10, 32),
            "{shader:?} layer 10"
        );
        assert_ne!(
            pixel(&image, 30, 32),
            pixel(&without, 30, 32),
            "{shader:?} sky"
        );
    }
}
