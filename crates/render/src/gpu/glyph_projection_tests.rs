//! Headless GPU check of the `GlyphProjection` shader (`glyph_projection.wgsl`) at the
//! game's 1366x768, against `GlyphProjection.shader`'s fragment worked out per pixel on
//! the CPU. `_NoiseTex` is one grey (so its two reads are constants) and the `glyphs`
//! page a synthetic pattern, point sampled. The hashes `frac(sin(..) * 43758.5)` sit on
//! the GPU's `sin`, which is not the CPU's to the last bit, so a few cells may differ.
//! Skipped without a Vulkan adapter.

use rw_assets::{NoiseTexture, PaletteTexture};
use rw_graphics::{Atlas, Color as SpriteColor, DrawItem, ElementMesh, Mesh as SpriteMesh, Shader};
use std::borrow::Cow;

use super::full_screen_tests::{H, W, level, palette, renderer, rgb};
use super::test_support::Hlsl;
use super::*;
use crate::LevelParams;

/// The `glyphs` texture: 50 glyphs of 15 x 15 side by side.
const GLYPHS: (u32, u32) = (750, 15);

/// Whether the synthetic texel at column `x` and row `y` (from the bottom, as Unity's `v`)
/// is set.
fn texel_set(x: u32, y: u32) -> bool {
    (x / 15 * 3 + x % 15 + y * 5) % 7 < 3
}

fn glyphs_image() -> PngImage {
    let (w, h) = GLYPHS;
    PngImage::from_pixels(
        w as usize,
        h as usize,
        (0..w * h)
            .map(|i| {
                // PNG rows run top to bottom; the shader's `v` from the bottom.
                let (x, row) = (i % w, i / w);
                let v = if texel_set(x, h - 1 - row) { 255 } else { 0 };
                [v, v, v, 255]
            })
            .collect(),
    )
}

/// The shader's fragment for frame pixel `(x, y)` (y down): whether it is drawn black, or
/// `None` where the hash lies so near a glyph or alpha boundary that the GPU's `sin` may
/// fall on the other side.
#[allow(
    clippy::excessive_precision,
    reason = "the shader's literals, rounded to f32 as the GPU does"
)]
fn reference(
    x: u32,
    y: u32,
    grid: [f32; 2],
    noise: f32,
    clr: [f32; 4],
    margin: f32,
) -> Option<bool> {
    let (w, h) = (W as f32, H as f32);
    let scr = [(x as f32 + 0.5) / w, (h - y as f32 - 0.5) / h];
    let tex_size = [1400.0_f32, 800.0];
    let mut tc = [
        (scr[0] * tex_size[0]).floor() / tex_size[0],
        (scr[1] * tex_size[1]).floor() / tex_size[1],
    ];
    tc[0] -= grid[0] / w;
    tc[1] -= grid[1] / h;
    // `_spriteRect` of a level image at the window's origin.
    let rect = [-0.5 / w, 0.5 / h, (w - 0.5) / w, (h + 0.5) / h];
    tc[0] = (tc[0] - rect[0]) / (rect[2] - rect[0]);
    tc[1] = (tc[1] - rect[1]) / (rect[3] - rect[1]);
    let cell = [
        (tc[0] * tex_size[0] / 15.0).floor() / tex_size[0],
        (tc[1] * tex_size[1] / 15.0).floor() / tex_size[1],
    ];
    let rand = ((cell[0] * 12.98232 * cell[1] * 0.23532 - noise).sin()
        * 43758.5453_f32.lerp_to(23746.232, clr[1]))
    .frac();
    let keep = ((cell[0] * 7.62232 * cell[1] * 0.19532 - noise).sin() * 102_438.543).frac();
    if (keep - clr[3]).abs() < 0.05 || (rand * 50.0 - (rand * 50.0).round()).abs() < margin {
        return None;
    }
    if keep > clr[3] {
        return Some(false);
    }
    let strt = [
        (tc[0] * tex_size[0] / 15.0).frac(),
        (tc[1] * tex_size[1] / 15.0).frac(),
    ];
    let glyph = (rand * 50.0).floor() / 50.0;
    let u = glyph + strt[0] / 50.0;
    let (tx, ty) = (
        ((u * GLYPHS.0 as f32).floor() as u32).min(GLYPHS.0 - 1),
        ((strt[1] * GLYPHS.1 as f32).floor() as u32).min(GLYPHS.1 - 1),
    );
    Some((!texel_set(tx, ty)) == (clr[0] == 0.0))
}

