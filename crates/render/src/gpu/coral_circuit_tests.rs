//! Headless GPU check of the `CoralCircuit` and `DeadCoralCircuit` shaders (`sprite.wgsl`,
//! `fs_coral_circuit`, `fs_dead_coral_circuit`) at the game's 1366x768, against the
//! `.shader` fragments worked out per pixel on the CPU. `_NoiseTex` is one grey (so its read
//! is a constant) and the `maze` page a synthetic checker, point sampled. The live
//! shader's hash `frac(sin(..) * 43758.5)` sits on the GPU's `sin`: its red is compared
//! within three steps and pixels whose hash lies at the wrap are left out. Skipped without
//! a Vulkan adapter.
//!
//! Mirrors: - (port-only: tests)

use std::borrow::Cow;

use rw_assets::NoiseTexture;
use rw_graphics::{Atlas, Color as SpriteColor, DrawItem, ElementMesh, Mesh as SpriteMesh, Shader};

use super::full_screen_tests::{H, W, level, palette, renderer, rgb};
use super::test_support::{FramePixel, Hlsl};
use super::*;
use crate::LevelParams;

/// `_NoiseTex`'s every texel's red.
const NOISE: u8 = 90;
const RECT: [u32; 4] = [100, 100, 700, 500];
/// The maze texture's side.
const MAZE: u32 = 128;

/// Whether the maze texel at column `x` and row `y` (from the bottom) is lit (red above
/// 0.5): a checker of 8 x 8 texel blocks.
fn lit(x: u32, y: u32) -> bool {
    (x / 8 + y / 8).is_multiple_of(2)
}

fn maze_image() -> PngImage {
    PngImage::from_pixels(
        MAZE as usize,
        MAZE as usize,
        (0..MAZE * MAZE)
            .map(|i| {
                let (x, row) = (i % MAZE, i / MAZE);
                let v = if lit(x, MAZE - 1 - row) { 255 } else { 0 };
                [v, 0, 0, 255]
            })
            .collect(),
    )
}

/// The vertex colour: the window of the texture the quad shows.
const WINDOW: [f32; 4] = [0.25, 0.375, 0.5, 0.25];

/// What the shader draws at frame pixel `(x, y)`: `None` for nothing, else the red channel
/// (0 to 1); `Err` where a boundary lies so near that the GPU's rounding may decide.
fn reference(x: u32, y: u32, rain: f32, live: bool) -> Result<Option<f32>, ()> {
    let [x0, y0, x1, y1] = RECT;
    // The stage is Futile's: screen = stage + (-0.5, 0.5), so the quad lies half a pixel
    // left of and above where its stage corners say.
    let uv = [
        (x as f32 + 1.0 - x0 as f32) / (x1 - x0) as f32,
        ((y1 - y) as f32 - 1.0) / (y1 - y0) as f32,
    ];
    let radius = ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt();
    let window = [
        WINDOW[0].lerp_to(WINDOW[0] + WINDOW[2], uv[0]) * MAZE as f32,
        WINDOW[1].lerp_to(WINDOW[1] + WINDOW[3], uv[1]) * MAZE as f32,
    ];
    let near_texel_edge = |v: f32| (v - v.round()).abs() < 0.02;
    if (radius - 0.65).abs() < 0.004 || near_texel_edge(window[0]) || near_texel_edge(window[1]) {
        return Err(());
    }
    if radius > 0.65 || lit(window[0] as u32, window[1] as u32) {
        return Ok(None);
    }
    if !live {
        return Ok(Some(0.5));
    }
    let text = FramePixel(x, y).level_coord();
    let (w, h) = (W as f32, H as f32);
    let cell = [
        (text[0] * w / 2.0).floor() / w,
        (text[1] * h / 2.0).floor() / h,
    ];
    let noise = f32::from(NOISE) / 255.0;
    let rand = ((cell[0] * 12.98232 * cell[1] * 0.23532 + rain - noise).sin() * 43_758.547).frac();
    // Near the hash's wrap the GPU's `sin` may land on the other side.
    if !(0.02..=0.98).contains(&rand) {
        return Err(());
    }
    Ok(Some(0.5 + 0.5 * rand))
}

fn draw(r: &mut WgpuRenderer, rain: f32, shader: Option<Shader>) -> FrameImage {
    r.set_level(&LevelGraphics {
        camera_pos: [0.0, 0.0],
        image: &level(|_, _| [255, 255, 255]),
        palette: &palette([10, 10, 10], &[]),
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
    if let Some(shader) = shader {
        let [x0, y0, x1, y1] = RECT;
        let (l, t) = (x0 as f32, H as f32 - y0 as f32);
        let (rt, b) = (x1 as f32, H as f32 - y1 as f32);
        let quad = DrawItem::ElementMesh(ElementMesh {
            element: Cow::Borrowed("maze"),
            mesh: SpriteMesh {
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
                vertex_colors: vec![SpriteColor::from(WINDOW); 4],
                shader,
                layer: Layer::Foreground,
            },
        });
        r.draw_sprites(
            &DrawList { items: vec![quad] },
            Layer::Foreground,
            [0.0, 0.0],
        );
    }
    r.render_image().unwrap()
}

/// Every compared pixel is left alone where the CPU reading cuts it and drawn with the
/// reference red (green and blue 0) elsewhere.
fn check(rain: f32, shader: Shader) {
    let Some(mut r) = renderer() else { return };
    r.add_atlas_page(
        Atlas::single("maze", MAZE, MAZE),
        &maze_image(),
        TextureSampling::POINT_CLAMP,
    );
    let live = shader == Shader::CoralCircuit;
    let before = draw(&mut r, rain, None);
    let after = draw(&mut r, rain, Some(shader));
    let [x0, y0, x1, y1] = RECT;
    let (mut drawn, mut total, mut wrong) = (0, 0, 0);
    for y in y0 + 2..y1 - 2 {
        for x in x0 + 2..x1 - 2 {
            let Ok(want) = reference(x, y, rain, live) else {
                continue;
            };
            total += 1;
            let (was, got) = (rgb(&before, x, y), rgb(&after, x, y));
            match want {
                None => wrong += usize::from(got != was),
                Some(red) => {
                    drawn += 1;
                    let near = (f32::from(got[0]) - red * 255.0).abs() <= 3.0
                        && got[1] == 0
                        && got[2] == 0;
                    wrong += usize::from(!near);
                }
            }
        }
    }
    assert!(total > 200_000, "only {total} pixels compared");
    assert!(
        drawn > total / 5 && drawn < total - total / 5,
        "{drawn} of {total} pixels drawn"
    );
    assert!(wrong == 0, "{wrong} of {total} pixels differ");
}

/// The dead bit: a flat `(0.5, 0, 0)` where the window's texel is dark inside the circle.
#[test]
fn a_displaced_bit_is_flat_dim_red() {
    check(0.0, Shader::DeadCoralCircuit);
}

/// The live bit: `0.5 + 0.5 * rand` red, hashed per 2 px of the screen.
#[test]
fn a_bit_in_place_is_noisy_red() {
    check(0.0, Shader::CoralCircuit);
}

/// `_RAIN` moves the hash.
#[test]
fn the_rain_changes_the_hash() {
    check(0.37, Shader::CoralCircuit);
}
