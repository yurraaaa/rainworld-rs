//! The backend-agnostic draw list: what a renderer needs to draw one frame of an
//! object's Futile sprites.

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

    /// Opaque colour.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

pub use rw_assets::SlugcatName;

/// The room camera's sprite containers an object draws into (`RoomCamera.ReturnFContainer`).
///
/// Midground, then Items, are drawn before the level image, so level pixels nearer
/// than depth 5 cover them; Foreground sprites added after the level image draw over
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    Midground,
    Items,
    Foreground,
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

impl DrawList {
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
