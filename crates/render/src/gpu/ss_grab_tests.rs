//! Headless GPU checks of the screen-grab shaders (`ss_grab.wgsl`: `DeepProcessing` and
//! `GravityDisruptor`) at the game's 1366x768, against the fragments of
//! `DeepProcessing.shader` and `GravityDisruptor.shader` worked out per pixel on the CPU
//! (with `_NoiseTex` read at the mip level the 2x2 pixel block's derivatives pick), and of
//! what a shader without a `GrabPass` reads after a grabbing draw. Skipped without a Vulkan
//! adapter.

use std::borrow::Cow;

use rw_assets::NoiseTexture;
use rw_graphics::{Color as SpriteColor, DrawItem, Mesh as SpriteMesh, Shader, Sprite};

use super::full_screen_tests::{H, Rect, W, level, palette, renderer, rgb};
use super::test_support::{FramePixel, Hlsl};
use super::*;
use crate::{LevelParams, SpriteVertex, StageBatch, StageShader, StageTextureId};

/// The shaders' `3.14`, not pi.
#[allow(clippy::approx_constant, reason = "the game's shaders use 3.14")]
const GAME_PI: f32 = 3.14;

/// `_RAIN`.
const RAIN: f32 = 0.7;

/// `_NoiseTex` with its mip levels, as the shaders read it.
struct Noise(NoiseTexture);

impl Noise {
    /// Every texel's red 90.
    fn uniform() -> Self {
        Self::of(|_, _| 90)
    }

    /// A pattern of red values, so that taps and levels show.
    fn varied() -> Self {
        Self::of(|x, y| ((x * 37 + y * 91 + (x * y) % 13 * 7) % 251) as u8)
    }

    fn of(red: impl Fn(u32, u32) -> u8) -> Self {
        let pixels = (0..64 * 64)
            .map(|i| [red(i % 64, i / 64), 0, 0, 255])
            .collect();
        Self(NoiseTexture::from_image(&PngImage::from_pixels(64, 64, pixels)).unwrap())
    }

    /// Level `level` read bilinear, repeating, at `uv` (v up).
    fn at(&self, level: usize, uv: [f32; 2]) -> f32 {
        let texels = self.0.mip_bytes().nth(level).expect("a level");
        let side = 64usize >> level;
        let (fx, fy) = (uv[0] * side as f32 - 0.5, uv[1] * side as f32 - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let top =
            Self::texel(texels, side, x0, y0).lerp_to(Self::texel(texels, side, x0 + 1, y0), tx);
        let bottom = Self::texel(texels, side, x0, y0 + 1)
            .lerp_to(Self::texel(texels, side, x0 + 1, y0 + 1), tx);
        top.lerp_to(bottom, ty)
    }

    /// The texel `(x, y)` of a level `side` wide, wrapping.
    fn texel(texels: &[u8], side: usize, x: i64, y: i64) -> f32 {
        let side_i = side as i64;
        let (x, y) = (x.rem_euclid(side_i) as usize, y.rem_euclid(side_i) as usize);
        f32::from(texels[y * side + x]) / 255.0
    }

    /// The length in texels of the step of a tap's coordinate from `here` to pixel `q`'s.
    fn step(coord: &dyn Fn(FramePixel) -> [f32; 2], here: [f32; 2], q: FramePixel) -> f32 {
        let other = coord(q);
        ((other[0] - here[0]) * 64.0).hypot((other[1] - here[1]) * 64.0)
    }

    /// The level the pixel block of `p` picks for a tap at `coord(p)`: `log2` of the longer
    /// of the coordinate's steps to the block's other pixels in texels, rounded (bilinear
    /// filtering takes the nearest mip). `None` where that is too near a rounding edge for
    /// the GPU's own derivative to be trusted.
    fn level_of(coord: &dyn Fn(FramePixel) -> [f32; 2], p: FramePixel) -> Option<usize> {
        let here = coord(p);
        let len = Self::step(coord, here, FramePixel(p.0 ^ 1, p.1)).max(Self::step(
            coord,
            here,
            FramePixel(p.0, p.1 ^ 1),
        ));
        let lambda = len.log2();
        if lambda > -0.5 && (lambda - lambda.floor() - 0.5).abs() < 0.1 {
            return None;
        }
        Some(if lambda.is_finite() {
            lambda.round().clamp(0.0, 6.0) as usize
        } else {
            0
        })
    }

    /// `tex2D(_NoiseTex, coord(p))` at the level [`Self::level_of`] picks.
    fn tap_with(&self, coord: &dyn Fn(FramePixel) -> [f32; 2], p: FramePixel) -> Option<f32> {
        Some(self.at(Self::level_of(coord, p)?, coord(p)))
    }

    /// `DeepProcessing`'s tap `k` at pixel `p`.
    fn tap(&self, k: usize, p: FramePixel) -> Option<f32> {
        self.tap_with(&|q| DeepPixel::of(q).taps()[k], p)
    }
}

/// The level image, a palette colour, the noise, sprites behind the level, over it and in
/// `HUD`.
struct GrabScene {
    level: PngImage,
    fill: [u8; 3],
    noise: Noise,
    texels: Vec<(usize, usize, [u8; 3])>,
    behind: Vec<DrawItem>,
    over: Vec<DrawItem>,
    stage: Option<StageBatch>,
    hud: Vec<DrawItem>,
}

impl GrabScene {
    fn new(level: PngImage) -> Self {
        Self {
            level,
            fill: [30, 30, 30],
            noise: Noise::uniform(),
            texels: Vec::new(),
            behind: Vec::new(),
            over: Vec::new(),
            stage: None,
            hud: Vec::new(),
        }
    }