struct FrameScene {
    level_image: PngImage,
    pal: PaletteTexture,
    noise: NoiseTexture,
    quad: DrawItem,
    grid: [f32; 2],
}

impl FrameScene {
    fn draw(&self, r: &mut WgpuRenderer, with: bool) -> FrameImage {
        r.set_level(&LevelGraphics {
            camera_pos: [0.0, 0.0],
            image: &self.level_image,
            palette: &self.pal,
            noise: &self.noise,
            params: LevelParams::DEFAULT,
            rain_mask: None,
        });
        r.begin_frame(Camera::window([0.0, 0.0], [W as f32, H as f32], (W, H)));
        r.set_grid_offset(self.grid);
        r.draw_level(LevelView {
            origin: [0.0, 0.0],
            size: [W as f32, H as f32],
            rain: 0.0,
            light: 1.0,
            water_level: 1.0,
        });
        if with {
            r.draw_sprites(
                &DrawList {
                    items: vec![self.quad.clone()],
                },
                Layer::Foreground,
                [0.0, 0.0],
            );
        }
        r.render_image().unwrap()
    }
}

/// The frame without and with a `rect` quad of the shader and vertex colour `color`.
fn frames(
    r: &mut WgpuRenderer,
    rect: [u32; 4],
    color: [f32; 4],
    grid: [f32; 2],
) -> [FrameImage; 2] {
    let [x0, y0, x1, y1] = rect;
    let (l, t) = (x0 as f32, H as f32 - y0 as f32);
    let (rt, b) = (x1 as f32, H as f32 - y1 as f32);
    let quad = DrawItem::ElementMesh(ElementMesh {
        element: Cow::Borrowed("glyphs"),
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
            vertex_colors: vec![SpriteColor::from(color); 4],
            shader: Shader::GlyphProjection,
            layer: Layer::Foreground,
        },
    });
    let scene = FrameScene {
        level_image: level(|_, _| [255, 255, 255]),
        pal: palette([10, 10, 10], &[]),
        noise: NoiseTexture::from_image(&PngImage::from_pixels(
            64,
            64,
            vec![[NOISE, 0, 0, 255]; 4096],
        ))
        .unwrap(),
        quad,
        grid,
    };
    [scene.draw(r, false), scene.draw(r, true)]
}

/// `_NoiseTex`'s every texel's red.
const NOISE: u8 = 90;

/// Counts the pixels of `rect` where the frame with the quad is black when the reference
/// says so and unchanged when it does not.
fn compare(color: [f32; 4], grid: [f32; 2], margin: f32) -> Option<(usize, usize, usize)> {
    let mut r = renderer()?;
    r.add_atlas_page(
        Atlas::single("glyphs", GLYPHS.0, GLYPHS.1),
        &glyphs_image(),
        TextureSampling::POINT_CLAMP,
    );
    let rect = [100, 100, 700, 500];
    let [before, after] = frames(&mut r, rect, color, grid);
    let noise = f32::from(NOISE) / 255.0;
    let (mut black, mut total, mut wrong) = (0, 0, 0);
    for y in rect[1]..rect[3] {
        for x in rect[0]..rect[2] {
            let Some(want) = reference(x, y, grid, noise, color, margin) else {
                continue;
            };
            total += 1;
            let got = rgb(&after, x, y);
            let is_black = got == [0, 0, 0] && rgb(&before, x, y) != [0, 0, 0];
            let unchanged = got == rgb(&before, x, y);
            black += usize::from(want);
            if (want && !is_black) || (!want && !unchanged) {
                wrong += 1;
            }
        }
    }
    Some((black, total, wrong))
}

fn check(color: [f32; 4], grid: [f32; 2], margin: f32) {
    let Some((black, total, wrong)) = compare(color, grid, margin) else {
        return;
    };
    // Some of the quad is drawn and some left alone. The hash multiplies a sine by 43758,
    // so the GPU's `sin` differing from the CPU's in its last bits can pick another glyph:
    // cells whose hash lies near a boundary are left out of the comparison, and a wrong
    // shift or orientation would differ in half of the rest.
    assert!(total > 240_000 / 25, "only {total} pixels compared");
    assert!(
        black > total / 20 && black < total - total / 20,
        "{black} of {total}"
    );
    assert!(wrong * 100 < total, "{wrong} of {total} pixels differ");
}

