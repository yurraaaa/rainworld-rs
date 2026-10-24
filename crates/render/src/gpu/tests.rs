//! Headless GPU checks of the room sprite shaders; skipped without a Vulkan adapter.

use rw_assets::{NoiseTexture, PaletteImage, PaletteTexture};
use rw_graphics::{Atlas, Color as SpriteColor, DrawItem, Mesh as SpriteMesh, Shader, Sprite};

use crate::{LevelParams, TextureFilter};

use super::*;

mod mips;

const JSON: &str = r#"{"frames": {
  "RainWorld_White.png": {"frame": {"x": 0, "y": 0, "w": 16, "h": 16}, "rotated": false,
            "trimmed": false, "spriteSourceSize": {"x": 0, "y": 0, "w": 16, "h": 16},
            "sourceSize": {"w": 16, "h": 16}}},
  "meta": {"image": "t.png", "size": {"w": 16, "h": 16}}}"#;

const SIZE: (u32, u32) = (64, 64);

pub(super) fn renderer() -> Option<WgpuRenderer> {
    let mut r = match WgpuRenderer::headless(SIZE) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let atlas = SpriteAtlas::new(Atlas::parse(JSON).unwrap()).unwrap();
    r.set_atlas(
        &atlas,
        &[&PngImage::from_pixels(16, 16, vec![[255; 4]; 256])],
        &[],
    );
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], SIZE));
    r.set_background(Color::rgb(0.0, 0.0, 0.0));
    Some(r)
}

pub(super) fn pixel(image: &FrameImage, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * image.width() + x) * 4) as usize;
    let p = &image.rgba()[i..i + 3];
    [p[0], p[1], p[2]]
}

fn white_sprite(shader: Shader, color: SpriteColor) -> DrawItem {
    DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(32.0, 32.0),
        rotation: 0.0,
        scale: Vec2::new(2.0, 2.0),
        anchor: Vec2::new(0.5, 0.5),
        color,
        shader,
        layer: Layer::Items,
    })
}

#[test]
fn flat_light_is_a_disc_over_futile_white() {
    let Some(mut r) = renderer() else { return };
    let list = DrawList {
        items: vec![white_sprite(
            Shader::FlatLight,
            SpriteColor::rgb(1.0, 0.0, 0.0),
        )],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    let image = r.render_image().unwrap();
    // The 32x32 quad spans 16..48. Pixel (32, 32) is screen (32.5, 31.5), stage (33,
    // 31) behind `screenPixelOffset`, a pixel from the centre on each axis: d = 2 *
    // sqrt(2) / 32 = 0.0884, (1 - d^2)^3.5 = 0.9729 -> 248. Nothing in its corner
    // (centre distance clamped to 1) or outside it.
    assert_eq!(pixel(&image, 32, 32), [248, 0, 0]);
    assert_eq!(pixel(&image, 17, 17), [0, 0, 0]);
    assert_eq!(pixel(&image, 5, 5), [0, 0, 0]);
}

#[test]
fn shaders_split_and_overseer_zip_is_opaque_at_full_alpha() {
    let Some(mut r) = renderer() else { return };
    let mesh = SpriteMesh {
        index: 1,
        vertices: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(64.0, 0.0),
            Vec2::new(0.0, 64.0),
        ],
        uvs: vec![Vec2::new(0.5, 0.5); 3],
        triangles: std::borrow::Cow::Owned(vec![[0, 1, 2]]),
        color: SpriteColor::rgb(0.0, 1.0, 0.0),
        vertex_colors: Vec::new(),
        shader: Shader::OverseerZip,
        layer: Layer::Items,
    };
    let list = DrawList {
        items: vec![
            DrawItem::Mesh(mesh),
            white_sprite(Shader::Basic, SpriteColor::rgb(0.0, 0.0, 1.0)),
        ],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    let image = r.render_image().unwrap();
    // `dst <= lerp(0, 3, 1)` whatever the noise: the mesh colour, opaque; the Basic
    // quad over it in its own draw.
    assert_eq!(pixel(&image, 8, 60), [0, 255, 0]);
    assert_eq!(pixel(&image, 32, 32), [0, 0, 255]);
    assert_eq!(pixel(&image, 60, 4), [0, 0, 0]);
}

/// `ShortcutGraphics` puts a pipe tile over air in `BackgroundShortcuts` with
/// `ShortCut0`, drawn before the level graphic. `ShortCut0` reads the global
/// `_LevelTex` and shows its colour only on shortcut texels (green 8..10, blue 0); the
/// level (`LevelColor`) then shows what was drawn behind it at depth >= 5. A background
/// pipe dot in the level image (`(51, 8, 0)`: depth 20, as along SU_A15's x = 26 below
/// y = 8) shows the lit sprite; a texel without the shortcut code (`(51, 0, 0)`) hides
/// it.
#[test]
fn background_shortcut_reads_the_level_behind_it() {
    let Some(mut r) = renderer() else { return };
    let image = PngImage::from_pixels(
        64,
        64,
        (0..64 * 64)
            .map(|i| {
                if i % 64 < 32 {
                    [51, 8, 0, 255]
                } else {
                    [51, 0, 0, 255]
                }
            })
            .collect(),
    );
    let palette = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, vec![[90, 90, 90, 255]; 512]))
            .unwrap(),
        None,
        0.0,
    );
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams {
            fog_amount: 0.0,
            ..LevelParams::DEFAULT
        },
        rain_mask: None,
        room_size: None,
    });
    let DrawItem::Sprite(mut pipe) =
        white_sprite(Shader::ShortCut0, SpriteColor::rgb(1.0, 0.0, 0.0))
    else {
        unreachable!()
    };
    pipe.layer = Layer::BackgroundShortcuts;
    let list = DrawList {
        items: vec![DrawItem::Sprite(pipe)],
    };
    r.draw_sprites(&list, Layer::BackgroundShortcuts, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let frame = r.render_image().unwrap();
    // The quad spans 16..48: lit over the dot texels, hidden over the others.
    assert_eq!(pixel(&frame, 20, 32), [255, 0, 0]);
    assert_ne!(pixel(&frame, 44, 32), [255, 0, 0]);
    assert_eq!(pixel(&frame, 44, 32), pixel(&frame, 60, 32));
}

