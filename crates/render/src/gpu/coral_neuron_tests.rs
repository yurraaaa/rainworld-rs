//! Headless GPU check of the `CoralNeuron` shader (`sprite.wgsl`, `fs_coral_neuron`) at the
//! game's 1366x768, against `CoralNeuron.shader`'s fragment worked out per pixel on the CPU.
//! `_NoiseTex` is one grey (so its two reads are constants), the palette texel (30, 7) a
//! known grey. Skipped without a Vulkan adapter.
//!
//! Mirrors: - (port-only: tests)

use std::borrow::Cow;

use rw_assets::NoiseTexture;
use rw_graphics::{Color as SpriteColor, DrawItem, Mesh as SpriteMesh, Shader};

use super::full_screen_tests::{H, W, level, palette, renderer, rgb};
use super::*;
use crate::LevelParams;

/// `_NoiseTex`'s every texel's red.
const NOISE: u8 = 60;
/// The palette texel (30, 7)'s red.
const PALETTE: u8 = 200;
const RECT: [u32; 4] = [100, 100, 700, 500];

/// The fragment for a pixel's `uv`: the alpha it draws with, or `None` for nothing, and the
/// `h` it was decided by.
#[allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    reason = "the shader's literals (3.14 is the game's), rounded to f32 as the GPU does"
)]
fn reference(uv: [f32; 2], rain: f32, alpha: f32) -> (Option<f32>, f32) {
    let noise = f32::from(NOISE) / 255.0;
    let y = (uv[1] + rain * 0.1) * (1.0 / alpha) * 0.1;
    let mut h = noise + y * 0.5;
    h = 0.5 + ((h + rain * 6.0) * 3.14 * 2.0).sin();
    h = 1.0 - (0.5 - h).abs() * 2.0;
    let mut h2 = noise - y * 0.5;
    h2 = 0.5 + ((h2 + rain * 6.21) * 3.14 * 2.0).sin();
    h2 = 1.0 - (0.5 - h2).abs() * 2.0;
    h *= h2;
    h *= (1.0 - (0.5 - uv[0]).abs() * 2.0).powf(0.2);
    h = 1.0 - (0.5 - h).abs() * 2.0;
    if h < 0.75 {
        return (None, h);
    }
    let pal = f32::from(PALETTE) / 255.0;
    (Some(0.5 + (0.25 - 0.5) * pal), h)
}

fn draw(r: &mut WgpuRenderer, rain: f32, alpha: f32, with: bool) -> FrameImage {
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &level(|_, _| [255, 255, 255]),
        palette: &palette([10, 10, 10], &[(30, 7, [PALETTE, PALETTE, PALETTE])]),
        noise: &NoiseTexture::from_image(&PngImage::from_pixels(
            64,
            64,
            vec![[NOISE, 0, 0, 255]; 4096],
        ))
        .unwrap(),
        params: LevelParams::DEFAULT,
        rain_mask: None,
    });
    r.set_stage_globals(StageGlobals {
        rain,
        ..StageGlobals::default()
    });
    r.begin_frame(Camera::window([0.0, 0.0], [W as f32, H as f32], (W, H)));
    r.draw_level(LevelView {
        origin: [0.0, 0.0],
        size: [W as f32, H as f32],
        rain,
        light: 1.0,
        water_level: 1.0,
    });
    if with {
        let [x0, y0, x1, y1] = RECT;
        let (l, t) = (x0 as f32, H as f32 - y0 as f32);
        let (rt, b) = (x1 as f32, H as f32 - y1 as f32);
        let quad = DrawItem::Mesh(SpriteMesh {
            index: 0,
            vertices: vec![
                Vec2::new(l, b),
                Vec2::new(l, t),
                Vec2::new(rt, t),
                Vec2::new(rt, b),
            ],
            uvs: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(1.0, 0.0),
            ],
            triangles: Cow::Borrowed(&[[0, 1, 2], [0, 2, 3]]),
            color: SpriteColor::WHITE,
            vertex_colors: vec![SpriteColor::rgb(1.0, 0.5, 0.25).with_alpha(alpha); 4],
            shader: Shader::CoralNeuron,
            layer: Layer::Foreground,
        });
        r.draw_sprites(
            &DrawList { items: vec![quad] },
            Layer::Foreground,
            [0.0, 0.0],
        );
    }
    r.render_image().unwrap()
}

/// Every compared pixel of the quad is drawn with the vertex colour at the palette's
/// alpha exactly where the CPU reading keeps its line, and left alone elsewhere.
fn check(rain: f32, alpha: f32) {
    let Some(mut r) = renderer() else { return };
    let before = draw(&mut r, rain, alpha, false);
    let after = draw(&mut r, rain, alpha, true);
    let [x0, y0, x1, y1] = RECT;
    let (mut lines, mut total, mut wrong) = (0, 0, 0);
    // The quad's outermost pixels are left out: where it ends is the rasteriser's.
    for y in y0 + 2..y1 - 2 {
        for x in x0 + 2..x1 - 2 {
            let uv = [
                (x as f32 + 0.5 - x0 as f32) / (x1 - x0) as f32,
                ((y1 - y) as f32 - 0.5) / (y1 - y0) as f32,
            ];
            let (kept, h) = reference(uv, rain, alpha);
            if (h - 0.75).abs() < 0.05 {
                continue;
            }
            total += 1;
            let (was, got) = (rgb(&before, x, y), rgb(&after, x, y));
            match kept {
                None => wrong += usize::from(got != was),
                Some(a) => {
                    lines += 1;
                    let want = [1.0f32, 0.5, 0.25];
                    let near = (0..3).all(|c| {
                        let w = f32::from(was[c]) * (1.0 - a) + want[c] * 255.0 * a;
                        (f32::from(got[c]) - w).abs() <= 2.0
                    });
                    wrong += usize::from(!near);
                }
            }
        }
    }
    assert!(total > 200_000, "only {total} pixels compared");
    assert!(
        lines > total / 50 && lines < total / 2,
        "{lines} of {total} pixels on a line"
    );
    assert!(wrong * 200 < total, "{wrong} of {total} pixels differ");
}

/// A neuron of 20 segments (vertex alpha 1/20) at rest.
#[test]
fn thin_lines_of_two_noise_bands() {
    check(0.0, 1.0 / 20.0);
}

/// `_RAIN` scrolls both bands (up to a quarter turn of `sin` arguments around 20 rad; at
/// larger ones this GPU's `sin` drifts from the CPU's by more than the lines are wide).
#[test]
fn the_rain_scrolls_the_bands() {
    check(0.37, 1.0 / 20.0);
}
