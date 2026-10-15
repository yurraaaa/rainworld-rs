//! Headless GPU checks of the room sprite shaders; skipped without a Vulkan adapter.

use rw_assets::{NoiseTexture, PaletteImage, PaletteTexture};
use rw_graphics::{Atlas, Color as SpriteColor, DrawItem, Mesh as SpriteMesh, Shader, Sprite};

use crate::LevelParams;

use super::*;

const JSON: &str = r#"{"frames": {
  "RainWorld_White.png": {"frame": {"x": 0, "y": 0, "w": 16, "h": 16}, "rotated": false,
            "trimmed": false, "spriteSourceSize": {"x": 0, "y": 0, "w": 16, "h": 16},
            "sourceSize": {"w": 16, "h": 16}}},
  "meta": {"image": "t.png", "size": {"w": 16, "h": 16}}}"#;

const SIZE: (u32, u32) = (64, 64);

fn renderer() -> Option<WgpuRenderer> {
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
    );
    r.begin_frame(Camera::window([0.0, 0.0], [64.0, 64.0], SIZE));
    r.set_background(Color::rgb(0.0, 0.0, 0.0));
    Some(r)
}

fn pixel(image: &FrameImage, x: u32, y: u32) -> [u8; 3] {
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
            light: 1.0,
            grime: 0.0,
            fog_amount: 0.0,
            clouds_speed: 1.0,
            darkness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            hue: 0.0,
            brightness: 0.0,
            swarm_room: 0.0,
        },
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
            light: 1.0,
            grime: 0.0,
            fog_amount: palette.fog_amount(),
            clouds_speed: 1.0,
            darkness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            hue: 0.0,
            brightness: 0.0,
            swarm_room: 0.0,
        },
    });
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [64.0, 64.0],
        rain: 0.0,
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
            light: 1.0,
            grime: 0.0,
            fog_amount: palette.fog_amount(),
            clouds_speed: 1.0,
            darkness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            hue: 0.0,
            brightness: 0.0,
            swarm_room: 0.0,
        },
    });
    let window = [-0.02, -0.02];
    r.begin_frame(Camera::window(window, [64.0, 64.0], SIZE));
    r.draw_level(LevelView {
        origin: window,
        size: [64.0, 64.0],
        rain: 0.0,
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
