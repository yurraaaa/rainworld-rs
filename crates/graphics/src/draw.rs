//! The backend-agnostic draw list: what a renderer needs to draw one frame of an
//! object's Futile sprites.
//!
//! Mirrors: - (backend-agnostic draw list replacing Futile sprites)

use std::borrow::Cow;

use rw_sim::Vec2;

/// Linear RGBA in `[0, 1]`, as Unity's `Color` (the game blends in gamma space, so
/// these are used as stored, without sRGB conversion).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// `Color.gray`.
    pub const GRAY: Self = Self::rgb(0.5, 0.5, 0.5);

    /// Opaque colour.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// `Custom.RGB2RGBA`.
    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// `Color.Lerp` (all four channels, `t` clamped).
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (to.r - self.r) * t,
            g: self.g + (to.g - self.g) * t,
            b: self.b + (to.b - self.b) * t,
            a: self.a + (to.a - self.a) * t,
        }
    }

    /// `Color * float` (all four channels).
    #[must_use]
    pub fn scale(self, s: f32) -> Self {
        Self {
            r: self.r * s,
            g: self.g * s,
            b: self.b * s,
            a: self.a * s,
        }
    }

    /// `Custom.HSL2RGB` (opaque).
    #[must_use]
    pub fn hsl(h: f32, sl: f32, l: f32) -> Self {
        let v = if l <= 0.5 {
            l * (1.0 + sl)
        } else {
            l + sl - l * sl
        };
        if v <= 0.0 {
            return Self::rgb(l, l, l);
        }
        let m = l + l - v;
        let sv = (v - m) / v;
        let h = h * 6.0;
        let sextant = h as i32;
        let fract = h - sextant as f32;
        let vsf = v * sv * fract;
        let mid1 = m + vsf;
        let mid2 = v - vsf;
        match sextant {
            0 => Self::rgb(v, mid1, m),
            1 => Self::rgb(mid2, v, m),
            2 => Self::rgb(m, v, mid1),
            3 => Self::rgb(m, mid2, v),
            4 => Self::rgb(mid1, m, v),
            5 => Self::rgb(v, m, mid2),
            _ => Self::rgb(l, l, l),
        }
    }
}

pub use rw_assets::SlugcatName;

/// The room camera's sprite containers an object draws into (`RoomCamera.ReturnFContainer`).
///
/// Shadows, BackgroundShortcuts, Background, Midground, then Items, are drawn before
/// the level image, so level pixels nearer than depth 5 cover them; Foreground sprites added
/// after the level image draw over it, then ForegroundLights, Shortcuts, Water,
/// GrabShaders and Bloom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    Shadows,
    BackgroundShortcuts,
    Background,
    Midground,
    Items,
    Foreground,
    ForegroundLights,
    Shortcuts,
    Water,
    GrabShaders,
    Bloom,
}

