//! Headless GPU checks of the `Decal` and `Rainbow` shaders (`placed_visuals.wgsl`) at
//! the game's 1366x768, with values worked out from `Decal.shader` and `Rainbow.shader`
//! (`StreamingAssets/shaders`) on the frame drawn without the object. `_NoiseTex2` is
//! white and `_NoiseTex` one grey, so the noise terms are constants. Skipped without a
//! Vulkan adapter.

use std::borrow::Cow;

use rw_assets::{NoiseTexture, PaletteTexture};
use rw_graphics::{
    Atlas, Color as SpriteColor, DrawItem, ElementMesh, Mesh as SpriteMesh, Shader, Sprite,
};

use super::full_screen_tests::{H, W, level, palette, renderer, rgb, unit};
use super::*;
use crate::LevelParams;

/// A frame: the level image, palette and noise grey, sprites behind the level and the
/// object's meshes over it in `layer`.
struct Frame {
    level: PngImage,
    palette: PaletteTexture,
    noise: u8,
    behind: Vec<DrawItem>,
    over: Vec<DrawItem>,
    layer: Layer,
}

impl Frame {
    fn draw(&self, r: &mut WgpuRenderer, with_object: bool) -> FrameImage {
        let noise = NoiseTexture::from_image(&PngImage::from_pixels(
            64,
            64,
            vec![[self.noise, 0, 0, 255]; 4096],
        ))
        .unwrap();
        r.set_level(&LevelGraphics {
            camera_pos: [0.0, 0.0],
            image: &self.level,
            palette: &self.palette,
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
            rain: 0.0,
            light: 1.0,
            water_level: 1.0,
        });
        if with_object {
            let list = DrawList {
                items: self.over.clone(),
            };
            r.draw_sprites(&list, self.layer, [0.0, 0.0]);
        }
        r.render_image().unwrap()
    }
}

/// The stage position of frame pixel `(x, y)`'s centre (`Camera::SCREEN_PIXEL_OFFSET`).
fn stage(x: u32, y: u32) -> [f32; 2] {
    [x as f32 + 1.0, H as f32 - y as f32 - 1.0]
}

/// A frame rectangle `[x0, x1) x [y0, y1)` (y down) as a mesh quad over the whole of
/// element `element` with vertex colour `color` on every vertex.
fn quad(
    element: &'static str,
    rect: [u32; 4],
    color: [f32; 4],
    shader: Shader,
    layer: Layer,
) -> DrawItem {
    let [x0, y0, x1, y1] = rect;
    let (a, b) = (stage(x0, y1 - 1), stage(x1 - 1, y0));
    // Stage corners of the rectangle's outer edges (half a pixel out of the centres).
    let (l, bot, r, top) = (a[0] - 0.5, a[1] - 0.5, b[0] + 0.5, b[1] + 0.5);
    DrawItem::ElementMesh(ElementMesh {
        element: Cow::Borrowed(element),
        mesh: SpriteMesh {
            index: 0,
            vertices: vec![
                Vec2::new(l, bot),
                Vec2::new(l, top),
                Vec2::new(r, top),
                Vec2::new(r, bot),
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
            shader,
            layer,
        },
    })
}

/// A `Futile_White` rectangle over frame pixels `[x0, x1) x [y0, y1)`, drawn behind the
/// level as a creature is.
fn creature(rect: [u32; 4]) -> DrawItem {
    let [x0, y0, x1, y1] = rect.map(|v| v as f32);
    DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(x0 + 0.5, H as f32 - y1 + 0.5),
        rotation: 0.0,
        scale: Vec2::new((x1 - x0) / 16.0, (y1 - y0) / 16.0),
        anchor: Vec2::ZERO,
        color: SpriteColor::rgb(0.8, 0.0, 0.0),
        shader: Shader::Basic,
        layer: Layer::Items,
    })
}

/// A solid `w` x `h` page named `name`.
fn add_page(r: &mut WgpuRenderer, name: &str, w: u32, h: u32, pixel: [u8; 4]) {
    r.add_atlas_page(
        Atlas::single(name, w, h),
        &PngImage::from_pixels(w as usize, h as usize, vec![pixel; (w * h) as usize]),
    );
}

/// The shaders' `6.28` and `3.14`, not 2π and π.
#[allow(
    clippy::approx_constant,
    reason = "the game's shader uses 6.28 and 3.14"
)]
const GAME_TAU: f32 = 6.28;
#[allow(
    clippy::approx_constant,
    reason = "the game's shader uses 6.28 and 3.14"
)]
const GAME_PI: f32 = 3.14;

