//! Headless GPU checks of the full-screen effect shaders (`full_screen.wgsl`), the
//! `DeathFall` shader (`death_fall.wgsl`), `WaterSplash` and `WaterLight`
//! (`sprite.wgsl`), here over
//! the whole screen, at the game's
//! 1366x768: each frame is drawn without the effect, then with it, and pixels are
//! compared with the game's shader maths applied to the frame without it (bilinear
//! taps, as the grab is read) and with values worked out by hand from the shader
//! sources. Skipped without a Vulkan adapter.

use rw_assets::{NoiseTexture, PaletteImage, PaletteTexture, RainMask, Room};
use rw_graphics::{Atlas, Color as SpriteColor, DrawItem, Shader, Sprite};

use crate::LevelParams;

use super::*;

/// The shaders' `6.28`, not 2π.
#[allow(clippy::approx_constant, reason = "the game's shader uses 6.28")]
const GAME_TAU: f32 = 6.28;

const W: u32 = 1366;
const H: u32 = 768;

const JSON: &str = r#"{"frames": {
  "RainWorld_White.png": {"frame": {"x": 0, "y": 0, "w": 16, "h": 16}, "rotated": false,
            "trimmed": false, "spriteSourceSize": {"x": 0, "y": 0, "w": 16, "h": 16},
            "sourceSize": {"w": 16, "h": 16}}},
  "meta": {"image": "t.png", "size": {"w": 16, "h": 16}}}"#;

/// A frame scene: the level image (one code per pixel), palette texels by runtime row,
/// sprites before the level and over it, and a full-screen effect.
struct Scene {
    level: PngImage,
    palette: PaletteTexture,
    behind: Vec<DrawItem>,
    over: Vec<DrawItem>,
    rain: f32,
    water_level: f32,
    /// Every `_NoiseTex` texel's red.
    noise: u8,
    rain_mask: Option<RainMask>,
}

/// A rectangle of frame pixels, y down: `[x0, y0, x1, y1)`.
#[derive(Clone, Copy)]
struct Rect([u32; 4]);

impl Rect {
    fn contains(self, x: u32, y: u32) -> bool {
        let [x0, y0, x1, y1] = self.0;
        x >= x0 && x < x1 && y >= y0 && y < y1
    }

    /// A `Futile_White` quad over these frame pixels (stage = frame + (1, 1) from the
    /// bottom: screen pixel centre minus Futile's (-0.5, 0.5) offset).
    fn quad(self, color: SpriteColor, layer: Layer) -> DrawItem {
        let [x0, y0, x1, y1] = self.0.map(|v| v as f32);
        DrawItem::Sprite(Sprite {
            index: 0,
            element: SpriteAtlas::FUTILE_WHITE,
            pos: Vec2::new(x0 + 0.5, H as f32 - y1 + 0.5),
            rotation: 0.0,
            scale: Vec2::new((x1 - x0) / 16.0, (y1 - y0) / 16.0),
            anchor: Vec2::ZERO,
            color,
            shader: Shader::Basic,
            layer,
        })
    }
}

/// The frame before the effect, read the way the effect reads it.
struct Before(FrameImage);

impl Before {
    fn px(&self, x: i64, y: i64) -> [f32; 4] {
        let x = x.clamp(0, i64::from(W) - 1) as u32;
        let y = y.clamp(0, i64::from(H) - 1) as u32;
        let i = ((y * W + x) * 4) as usize;
        let p = &self.0.rgba()[i..i + 4];
        [p[0], p[1], p[2], 255].map(|c| f32::from(c) / 255.0)
    }

    /// `tex2D(_GrabTexture, scr)`: bilinear at screen uv `scr` (y up), clamped.
    fn grab(&self, scr: [f32; 2]) -> [f32; 4] {
        let fx = scr[0] * W as f32 - 0.5;
        let fy = (1.0 - scr[1]) * H as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (a, b) = (self.px(x0, y0), self.px(x0 + 1, y0));
        let (c, d) = (self.px(x0, y0 + 1), self.px(x0 + 1, y0 + 1));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bottom = c[i] + (d[i] - c[i]) * tx;
            top + (bottom - top) * ty
        })
    }
}

/// Frame pixel `(x, y)` (y down): its `scrPos` (y up) and the effect quad's `i.uv`.
fn scr(x: u32, y: u32) -> [f32; 2] {
    [
        (x as f32 + 0.5) / W as f32,
        (H as f32 - y as f32 - 0.5) / H as f32,
    ]
}

fn quad_uv(x: u32, y: u32) -> [f32; 2] {
    [
        (x as f32 + 1.0) / W as f32,
        (H as f32 - y as f32 - 1.0) / H as f32,
    ]
}

fn hor() -> f32 {
    H as f32 / W as f32
}

fn unit(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| f32::from(v) / 255.0)
}