/// `ColoredSprite3` (`Futile/ColoredSprite2`, `no_grab_tex`) as the shelter door uses
/// it. Texel (102, 0, 255) at alpha 0.5: depth `lerp(0.5, 1, 0.4)` = 0.7, palette
/// column `floor(0.5 + 29 * 0.7)` = 20, row `floor(0.5 + 2 * 1)` = 2: (200, 100, 50).
/// White is not tinted; fog texel (1, 7) (0, 0, 250) at `0.7 * _fogAmount` (texel
/// (9, 7) red 153: 0.4) gives (144, 72, 106). Shown over sky and over red 116 (`- 90`,
/// depth 25 / 30), hidden behind red 11 (depth 10 / 30).
#[test]
fn colored_sprite3_colours_by_depth_from_the_palette() {
    let Some(mut r) = renderer() else { return };
    let atlas = SpriteAtlas::new(Atlas::parse(JSON).unwrap()).unwrap();
    r.set_atlas(
        &atlas,
        &[&PngImage::from_pixels(
            16,
            16,
            vec![[102, 0, 255, 255]; 256],
        )],
        &[],
    );
    let image = PngImage::from_pixels(
        64,
        64,
        (0..64 * 64)
            .map(|i| match i % 64 {
                0..24 => [255, 255, 255, 255],
                24..40 => [116, 0, 0, 255],
                _ => [11, 0, 0, 255],
            })
            .collect(),
    );
    // PNG row `7 - row` holds the runtime palette's row `row`.
    let mut pixels = vec![[90, 90, 90, 255]; 512];
    pixels[5 * 32 + 20] = [200, 100, 50, 255];
    pixels[1] = [0, 0, 250, 255];
    pixels[9] = [153, 0, 0, 255];
    let palette = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap(),
        None,
        0.0,
    );
    assert!((palette.fog_amount() - 0.4).abs() < 1e-6);
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams {
            fog_amount: palette.fog_amount(),
            ..LevelParams::DEFAULT
        },
        rain_mask: None,
        room_size: None,
    });
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let list = DrawList {
        items: vec![white_sprite(
            Shader::ColoredSprite3,
            SpriteColor::WHITE.with_alpha(0.5),
        )],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    let frame = r.render_image().unwrap();
    assert_eq!(pixel(&frame, 20, 32), [144, 72, 106]);
    assert_eq!(pixel(&frame, 32, 32), [144, 72, 106]);
    assert_eq!(pixel(&frame, 44, 32), pixel(&frame, 60, 32));
    assert_ne!(pixel(&frame, 44, 32), [144, 72, 106]);
}