/// A cursor's colour (red 1, green 1): the glyphs themselves.
#[test]
fn glyphs_of_a_cursor() {
    check([1.0, 1.0, 1.0, 1.0], [0.0, 0.0], 0.3);
}

/// A single glyph's (red 0, green 0): the field round the glyphs, the grid shifted by
/// `_gridOffset`.
#[test]
fn inverted_glyphs_follow_the_grid_offset() {
    check([0.0, 0.0, 0.0, 1.0], [7.3, -3.6], 0.3);
}

/// The game's offsets are `15 * intOffset` (1500 and up), so the shifted coordinates
/// leave 0 to 1 and the sines the GPU computes lose the digits the hash multiplies up
/// (this GPU picks other glyphs in some cells). What the offset decides is the cells' place
/// and the texel in them: nearly every cell of the frame must show one glyph of the
/// texture whole.
#[test]
fn a_big_grid_offset_moves_the_cells() {
    let grid = [1500.37, 224.61];
    let color = [0.0, 0.0, 0.5, 1.0];
    let Some(mut r) = renderer() else { return };
    r.add_atlas_page(
        Atlas::single("glyphs", GLYPHS.0, GLYPHS.1),
        &glyphs_image(),
        TextureSampling::POINT_CLAMP,
    );
    let rect = [100, 100, 700, 500];
    let [before, after] = frames(&mut r, rect, color, grid);
    // Per cell (its index on the shifted grid): for each glyph the pixels it explains, and
    // the number of pixels.
    let mut cells: std::collections::HashMap<(i32, i32), (Vec<u32>, u32)> = Default::default();
    for y in rect[1]..rect[3] {
        for x in rect[0]..rect[2] {
            let (cell, tx, ty) = place(x, y, grid);
            let drawn = rgb(&after, x, y) != rgb(&before, x, y);
            let entry = cells.entry(cell).or_insert_with(|| (vec![0; 50], 0));
            entry.1 += 1;
            for (k, hits) in entry.0.iter_mut().enumerate() {
                // Red 0: black where the texel is not set.
                if drawn == !texel_set(k as u32 * 15 + tx, ty) {
                    *hits += 1;
                }
            }
        }
    }
    let mut bad = 0;
    for (hits, n) in cells.values() {
        // The cells at the quad's edge hold few pixels.
        if *n >= 100 && hits.iter().all(|&h| h + 8 < *n) {
            bad += 1;
        }
    }
    assert!(cells.len() > 600, "{} cells", cells.len());
    assert!(
        bad * 50 < cells.len(),
        "{bad} of {} cells match no glyph",
        cells.len()
    );
}

/// The cell and the texel of the glyph that pixel `(x, y)` falls on, as the shader's
/// grid puts them.
fn place(x: u32, y: u32, grid: [f32; 2]) -> ((i32, i32), u32, u32) {
    let (w, h) = (W as f32, H as f32);
    let scr = [(x as f32 + 0.5) / w, (h - y as f32 - 0.5) / h];
    let tex_size = [1400.0_f32, 800.0];
    let rect = [-0.5 / w, 0.5 / h, (w - 0.5) / w, (h + 0.5) / h];
    let tc = [
        ((scr[0] * tex_size[0]).floor() / tex_size[0] - grid[0] / w - rect[0])
            / (rect[2] - rect[0]),
        ((scr[1] * tex_size[1]).floor() / tex_size[1] - grid[1] / h - rect[1])
            / (rect[3] - rect[1]),
    ];
    let g = [tc[0] * tex_size[0] / 15.0, tc[1] * tex_size[1] / 15.0];
    (
        (g[0].floor() as i32, g[1].floor() as i32),
        (g[0].frac() * 15.0).floor() as u32,
        (g[1].frac() * 15.0).floor() as u32,
    )
}

/// A matrix's alpha drops cells (the second hash above it).
#[test]
fn alpha_keeps_a_share_of_the_cells() {
    check([0.0, 0.0, 0.3, 0.5], [0.0, 0.0], 0.3);
}