fn byte(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A palette with every texel `fill` and `(x, row, colour)` overrides (runtime rows).
fn palette(fill: [u8; 3], texels: &[(usize, usize, [u8; 3])]) -> PaletteTexture {
    let mut pixels = vec![[fill[0], fill[1], fill[2], 255]; 512];
    for &(x, row, [r, g, b]) in texels {
        pixels[(7 - row) * 32 + x] = [r, g, b, 255];
    }
    PaletteTexture::blend(
        &PaletteImage::from_image(&PngImage::from_pixels(32, 16, pixels)).unwrap(),
        None,
        0.0,
    )
}

/// A level image of the screen's size with code `f(x, png_row)`.
fn level(f: impl Fn(u32, u32) -> [u8; 3]) -> PngImage {
    PngImage::from_pixels(
        W as usize,
        H as usize,
        (0..W * H)
            .map(|i| {
                let [r, g, b] = f(i % W, i / W);
                [r, g, b, 255]
            })
            .collect(),
    )
}

impl Scene {
    fn new(level: PngImage, palette: PaletteTexture) -> Self {
        Self {
            level,
            palette,
            behind: Vec::new(),
            over: Vec::new(),
            rain: 0.0,
            water_level: 1.0,
            noise: 128,
            rain_mask: None,
        }
    }

    /// The frame with `effect` (shader, alpha) over it, if any.
    fn draw(&self, r: &mut WgpuRenderer, effect: Option<(Shader, f32)>) -> FrameImage {
        let effect = effect.map(|(shader, alpha)| (shader, SpriteColor::WHITE.with_alpha(alpha)));
        self.draw_coloured(r, effect)
    }

    /// The frame with `effect` (shader, sprite colour with alpha) over it, if any.
    fn draw_coloured(
        &self,
        r: &mut WgpuRenderer,
        effect: Option<(Shader, SpriteColor)>,
    ) -> FrameImage {
        let n = self.noise;
        let noise =
            NoiseTexture::from_image(&PngImage::from_pixels(64, 64, vec![[n, 0, 0, 255]; 4096]))
                .unwrap();
        r.set_level(&LevelGraphics {
            camera_pos: [0.0, 0.0],
            image: &self.level,
            palette: &self.palette,
            noise: &noise,
            params: LevelParams::DEFAULT,
            rain_mask: self.rain_mask.as_ref(),
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
            rain: self.rain,
            light: 1.0,
            water_level: self.water_level,
        });
        if !self.over.is_empty() {
            let list = DrawList {
                items: self.over.clone(),
            };
            r.draw_sprites(&list, Layer::Foreground, [0.0, 0.0]);
        }
        if let Some((shader, color)) = effect {
            let layer = if shader == Shader::Fog {
                Layer::Foreground
            } else {
                Layer::Bloom
            };
            let list = DrawList {
                items: vec![DrawItem::Sprite(Sprite {
                    index: 0,
                    element: SpriteAtlas::FUTILE_WHITE,
                    pos: Vec2::ZERO,
                    rotation: 0.0,
                    scale: Vec2::new(W as f32 / 16.0, H as f32 / 16.0),
                    anchor: Vec2::ZERO,
                    color,
                    shader,
                    layer,
                })],
            };
            r.draw_sprites(&list, layer, [0.0, 0.0]);
        }
        r.render_image().unwrap()
    }

    /// The level code under frame pixel `(x, y)` moved by `offset` in level uv
    /// (`textCoord`): image column `x + 1`, row `y` at no offset. With the view at a
    /// whole pixel (the game's is 0.02 px off one) an unmoved coordinate falls on a
    /// texel edge, so tests keep such lookups off region edges.
    fn code(&self, x: u32, y: u32, offset: [f32; 2]) -> [u8; 4] {
        let s = scr(x, y);
        let tc = [
            (s[0] * W as f32 + 0.5) / W as f32 + offset[0],
            (s[1] * H as f32 - 0.5) / H as f32 + offset[1],
        ];
        let tx = ((tc[0] * W as f32).floor() as i64).clamp(0, i64::from(W) - 1) as usize;
        let ty = ((tc[1] * H as f32).floor() as i64).clamp(0, i64::from(H) - 1) as usize;
        self.level.pixel(tx, H as usize - 1 - ty)
    }

    /// `IsLevelLit` at frame pixel `(x, y)` moved by `offset`; `behind` is whether
    /// something was drawn before the level at `(x, y)` itself.
    fn lit(&self, x: u32, y: u32, offset: [f32; 2], behind: bool) -> bool {
        let red = i32::from(self.code(x, y, offset)[0]);
        red > 90 && red < 255 && !((red - 1) % 30 > 5 && behind)
    }
}

fn renderer() -> Option<WgpuRenderer> {
    let mut r = match WgpuRenderer::headless((W, H)) {
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
    Some(r)
}

fn rgb(image: &FrameImage, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * image.width() + x) * 4) as usize;
    let p = &image.rgba()[i..i + 3];
    [p[0], p[1], p[2]]
}

/// Within one 8-bit step per channel (the GPU's bilinear weights are not exact).
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

/// `Bloom.shader`'s fragment on the frame without it.
fn bloom(before: &Before, x: u32, y: u32, alpha: f32) -> [f32; 3] {
    let s = scr(x, y);
    let mut sum = [0.0, 0.0, 0.0, 1.0];
    let mut div = 0.0;
    let mut coef = 1.0;
    for j in 1..=4 {
        let d = j as f32 * 0.0012;
        coef *= 0.92;
        for o in [[0.0, d], [-d * hor(), 0.0], [d * hor(), 0.0], [0.0, -d]] {
            let g = before.grab([s[0] + o[0], s[1] + o[1]]);
            for c in 0..4 {
                sum[c] += g[c] * coef;
            }
        }
        div += 4.0 * coef;
    }
    let uv = quad_uv(x, y);
    let vignette = 1.0 + (0.5 - 1.0) * ((uv[0] - 0.5).hypot(uv[1] - 0.5) * 2.0);
    let k = alpha * vignette / div;
    let g = before.grab(s);
    std::array::from_fn(|c| g[c].max(sum[c] * k) + (sum[c] * k).powf(1.5))
}