/// The game shader (`FShader`) an item is drawn with (`StreamingAssets/shaders`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shader {
    /// `Futile/Basic`: texel times colour, alpha blended.
    #[default]
    Basic,
    /// `Futile/FlatLight`: a soft disc of the colour over the element's quad.
    FlatLight,
    /// `Futile/OverseerZip`: the colour where the distance from the texture centre
    /// plus `_NoiseTex2` noise stays under `3 * alpha`.
    OverseerZip,
    /// `Futile/Hologram`: noisy scan lines of the colour, the texel alpha read with a
    /// sideways jitter.
    Hologram,
    /// `Futile/HoloGrid`: the guide's light, a grid over the level's near layers and
    /// lit terrain edges, displaced toward the eye (the colour's xy) by depth.
    HoloGrid,
    /// `Futile/LightSource`: a disc of the colour lighting the level image by its depth,
    /// shadowed by nearer level pixels and hidden on creatures drawn behind the level.
    LightSource,
    /// `Futile/HologramImage`: a cell of a 5x5 projection texture (the colour's blue
    /// picks it, red and green pan it), tinted, screened over the frame and cut by
    /// noisy scan lines.
    HologramImage,
    /// `Futile/ShortCut0` (the `Shortcuts` shader): the colour where the level image's
    /// pixel is a shortcut pipe (green 8 to 10, blue 0), nothing elsewhere.
    ShortCut0,
    /// `ColoredSprite3` (`Futile/ColoredSprite2` with `no_grab_tex`): the room palette's
    /// colour at the texel's depth (red, deepened by `1 - alpha`) and row (blue), fogged,
    /// hidden behind nearer level pixels.
    ColoredSprite3,
    /// `Futile/UnderWaterLight`: a light source's glow under the water surface, faded by
    /// level depth and rippled by `_NoiseTex`.
    UnderWaterLight,
    /// `Futile/SpecificDepth` (the ant): the colour, with the texel's alpha, only where
    /// the level pixel lies in the layer `1 - alpha` names (`floor(30 * depth)`).
    SpecificDepth,
    /// `Futile/Bloom` (a room's full-screen effect): the frame grabbed, a plus-shaped
    /// blur of it brightening it (max, plus the blur to the power 1.5).
    Bloom,
    /// `Futile/SkyBloom` (full-screen): pixels of the palette's sky colour blurred over
    /// their neighbours, strongest toward the screen's lower left.
    SkyBloom,
    /// `Futile/LightBloom` (full-screen, `LightBurn` and `Lightning` rooms): the frame at
    /// sunlit level pixels around each pixel added on top.
    LightBloom,
    /// `Futile/LightAndSkyBloom` (full-screen, `SkyAndLightBloom` rooms): `SkyBloom`
    /// plus a glow from sunlit level pixels.
    LightAndSkyBloom,
    /// `Futile/Fog` (full-screen, `Fog` rooms): banded translucent fog in the palette's
    /// fog colours, thinner on near layers, with light patches in the sky.
    Fog,
    /// `Futile/DeathFall` (`DeathFallGraphic`, rooms with an open bottom): the grabbed
    /// frame darkened (dark pixels to a faint inverse) over far level layers, more
    /// opaque toward the sprite's bottom edge.
    DeathFall,
    /// `Futile/WaterSplash` (a `Splash`'s quads): noisy water in the palette's water
    /// colours over a radial (or, with green, sideways) falloff, its blue the white foam
    /// variant; over a creature drawn behind the level (the pre-level grab) that
    /// creature's colour, and the background bubbles' `(0, 1/255, 0)` as lit water.
    WaterSplash,
    /// `Futile/WaterLight` (the room camera's `WaterLight`): faint white caustics over
    /// the level where the room's `RainMask` texture is blue (just above the water),
    /// fading toward the sprite's top; none on near layers, sky or over what was drawn
    /// behind far layers.
    WaterLight,
}

impl Shader {
    /// The shaders with a `GrabPass {}`: each draw grabs the frame drawn so far.
    #[must_use]
    pub fn grabs_frame(self) -> bool {
        matches!(
            self,
            Self::HologramImage
                | Self::Bloom
                | Self::SkyBloom
                | Self::LightBloom
                | Self::LightAndSkyBloom
                | Self::DeathFall
        )
    }
}

/// One `FSprite`, placed as Futile does: the element's untrimmed source box is
/// anchored at `anchor` (fractions from its bottom-left), scaled by `scale` (negative
/// mirrors), rotated by `rotation` and moved to `pos`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// The game's sprite index in the object's sprite leaser (for debugging and order).
    pub index: usize,
    /// Atlas element name.
    pub element: &'static str,
    /// Anchor position in room pixels (y up); subtract the camera position to draw.
    pub pos: Vec2,
    /// Degrees, positive = clockwise on screen (Futile `rotation`).
    pub rotation: f32,
    pub scale: Vec2,
    pub anchor: Vec2,
    /// `color` with `alpha` in `a`.
    pub color: Color,
    pub shader: Shader,
    pub layer: Layer,
}

