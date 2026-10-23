//! Headless GPU checks of the screen-grab shaders (`ss_grab.wgsl`: `DeepProcessing` and
//! `GravityDisruptor`) at the game's 1366x768, against the fragments of
//! `DeepProcessing.shader` and `GravityDisruptor.shader` worked out per pixel on the CPU.
//! `_NoiseTex` is one grey, so every tap reads the same value. Skipped without a Vulkan
//! adapter.

use std::borrow::Cow;

use rw_assets::NoiseTexture;
use rw_graphics::{Color as SpriteColor, DrawItem, Mesh as SpriteMesh, Shader, Sprite};

use super::full_screen_tests::{H, Rect, W, level, palette, renderer, rgb};
use super::*;
use crate::LevelParams;

/// The shaders' `3.14`, not pi.
#[allow(clippy::approx_constant, reason = "the game's shaders use 3.14")]
const GAME_PI: f32 = 3.14;

/// `_NoiseTex`'s every texel's red.
const NOISE: u8 = 90;

/// `_RAIN`.
const RAIN: f32 = 0.7;

/// The level image, sprites behind the level, over it in `Foreground` and in `HUD`.
struct GrabScene {
    level: PngImage,
    behind: Vec<DrawItem>,
    over: Vec<DrawItem>,
    hud: Vec<DrawItem>,
}

impl GrabScene {
    fn draw(&self, r: &mut WgpuRenderer) -> FrameImage {
        let noise = NoiseTexture::from_image(&PngImage::from_pixels(
            64,
            64,
            vec![[NOISE, 0, 0, 255]; 4096],
        ))
        .unwrap();
        r.set_level(&LevelGraphics {
            camera_pos: [0.0, 0.0],
            image: &self.level,
            palette: &palette([30, 30, 30], &[]),
            noise: &noise,
            params: LevelParams::DEFAULT,
            rain_mask: None,
        });
        r.begin_frame(Camera::window([0.0, 0.0], [W as f32, H as f32], (W, H)));
        if !self.behind.is_empty() {
            let list = DrawList {
                items: self.behind.clone(),
            };
            r.draw_sprites(&list, Layer::Items, [0.0, 0.0]);
        }
        r.draw_level(LevelView {
            origin: [0.0, 0.0],
            size: [W as f32, H as f32],
            rain: RAIN,
            light: 1.0,
            water_level: 1.0,
        });
        for item in &self.over {
            let list = DrawList {
                items: vec![item.clone()],
            };
            r.draw_sprites(&list, item.layer(), [0.0, 0.0]);
        }
        for item in &self.hud {
            let list = DrawList {
                items: vec![item.clone()],
            };
            r.draw_sprites(&list, Layer::Hud, [0.0, 0.0]);
        }
        r.render_image().unwrap()
    }
}

fn fract(v: f32) -> f32 {
    v - v.floor()
}

/// `lerp`.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Frame pixel `(x, y)` (y down): its `scrPos` (y up).
fn scr(x: u32, y: u32) -> [f32; 2] {
    [
        (x as f32 + 0.5) / W as f32,
        (H as f32 - y as f32 - 0.5) / H as f32,
    ]
}

/// A 5 x 4 chequer of distinct colours in `Foreground`: a frame with structure to grab.
fn chequer() -> Vec<DrawItem> {
    let (w, h) = (W / 5 + 1, H / 4 + 1);
    let mut items = Vec::new();
    for j in 0..4 {
        for i in 0..5 {
            let k = (j * 5 + i) as f32;
            let color = SpriteColor::rgb(
                0.1 + 0.8 * fract(k * 0.37),
                0.1 + 0.8 * fract(k * 0.61 + 0.2),
                0.1 + 0.8 * fract(k * 0.83 + 0.5),
            );
            items.push(
                Rect([i * w, j * h, (i + 1) * w, (j + 1) * h]).quad(color, Layer::Foreground),
            );
        }
    }
    items
}

/// The `GravityDisruptor` disc at stage position `centre` with the vertex alpha `alpha`.
fn disc(centre: [f32; 2], alpha: f32, layer: Layer) -> DrawItem {
    DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(centre[0], centre[1]),
        rotation: 0.0,
        scale: Vec2::new(37.5, 37.5),
        anchor: Vec2::new(0.5, 0.5),
        color: SpriteColor::WHITE.with_alpha(alpha),
        shader: Shader::GravityDisruptor,
        layer,
    })
}

const CENTRE: [f32; 2] = [683.0, 384.0];