/// `Bloom`: on a uniform frame of 0.4 at the screen's centre (vignette 1) every tap is
/// 0.4, so `max(0.4, 0.4 * 0.6) + (0.24)^1.5` = 0.5176 (132); near a white quad the blur
/// brightens the grey, and the quad stays white.
#[test]
fn bloom_adds_its_blur() {
    let Some(mut r) = renderer() else { return };
    let mut scene = Scene::new(level(|_, _| [255, 255, 255]), palette([102, 102, 102], &[]));
    let quad = Rect([300, 300, 400, 400]);
    scene
        .over
        .push(quad.quad(SpriteColor::rgb(1.0, 1.0, 1.0), Layer::Foreground));
    let before = Before(scene.draw(&mut r, None));
    assert_eq!(rgb(&before.0, 683, 200), [102, 102, 102]);
    let after = scene.draw(&mut r, Some((Shader::Bloom, 0.6)));
    assert_near(rgb(&after, 683, 384), [0.5176; 3], (683, 384));
    for (x, y) in [
        (683, 384),
        (5, 5),
        (1300, 700),
        (298, 350),
        (402, 350),
        (350, 297),
        (350, 340),
    ] {
        assert_near(rgb(&after, x, y), bloom(&before, x, y, 0.6), (x, y));
    }
}

/// `SkyBloom.shader`'s fragment; `sky` is the palette's texel (0, 7).
fn sky_bloom(before: &Before, x: u32, y: u32, alpha: f32, sky: [f32; 3]) -> [f32; 3] {
    let s = scr(x, y);
    let amount = (1.0 - (s[0] - 0.2).hypot(s[1] - 0.2))
        .clamp(0.0, 1.0)
        .sqrt()
        * alpha;
    let blur = 0.0018 * amount;
    let mut texcol = [0.0, 0.0, 0.0, 1.0];
    let mut div = 1.0;
    let mut coef = 1.0;
    for j in 1..=5 {
        let d = j as f32 * blur;
        coef *= 0.62;
        for o in [[-1.0, 1.0], [1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]] {
            let g = before.grab([s[0] + o[0] * d * hor(), s[1] + o[1] * d]);
            if (0..3).all(|c| byte(g[c]) == byte(sky[c])) {
                for c in 0..4 {
                    texcol[c] += g[c] * coef;
                }
                div += coef;
            }
        }
    }
    let mut g = before.grab(s);
    div *= 0.75;
    for c in 0..3 {
        let t = (g[c] + texcol[c]) / div;
        if t > g[c] {
            g[c] += (t - g[c]) * amount;
        }
    }
    [g[0], g[1], g[2]]
}

/// `SkyBloom`: all-sky taps give `texcol = sky / 0.75`, so at (0.2, 0.2) of the screen
/// (amount = alpha = 1) the sky 0.4 becomes 0.5333 (136); at the far corner (amount 0)
/// it stays. A pixel of another colour with no sky tap still rises to `b / 0.75` by the
/// amount (the centre's own weight over `0.75`).
#[test]
fn sky_bloom_spreads_the_sky_colour() {
    let Some(mut r) = renderer() else { return };
    let sky = [102, 102, 102];
    let mut scene = Scene::new(
        level(|_, _| [255, 255, 255]),
        palette([0, 0, 0], &[(0, 7, sky)]),
    );
    let quad = Rect([800, 100, 1000, 300]);
    scene
        .over
        .push(quad.quad(SpriteColor::rgb(0.2, 0.3, 0.1), Layer::Foreground));
    let before = Before(scene.draw(&mut r, None));
    let after = scene.draw(&mut r, Some((Shader::SkyBloom, 1.0)));
    let (x, y) = (273, 614);
    assert_near(rgb(&after, x, y), [0.5333; 3], (x, y));
    assert_eq!(rgb(&after, 1360, 5), sky);
    let sky = [0.4; 3];
    for (x, y) in [
        (273, 614),
        (683, 384),
        (1300, 50),
        (900, 200),
        (798, 200),
        (1002, 250),
        (900, 98),
    ] {
        assert_near(
            rgb(&after, x, y),
            sky_bloom(&before, x, y, 1.0, sky),
            (x, y),
        );
    }
}

/// `LightBloom.shader`'s fragment; `behind` is whether something was drawn before the
/// level at the pixel.
fn light_bloom(
    scene: &Scene,
    before: &Before,
    x: u32,
    y: u32,
    alpha: f32,
    behind: bool,
) -> [f32; 3] {
    let s = scr(x, y);
    let mut texcol = [0.0, 0.0, 0.0, 1.0];
    let mut div = 0.0;
    let mut coef = 1.0;
    for j in 1..=4 {
        let d = j as f32 * 0.0012;
        coef *= 0.82;
        for o in [[0.0, d], [-d * hor(), 0.0], [d * hor(), 0.0], [0.0, -d]] {
            if scene.lit(x, y, o, behind) {
                let g = before.grab([s[0] + o[0], s[1] + o[1]]);
                for c in 0..4 {
                    texcol[c] += g[c] * coef;
                }
            }
        }
        div += 4.0 * coef;
    }
    let g = before.grab(s);
    div *= 0.5;
    std::array::from_fn(|c| g[c] + texcol[c] / div * alpha)
}