/// The terrain a `ColoredSprite3` pixel is hidden behind is the level texel shown at that
/// pixel: the camera's `screenPixelOffset` (screen = stage + (-0.5, +0.5)) and
/// `_spriteRect`'s matching shift make `floor(scrPos)` read the texel the pixel shows;
/// with the window at `floor - 0.02` no pixel straddles a texel edge. Here the level is
/// sky above a wall in its bottom 16 rows and left 16 columns; at the window -0.02 the
/// image sits at screen (-0.48, 0.52), so screen pixel (x, y) (y up) shows texel (x,
/// y - 1) and the wall up to y = 16. The sprite (as in the colour test above) spans
/// stage 16.02..48.02, screen x 15.52..47.52 and y 16.52..48.52: its bottom row (y 17,
/// texel 16) and left column (x 16) lie over sky and are drawn; the pixels below and
/// left of it show the wall.
#[test]
fn colored_sprite3_tests_the_level_texel_under_the_pixel() {
    let Some(mut r) = renderer() else { return };
    let atlas = SpriteAtlas::new(Atlas::parse(JSON).unwrap()).unwrap();
    r.set_atlas(
        &atlas,
        &[&PngImage::from_pixels(
            16,
            16,
            vec![[102, 0, 255, 255]; 256],
        )],
        &[],
    );
    // PNG rows top first: the bottom 16 rows are rows 48..64.
    let image = PngImage::from_pixels(
        64,
        64,
        (0..64 * 64)
            .map(|i| {
                if i % 64 < 16 || i / 64 >= 48 {
                    [11, 0, 0, 255]
                } else {
                    [255, 255, 255, 255]
                }
            })
            .collect(),
    );
    let mut pixels = vec![[90, 90, 90, 255]; 512];
    pixels[5 * 32 + 20] = [200, 100, 50, 255];
    pixels[1] = [0, 0, 250, 255];
    pixels[9] = [153, 0, 0, 255];
    let palette = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap(),
        None,
        0.0,
    );
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams {
            fog_amount: palette.fog_amount(),
            ..LevelParams::DEFAULT
        },
        rain_mask: None,
        room_size: None,
    });
    let window = [-0.02, -0.02];
    r.begin_frame(Camera::window(window, [64.0, 64.0], SIZE));
    r.draw_level(LevelView {
        origin: window,
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let list = DrawList {
        items: vec![white_sprite(
            Shader::ColoredSprite3,
            SpriteColor::WHITE.with_alpha(0.5),
        )],
    };
    r.draw_sprites(&list, Layer::Items, window);
    let frame = r.render_image().unwrap();
    let sprite = [144, 72, 106];
    assert_eq!(pixel(&frame, 32, 32), sprite);
    assert_eq!(pixel(&frame, 32, 46), sprite, "bottom row");
    assert_eq!(pixel(&frame, 16, 32), sprite, "left column");
    assert_ne!(pixel(&frame, 32, 47), sprite);
    assert_ne!(pixel(&frame, 15, 32), sprite);
}

/// The debug overlay goes over the headless frame in target pixels (y down), textured
/// by its atlas times the vertex colour and clipped to its draw's rectangle.
#[test]
fn overlay_draws_clipped_over_the_frame() {
    let Some(mut r) = renderer() else { return };
    r.set_overlay_texture(2, 1, &[255, 255, 255, 255, 0, 0, 0, 0]);
    let overlay = Overlay {
        vertices: [[0.0, 0.0], [32.0, 0.0], [32.0, 16.0], [0.0, 16.0]]
            .map(|pos| crate::OverlayVertex {
                pos,
                uv: [0.25, 0.5],
                color: [255, 0, 0, 255],
            })
            .to_vec(),
        indices: vec![0, 1, 2, 0, 2, 3],
        draws: vec![crate::OverlayDraw {
            first: 0,
            count: 6,
            clip: [0.0, 0.0, 16.0, 64.0],
        }],
    };
    r.draw_overlay(overlay);
    let image = r.render_image().unwrap();
    assert_eq!(pixel(&image, 4, 4), [255, 0, 0]);
    assert_eq!(pixel(&image, 20, 4), [0, 0, 0]);
    assert_eq!(pixel(&image, 4, 40), [0, 0, 0]);
}

/// A 64x64 level of sunlit depth-10 pixels (`R = 101`) under full sun: lit (200, 0, 0)
/// from palette row 3, (0, 0, 200) from row 0 when shadowed; `light_dir` its angle.
fn set_sunlit_level(r: &mut WgpuRenderer, light_dir: [f32; 2]) {
    let image = PngImage::from_pixels(64, 64, vec![[101, 0, 0, 255]; 64 * 64]);
    // PNG row `7 - row` holds the runtime palette's row `row`.
    let mut pixels = vec![[90, 90, 90, 255]; 512];
    pixels[4 * 32 + 10] = [200, 0, 0, 255];
    pixels[7 * 32 + 10] = [0, 0, 200, 255];
    let palette = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap(),
        None,
        0.0,
    );
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams {
            light_dir,
            ..LevelParams::DEFAULT
        },
        rain_mask: None,
        room_size: None,
    });
}