/// `GravityDisruptor.shader`: the frame pixel the fragment at `(x, y)` copies, `None`
/// outside the disc's 600 px quad. The quad's `i.uv` at a fragment is its stage position
/// (frame x + 1, frame y from the bottom - 1... as the full-screen effects') over the quad.
fn disruptor_source(x: u32, y: u32, alpha: f32) -> Option<(i64, i64)> {
    let uv = [
        ((x as f32 + 1.0) - (CENTRE[0] - 300.0)) / 600.0,
        ((H as f32 - y as f32 - 1.0) - (CENTRE[1] - 300.0)) / 600.0,
    ];
    if !(0.0..=1.0).contains(&uv[0]) || !(0.0..=1.0).contains(&uv[1]) {
        return None;
    }
    let d = [uv[0] - 0.5, uv[1] - 0.5];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let dist = (len * 2.0).clamp(0.0, 1.0);
    let mut effect = (((dist - 0.5) * 2.0).clamp(0.0, 1.0) * GAME_PI)
        .sin()
        .powf(5.0)
        .powf(0.65);
    if dist < 0.75 {
        effect = effect.max(((dist - 0.2) * 1.8).clamp(0.0, 1.0).powf(2.0));
    }
    let dir = [-d[1] / len, d[0] / len];
    let s = scr(x, y);
    let push = 0.05 * effect * (dist * 20.0 - RAIN * 3.1).sin() * alpha;
    let g = [s[0] + dir[0] * push, s[1] + dir[1] * push];
    let px = (g[0] * W as f32).floor() as i64;
    let py = (g[1] * H as f32).floor() as i64;
    Some((
        px.clamp(0, i64::from(W) - 1),
        i64::from(H) - 1 - py.clamp(0, i64::from(H) - 1),
    ))
}

/// With the vertex alpha 0 the disc samples each pixel's own place: the grab of the frame
/// so far (the chequer included) is put back unchanged.
#[test]
fn the_disc_with_no_power_changes_nothing() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene {
        level: level(|_, _| [255, 255, 255]),
        behind: Vec::new(),
        over: chequer(),
        hud: Vec::new(),
    };
    let before = scene.draw(&mut r);
    scene.hud.push(disc(CENTRE, 0.0, Layer::Hud));
    let after = scene.draw(&mut r);
    for y in 0..H {
        for x in 0..W {
            assert_eq!(rgb(&after, x, y), rgb(&before, x, y), "pixel {x},{y}");
        }
    }
}

/// With the vertex alpha 1 each fragment copies another pixel of the frame, rippling out
/// from the centre; the pixel it copies is the shader's maths on the CPU. The disc's
/// corners (beyond radius 300) are not displaced, and the chequer shows through shifted.
#[test]
fn the_disc_ripples_the_grabbed_frame() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene {
        level: level(|_, _| [255, 255, 255]),
        behind: Vec::new(),
        over: chequer(),
        hud: Vec::new(),
    };
    let before = scene.draw(&mut r);
    scene.hud.push(disc(CENTRE, 1.0, Layer::Hud));
    let after = scene.draw(&mut r);
    let (mut inside, mut moved, mut wrong) = (0, 0, 0);
    for y in 0..H {
        for x in 0..W {
            match disruptor_source(x, y, 1.0) {
                None => assert_eq!(rgb(&after, x, y), rgb(&before, x, y), "outside {x},{y}"),
                Some((sx, sy)) => {
                    inside += 1;
                    moved += usize::from((sx, sy) != (i64::from(x), i64::from(y)));
                    wrong += usize::from(rgb(&after, x, y) != rgb(&before, sx as u32, sy as u32));
                }
            }
        }
    }
    assert!(inside > 250_000, "{inside}");
    assert!(moved > inside / 4, "only {moved} of {inside} pixels move");
    // The GPU's `sin` and `pow` differ from the CPU's in the last bits, which can pick a
    // neighbouring pixel on a boundary.
    assert!(wrong * 200 < inside, "{wrong} of {inside} pixels differ");
}