/// `LightBloom`: where every tap is a sunlit level pixel (red 101) of colour 0.4 the
/// sum is `0.4 * div`, over `div / 2`, times alpha 0.25: 0.4 + 0.2 (153); unlit pixels
/// (red 11) add nothing; a creature drawn behind the level at a deep (layer 10) pixel
/// masks every tap of that pixel. `CreatureMask` compares the vector `deep * grab ==
/// 0` and returns it as a `bool`: HLSL keeps its first component, the grab's red, so a
/// creature without red masks nothing (INFERRED from HLSL's implicit truncation).
#[test]
fn light_bloom_adds_the_lit_level_around() {
    let Some(mut r) = renderer() else { return };
    let mut scene = Scene::new(
        level(|x, _| if x < 683 { [101, 0, 0] } else { [11, 0, 0] }),
        palette([102, 102, 102], &[]),
    );
    let creature = Rect([200, 200, 300, 300]);
    scene
        .behind
        .push(creature.quad(SpriteColor::rgb(0.5, 0.25, 0.0), Layer::Items));
    let green = Rect([200, 500, 300, 600]);
    scene
        .behind
        .push(green.quad(SpriteColor::rgb(0.0, 0.5, 0.0), Layer::Items));
    let before = Before(scene.draw(&mut r, None));
    assert_eq!(
        rgb(&before.0, 250, 250),
        [127, 64, 0],
        "the creature shows through"
    );
    let after = scene.draw(&mut r, Some((Shader::LightBloom, 0.25)));
    assert_near(rgb(&after, 400, 500), [0.6; 3], (400, 500));
    assert_eq!(rgb(&after, 1000, 500), [102, 102, 102]);
    assert_eq!(rgb(&after, 250, 250), [127, 64, 0], "masked");
    assert_ne!(rgb(&after, 250, 550), [0, 127, 0], "not masked");
    for (x, y) in [
        (400, 500),
        (1000, 500),
        (680, 400),
        (681, 400),
        (684, 400),
        (690, 100),
        (302, 250),
        (250, 550),
    ] {
        let behind = creature.contains(x, y);
        assert_near(
            rgb(&after, x, y),
            light_bloom(&scene, &before, x, y, 0.25, behind),
            (x, y),
        );
    }
}

/// `LightAndSkyBloom.shader`'s fragment.
fn light_and_sky_bloom(
    scene: &Scene,
    before: &Before,
    x: u32,
    y: u32,
    alpha: f32,
    sky: [f32; 3],
) -> [f32; 3] {
    let s = scr(x, y);
    let amount = (1.0 - (s[0] - 0.2).hypot(s[1] - 0.2))
        .clamp(0.0, 1.0)
        .sqrt()
        * alpha;
    let mid = before.grab(s);
    let mut texcol = [0.0, 0.0, 0.0, 1.0];
    let mut add = [0.0; 4];
    let mut div = 1.0;
    let mut coef = 1.0;
    let mut coef2 = 1.0;
    for j in 1..=6 {
        let d = j as f32 * 0.0018;
        coef *= 0.62;
        for o in [[-1.0, 1.0], [1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]] {
            let g = before.grab([s[0] + o[0] * d * hor(), s[1] + o[1] * d]);
            if (0..3).all(|c| byte(g[c]) == byte(sky[c])) {
                for c in 0..4 {
                    texcol[c] += g[c] * coef;
                }
                div += coef;
            }
            if scene.lit(x, y, [o[0] * d * hor(), -o[1] * d], false) {
                for c in 0..3 {
                    add[c] += mid[c].max(g[c]) * coef2;
                }
                add[3] += coef2;
            }
        }
        coef2 *= 0.9;
    }
    let mut g = before.grab(s);
    for c in 0..4 {
        g[c] += texcol[c] * 0.5;
    }
    div *= 0.75;
    let cut = 0.7 + (0.35 - 0.7) * amount;
    std::array::from_fn(|c| {
        let t = (g[c] + texcol[c]) / div;
        let mut out = g[c].max(t);
        let a = (add[c] - cut).max(0.0) * 0.5;
        if a > out {
            out += (a - out) * amount;
        }
        out
    })
}

/// `LightAndSkyBloom`: all-sky taps around a sky pixel give `texcol = sky * 6.1556`
/// (`4 * sum(0.62^j)`, j = 1..6), half of it added to the grab: `sky * 4.0778`, so the
/// sky (10, 20, 30) becomes (40.8, 81.6, 122.3) whatever the alpha. All-lit taps of 0.102 sum
/// `4 * sum(0.9^k) = 18.742` times 0.102: at (0.2, 0.2) of the screen (amount 1) the
/// cut is 0.35: `(1.9110 - 0.35) * 0.5` = 0.7805 (199); at the far corner (amount 0)
/// the pixel is the grab over 0.75: 0.1359 (35).
#[test]
fn light_and_sky_bloom_brightens_sky_and_lit_level() {
    let Some(mut r) = renderer() else { return };
    let sky: [u8; 3] = [10, 20, 30];
    let scene = Scene::new(
        level(|x, _| {
            if x < 683 {
                [255, 255, 255]
            } else {
                [101, 0, 0]
            }
        }),
        palette([26, 26, 26], &[(0, 7, sky)]),
    );
    let before = Before(scene.draw(&mut r, None));
    let after = scene.draw(&mut r, Some((Shader::LightAndSkyBloom, 1.0)));
    assert_near(
        rgb(&after, 300, 384),
        [0.15991, 0.31982, 0.47973],
        (300, 384),
    );
    let lit = Scene::new(
        level(|_, _| [101, 0, 0]),
        palette([26, 26, 26], &[(0, 7, sky)]),
    );
    let lit_after = lit.draw(&mut r, Some((Shader::LightAndSkyBloom, 1.0)));
    assert_near(rgb(&lit_after, 273, 614), [0.7805; 3], (273, 614));
    assert_near(rgb(&lit_after, 1360, 5), [0.1359; 3], (1360, 5));
    let sky = unit(sky);
    for (x, y) in [
        (300, 384),
        (273, 614),
        (676, 400),
        (681, 400),
        (686, 400),
        (695, 600),
        (1300, 100),
    ] {
        assert_near(
            rgb(&after, x, y),
            light_and_sky_bloom(&scene, &before, x, y, 1.0, sky),
            (x, y),
        );
    }
}

