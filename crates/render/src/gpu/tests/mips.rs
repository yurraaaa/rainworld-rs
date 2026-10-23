//! Headless GPU checks that `_NoiseTex2` and the `maze` page are read at the mip level the
//! pixel block's derivatives pick (D42, D37): a texture whose level 0 is one value and
//! every smaller level another gives the same frame as a single-level texture of the
//! smaller levels' value wherever the read minifies, and a different frame without the
//! smaller levels. No random draws. Skipped without a Vulkan adapter.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::NoiseTexture;
use rw_graphics::Shader;

use crate::{SpriteVertex, StageShader, TextureFilter};

use super::super::coral_circuit_tests::maze_quad;
use super::super::full_screen_tests::{H, W, level, palette, renderer};
use super::super::*;
use crate::LevelParams;

/// A `side` x `side` texture of one red value.
fn flat(side: usize, red: u8) -> PngImage {
    PngImage::from_pixels(side, side, vec![[red, red, red, 255]; side * side])
}

/// Level 0 `first`, every smaller level `rest`, down to 1 x 1.
fn chain(side: usize, first: u8, rest: u8) -> PngImage {
    let mips = (1..=side.ilog2()).map(|k| flat(side >> k, rest)).collect();
    flat(side, first).with_mips(mips)
}

const BILINEAR_REPEAT: TextureSampling = TextureSampling {
    filter: TextureFilter::Bilinear,
    clamp: false,
};

fn begin(r: &mut WgpuRenderer) {
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &level(|_, _| [255, 255, 255]),
        palette: &palette([10, 10, 10], &[]),
        noise: &NoiseTexture::from_image(&flat(64, 90)).unwrap(),
        params: LevelParams::DEFAULT,
        rain_mask: None,
    });
    r.begin_frame(Camera::window([0.0, 0.0], [W as f32, H as f32], (W, H)));
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [W as f32, H as f32],
        rain: 0.0,
        light: 1.0,
        water_level: 1.0,
    });
}

/// The frame after `draw` with `noise2` bound as `_NoiseTex2`.
fn frame_with_noise2(
    r: &mut WgpuRenderer,
    noise2: &PngImage,
    draw: &dyn Fn(&mut WgpuRenderer),
) -> FrameImage {
    r.set_stage_texture(StageTextureId(7), noise2, BILINEAR_REPEAT);
    r.set_stage_globals(StageGlobals {
        noise2: Some(StageTextureId(7)),
        sprite_rect: [0.0, 0.0, 0.1, 0.1],
        ..StageGlobals::default()
    });
    begin(r);
    draw(r);
    r.render_image().unwrap()
}

fn assert_mips_picked(draw: &dyn Fn(&mut WgpuRenderer)) {
    let Some(mut r) = renderer() else { return };
    r.add_atlas_page(
        rw_graphics::Atlas::single("maze", 4, 4),
        &flat(4, 255),
        BILINEAR_REPEAT,
    );
    let with_mips = frame_with_noise2(&mut r, &chain(256, 0, 255), draw);
    let coarse = frame_with_noise2(&mut r, &flat(256, 255), draw);
    let fine_only = frame_with_noise2(&mut r, &flat(256, 0), draw);
    assert!(with_mips == coarse, "the minified read stayed on level 0");
    assert!(fine_only != coarse, "the two values draw the same");
}

/// `FlatLightNoisy`: four taps at `(tc.x * 6, tc.y * 3)`, `tc` stepping 10 / screen a pixel
/// (a `_spriteRect` a tenth of the screen): about 10 texels a pixel, level 3 or 4.
#[test]
fn flat_light_noisy_reads_a_coarse_level_when_minified() {
    assert_mips_picked(&|r| {
        let mut batch = StageBatch::default();
        batch.push_quad(
            StageTextureId(1),
            StageShader::FlatLightNoisy,
            [
                (400.0, 200.0),
                (800.0, 200.0),
                (800.0, 500.0),
                (400.0, 500.0),
            ]
            .map(|(x, y)| SpriteVertex {
                pos: [x, y],
                uv: [(x - 400.0) / 400.0, (y - 200.0) / 300.0],
                color: [1.0, 0.0, 0.0, 0.5],
            }),
        );
        r.set_stage_texture(
            StageTextureId(1),
            &flat(1, 255),
            TextureSampling::POINT_CLAMP,
        );
        r.draw_stage(&batch, [W as f32, H as f32]);
    });
}