/// `DeepProcessing.shader`'s class for the fragment: 0 nothing, 1 dim blue, 2 blue; `None`
/// where a value lies too near a threshold for the GPU's `sin` to be trusted. `red` is the
/// level code under the pixel (the level is uniform in bands, so its place is not
/// needed), `behind` whether something was drawn behind the level there, `uv` the
/// mesh's `i.uv`.
fn deep_class(
    (x, y): (u32, u32),
    uv: [f32; 2],
    red: u8,
    behind: bool,
    clr: [f32; 4],
) -> Option<u8> {
    let (w, h) = (W as f32, H as f32);
    let s = scr(x, y);
    let dp = if red == 255 {
        1.0
    } else {
        ((i32::from(red) - 1) % 30) as f32 / 30.0
    };
    let d = ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt();
    let mut dist = 1.0 - (d * 2.0).clamp(0.0, 1.0);
    if dp < clr[0] || dp > clr[1] || (dp > 6.0 / 30.0 && behind) {
        return Some(0);
    }
    let rect = [-0.5 / w, 0.5 / h, (w - 0.5) / w, (h + 0.5) / h];
    let text_coord = [
        ((s[0] * w).floor() / w - rect[0]) / (rect[2] - rect[0]),
        ((s[1] * h).floor() / h - rect[1]) / (rect[3] - rect[1]),
    ];
    let dsplace = [(s[0] - 0.5) * 0.06 * dp, ((1.0 - s[1]) - 0.5) * 0.06 * dp];
    let grid = mix(10.0, 6.0, dp);
    let sample = [
        (text_coord[0] * w / grid).floor() / w * grid + dsplace[0],
        (text_coord[1] * h / grid).floor() / h * grid + dsplace[1],
    ];
    let noise = f32::from(NOISE) / 255.0;
    let speed = mix(0.5, 1.5, clr[3]);
    let h2 = (((dp * 1.2 + 0.75 * RAIN * speed + noise * mix(1.0, 1.0, dist.powf(0.5)))
        * GAME_PI
        * 2.0)
        .sin())
        * 0.5
        + 0.5;
    let mut hh =
        (((dp * mix(0.12, 2.2, h2) + 1.8 * RAIN * speed + noise * mix(1.0, 4.0, h2.powi(3)))
            * GAME_PI
            * 2.0)
            .sin())
            * 0.5
            + 0.5;
    // Full power draws no hash.
    assert_eq!(clr[2], 1.0);
    let cell_centre = [sample[0] + grid * 0.5 / w, sample[1] + grid * 0.5 / h];
    let from = [text_coord[0] + dsplace[0], text_coord[1] + dsplace[1]];
    let away = ((from[0] - cell_centre[0]).powi(2) + (from[1] - cell_centre[1]).powi(2)).sqrt();
    hh *= 1.0 - away * (w + h) / (grid * 2.0);
    dist = dist.powf(mix(1.2, mix(0.199, 0.001, clr[3]), h2));
    hh += mix(-1.0, 1.0, clr[3]) * 0.2;
    let (low, high) = (hh * dist, hh * mix(dist, 1.0, 0.5));
    if (low - 0.33).abs() < 2e-3 || (high - 0.66).abs() < 2e-3 {
        return None;
    }
    Some(if low < 0.33 {
        0
    } else if high < 0.66 {
        1
    } else {
        2
    })
}

/// The mesh's vertices and uvs over stage rectangle `[l, b, r, t]`.
fn mesh(rect: [f32; 4], color: [f32; 4]) -> DrawItem {
    let [l, b, rt, t] = rect;
    DrawItem::Mesh(SpriteMesh {
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
        color: SpriteColor::from(color),
        vertex_colors: Vec::new(),
        shader: Shader::DeepProcessing,
        layer: Layer::Foreground,
    })
}

/// Bands of the level by frame column: sky, layer 0, layer 9, layer 4.
fn band_red(x: u32) -> u8 {
    match x {
        0..300 => 255,
        300..600 => 1,
        600..900 => 10,
        _ => 5,
    }
}

const BAND_EDGES: [u32; 3] = [300, 600, 900];

/// The share of the pixels drawn dim and bright, and of the ones compared.
struct Tally {
    compared: usize,
    classes: [usize; 3],
    wrong: usize,
}

fn compare_deep(
    after: &FrameImage,
    before: &FrameImage,
    quad: [f32; 4],
    clr: [f32; 4],
    behind: &[Rect],
    hidden: bool,
) -> Tally {
    let mut t = Tally {
        compared: 0,
        classes: [0; 3],
        wrong: 0,
    };
    let [l, b, rt, top] = quad;
    for y in 4..H - 4 {
        for x in 4..W - 4 {
            if BAND_EDGES
                .iter()
                .any(|&e| (i64::from(x) - i64::from(e)).abs() < 3)
            {
                continue;
            }
            let uv = [
                ((x as f32 + 1.0) - l) / (rt - l),
                ((H as f32 - y as f32 - 1.0) - b) / (top - b),
            ];
            let red = band_red(x);
            let behind = behind.iter().any(|r| r.contains(x, y)) || (hidden && red != 255);
            let Some(class) = deep_class((x, y), uv, red, behind, clr) else {
                continue;
            };
            t.compared += 1;
            t.classes[usize::from(class)] += 1;
            let (got, was) = (rgb(after, x, y), rgb(before, x, y));
            let want = match class {
                0 => was,
                1 => was.map(|c| (f32::from(c) * 0.8).round() as u8),
                _ => was.map(|c| (f32::from(c) * 0.6).round() as u8),
            };
            let mut want = want;
            match class {
                1 => want[2] = (f32::from(was[2]) * 0.8 + 0.5 * 255.0 * 0.2).round() as u8,
                2 => want[2] = (f32::from(was[2]) * 0.6 + 255.0 * 0.4).round() as u8,
                _ => {}
            }
            let off = got
                .iter()
                .zip(want)
                .any(|(&g, w)| (i32::from(g) - i32::from(w)).abs() > 2);
            t.wrong += usize::from(off);
        }
    }
    t
}