/// A `SunBlocker`'s `(1/255, 0, 0)` in the `Shadows` container shadows every sunlit
/// pixel at depth >= 5 it lies behind, and shows through none.
#[test]
fn sun_block_takes_the_sun_without_masking() {
    let Some(mut r) = renderer() else { return };
    set_sunlit_level(&mut r, [0.0, 0.0]);
    let DrawItem::Sprite(mut block) =
        white_sprite(Shader::Basic, SpriteColor::rgb(1.0 / 255.0, 0.0, 0.0))
    else {
        unreachable!()
    };
    block.layer = Layer::Shadows;
    let list = DrawList {
        items: vec![DrawItem::Sprite(block)],
    };
    r.draw_sprites(&list, Layer::Shadows, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let frame = r.render_image().unwrap();
    // The quad spans 16..48.
    assert_eq!(
        pixel(&frame, 32, 32),
        [0, 0, 200],
        "shadowed behind the blocker"
    );
    assert_eq!(pixel(&frame, 4, 4), [200, 0, 0], "lit elsewhere");
}

/// Object shadows look `-lightDir.x * (depth - 5) / 1400` of the screen to the side:
/// with `lightDir.x = -70` a depth-10 pixel reads the grab 0.25 of the 64-pixel window
/// (16 pixels) to its right, so pixel 24 is shadowed by the quad at 16..48 and pixel 40
/// is lit.
#[test]
fn object_shadow_reads_the_grab_toward_the_light() {
    let Some(mut r) = renderer() else { return };
    set_sunlit_level(&mut r, [-70.0, 0.0]);
    let list = DrawList {
        items: vec![white_sprite(
            Shader::Basic,
            SpriteColor::rgb(1.0 / 255.0, 0.0, 0.0),
        )],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let frame = r.render_image().unwrap();
    assert_eq!(pixel(&frame, 24, 32), [0, 0, 200]);
    assert_eq!(pixel(&frame, 40, 32), [200, 0, 0]);
    assert_eq!(pixel(&frame, 56, 32), [200, 0, 0]);
}

/// `ApplyFade` ends with `ApplyPalette`, which takes `_fogAmount` from the new texture:
/// a depth-10 unlit pixel (`R = 11`) over black base texels is fog-free under a palette
/// whose texel (9, 7) has red 255 (amount 0), and a third of the way to the fog colour
/// (240, 0, 0) after the palette is replaced by one whose texel (9, 7) is black (amount
/// 1): `lerp(0, 240, 10 * 1 / 30)` = 80.
#[test]
fn palette_rewrite_refreshes_the_fog_amount() {
    let Some(mut r) = renderer() else { return };
    let image = PngImage::from_pixels(64, 64, vec![[11, 0, 0, 255]; 64 * 64]);
    let mut pixels = vec![[0, 0, 0, 255]; 512];
    pixels[1] = [240, 0, 0, 255];
    pixels[9] = [255, 0, 0, 255];
    let clear = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels.clone())).unwrap(),
        None,
        0.0,
    );
    pixels[9] = [0, 0, 0, 255];
    let foggy = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap(),
        None,
        0.0,
    );
    assert_eq!((clear.fog_amount(), foggy.fog_amount()), (0.0, 1.0));
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &image,
        palette: &clear,
        noise: &noise,
        params: LevelParams {
            fog_amount: clear.fog_amount(),
            ..LevelParams::DEFAULT
        },
        rain_mask: None,
        room_size: None,
    });
    let view = LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    };
    r.draw_level(view);
    assert_eq!(pixel(&r.render_image().unwrap(), 32, 32), [0, 0, 0]);
    r.set_level_palette(&foggy);
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], SIZE));
    r.draw_level(view);
    assert_eq!(pixel(&r.render_image().unwrap(), 32, 32), [80, 0, 0]);
}

