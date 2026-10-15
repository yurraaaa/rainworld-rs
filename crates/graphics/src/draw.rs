//! The backend-agnostic draw list: what a renderer needs to draw one frame of an
//! object's Futile sprites.

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
/// Midground is drawn before the level image, so level pixels nearer than depth 5
/// cover it; Foreground sprites added after the level image draw over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    Midground,
    Foreground,
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
    pub color: Color,
    pub layer: Layer,
}

/// A `TriangleMesh` drawn with one flat colour (`customColor` false, every UV at the
/// `Futile_White` texture's corner).
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub index: usize,
    /// Vertices in room pixels.
    pub vertices: Vec<Vec2>,
    /// Vertex index triples.
    pub triangles: &'static [[u16; 3]],
    /// Every vertex's colour.
    pub color: Color,
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