/// `DeepProcessing` at full power over bands of the level: nothing over sky (depth 1) and
/// layer 0 (below `fromDepth`), blue cells over layer 9, hidden there where a creature
/// is drawn behind the level (past layer 6) but not at layer 4. The cells are dim
/// `(0, 0, 0.5, 0.2)` or blue `(0, 0, 1, 0.4)` blended over the frame.
#[test]
fn deep_processing_rains_over_the_deep_layers() {
    let Some(mut r) = renderer() else { return };
    let creatures = [Rect([650, 100, 850, 300]), Rect([1000, 400, 1200, 600])];
    let mut scene = GrabScene {
        level: level(|x, _| {
            let red = band_red(x);
            [
                red,
                if red == 255 { 255 } else { 0 },
                if red == 255 { 255 } else { 0 },
            ]
        }),
        behind: creatures
            .iter()
            .map(|c| c.quad(SpriteColor::rgb(0.0, 0.5, 0.0), Layer::Items))
            .collect(),
        over: Vec::new(),
        hud: Vec::new(),
    };
    let before = scene.draw(&mut r);
    let clr = [0.1, 0.9, 1.0, 0.5];
    let quad = [0.0, 0.0, W as f32 + 2.0, H as f32 + 2.0];
    scene.over.push(mesh(quad, clr));
    let after = scene.draw(&mut r);
    let t = compare_deep(&after, &before, quad, clr, &creatures, false);
    assert!(t.compared > 300_000, "{} pixels compared", t.compared);
    assert!(
        t.classes[1] > 1000 && t.classes[2] > 1000,
        "dim {} and blue {} pixels",
        t.classes[1],
        t.classes[2]
    );
    assert!(
        t.wrong * 500 < t.compared,
        "{} of {} differ",
        t.wrong,
        t.compared
    );
}

/// With no power (vertex blue 0) a cell shows only when `h` wins over a hash in `[0, 1)`
/// that is `lerp(h, 0, 1) = 0 < rand`: almost none do.
#[test]
fn deep_processing_without_power_is_nearly_empty() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene {
        level: level(|x, _| {
            let red = band_red(x);
            [
                red,
                if red == 255 { 255 } else { 0 },
                if red == 255 { 255 } else { 0 },
            ]
        }),
        behind: Vec::new(),
        over: Vec::new(),
        hud: Vec::new(),
    };
    let before = scene.draw(&mut r);
    scene.over.push(mesh(
        [0.0, 0.0, W as f32 + 2.0, H as f32 + 2.0],
        [0.1, 0.9, 0.0, 0.5],
    ));
    let after = scene.draw(&mut r);
    let changed = (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .filter(|&(x, y)| rgb(&after, x, y) != rgb(&before, x, y))
        .count();
    assert!(
        changed * 1000 < (W * H) as usize,
        "{changed} pixels changed"
    );
}

/// `DeepProcessing` has no `GrabPass`: it reads the last one. A grabbing draw before it (the
/// disc, here at alpha 0) leaves the frame so far, non-black everywhere, so every pixel
/// past layer 6 is hidden; layer 4 (no grab test) still rains.
#[test]
fn deep_processing_reads_the_last_grab() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene {
        level: level(|x, _| {
            let red = band_red(x);
            [
                red,
                if red == 255 { 255 } else { 0 },
                if red == 255 { 255 } else { 0 },
            ]
        }),
        behind: Vec::new(),
        over: Vec::new(),
        hud: Vec::new(),
    };
    let before = scene.draw(&mut r);
    let clr = [0.1, 0.9, 1.0, 0.5];
    let quad = [0.0, 0.0, W as f32 + 2.0, H as f32 + 2.0];
    scene.over.push(disc(CENTRE, 0.0, Layer::Foreground));
    scene.over.push(mesh(quad, clr));
    let after = scene.draw(&mut r);
    let t = compare_deep(&after, &before, quad, clr, &[], true);
    assert!(t.compared > 300_000, "{} pixels compared", t.compared);
    assert!(t.classes[1] > 500, "{:?}", t.classes);
    let layer_9_rained = (0..H)
        .flat_map(|y| (610..890).map(move |x| (x, y)))
        .filter(|&(x, y)| rgb(&after, x, y) != rgb(&before, x, y))
        .count();
    assert_eq!(layer_9_rained, 0, "layer 9 is hidden by the grab");
    assert!(
        t.wrong * 500 < t.compared,
        "{} of {} differ",
        t.wrong,
        t.compared
    );
}