/// A `Futile_White` quad over frame pixels `x0..x1`, `y0..y1` (y down) of the 64x64
/// frame.
pub(super) fn frame_quad(
    rect: [f32; 4],
    shader: Shader,
    color: SpriteColor,
    layer: Layer,
) -> DrawItem {
    let [x0, y0, x1, y1] = rect;
    DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(x0 + 0.5, 64.0 - y1 + 0.5),
        rotation: 0.0,
        scale: Vec2::new((x1 - x0) / 16.0, (y1 - y0) / 16.0),
        anchor: Vec2::ZERO,
        color,
        shader,
        layer,
    })
}

/// `SpecificDepth` (the ant): a quad of alpha `1 - 10.5 / 30` (layer 10) shows its
/// colour, at the texel's alpha, only over layer-10 level pixels (red 11 or 101), not
/// over sky (layer 30) or layer 20 (red 21); one of alpha `1 - 6.5 / 30` only where
/// something drawn before the level covers a layer-20 pixel (counted as layer 6).
#[test]
fn specific_depth_shows_on_its_own_layer() {
    let Some(mut r) = renderer() else { return };
    let image = PngImage::from_pixels(
        64,
        64,
        (0..64 * 64)
            .map(|i| match i % 64 {
                0..21 => [11, 0, 0, 255],
                21..42 => [255, 255, 255, 255],
                _ => [21, 0, 0, 255],
            })
            .collect(),
    );
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
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams::DEFAULT,
        rain_mask: None,
        room_size: None,
    });
    let behind = DrawList {
        items: vec![frame_quad(
            [48.0, 10.0, 62.0, 30.0],
            Shader::Basic,
            SpriteColor::rgb(0.0, 1.0, 0.0),
            Layer::Items,
        )],
    };
    r.draw_sprites(&behind, Layer::Items, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let ants = DrawList {
        items: vec![
            frame_quad(
                [0.0, 0.0, 64.0, 32.0],
                Shader::SpecificDepth,
                SpriteColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0 - 10.5 / 30.0,
                },
                Layer::Foreground,
            ),
            frame_quad(
                [0.0, 32.0, 64.0, 64.0],
                Shader::SpecificDepth,
                SpriteColor {
                    r: 1.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0 - 6.5 / 30.0,
                },
                Layer::Foreground,
            ),
        ],
    };
    r.draw_sprites(&ants, Layer::Foreground, [0.0, 0.0]);
    let frame = r.render_image().unwrap();
    assert_eq!(pixel(&frame, 10, 16), [255, 0, 0], "layer 10");
    assert_eq!(pixel(&frame, 30, 16), [0, 0, 90], "sky");
    assert_eq!(pixel(&frame, 44, 5), [0, 0, 90], "layer 20");
    assert_eq!(
        pixel(&frame, 55, 20),
        [0, 255, 0],
        "behind the level, not layer 10"
    );
    assert_eq!(pixel(&frame, 10, 48), [0, 0, 90], "layer 10, not 6");
    assert_eq!(pixel(&frame, 55, 48), [0, 0, 90], "layer 20");
    // Layer 6 over the creature's rows.
    let over = DrawList {
        items: vec![frame_quad(
            [40.0, 12.0, 64.0, 28.0],
            Shader::SpecificDepth,
            SpriteColor {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 1.0 - 6.5 / 30.0,
            },
            Layer::Foreground,
        )],
    };
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], SIZE));
    r.draw_sprites(&behind, Layer::Items, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    r.draw_sprites(&over, Layer::Foreground, [0.0, 0.0]);
    let frame = r.render_image().unwrap();
    assert_eq!(
        pixel(&frame, 55, 20),
        [255, 255, 0],
        "layer 6 over the creature"
    );
    assert_eq!(pixel(&frame, 44, 20), [0, 0, 90], "layer 20 beside it");
}