/// `OverseerZip`: `tc.y * 8` over the 768 pixel frame is 2.7 texels a pixel, level 1; a
/// level-1 value of 1 cuts every pixel (`dst >= 2` against `lerp(0, 3, 0.5)`).
#[test]
fn overseer_zip_reads_level_one() {
    assert_mips_picked(&|r| {
        let quad = maze_quad(
            [100, 100, 700, 500],
            [1.0, 1.0, 1.0, 0.5],
            Shader::OverseerZip,
        );
        let list = DrawList { items: vec![quad] };
        r.draw_sprites(&list, Layer::Foreground, [0.0, 0.0]);
    });
}

/// The `maze` window of the whole texture over 40 pixels is 3.2 texels a pixel, level 2: a
/// page whose smaller levels are all lit draws no dark bit, one with only level 0 (a
/// checker) draws dark texels.
#[test]
fn a_minified_maze_window_reads_a_smaller_level() {
    let checker = PngImage::from_pixels(
        128,
        128,
        (0..128 * 128)
            .map(|i| {
                let lit = (i % 128 / 8 + i / 128 / 8) % 2 == 0;
                [if lit { 255 } else { 0 }, 0, 0, 255]
            })
            .collect(),
    );
    let chained = checker
        .clone()
        .with_mips((1..8).map(|k| flat(128 >> k, 255)).collect());
    let Some(mut r) = renderer() else { return };
    let blank = {
        begin(&mut r);
        r.render_image().unwrap()
    };
    let mut frames = Vec::new();
    for page in [&chained, &checker] {
        let Some(mut r) = renderer() else { return };
        r.add_atlas_page(
            rw_graphics::Atlas::single("maze", 128, 128),
            page,
            BILINEAR_REPEAT,
        );
        begin(&mut r);
        let quad = maze_quad(
            [100, 100, 140, 140],
            [0.0, 0.0, 1.0, 1.0],
            Shader::DeadCoralCircuit,
        );
        r.draw_sprites(
            &DrawList { items: vec![quad] },
            Layer::Foreground,
            [0.0, 0.0],
        );
        frames.push(r.render_image().unwrap());
    }
    assert!(frames[0] == blank, "a smaller level was not read");
    assert!(frames[1] != blank, "the checker draws nothing");
}

struct RainbowFrame;

impl RainbowFrame {
    /// The frame with a Rainbow quad over `page`; `None` without a GPU.
    fn of(page: &PngImage) -> Option<FrameImage> {
        let mut r = renderer()?;
        r.add_atlas_page(
            rw_graphics::Atlas::single("maze", 64, 64),
            page,
            BILINEAR_REPEAT,
        );
        begin(&mut r);
        let quad = maze_quad([600, 300, 616, 316], [0.5, 1.0, 1.0, 1.0], Shader::Rainbow);
        r.draw_sprites(
            &DrawList { items: vec![quad] },
            Layer::Foreground,
            [0.0, 0.0],
        );
        Some(r.render_image().unwrap())
    }

    /// The number of differing bytes.
    fn differing(a: &FrameImage, b: &FrameImage) -> usize {
        a.rgba()
            .iter()
            .zip(b.rgba())
            .filter(|(x, y)| x != y)
            .count()
    }
}

/// `Rainbow`'s gradient read: a 16 px quad spans the ring's 64 rows in under 8 px, 8
/// texels a pixel, level 3. A page whose smaller levels are 255 gives nearly the frame of
/// a single-level 255 page, which differs much from a single-level 0 page.
#[test]
fn the_rainbow_gradient_reads_a_coarse_level_when_minified() {
    let (Some(with_mips), Some(coarse), Some(fine_only)) = (
        RainbowFrame::of(&chain(64, 0, 255)),
        RainbowFrame::of(&flat(64, 255)),
        RainbowFrame::of(&flat(64, 0)),
    ) else {
        return;
    };
    // The ring's edge blocks have a clamped `d`, so their derivatives (and levels) are
    // finer: only the blocks inside the ring must agree.
    let (mips_off, level_zero_off) = (
        RainbowFrame::differing(&with_mips, &coarse),
        RainbowFrame::differing(&fine_only, &coarse),
    );
    assert!(level_zero_off > 100, "the two values draw the same");
    assert!(
        mips_off * 4 < level_zero_off,
        "{mips_off} of {level_zero_off} differ"
    );
}