/// A `TriangleMesh` over `Futile_White`.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub index: usize,
    /// Vertices in room pixels.
    pub vertices: Vec<Vec2>,
    /// `UVvertices` (0 to 1 over `Futile_White`), one per vertex; empty puts every
    /// vertex on the texture's centre.
    pub uvs: Vec<Vec2>,
    /// Vertex index triples.
    pub triangles: Cow<'static, [[u16; 3]]>,
    /// Every vertex's colour, unless `vertex_colors` is set.
    pub color: Color,
    /// `verticeColors` (`customColor`), one per vertex; empty uses `color`.
    pub vertex_colors: Vec<Color>,
    pub shader: Shader,
    pub layer: Layer,
}

/// One drawable.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawItem {
    Sprite(Sprite),
    Mesh(Mesh),
}

impl DrawItem {
    #[must_use]
    pub fn layer(&self) -> Layer {
        match self {
            Self::Sprite(s) => s.layer,
            Self::Mesh(m) => m.layer,
        }
    }

    pub fn set_layer(&mut self, layer: Layer) {
        match self {
            Self::Sprite(s) => s.layer = layer,
            Self::Mesh(m) => m.layer = layer,
        }
    }

    /// The game's sprite index.
    #[must_use]
    pub fn index(&self) -> usize {
        match self {
            Self::Sprite(s) => s.index,
            Self::Mesh(m) => m.index,
        }
    }
}

/// The visible sprites of one object for one frame, in container order: within each
/// [`Layer`], later items draw on top. Hidden sprites are left out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrawList {
    pub items: Vec<DrawItem>,
}

/// A container inside an object's sprites (`sLeaser.containers[i]`): its place among
/// the items of the object's last draw list, and its layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalContainer {
    pub at: usize,
    pub layer: Layer,
}

impl DrawList {
    /// `MoveObjectToInternalContainer`: `other`'s items in the given layers move into
    /// this list's `container`, after what it already holds (the rest stay in
    /// `other`); returns the container moved past them.
    pub fn take_into_container(
        &mut self,
        container: InternalContainer,
        other: &mut DrawList,
        layers: &[Layer],
    ) -> InternalContainer {
        let InternalContainer { at, layer } = container;
        let (moved, kept): (Vec<DrawItem>, Vec<DrawItem>) = std::mem::take(&mut other.items)
            .into_iter()
            .partition(|i| layers.contains(&i.layer()));
        other.items = kept;
        let n = moved.len();
        self.items.splice(
            at..at,
            moved.into_iter().map(|mut i| {
                i.set_layer(layer);
                i
            }),
        );
        InternalContainer { at: at + n, layer }
    }

    /// The items of one layer, in draw order.
    pub fn layer(&self, layer: Layer) -> impl Iterator<Item = &DrawItem> {
        self.items.iter().filter(move |i| i.layer() == layer)
    }

    /// The sprite with the game's index `index`, if visible.
    #[must_use]
    pub fn sprite(&self, index: usize) -> Option<&Sprite> {
        self.items.iter().find_map(|i| match i {
            DrawItem::Sprite(s) if s.index == index => Some(s),
            _ => None,
        })
    }

    /// The mesh with the game's index `index`, if visible.
    #[must_use]
    pub fn mesh(&self, index: usize) -> Option<&Mesh> {
        self.items.iter().find_map(|i| match i {
            DrawItem::Mesh(m) if m.index == index => Some(m),
            _ => None,
        })
    }
}

/// From `[r, g, b, a]`.
impl From<[f32; 4]> for Color {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self { r, g, b, a }
    }
}

/// `Color + Color` (all four channels).
impl std::ops::Add for Color {
    type Output = Self;

    fn add(self, o: Self) -> Self {
        Self {
            r: self.r + o.r,
            g: self.g + o.g,
            b: self.b + o.b,
            a: self.a + o.a,
        }
    }
}