const DECAL: [u8; 4] = [204, 102, 51, 255];

/// Terrain colours of runtime row 0 (lit) at palette columns 3, 10 and 20, and of row 3
/// (shadowed) at column 10.
fn decal_frame() -> Frame {
    let level = level(|x, _| match x {
        683..783 => [4, 0, 0],
        783..883 => [11, 0, 0],
        883..983 => [21, 0, 0],
        983..1083 => [101, 0, 0],
        _ => [255, 255, 255],
    });
    let palette = palette(
        [0, 0, 0],
        &[
            (3, 0, [40, 90, 140]),
            (10, 0, [200, 150, 100]),
            (20, 0, [30, 220, 120]),
            (10, 3, [60, 10, 160]),
        ],
    );
    Frame {
        level,
        palette,
        noise: 128,
        behind: Vec::new(),
        over: vec![quad(
            "decal",
            [600, 200, 1100, 500],
            [0.0, 1.0, 0.0, 1.0],
            Shader::Decal,
            Layer::Foreground,
        )],
        layer: Layer::Foreground,
    }
}

/// The shader's colour from terrain `t` and decal texel `m` (rgb 0 to 1) at the shader's
/// `shadow` (1 where the cloud shadow is off, which is also the near layers): at 1,
/// `lerp(lerp(t, m, .5), t * m, .3)`; at 0 a quarter of `Screen(t, m)` is mixed in
/// first and the multiply weighs `.2`.
fn mixed(t: [f32; 3], m: [f32; 3], shadow: f32) -> [f32; 3] {
    std::array::from_fn(|c| {
        let lit = t[c] + (m[c] - t[c]) * 0.5;
        let screen = 1.0 - (1.0 - t[c]) * (1.0 - m[c]);
        let lit = lit + (screen - lit) * ((1.0 - shadow) * 0.25);
        let multiply = t[c] * m[c];
        lit + (multiply - lit) * (0.2 + 0.1 * shadow)
    })
}

fn assert_near(got: [u8; 3], want: [f32; 3], at: (u32, u32)) {
    for c in 0..3 {
        let w = want[c].clamp(0.0, 1.0) * 255.0;
        assert!(
            (f32::from(got[c]) - w).abs() <= 1.0,
            "pixel {at:?}: got {got:?}, want {:?}",
            want.map(|v| v * 255.0)
        );
    }
}

/// `src` over `under` with `alpha` (`Blend SrcAlpha OneMinusSrcAlpha`).
fn over(src: [f32; 3], alpha: f32, under: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|c| src[c] * alpha + under[c] * (1.0 - alpha))
}

/// Layers 1 to 90 are never in the cloud shadow (`shadow` 1): on layer 3, 10 and 20 the
/// colour is the terrain's palette colour mixed with the texel at the alpha `lerp(1,
/// 0.25 + 0.25 * clr.w, shadow)` = 0.5; sky and the pixels outside the quad are
/// untouched.
#[test]
fn decal_tints_with_the_terrain_colour_and_skips_sky() {
    let Some(mut r) = renderer() else { return };
    add_page(&mut r, "decal", 4, 4, DECAL);
    let frame = decal_frame();
    let before = frame.draw(&mut r, false);
    let after = frame.draw(&mut r, true);
    let m = DECAL.map(|v| f32::from(v) / 255.0);
    let m = [m[0], m[1], m[2]];
    for (x, terrain) in [
        (730, [40, 90, 140]),
        (830, [200, 150, 100]),
        (930, [30, 220, 120]),
    ] {
        let want = over(
            mixed(unit(terrain), m, 1.0),
            0.5,
            unit(rgb(&before, x, 300)),
        );
        assert_near(rgb(&after, x, 300), want, (x, 300));
        assert_ne!(rgb(&after, x, 300), rgb(&before, x, 300));
    }
    // Sky (the quad starts at 600), and beyond the quad's rows.
    for (x, y) in [(650, 300), (730, 100), (1050, 600)] {
        assert_eq!(rgb(&after, x, y), rgb(&before, x, y), "({x}, {y})");
    }
}

/// `red / 30` outside the vertex colour's red and green (`fromDepth`, `toDepth`) shows
/// nothing: with 0.2 and 0.5 (51/255 and 128/255) depth 3 (0.1) and 20 (0.67) are cut,
/// 10 (0.33) stays.
#[test]
fn decal_is_clipped_to_its_depth_range() {
    let Some(mut r) = renderer() else { return };
    add_page(&mut r, "decal", 4, 4, DECAL);
    let mut frame = decal_frame();
    frame.over = vec![quad(
        "decal",
        [600, 200, 1100, 500],
        [51.0 / 255.0, 128.0 / 255.0, 0.0, 1.0],
        Shader::Decal,
        Layer::Foreground,
    )];
    let before = frame.draw(&mut r, false);
    let after = frame.draw(&mut r, true);
    assert_eq!(rgb(&after, 730, 300), rgb(&before, 730, 300), "depth 3");
    assert_ne!(rgb(&after, 830, 300), rgb(&before, 830, 300), "depth 10");
    assert_eq!(rgb(&after, 930, 300), rgb(&before, 930, 300), "depth 20");
}