/// `CustomDepth` (the sky dandelions' and green sparks' depth-faded sprites): a quad
/// shows its colour where its depth (`1 - alpha`) is no deeper than the level layer
/// under the pixel: alpha 0 (depth 1) only over sky, alpha `1 - 15 / 30` over layer 20
/// and sky but not layer 10, alpha `1 - 5 / 30` over all three.
#[test]
fn custom_depth_shows_in_front_of_the_level_layer() {
    let Some(mut r) = renderer() else { return };
    let image = PngImage::from_pixels(
        64,
        64,
        (0..64 * 64)
            .map(|i| match i % 64 {
                0..21 => [11, 0, 0, 255],
                21..42 => [255, 255, 255, 255],
                _ => [21, 0, 0, 255],
            })
            .collect(),
    );
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
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams::DEFAULT,
        rain_mask: None,
        room_size: None,
    });
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let alphas = [0.0, 1.0 - 15.0 / 30.0, 1.0 - 5.0 / 30.0];
    let quads = DrawList {
        items: alphas
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                let top = 21.0 * i as f32;
                frame_quad(
                    [0.0, top, 64.0, top + 20.0],
                    Shader::CustomDepth,
                    SpriteColor {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a,
                    },
                    Layer::Foreground,
                )
            })
            .collect(),
    };
    r.draw_sprites(&quads, Layer::Foreground, [0.0, 0.0]);
    let frame = r.render_image().unwrap();
    let (layer10, sky, layer20) = (10, 30, 50);
    assert_eq!(
        pixel(&frame, layer10, 10),
        [0, 0, 90],
        "depth 1 over layer 10"
    );
    assert_eq!(pixel(&frame, sky, 10), [255, 0, 0], "depth 1 over sky");
    assert_eq!(
        pixel(&frame, layer20, 10),
        [0, 0, 90],
        "depth 1 over layer 20"
    );
    assert_eq!(
        pixel(&frame, layer10, 31),
        [0, 0, 90],
        "depth 0.5 over layer 10"
    );
    assert_eq!(pixel(&frame, sky, 31), [255, 0, 0], "depth 0.5 over sky");
    assert_eq!(
        pixel(&frame, layer20, 31),
        [255, 0, 0],
        "depth 0.5 over layer 20"
    );
    assert_eq!(
        pixel(&frame, layer10, 52),
        [255, 0, 0],
        "depth 1/6 over layer 10"
    );
    assert_eq!(
        pixel(&frame, layer20, 52),
        [255, 0, 0],
        "depth 1/6 over layer 20"
    );
}

/// A 64 px wide, 4096 px tall level (only its bottom 64 rows show) whose row `n` from
/// the bottom has depth `n % 30` (red `1 + n % 30`, unlit), coloured red `8 * depth`
/// from palette row 0, under a constant noise of 128, drawn with `_WetTerrain` and
/// `_waterLevel` as given; returns each frame pixel's image row modulo 30.
fn wet_rows(r: &mut WgpuRenderer, wet: f32, water_level: f32) -> Vec<[u8; 64]> {
    let image = PngImage::from_pixels(
        64,
        4096,
        (0..64 * 4096)
            .map(|i| [1 + ((4095 - i / 64) % 30) as u8, 0, 0, 255])
            .collect(),
    );
    let mut pixels = vec![[0, 0, 0, 255]; 512];
    for k in 0..30 {
        pixels[7 * 32 + k] = [8 * k as u8, 0, 0, 255];
    }
    let palette = PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap(),
        None,
        0.0,
    );
    let noise =
        NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[128, 0, 0, 255]; 4096]))
            .unwrap();
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &image,
        palette: &palette,
        noise: &noise,
        params: LevelParams {
            wet_terrain: wet,
            ..LevelParams::DEFAULT
        },
        rain_mask: None,
        room_size: None,
    });
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], SIZE));
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 4.909,
        light: 1.0,
        water_level,
    });
    let frame = r.render_image().unwrap();
    (0..64)
        .map(|y| std::array::from_fn(|x| pixel(&frame, x as u32, y)[0] / 8))
        .collect()
}