/// `Fog.shader`'s fragment as (rgb, alpha) before blending; `noise` is the constant
/// noise texel, `pal2`/`sky` the palette's texels (0, 2) and (0, 7).
#[allow(clippy::too_many_arguments)]
fn fog(
    scene: &Scene,
    x: u32,
    y: u32,
    alpha: f32,
    behind: bool,
    noise: f32,
    pal2: [f32; 3],
    sky: [f32; 3],
) -> Option<([f32; 3], f32)> {
    let s = scr(x, y);
    let uv = quad_uv(x, y);
    let rain = scene.rain;
    let amount = ((s[1] - ((1.0 - scene.water_level) - 0.11)) * 3.0).clamp(0.0, 1.0);
    let mut fog2 = 0.5 + 0.5 * ((noise + rain * 0.04 + uv[0]) * GAME_TAU).sin();
    let mut fog1 = 0.5 + 0.5 * ((noise + rain * 0.05 + uv[1]) * GAME_TAU).sin();
    fog1 += (fog2 - fog1) * 0.5;
    let code = scene.code(x, y, [0.0, 0.0]);
    let mut dp = ((f32::from(code[0]) - 1.0) % 30.0) / 30.0;
    if code[..3] == [255, 255, 255] {
        dp = 1.0;
    }
    if dp > 6.0 / 30.0 && behind {
        dp = 6.0 / 30.0;
    }
    if dp >= 0.999_999_97 {
        fog2 = 0.5 + 0.5 * ((noise + rain * 0.14 - uv[0]) * GAME_TAU).sin();
        fog2 *= (1.0 - uv[0].hypot(uv[1] - 0.9)).clamp(0.0, 1.0);
        fog2 = fog2.powf(0.2) * amount * (1.0 - fog1.powf(1.5)) * alpha;
        if fog2 > 0.5 {
            let t = if fog2 > 0.6 { 0.25 } else { 0.1 };
            return Some((sky.map(|c| c + (1.0 - c) * t), 1.0));
        }
    }
    fog1 = fog1.powf(3.0) * alpha;
    fog1 = fog1.powf(1.0 + (1.0 - dp.powf(0.1)) * 30.0);
    fog1 = (fog1 - (1.0 - amount)).max(0.0).powf(0.2);
    if fog1 > 0.1 {
        let t = 0.5 + 0.5 * dp;
        let rgb = std::array::from_fn(|c| pal2[c] + (sky[c] - pal2[c]) * t);
        return Some((rgb, if fog1 > 0.5 { 0.6 } else { 0.2 }));
    }
    None
}

/// `Fog` over sky (left) and layer-2 terrain (right, red 3), with noise 128: drawn with
/// `SrcAlpha OneMinusSrcAlpha` over the frame; below the water line (`_waterLevel` 0.5:
/// the lower half) it thins out. Light patches over the sky are opaque.
#[test]
fn fog_bands_over_sky_and_terrain() {
    let Some(mut r) = renderer() else { return };
    let (pal2, sky, base) = ([200, 40, 40], [40, 40, 200], [90, 90, 90]);
    let mut scene = Scene::new(
        level(|x, _| if x < 683 { [255, 255, 255] } else { [3, 0, 0] }),
        palette(base, &[(0, 2, pal2), (0, 7, sky)]),
    );
    scene.rain = 1.3;
    scene.water_level = 0.5;
    let creature = Rect([100, 100, 200, 200]);
    scene
        .behind
        .push(creature.quad(SpriteColor::rgb(0.0, 0.5, 0.0), Layer::Items));
    let before = Before(scene.draw(&mut r, None));
    let after = scene.draw(&mut r, Some((Shader::Fog, 0.9)));
    let mut patches = 0;
    let mut fogged = 0;
    for y in (3..H).step_by(29) {
        for x in (5..W).step_by(37) {
            if (x as i64 - 683).abs() < 3 {
                continue;
            }
            let behind = creature.contains(x, y);
            let below = before.px(i64::from(x), i64::from(y));
            let want = match fog(
                &scene,
                x,
                y,
                0.9,
                behind,
                128.0 / 255.0,
                unit(pal2),
                unit(sky),
            ) {
                Some((rgb, a)) => {
                    if a == 1.0 {
                        patches += 1;
                    } else {
                        fogged += 1;
                    }
                    std::array::from_fn(|c| rgb[c] * a + below[c] * (1.0 - a))
                }
                None => [below[0], below[1], below[2]],
            };
            assert_near(rgb(&after, x, y), want, (x, y));
        }
    }
    assert!(
        patches > 0 && fogged > 0,
        "{patches} patches, {fogged} fogged"
    );
}