/// From layer 5 on, a pixel with something drawn behind the level at itself shows no
/// decal; a layer-3 pixel does (`red >= 5`).
#[test]
fn decal_hides_behind_what_was_drawn_behind_the_level() {
    let Some(mut r) = renderer() else { return };
    add_page(&mut r, "decal", 4, 4, DECAL);
    let mut frame = decal_frame();
    for x0 in [683, 783] {
        frame.behind.push(creature([x0, 250, x0 + 100, 350]));
    }
    let before = frame.draw(&mut r, false);
    let after = frame.draw(&mut r, true);
    assert_eq!(
        rgb(&after, 830, 300),
        rgb(&before, 830, 300),
        "depth 10 behind"
    );
    assert_ne!(
        rgb(&after, 830, 400),
        rgb(&before, 830, 400),
        "depth 10 clear"
    );
    assert_ne!(
        rgb(&after, 730, 300),
        rgb(&before, 730, 300),
        "depth 3 behind"
    );
}

/// Layer 10 of red 101 (91 to 120 are the cloud-shadowed group): the cloud shadow is 0 at
/// full sun (`_light` 1 gives `clamp(.. - 4)`), so the terrain colour is the dark row's
/// (row 3) with the screen blend a quarter in, the texel's alpha is not reduced, and the
/// vertex alpha 0.5 blends the result with the level colour.
#[test]
fn decal_in_cloud_shadow_uses_the_dark_row() {
    let Some(mut r) = renderer() else { return };
    add_page(&mut r, "decal", 4, 4, DECAL);
    let mut frame = decal_frame();
    frame.over = vec![quad(
        "decal",
        [600, 200, 1100, 500],
        [0.0, 1.0, 0.0, 128.0 / 255.0],
        Shader::Decal,
        Layer::Foreground,
    )];
    let before = frame.draw(&mut r, false);
    let after = frame.draw(&mut r, true);
    let m = DECAL.map(|v| f32::from(v) / 255.0);
    let colour = mixed(unit([60, 10, 160]), [m[0], m[1], m[2]], 0.0);
    let under = unit(rgb(&before, 1030, 300));
    let a = 128.0 / 255.0;
    assert_near(rgb(&after, 1030, 300), over(colour, a, under), (1030, 300));
}

/// The rainbow gradient: row `k` of 64 counted from the bottom (`v` = `k / 64`) is
/// `(4k, 255 - 4k, 128)`; image rows are stored top first.
fn gradient() -> PngImage {
    PngImage::from_pixels(
        64,
        64,
        (0..64u32)
            .flat_map(|row| {
                let k = 63 - row;
                (0..64).map(move |_| [(4 * k) as u8, (255 - 4 * k) as u8, 128, 255])
            })
            .collect(),
    )
}

/// `tex2D(_MainTex, (0.5, d))` of [`gradient`]: bilinear, repeating.
fn gradient_at(d: f32) -> [f32; 3] {
    let p = d * 64.0 - 0.5;
    let (i0, f) = (p.floor(), p - p.floor());
    let [a, b] = [i0, i0 + 1.0].map(|i| {
        let k = ((i as i32).rem_euclid(64)) as f32;
        [4.0 * k / 255.0, (255.0 - 4.0 * k) / 255.0, 128.0 / 255.0]
    });
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * f)
}

/// A rainbow of half-side 200 about frame pixel (900, 400), inner ring 0.5 (128/255),
/// over a level whose columns left of 900 are sky, 900 to 1060 layer 25 (red 26) and
/// the rest layer 3 (red 4).
fn rainbow_frame(alpha: f32) -> Frame {
    let level = level(|x, _| match x {
        ..900 => [255, 255, 255],
        900..1060 => [26, 0, 0],
        _ => [4, 0, 0],
    });
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let centre = stage(900, 400);
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let colour = SpriteColor::from([128.0 / 255.0, 0.0, 0.0, alpha]);
    Frame {
        level,
        palette: palette([30, 60, 90], &[]),
        noise: 100,
        behind: Vec::new(),
        over: vec![DrawItem::ElementMesh(ElementMesh {
            element: Cow::Borrowed("rainbow"),
            mesh: SpriteMesh {
                index: 0,
                vertices: corners
                    .iter()
                    .map(|&(x, y)| Vec2::new(centre[0] + x * 200.0, centre[1] + y * 200.0))
                    .collect(),
                uvs: uvs.iter().map(|&[u, v]| Vec2::new(u, v)).collect(),
                triangles: Cow::Borrowed(&[[0, 1, 2], [0, 2, 3]]),
                color: SpriteColor::WHITE,
                vertex_colors: vec![colour; 4],
                shader: Shader::Rainbow,
                layer: Layer::GrabShaders,
            },
        })],
        layer: Layer::GrabShaders,
    }
}