    /// The frame of `over` = `[under, (the disc)]` without, then with `light` drawn after.
    fn frames_with_light(
        &mut self,
        r: &mut WgpuRenderer,
        under: &DrawItem,
        with_disc: bool,
        light: &DrawItem,
    ) -> (FrameImage, FrameImage) {
        self.over = vec![under.clone()];
        if with_disc {
            self.over.push(disc(CENTRE, 1.0, Layer::Foreground));
        }
        let without = self.draw(r);
        self.over.push(light.clone());
        (without, self.draw(r))
    }

    fn draw(&self, r: &mut WgpuRenderer) -> FrameImage {
        r.set_level(&LevelGraphics {
            camera_pos: [0.0, 0.0],
            image: &self.level,
            palette: &palette(self.fill, &self.texels),
            noise: &self.noise.0,
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
        if let Some(batch) = &self.stage {
            r.draw_stage(batch, [W as f32, H as f32]);
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

/// Pixel tests on a frame.
trait FramePixels {
    /// Pixel `(x, y)` is mostly red.
    fn is_red(&self, x: u32, y: u32) -> bool;
    /// Pixel `(x, y)` differs from `other`'s.
    fn differs(&self, other: &Self, x: u32, y: u32) -> bool;
}

impl FramePixels for FrameImage {
    fn is_red(&self, x: u32, y: u32) -> bool {
        let c = rgb(self, x, y);
        c[0] > 200 && c[1] < 60 && c[2] < 60
    }

    fn differs(&self, other: &Self, x: u32, y: u32) -> bool {
        rgb(self, x, y) != rgb(other, x, y)
    }
}

/// A 5 x 4 chequer of distinct colours in `Foreground`: a frame with structure to grab.
fn chequer() -> Vec<DrawItem> {
    let (w, h) = (W / 5 + 1, H / 4 + 1);
    let mut items = Vec::new();
    for j in 0..4 {
        for i in 0..5 {
            let k = (j * 5 + i) as f32;
            let color = SpriteColor::rgb(
                0.1 + 0.8 * (k * 0.37).frac(),
                0.1 + 0.8 * (k * 0.61 + 0.2).frac(),
                0.1 + 0.8 * (k * 0.83 + 0.5).frac(),
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

/// `GravityDisruptor.shader`: the frame pixel the fragment at `p` copies, `None` outside
/// the disc's 600 px quad. The quad's `i.uv` at a fragment is its stage position (frame x
/// + 1, frame y from the bottom - 1, as the full-screen effects') over the quad.
fn disruptor_source(p: FramePixel, alpha: f32) -> Option<(i64, i64)> {
    let uv = [
        ((p.0 as f32 + 1.0) - (CENTRE[0] - 300.0)) / 600.0,
        ((H as f32 - p.1 as f32 - 1.0) - (CENTRE[1] - 300.0)) / 600.0,
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
    let s = p.scr();
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
    let mut scene = GrabScene::new(level(|_, _| [255, 255, 255]));
    scene.over = chequer();
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
    let mut scene = GrabScene::new(level(|_, _| [255, 255, 255]));
    scene.over = chequer();
    let before = scene.draw(&mut r);
    scene.hud.push(disc(CENTRE, 1.0, Layer::Hud));
    let after = scene.draw(&mut r);
    let (mut inside, mut moved, mut wrong) = (0, 0, 0);
    for y in 0..H {
        for x in 0..W {
            match disruptor_source(FramePixel(x, y), 1.0) {
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

/// A stage quad (the HUD's `HUD` container, drawn before the disc) is in the frame the
/// disc grabs: it is rippled like the room's pixels.
#[test]
fn the_disc_distorts_stage_quads_drawn_before_it() {
    let Some(mut r) = renderer() else { return };
    r.set_stage_texture(
        StageTextureId(1),
        &PngImage::from_pixels(1, 1, vec![[255; 4]]),
        TextureSampling::POINT_CLAMP,
    );
    let (x0, y0, x1, y1) = (560.0, 300.0, 760.0, 460.0);
    let mut batch = StageBatch::default();
    batch.push_quad(
        StageTextureId(1),
        StageShader::Basic,
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| SpriteVertex {
            pos: [x, y],
            uv: [0.5, 0.5],
            color: [1.0, 0.0, 0.0, 1.0],
        }),
    );
    let mut scene = GrabScene::new(level(|_, _| [255, 255, 255]));
    scene.stage = Some(batch);
    let before = scene.draw(&mut r);
    scene.hud.push(disc(CENTRE, 1.0, Layer::Hud));
    let after = scene.draw(&mut r);
    let (mut quad, mut agree, mut inside) = (0, 0, 0);
    for y in 0..H {
        for x in 0..W {
            quad += usize::from(before.is_red(x, y));
            if let Some((sx, sy)) = disruptor_source(FramePixel(x, y), 1.0) {
                inside += 1;
                agree += usize::from(rgb(&after, x, y) == rgb(&before, sx as u32, sy as u32));
            }
        }
    }
    assert!(quad > 20_000, "the stage quad shows ({quad} pixels)");
    assert!(
        agree * 200 > inside * 199,
        "{agree} of {inside} pixels copy their source"
    );
    let moved = (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .filter(|&(x, y)| before.is_red(x, y) != after.is_red(x, y))
        .count();
    assert!(moved > 300, "the quad's outline moved at {moved} pixels");
}

/// A `LightSource` (no `GrabPass`: it hides itself past layer 5 where `_GrabTexture` is not
/// black) over a deep level: with nothing grabbing the grab is the pre-level frame, black
/// everywhere, so it shows over the bright rectangle; after the disc (a grabbing draw) the
/// grab is the frame the disc saw, rectangle included, so the light is hidden there, but not
/// beside it. The grab is the frame as the disc found it: not the rippled one it drew.
#[test]
fn a_grab_reader_after_the_disc_reads_the_discs_grab() {
    let Some(mut r) = renderer() else { return };
    let rect = Rect([600, 330, 760, 450]);
    let bright = rect.quad(SpriteColor::rgb(0.4, 0.4, 0.4), Layer::Foreground);
    let light = DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(CENTRE[0] - 150.0, CENTRE[1] - 150.0),
        rotation: 0.0,
        scale: Vec2::new(300.0 / 16.0, 300.0 / 16.0),
        anchor: Vec2::ZERO,
        color: SpriteColor::WHITE,
        shader: Shader::LightSource,
        layer: Layer::Foreground,
    });
    let mut scene = GrabScene::new(level(|_, _| [10, 0, 0]));
    scene.fill = [0, 0, 0];
    let (without, with) = scene.frames_with_light(&mut r, &bright, false, &light);
    let (mut over_rect, mut beside) = (0, 0);
    for y in 0..H {
        for x in 0..W {
            let shows = with.differs(&without, x, y);
            if rect.contains(x, y) && shows {
                over_rect += 1;
            } else if !rect.contains(x, y) && shows {
                beside += 1;
            }
        }
    }
    assert!(
        over_rect > 1000 && beside > 1000,
        "{over_rect} over, {beside} beside"
    );
    let (without, with) = scene.frames_with_light(&mut r, &bright, true, &light);
    // The rectangle's own edge pixels sit half a pixel off the grab's (`frame_px`).
    let inner = Rect([603, 333, 757, 447]);
    let (mut over_rect, mut beside) = (0, 0);
    for y in 0..H {
        for x in 0..W {
            let changed = with.differs(&without, x, y);
            if inner.contains(x, y) {
                over_rect += usize::from(changed);
            } else if !rect.contains(x, y) {
                beside += usize::from(changed);
            }
        }
    }
    assert_eq!(over_rect, 0, "hidden where the disc's grab is bright");
    assert!(beside > 1000, "{beside} pixels beside it still lit");
}

/// The state `DeepProcessing.shader` derives from a pixel before it reads `_NoiseTex`.
struct DeepPixel {
    dp: f32,
    text_coord: [f32; 2],
    dsplace: [f32; 2],
    grid: f32,
    sample: [f32; 2],
}

impl DeepPixel {
    fn of(p: FramePixel) -> Self {
        let (w, h) = (W as f32, H as f32);
        let s = p.scr();
        let red = band_red(p.0);
        let dp = if red == 255 {
            1.0
        } else {
            ((i32::from(red) - 1) % 30) as f32 / 30.0
        };
        let rect = [-0.5 / w, 0.5 / h, (w - 0.5) / w, (h + 0.5) / h];
        let text_coord = [
            ((s[0] * w).floor() / w - rect[0]) / (rect[2] - rect[0]),
            ((s[1] * h).floor() / h - rect[1]) / (rect[3] - rect[1]),
        ];
        let dsplace = [(s[0] - 0.5) * 0.06 * dp, ((1.0 - s[1]) - 0.5) * 0.06 * dp];
        let grid = 10.0_f32.lerp_to(6.0, dp);
        let sample = [
            (text_coord[0] * w / grid).floor() / w * grid + dsplace[0],
            (text_coord[1] * h / grid).floor() / h * grid + dsplace[1],
        ];
        Self {
            dp,
            text_coord,
            dsplace,
            grid,
            sample,
        }
    }

    /// The three `_NoiseTex` coordinates.
    fn taps(&self) -> [[f32; 2]; 3] {
        let [x, y] = self.sample;
        [
            [y.lerp_to(0.265_f32.lerp_to(0.9455, x), 0.5), RAIN],
            [x * 1.2, y * 0.6],
            [x * 8.2, y * 4.2],
        ]
    }
}

/// `DeepProcessing.shader`'s class for the fragment at `p`: 0 nothing, 1 dim blue, 2 blue;
/// `None` where a value lies too near a threshold or a mip rounding edge for the GPU's
/// `sin` and derivatives to be trusted. `behind` is whether the last grab is not black
/// there, `uv` the mesh's `i.uv`.
#[allow(
    clippy::excessive_precision,
    reason = "the shader's literals, rounded to f32 as the GPU does"
)]
fn deep_class(
    p: FramePixel,
    uv: [f32; 2],
    behind: bool,
    clr: [f32; 4],
    noise: &Noise,
) -> Option<u8> {
    let (w, h) = (W as f32, H as f32);
    let px = DeepPixel::of(p);
    let d = ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt();
    let mut dist = 1.0 - (d * 2.0).clamp(0.0, 1.0);
    if px.dp < clr[0] || px.dp > clr[1] || (px.dp > 6.0 / 30.0 && behind) {
        return Some(0);
    }
    let (n0, n1, n2) = (noise.tap(0, p)?, noise.tap(1, p)?, noise.tap(2, p)?);
    let [sx, sy] = px.sample;
    let rand = ((sx * 12.98232 * sy * 0.23532 + RAIN - n0).sin() * 43758.5453).frac();
    let speed = 0.5_f32.lerp_to(1.5, clr[3]);
    let h2 = ((px.dp * 1.2 + 0.75 * RAIN * speed + n1 * 1.0_f32.lerp_to(1.0, dist.powf(0.5)))
        * GAME_PI
        * 2.0)
        .sin()
        * 0.5
        + 0.5;
    let mut hh = ((px.dp * 0.12_f32.lerp_to(2.2, h2)
        + 1.8 * RAIN * speed
        + n2 * 1.0_f32.lerp_to(4.0, h2.powi(3)))
        * GAME_PI
        * 2.0)
        .sin()
        * 0.5
        + 0.5;
    let (lhs, rhs) = (hh.lerp_to(0.0, 1.0 - clr[2]), rand * (1.0 - clr[2]));
    if clr[2] < 1.0 && (lhs - rhs).abs() < 0.03 {
        return None;
    }
    if lhs < rhs {
        return Some(0);
    }
    let cell_centre = [sx + px.grid * 0.5 / w, sy + px.grid * 0.5 / h];
    let from = [
        px.text_coord[0] + px.dsplace[0],
        px.text_coord[1] + px.dsplace[1],
    ];
    let away = ((from[0] - cell_centre[0]).powi(2) + (from[1] - cell_centre[1]).powi(2)).sqrt();
    hh *= 1.0 - away * (w + h) / (px.grid * 2.0);
    dist = dist.powf(1.2_f32.lerp_to(0.199_f32.lerp_to(0.001, clr[3]), h2));
    hh += (-1.0_f32).lerp_to(1.0, clr[3]) * 0.2;
    let (low, high) = (hh * dist, hh * dist.lerp_to(1.0, 0.5));
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

fn banded_level() -> PngImage {
    level(|x, _| {
        let red = band_red(x);
        let sky = if red == 255 { 255 } else { 0 };
        [red, sky, sky]
    })
}

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
    noise: &Noise,
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
            let behind = behind.iter().any(|r| r.contains(x, y)) || (hidden && band_red(x) != 255);
            let Some(class) = deep_class(FramePixel(x, y), uv, behind, clr, noise) else {
                continue;
            };
            t.compared += 1;
            t.classes[usize::from(class)] += 1;
            let (got, was) = (rgb(after, x, y), rgb(before, x, y));
            let want = match class {
                0 => was,
                1 => [
                    (f32::from(was[0]) * 0.8).round() as u8,
                    (f32::from(was[1]) * 0.8).round() as u8,
                    (f32::from(was[2]) * 0.8 + 0.5 * 255.0 * 0.2).round() as u8,
                ],
                _ => [
                    (f32::from(was[0]) * 0.6).round() as u8,
                    (f32::from(was[1]) * 0.6).round() as u8,
                    (f32::from(was[2]) * 0.6 + 255.0 * 0.4).round() as u8,
                ],
            };
            let off = got
                .iter()
                .zip(want)
                .any(|(&g, w)| (i32::from(g) - i32::from(w)).abs() > 2);
            t.wrong += usize::from(off);
        }
    }
    t
}

const FULL_QUAD: [f32; 4] = [0.0, 0.0, W as f32 + 2.0, H as f32 + 2.0];

/// `DeepProcessing` at full power over bands of the level: nothing over sky (depth 1) and
/// layer 0 (below `fromDepth`), blue cells over layer 9, hidden there where a creature
/// is drawn behind the level (past layer 6) but not at layer 4. The cells are dim
/// `(0, 0, 0.5, 0.2)` or blue `(0, 0, 1, 0.4)` blended over the frame.
#[test]
fn deep_processing_rains_over_the_deep_layers() {
    let Some(mut r) = renderer() else { return };
    let creatures = [Rect([650, 100, 850, 300]), Rect([1000, 400, 1200, 600])];
    let mut scene = GrabScene::new(banded_level());
    scene.behind = creatures
        .iter()
        .map(|c| c.quad(SpriteColor::rgb(0.0, 0.5, 0.0), Layer::Items))
        .collect();
    let before = scene.draw(&mut r);
    let clr = [0.1, 0.9, 1.0, 0.5];
    scene.over.push(mesh(FULL_QUAD, clr));
    let after = scene.draw(&mut r);
    let t = compare_deep(
        &after,
        &before,
        FULL_QUAD,
        clr,
        &creatures,
        false,
        &scene.noise,
    );
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

/// With a varied `_NoiseTex` (every tap a different value) and a power of 0.6 (the hash
/// `rand` decides which cells show), the coordinates of the three taps, the mip level
/// Unity picks from the 2x2 block's derivatives and the hash all matter: the GPU's classes
/// match the CPU's, which reads the level the block's longer coordinate step names.
#[test]
fn deep_processing_reads_a_varied_noise_at_the_mip_level_the_block_picks() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene::new(banded_level());
    scene.noise = Noise::varied();
    let before = scene.draw(&mut r);
    let clr = [0.1, 0.9, 0.6, 0.5];
    scene.over.push(mesh(FULL_QUAD, clr));
    let after = scene.draw(&mut r);
    let t = compare_deep(&after, &before, FULL_QUAD, clr, &[], false, &scene.noise);
    assert!(t.compared > 150_000, "{} pixels compared", t.compared);
    assert!(
        t.classes[1] > 1000 && t.classes[2] > 1000,
        "dim {} and blue {} pixels",
        t.classes[1],
        t.classes[2]
    );
    assert!(
        t.wrong * 50 < t.compared,
        "{} of {} differ",
        t.wrong,
        t.compared
    );
    // The test is about the mips: a block at a cell's edge has a step over one texel.
    let minified = (4..H - 4)
        .flat_map(|y| (610..890).map(move |x| FramePixel(x, y)))
        .filter(|&p| {
            let here = DeepPixel::of(p).taps()[2];
            Noise::step(
                &|q| DeepPixel::of(q).taps()[2],
                here,
                FramePixel(p.0 ^ 1, p.1),
            ) > 1.5
        })
        .count();
    assert!(minified > 5000, "{minified} pixels pick a coarser level");
}

/// With no power (vertex blue 0) a cell shows only when `h` wins over a hash in `[0, 1)`
/// that is `lerp(h, 0, 1) = 0 < rand`: almost none do.
#[test]
fn deep_processing_without_power_is_nearly_empty() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene::new(banded_level());
    let before = scene.draw(&mut r);
    scene.over.push(mesh(FULL_QUAD, [0.1, 0.9, 0.0, 0.5]));
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
    let mut scene = GrabScene::new(banded_level());
    let before = scene.draw(&mut r);
    let clr = [0.1, 0.9, 1.0, 0.5];
    scene.over.push(disc(CENTRE, 0.0, Layer::Foreground));
    scene.over.push(mesh(FULL_QUAD, clr));
    let after = scene.draw(&mut r);
    let t = compare_deep(&after, &before, FULL_QUAD, clr, &[], true, &scene.noise);
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

/// `WaterSplash.shader` for the vertex colour `(0, 0, 0, 0.5)` over a noise.
struct Splash<'a>(&'a Noise);

impl Splash<'_> {
    /// The 30 px square quad at frame (500..530, 400..430): its `i.uv` at a pixel.
    fn uv(p: FramePixel) -> [f32; 2] {
        [
            ((p.0 as f32 + 1.0) - 500.0) / 30.0,
            ((H as f32 - p.1 as f32 - 1.0) - (H as f32 - 430.0)) / 30.0,
        ]
    }

    /// `dist` of the quad's pixel.
    fn dist(p: FramePixel) -> f32 {
        let uv = Self::uv(p);
        (((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)).sqrt() * 2.0).clamp(0.0, 1.0)
    }

    /// The distortion added to `textCoord`: a noise sine (the tap at level 0: its step is
    /// 0.06 texels a pixel), quantised to pixels.
    fn distortion(&self, q: FramePixel) -> [f32; 2] {
        let c = q.level_coord();
        let tap = self.0.at(0, [c[0] * 1.2, c[1] * 1.2]);
        let dstrtn = ((RAIN + tap * 3.0) * GAME_PI * 2.0).sin() * 0.5 + 0.5;
        let amount = (-0.002_f32).lerp_to(0.002, dstrtn) * (1.0 - Self::dist(q));
        [
            (amount * W as f32).floor() / W as f32,
            (amount * H as f32).floor() / H as f32,
        ]
    }

    /// The coordinate of the `pattern` tap.
    fn pattern_coord(&self, q: FramePixel) -> [f32; 2] {
        let (c, d) = (q.level_coord(), self.distortion(q));
        [(c[0] + d[0]) * 18.2, RAIN * 2.4 + (c[1] + d[1]) * 12.2]
    }

    /// Whether the pixel shows, and by how much `h` is over the cut (`None` where a tap's
    /// level is too near a rounding edge); `coarse` reads the `i.uv` tap at the block's
    /// level, else at level 0.
    fn visible(&self, p: FramePixel, coarse: bool) -> Option<(bool, f32)> {
        let uv = Self::uv(p);
        let dist = Self::dist(p);
        let pattern = self.0.tap_with(&|q| self.pattern_coord(q), p)?;
        let uv_tap = if coarse {
            self.0.tap_with(&Self::uv, p)?
        } else {
            self.0.at(0, uv)
        };
        let mut h = 0.5 + ((18.5 * RAIN + pattern * 3.0) * GAME_PI).sin() * 0.5;
        h *= uv_tap;
        h = (h.lerp_to(1.0, 0.5) - dist).max(0.0) * 0.5;
        let threshold = 0.05_f32.lerp_to(0.01, 0.5);
        Some((h >= threshold, h - threshold))
    }
}

/// A small `WaterSplash` quad (30 px: its `i.uv` steps 2.1 texels a pixel across the
/// texture, so the block picks mip 1) over a varied `_NoiseTex`: which pixels show matches
/// the shader read at the block's level, and differs from the level-0 read.
#[test]
fn a_small_splash_reads_the_noise_at_a_coarser_level() {
    let Some(mut r) = renderer() else { return };
    let mut scene = GrabScene::new(level(|_, _| [10, 0, 0]));
    scene.noise = Noise::varied();
    scene.texels = vec![
        (0, 7, [200, 0, 0]),
        (4, 7, [200, 0, 0]),
        (5, 7, [200, 0, 0]),
    ];
    let before = scene.draw(&mut r);
    scene.over.push(DrawItem::Sprite(Sprite {
        index: 0,
        element: SpriteAtlas::FUTILE_WHITE,
        pos: Vec2::new(500.0, H as f32 - 430.0),
        rotation: 0.0,
        scale: Vec2::new(30.0 / 16.0, 30.0 / 16.0),
        anchor: Vec2::ZERO,
        color: SpriteColor::rgb(0.0, 0.0, 0.0).with_alpha(0.5),
        shader: Shader::WaterSplash,
        layer: Layer::Foreground,
    }));
    let after = scene.draw(&mut r);
    let (mut compared, mut wrong, mut level0_differs) = (0, 0, 0);
    for y in 400..430 {
        for x in 500..530 {
            let p = FramePixel(x, y);
            let Some((visible, margin)) = Splash(&scene.noise).visible(p, true) else {
                continue;
            };
            if margin.abs() < 0.02 {
                continue;
            }
            compared += 1;
            wrong += usize::from(after.differs(&before, x, y) != visible);
            if let Some((fine, _)) = Splash(&scene.noise).visible(p, false) {
                level0_differs += usize::from(fine != visible);
            }
        }
    }
    assert!(compared > 500, "{compared} pixels compared");
    assert!(wrong * 50 < compared, "{wrong} of {compared} differ");
    assert!(
        level0_differs > 8,
        "level 0 would differ at {level0_differs} pixels"
    );
}