/// `DeathFall.shader`'s fragment as (rgb, alpha) before blending; `behind` is whether
/// something was drawn before the level at the pixel.
fn death_fall(
    scene: &Scene,
    before: &Before,
    x: u32,
    y: u32,
    alpha: f32,
    behind: bool,
) -> Option<([f32; 3], f32)> {
    let red = scene.code(x, y, [0.0, 0.0])[0];
    let depth = if red == 255 {
        1.0
    } else {
        ((f32::from(red) - 1.0) % 30.0) / 30.0
    };
    if depth > 6.0 / 30.0 && behind {
        return None;
    }
    let g = before.grab(scr(x, y));
    let dist = ((depth - 0.1) / 0.2).clamp(0.0, 1.0);
    let t = ((quad_uv(x, y)[1] - 1.0) / -1.5).clamp(0.0, 1.0);
    let amount = dist.powf(0.3) * t * t * (3.0 - 2.0 * t);
    let l = g[0].max(g[1]).max(g[2]);
    let edge = 0.2 + (scene.rain * 10.0).sin() * 0.1;
    let k = ((l - edge) / -edge).clamp(0.0, 1.0);
    let rgb = std::array::from_fn(|c| {
        let dark = g[c] * l * 0.2;
        dark + ((1.0 - g[c]) * 0.1 * alpha - dark) * k
    });
    Some((rgb, amount / (1.0 - alpha * 0.1) * alpha))
}

/// `DeathFall` over columns of layer 0 (red 1: no effect), layer 6 (red 7: half the
/// depth ramp), layer 9 (red 10) and sky, under a bright quad and over a creature drawn
/// behind the sky (left alone): bright pixels go to `c * l * 0.2`, dark ones (palette
/// 0.12, under `0.2 + 0.1 sin(10 _RAIN)`) toward `(1 - c) * 0.1 * a`, more opaque toward
/// the bottom.
#[test]
fn death_fall_darkens_far_layers_toward_the_bottom() {
    let Some(mut r) = renderer() else { return };
    let mut scene = Scene::new(
        level(|x, _| match x {
            0..300 => [1, 0, 0],
            300..600 => [7, 0, 0],
            600..900 => [10, 0, 0],
            _ => [255, 255, 255],
        }),
        palette([30, 30, 30], &[]),
    );
    scene.rain = 0.7;
    let bright = Rect([650, 500, 850, 700]);
    scene
        .over
        .push(bright.quad(SpriteColor::rgb(0.9, 0.6, 0.3), Layer::Foreground));
    let creature = Rect([1000, 600, 1200, 700]);
    scene
        .behind
        .push(creature.quad(SpriteColor::rgb(0.0, 0.5, 0.0), Layer::Items));
    let before = Before(scene.draw(&mut r, None));
    let after = scene.draw(&mut r, Some((Shader::DeathFall, 0.9)));
    let mut drawn = 0;
    for y in (3..H).step_by(23) {
        for x in (5..W).step_by(31) {
            if [300, 600, 900].iter().any(|&e| (x as i64 - e).abs() < 3) {
                continue;
            }
            let below = before.px(i64::from(x), i64::from(y));
            let behind = creature.contains(x, y);
            let want = match death_fall(&scene, &before, x, y, 0.9, behind) {
                Some((c, a)) => {
                    let a = a.clamp(0.0, 1.0);
                    if a > 0.0 {
                        drawn += 1;
                    }
                    std::array::from_fn(|i| c[i] * a + below[i] * (1.0 - a))
                }
                None => [below[0], below[1], below[2]],
            };
            assert_near(rgb(&after, x, y), want, (x, y));
        }
    }
    assert!(drawn > 0);
    assert_eq!(rgb(&after, 100, 700), rgb(&before.0, 100, 700), "layer 0");
    assert_eq!(
        rgb(&after, 1100, 650),
        rgb(&before.0, 1100, 650),
        "creature"
    );
}

/// `lerp` of each channel.
fn mix3(x: [f32; 3], y: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|c| x[c] + (y[c] - x[c]) * t)
}

/// `WaterSplash.shader`'s colour at a pixel where the quad shows (`h` over its cut),
/// with `dp` the level depth after the grab test, `grab` the pre-level frame there when
/// something was drawn, `proximity` the water line's, `a` and `foam` the sprite's alpha
/// and blue; `pal` the palette's fog, `waterColor1` and `waterColor2` (row 7, x 0, 4,
/// 5). `glint` is left out: at these pixels `0.2 * pow(0.5 + 0.5 sin((tc.x - 2 tc.y) /
/// 3), 7)` stays under 0.001.
fn water_splash(
    dp: f32,
    grab: Option<[f32; 3]>,
    proximity: f32,
    alpha_and_foam: (f32, bool),
    pal: [[f32; 3]; 3],
) -> [f32; 3] {
    let (a, foam) = alpha_and_foam;
    let [pal0, pal4, pal5] = pal;
    let mut col = mix3(pal5, pal4, dp);
    col = mix3(col, pal0, (1.0 - a) * 0.5 * dp * (1.0 - proximity));
    if foam {
        col = mix3(col, [1.0; 3], (0.3 + 0.5 * dp) * (1.0 - proximity));
    }
    match grab {
        Some(g) if dp >= 6.0 / 30.0 => mix3(col, g, 1.0 + (0.5 - 1.0) * proximity),
        _ => col,
    }
}