/// `Rainbow.shader` on the frame without it, at a pixel `dx` right of the centre on the
/// centre row, over a level of depth factor `dp`.
fn rainbow(before: [f32; 3], dx: f32, dp: f32) -> [f32; 3] {
    let inner = 128.0_f32 / 255.0;
    let dst = dx / 200.0;
    let d = ((dst - inner) / (1.0 - inner)).clamp(0.0, 1.0);
    // `_NoiseTex2` is 1 and `_NoiseTex` 100/255: both noise taps are constants.
    let mut n: f32 = 0.5 + 0.5 * ((3.0_f32 + 100.0 / 255.0) * GAME_TAU * 2.5).sin();
    n = n * 3.0 - (n * 3.0).floor();
    n = 0.5 + 0.5 * n;
    assert!(n <= 0.99995, "a glint would change the maths");
    let mut weight = n * ((d * GAME_PI).sin() * 1.5).clamp(0.0, 1.0);
    weight *= ((dp - 0.2) * 1.5).max(0.0);
    let col = gradient_at(d);
    std::array::from_fn(|c| 1.0 - (1.0 - before[c]) * (1.0 - col[c] * weight))
}

/// Over a layer-25 pixel (depth factor `(int)(26/255 * 254) % 30 / 30` = 25/30) and over
/// sky (1) the ring screens its gradient onto the frame; over layer 3 (factor 0.1, under
/// 0.2) it adds nothing, and outside the ring (under the inner radius, or past the
/// inscribed circle) the frame is untouched.
#[test]
fn rainbow_screens_its_ring_by_level_depth() {
    let Some(mut r) = renderer() else { return };
    r.add_atlas_page(Atlas::single("rainbow", 64, 64), &gradient());
    let frame = rainbow_frame(1.0);
    let before = frame.draw(&mut r, false);
    let after = frame.draw(&mut r, true);
    // Centre row 400, `dx` px from the quad's centre (frame pixel 900's centre).
    for (x, dx, dp) in [
        (1020, 120.0, 25.0 / 30.0),
        (1050, 150.0, 25.0 / 30.0),
        (780, 120.0, 1.0),
        (720, 180.0, 1.0),
    ] {
        let b = unit(rgb(&before, x, 400));
        assert_near(rgb(&after, x, 400), rainbow(b, dx, dp), (x, 400));
        assert_ne!(rgb(&after, x, 400), rgb(&before, x, 400));
    }
    // Layer 3 (1090), inside the inner radius (850, 1000: exactly 100 px < 0.502 * 200),
    // past the circle (1120) and off the quad (row 650): nothing.
    for (x, y) in [
        (1090, 400),
        (850, 400),
        (1000, 400),
        (1120, 400),
        (900, 650),
    ] {
        assert_eq!(rgb(&after, x, y), rgb(&before, x, y), "({x}, {y})");
    }
}

/// The vertex alpha is the output alpha and scales the screening (here the maths is
/// `1 - (1 - grab)(1 - col * w)` at alpha 0.5, blended over the frame): half way.
#[test]
fn rainbow_alpha_blends_it_over_the_frame() {
    let Some(mut r) = renderer() else { return };
    r.add_atlas_page(Atlas::single("rainbow", 64, 64), &gradient());
    let full = rainbow_frame(1.0);
    let half = rainbow_frame(0.5);
    let before = full.draw(&mut r, false);
    let on = full.draw(&mut r, true);
    let faded = half.draw(&mut r, true);
    for c in 0..3 {
        let (b, f, h) = (
            f32::from(rgb(&before, 950, 400)[c]),
            f32::from(rgb(&on, 950, 400)[c]),
            f32::from(rgb(&faded, 950, 400)[c]),
        );
        assert!(
            (h - (b + (f - b) * 0.5)).abs() <= 1.5,
            "channel {c}: {b} {f} {h}"
        );
    }
}