/// How many image rows (modulo 30) `wet` reads above `dry`.
fn row_shift(wet: u8, dry: u8) -> i32 {
    (i32::from(wet) - i32::from(dry)).rem_euclid(30)
}

/// `LevelColor`'s wet terrain: `displace = saturate((sin((n + u + v + _RAIN * 0.1) * 3 *
/// 3.14) - 0.95) * 20)` moves the colour lookup up by `displace * 0.001` of the image.
/// With noise 0.502 and `_RAIN` 4.909 the sine peaks at `u + v` = 0.508, mid-screen:
/// there the 4096-row image is read 3 to 4 rows higher (`4.096 * displace`), and
/// nowhere a few pixels away; the band follows `u + v`. Off with `_WetTerrain` 0, and below the water
/// line: with `_waterLevel` 0.5 only the top half of the screen is wet.
#[test]
fn wet_terrain_moves_the_lookup_up_in_bands() {
    let Some(mut r) = renderer() else { return };
    let dry = wet_rows(&mut r, 0.0, 1.0);
    let wet = wet_rows(&mut r, 1.0, 1.0);
    assert_eq!(wet_rows(&mut r, 1.0, 0.0), dry, "all below the water line");
    // The bottom row lies on the water line's edge (`screenPos.y` just past 1): dry.
    for y in 0..63 {
        let shifts: Vec<i32> = (0..64).map(|x| row_shift(wet[y][x], dry[y][x])).collect();
        let peak = shifts[28..=35].iter().copied().max().unwrap_or(0);
        assert!((3..=5).contains(&peak), "row {y}: {shifts:?}");
        assert!(
            shifts
                .iter()
                .enumerate()
                .all(|(x, &s)| s == 0 || (26..=37).contains(&x)),
            "row {y}: {shifts:?}"
        );
    }
    let half = wet_rows(&mut r, 1.0, 0.5);
    for y in 0..64 {
        let moved = (28..=35).any(|x| half[y][x] != dry[y][x]);
        assert_eq!(moved, y < 32, "row {y}: wet only above the line");
    }
}

/// A mesh over room pixels 16..48 (a quad, UVs 0 to 1) in `layer`.
fn uv_quad_mesh(shader: Shader, color: SpriteColor, layer: Layer) -> DrawItem {
    DrawItem::Mesh(SpriteMesh {
        index: 0,
        vertices: vec![
            Vec2::new(16.0, 16.0),
            Vec2::new(48.0, 16.0),
            Vec2::new(16.0, 48.0),
            Vec2::new(48.0, 48.0),
        ],
        uvs: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
        ],
        triangles: std::borrow::Cow::Owned(vec![[0, 1, 2], [1, 2, 3]]),
        color,
        vertex_colors: Vec::new(),
        shader,
        layer,
    })
}