/// `WaterSplash` over the screen. Level: sky left of x 683, layer 2 (red 3) right of it.
/// Before the level: a background bubble's `(0, 1/255, 0)` at (200..400, 100..250) and
/// a red creature at (200..400, 450..650) and (800..1000, 450..650). `_waterLevel` 0.5:
/// `waterProximity` 0 above frame row 338, 1 below row 492. Uniform noise: the
/// distortion is one pixel down-left wherever the quad shows. Sprite 0 `(0, 0, 0, 1)`:
/// sky is `dp` 1 (`waterColor1`), the bubble turns it into `waterColor1` too, the
/// creature on sky gives `dp` 6/30 and its own colour (half by the water line), layer 2
/// is `dp` 2/30 whatever was behind. Alpha 0.5 mixes in the fog colour by
/// `0.25 * dp` away from the water line. Foam `(0, 0, 1, 1)` (noise 255 so its `h`
/// passes) is whitened by `0.3 + 0.5 dp`.
#[test]
fn water_splash_colours_by_depth_and_the_pre_level_grab() {
    let Some(mut r) = renderer() else { return };
    let (pal0, pal4, pal5) = ([100u8, 100, 100], [40, 60, 80], [10, 20, 30]);
    let mut scene = Scene::new(
        level(|x, _| if x < 683 { [255, 255, 255] } else { [3, 0, 0] }),
        palette([0, 0, 0], &[(0, 7, pal0), (4, 7, pal4), (5, 7, pal5)]),
    );
    scene.water_level = 0.5;
    let bubble = Rect([200, 100, 400, 250]);
    scene
        .behind
        .push(bubble.quad(SpriteColor::rgb(0.0, 1.0 / 255.0, 0.0), Layer::Items));
    let red = SpriteColor::rgb(0.8, 0.0, 0.0);
    for creature in [Rect([200, 450, 400, 650]), Rect([800, 450, 1000, 650])] {
        scene.behind.push(creature.quad(red, Layer::Items));
    }
    let pal = [unit(pal0), unit(pal4), unit(pal5)];
    let creature = Some([0.8, 0.0, 0.0]);
    let splash = SpriteColor::rgb(0.0, 0.0, 0.0);
    let after = scene.draw_coloured(&mut r, Some((Shader::WaterSplash, splash)));
    for (x, y, dp, grab, proximity) in [
        (300, 175, 1.0, None, 0.0),
        (500, 300, 1.0, None, 0.0),
        (300, 550, 6.0 / 30.0, creature, 1.0),
        (900, 200, 2.0 / 30.0, None, 0.0),
        (900, 550, 2.0 / 30.0, creature, 1.0),
    ] {
        let want = water_splash(dp, grab, proximity, (1.0, false), pal);
        assert_near(rgb(&after, x, y), want, (x, y));
    }
    assert_eq!(rgb(&after, 300, 175), pal4, "lit water");
    let half = splash.with_alpha(0.5);
    let after = scene.draw_coloured(&mut r, Some((Shader::WaterSplash, half)));
    for (x, y, dp) in [(500, 300, 1.0), (900, 300, 2.0 / 30.0)] {
        let want = water_splash(dp, None, 0.0, (0.5, false), pal);
        assert_near(rgb(&after, x, y), want, (x, y));
    }
    scene.noise = 255;
    let foam = SpriteColor::rgb(0.0, 0.0, 1.0);
    let after = scene.draw_coloured(&mut r, Some((Shader::WaterSplash, foam)));
    for (x, y, dp) in [(500, 300, 1.0), (900, 300, 2.0 / 30.0)] {
        let want = water_splash(dp, None, 0.0, (1.0, true), pal);
        assert_near(rgb(&after, x, y), want, (x, y));
    }
}

/// A 69 x 39 tile room (1380 x 780 px) of air, water at row 10, a solid ledge at
/// rows 20..=21 over tile columns 30..40: its mask is blue on rows 10..=30, and only
/// up to row 20 under the ledge.
fn water_light_room() -> Room {
    let (w, h) = (69, 39);
    let tiles: String = (0..w)
        .flat_map(|x| (0..h).rev().map(move |y| (x, y)))
        .map(|(x, y)| {
            if (30..40).contains(&x) && (20..=21).contains(&y) {
                "1|"
            } else {
                "0|"
            }
        })
        .collect();
    let text = format!(
        "Test\n{w}*{h}|10|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{tiles}\n"
    );
    Room::parse(&text).expect("test room parses")
}

impl Scene {
    /// `tex2D(_MainTex, ...)` of the room mask, bilinear and clamped, at room pixel `p`:
    /// blue.
    fn mask_blue(&self, p: [f32; 2]) -> f32 {
        let fx = p[0] / 20.0 - 0.5;
        let fy = p[1] / 20.0 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let [a, b, c, d] = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
            .map(|(dx, dy)| self.mask_texel_blue(x0 + dx, y0 + dy));
        let bottom = a + (b - a) * tx;
        let top = c + (d - c) * tx;
        bottom + (top - bottom) * ty
    }

    /// The room mask's blue at texel `(x, y)`, clamped to its edges.
    fn mask_texel_blue(&self, x: f32, y: f32) -> f32 {
        let mask = self.rain_mask.as_ref().expect("mask");
        let x = (x as i32).clamp(0, mask.width() - 1);
        let y = (y as i32).clamp(0, mask.height() - 1);
        f32::from(mask.texel(x, y)[2]) / 255.0
    }

    /// `WaterLight.shader`'s `col` at frame pixel `(x, y)` under uniform noise of 1
    /// (every noise tap is 1), or `None` where an early test leaves the pixel: the level
    /// depth under 2/30, a pixel deeper than 6/30 with something drawn behind it
    /// (`behind`), or sky. `uv_y` is the sprite's `i.uv.y` there.
    fn water_light_col(&self, x: u32, y: u32, uv_y: f32, behind: bool) -> Option<f32> {
        // `textCoord` floors the screen pixel: image column x, row H - y - 2 from the
        // bottom.
        let c = self.level.pixel(x as usize, y as usize + 1);
        let dist = ((f32::from(c[0]) - 1.0) % 30.0) / 30.0;
        if dist < 2.0 / 30.0 || (dist > 6.0 / 30.0 && behind) || c[..3] == [255, 255, 255] {
            return None;
        }
        let col1 = ((self.rain * 6.0 + 7.0) * GAME_TAU).sin() * 0.5 + 0.5;
        let col2 = ((self.rain * 2.7 + 6.33) * GAME_TAU).sin() * 0.5 + 0.5;
        let s = scr(x, y);
        let blue = self.mask_blue([s[0] * W as f32, s[1] * H as f32]);
        Some(col1.max(col2).powf(47.0) * (1.0 - uv_y) * blue)
    }
}

/// `WaterLight` over a level of layer 0 (red 1), layer 3 (red 4), layer 9 (red 10) and
/// sky columns, with creatures drawn behind layers 3 and 9, the room mask of
/// [`water_light_room`] and the sprite from stage y 100, 39 x 18.75 px tall. `_RAIN`
/// 0.042283 puts the first sine at its peak (`(6 r + 7) * 6.28` = pi/2 + 14 pi), so
/// `col = (1 - uv.y) * blue`: white at 0.05 where that reaches 0.1, on layers 3 and 9
/// over the mask's water rows, not over the creature behind layer 9, never on layer 0
/// or sky.
#[test]
fn water_light_shows_caustics_over_the_masked_water_rows() {
    let Some(mut r) = renderer() else { return };
    let mut scene = Scene::new(
        level(|x, _| match x {
            0..300 => [1, 0, 0],
            300..700 => [4, 0, 0],
            700..1000 => [10, 0, 0],
            _ => [255, 255, 255],
        }),
        palette([30, 30, 30], &[]),
    );
    scene.noise = 255;
    scene.rain = 0.042_283;
    let mask = RainMask::generate(&water_light_room());
    assert_eq!(mask.texel(5, 30)[2], 255);
    assert_eq!(mask.texel(35, 25)[2], 0, "under the ledge");
    scene.rain_mask = Some(mask);
    let creatures = [Rect([400, 300, 600, 400]), Rect([750, 300, 950, 400])];
    for c in creatures {
        scene
            .behind
            .push(c.quad(SpriteColor::rgb(0.5, 0.0, 0.0), Layer::Items));
    }
    let before = Before(scene.draw(&mut r, None));
    let (y0, height) = (100.0, 39.0 * 18.75);
    scene.over.push(DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(0.0, y0),
        rotation: 0.0,
        scale: Vec2::new(W as f32 / 16.0, height / 16.0),
        anchor: Vec2::ZERO,
        color: SpriteColor::WHITE,
        shader: Shader::WaterLight,
        layer: Layer::Foreground,
    }));
    let after = scene.draw(&mut r, None);
    let (mut lit, mut dark) = (0, 0);
    for y in (2..H - 2).step_by(7) {
        for x in (3..W).step_by(11) {
            if [300, 700, 1000].iter().any(|&e| (x as i64 - e).abs() < 3) {
                continue;
            }
            let below = before.px(i64::from(x), i64::from(y));
            let stage_y = H as f32 - y as f32 - 1.0;
            if stage_y < y0 {
                assert_eq!(rgb(&after, x, y), rgb(&before.0, x, y));
                continue;
            }
            let uv_y = (stage_y - y0) / height;
            let behind = creatures.iter().any(|c| c.contains(x, y));
            let col = scene.water_light_col(x, y, uv_y, behind);
            if col.is_some_and(|c| (c - 0.1).abs() < 0.01) {
                continue;
            }
            let shown = col.is_some_and(|c| c >= 0.1);
            let want = if shown {
                lit += 1;
                std::array::from_fn(|i| 1.0 * 0.05 + below[i] * 0.95)
            } else {
                dark += 1;
                [below[0], below[1], below[2]]
            };
            assert_near(rgb(&after, x, y), want, (x, y));
        }
    }
    eprintln!("water light: {lit} lit, {dark} left");
    assert!(lit > 100 && dark > 100, "{lit} lit, {dark} left");
    // Layer 3 over a creature drawn behind it still shows; layer 9 over one does not.
    let (x, y) = (500, 350);
    assert!(scene.water_light_col(x, y, 0.2, true).is_some());
    assert_ne!(rgb(&after, x, y), rgb(&before.0, x, y));
    assert!(scene.water_light_col(850, 350, 0.2, true).is_none());
    assert_eq!(rgb(&after, 850, 350), rgb(&before.0, 850, 350));
}