/// A `SpotLight` (`VectorCircle`, black, `Shadows`) drawn after a `SunBlocker` puts the
/// grab back to 0 inside the ellipse inscribed in its quad: the sun is back there (lit
/// (200, 0, 0)) and the quad's corners keep the blocker's shadow (0, 0, 200).
#[test]
fn spot_light_lifts_the_sun_block_inside_its_ellipse() {
    let Some(mut r) = renderer() else { return };
    set_sunlit_level(&mut r, [0.0, 0.0]);
    let DrawItem::Sprite(mut block) =
        white_sprite(Shader::Basic, SpriteColor::rgb(1.0 / 255.0, 0.0, 0.0))
    else {
        unreachable!()
    };
    block.layer = Layer::Shadows;
    let spot = uv_quad_mesh(Shader::VectorCircle, SpriteColor::BLACK, Layer::Shadows);
    let list = DrawList {
        items: vec![DrawItem::Sprite(block), spot],
    };
    r.draw_sprites(&list, Layer::Shadows, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let frame = r.render_image().unwrap();
    assert_eq!(pixel(&frame, 32, 32), [200, 0, 0], "lit in the ellipse");
    assert_eq!(pixel(&frame, 17, 17), [0, 0, 200], "blocked at the corner");
    assert_eq!(
        pixel(&frame, 4, 4),
        [200, 0, 0],
        "no blocker outside 16..48"
    );
}

/// `LightBeam` over a depth-10 level (`dp = (int)(101/255 * 254) % 30 / 30 = 1/3`) with
/// vertex alpha 0.5 (`(dp - 0.2) * 10` clamps to 1 in the alpha's second third, times
/// `0.5 * 3 - 1`) draws `uv.y * sin(uv.x * 3.14) * 0.5` of its white over the lit level
/// pixel (200, 0, 0).
#[test]
#[allow(clippy::approx_constant, reason = "the game's shader uses 3.14")]
fn light_beam_alpha_is_uv_sine_in_its_third() {
    let Some(mut r) = renderer() else { return };
    set_sunlit_level(&mut r, [0.0, 0.0]);
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
    let list = DrawList {
        items: vec![uv_quad_mesh(
            Shader::LightBeam,
            SpriteColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.5,
            },
            Layer::ForegroundLights,
        )],
    };
    r.draw_sprites(&list, Layer::ForegroundLights, [0.0, 0.0]);
    let frame = r.render_image().unwrap();
    // Pixel (32, 32)'s centre sits at stage (32, 31): Futile's `SCREEN_PIXEL_OFFSET`
    // moves it half a pixel.
    let (ux, uy) = ((32.0 - 16.0) / 32.0, (64.0 - 32.0 - 1.0 - 16.0) / 32.0);
    let alpha = uy * (ux * 3.14f32).sin() * 0.5;
    let want_r = 255.0 * alpha + 200.0 * (1.0 - alpha);
    let want_g = 255.0 * alpha;
    let got = pixel(&frame, 32, 32);
    assert!(
        (f32::from(got[0]) - want_r).abs() <= 2.0,
        "{got:?} {want_r}"
    );
    assert!(
        (f32::from(got[1]) - want_g).abs() <= 2.0,
        "{got:?} {want_g}"
    );
    // Outside the quad the level stays.
    assert_eq!(pixel(&frame, 4, 4), [200, 0, 0]);
}

/// A 2x1 black and white page drawn 16 times wider than the page: with `sampling`, the
/// red channel of the pixel row through its centre at screen x 24, 28, 32, 36 and 40.
fn ramp_row(sampling: TextureSampling) -> Option<[u8; 5]> {
    let mut r = renderer()?;
    r.add_atlas_page(
        Atlas::single("ramp", 2, 1),
        &PngImage::from_pixels(2, 1, vec![[0, 0, 0, 255], [255, 255, 255, 255]]),
        sampling,
    );
    let list = DrawList {
        items: vec![DrawItem::Sprite(Sprite {
            index: 0,
            element: "ramp",
            pos: Vec2::new(32.0, 32.0),
            rotation: 0.0,
            scale: Vec2::new(16.0, 16.0),
            anchor: Vec2::new(0.5, 0.5),
            color: SpriteColor::rgb(1.0, 1.0, 1.0),
            shader: Shader::Basic,
            layer: Layer::Items,
        })],
    };
    r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
    let image = r.render_image().unwrap();
    Some([24, 28, 32, 36, 40].map(|x| pixel(&image, x, 32)[0]))
}

/// The quad spans screen x 16..48 for u 0..1; texel centres are at u 0.25 and 0.75
/// (x 24 and 40), so a bilinear read ramps from 0 to 255 between them (x 32 is the
/// midpoint, x 28 and 36 a quarter in; the shaders' pixel offset moves it about a pixel,
/// 16/80/143/207/255 as drawn), while a point read gives only 0 or 255.
#[test]
fn a_bilinear_page_interpolates_and_a_point_page_does_not() {
    let (Some(linear), Some(point)) = (
        ramp_row(TextureSampling {
            filter: TextureFilter::Bilinear,
            clamp: true,
        }),
        ramp_row(TextureSampling::POINT_CLAMP),
    ) else {
        return;
    };
    for (got, want) in linear.into_iter().zip([16u8, 80, 143, 207, 255]) {
        assert!(got.abs_diff(want) <= 4, "{linear:?}");
    }
    assert!(point.iter().all(|&v| v == 0 || v == 255), "{point:?}");
}
