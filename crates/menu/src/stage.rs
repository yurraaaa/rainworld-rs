//! Futile's display tree: containers holding sprites, labels and containers, drawn in
//! tree order (`FStage` depth order) into a [`StageBatch`] of screen-pixel quads.
//! Positions are stage pixels, y up, origin at the screen's bottom-left.
//!
//! Mirrors: FStage.cs, FContainer.cs

use rw_render::{SpriteVertex, StageBatch, StageShader};

use crate::atlas::{AtlasManager, Element};
use crate::geometry::Matrix;
use crate::label::Label;

/// Unity's `Color` (gamma-space values, used as stored).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl StageColor {
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);

    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// `CloneWithMultipliedAlpha`.
    #[must_use]
    pub fn with_alpha_multiplied(self, alpha: f32) -> Self {
        Self {
            a: self.a * alpha,
            ..self
        }
    }

    #[must_use]
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// `FNode`'s position, scale, rotation (degrees, clockwise), alpha and visibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeTransform {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub alpha: f32,
    /// `isVisible`: an invisible node is drawn at scale 0, i.e. not at all.
    pub visible: bool,
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            alpha: 1.0,
            visible: true,
        }
    }
}

impl NodeTransform {
    #[must_use]
    pub fn at(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            ..Self::default()
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale_x = scale;
        self.scale_y = scale;
    }

    /// The concatenated matrix and alpha inside a parent's.
    #[must_use]
    pub fn concat(&self, parent: &Matrix, parent_alpha: f32) -> (Matrix, f32) {
        let own = Matrix::node(self.x, self.y, self.scale_x, self.scale_y, self.rotation);
        (own.then(parent), parent_alpha * self.alpha)
    }
}

/// One `FSprite` showing an atlas element.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pub element: String,
    pub transform: NodeTransform,
    /// Fractions of the untrimmed element box, from its bottom-left (0.5, 0.5 default).
    pub anchor: [f32; 2],
    pub color: StageColor,
    pub shader: StageShader,
}

impl Sprite {
    #[must_use]
    pub fn new(element: &str) -> Self {
        Self {
            element: element.to_owned(),
            transform: NodeTransform::default(),
            anchor: [0.5, 0.5],
            color: StageColor::WHITE,
            shader: StageShader::Basic,
        }
    }

    /// `UpdateLocalVertices` + `PopulateRenderLayer`; nothing when the element is not
    /// loaded.
    pub fn push_quad(
        &self,
        atlases: &AtlasManager,
        batch: &mut StageBatch,
        matrix: &Matrix,
        alpha: f32,
    ) {
        let Some(e) = atlases.element(&self.element) else {
            return;
        };
        let [w, h] = e.source_size;
        let left = -self.anchor[0] * w + e.source_rect.x;
        let bottom = -self.anchor[1] * h + (h - e.source_rect.y - e.source_rect.height);
        let (right, top) = (left + e.source_rect.width, bottom + e.source_rect.height);
        let corners = [[left, top], [right, top], [right, bottom], [left, bottom]];
        let uv = &e.uv_rect;
        let uvs = [
            [uv.x_min(), uv.y_max()],
            [uv.x_max(), uv.y_max()],
            [uv.x_max(), uv.y_min()],
            [uv.x_min(), uv.y_min()],
        ];
        let color = self.color.with_alpha_multiplied(alpha).to_array();
        let vertices = std::array::from_fn(|i| SpriteVertex {
            pos: matrix.apply(corners[i]),
            uv: uvs[i],
            color,
        });
        batch.push_quad(e.texture, self.shader, vertices);
    }
}

/// A `TriangleMesh` made by `MakeLongMesh` with custom colours: a triangle strip
/// `(k, k + 1, k + 2)` over its vertices, in local pixels, with texture coordinates
/// within the element.
#[derive(Debug, Clone, PartialEq)]
pub struct TriangleStrip {
    pub element: String,
    pub transform: NodeTransform,
    pub shader: StageShader,
    pub vertices: Vec<[f32; 2]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<StageColor>,
}

impl TriangleStrip {
    /// `TriangleMesh.MakeLongMesh(segments, false, true)`: `4 * segments` vertices
    /// in left/right pairs, `u` 0 or 1, `v` rising by `1 / (2 * segments - 1)` a pair.
    #[must_use]
    pub fn long_mesh(segments: usize, element: &str) -> Self {
        let n = segments * 4;
        let step = 1.0 / ((segments as f32 - 1.0) * 2.0 + 1.0);
        Self {
            element: element.to_owned(),
            transform: NodeTransform::default(),
            shader: StageShader::Basic,
            vertices: vec![[0.0; 2]; n],
            uvs: (0..n)
                .map(|k| [if k % 2 == 0 { 0.0 } else { 1.0 }, (k / 2) as f32 * step])
                .collect(),
            colors: vec![StageColor::WHITE; n],
        }
    }

    /// Each pair of strip triangles as one quad.
    pub fn push_quads(
        &self,
        atlases: &AtlasManager,
        batch: &mut StageBatch,
        matrix: &Matrix,
        alpha: f32,
    ) {
        let Some(e) = atlases.element(&self.element) else {
            return;
        };
        let mut k = 0;
        while k + 3 < self.vertices.len() {
            // Triangles (k, k+1, k+2) and (k+1, k+2, k+3) as the quad (k+1, k, k+2, k+3).
            let quad = [k + 1, k, k + 2, k + 3].map(|i| self.vertex(e, i, matrix, alpha));
            batch.push_quad(e.texture, self.shader, quad);
            k += 2;
        }
    }

    fn vertex(&self, e: &Element, k: usize, matrix: &Matrix, alpha: f32) -> SpriteVertex {
        let uv = &e.uv_rect;
        SpriteVertex {
            pos: matrix.apply(self.vertices[k]),
            uv: [
                uv.x_min() + self.uvs[k][0] * (uv.x_max() - uv.x_min()),
                uv.y_min() + self.uvs[k][1] * (uv.y_max() - uv.y_min()),
            ],
            color: self.colors[k].with_alpha_multiplied(alpha).to_array(),
        }
    }
}

/// A child of a [`Container`].
#[derive(Debug, Clone)]
pub enum Node {
    Sprite(Sprite),
    Label(Label),
    Container(Container),
}

impl Node {
    #[must_use]
    pub fn transform(&self) -> &NodeTransform {
        match self {
            Self::Sprite(s) => &s.transform,
            Self::Label(l) => &l.transform,
            Self::Container(c) => &c.transform,
        }
    }

    fn push_quads(
        &self,
        atlases: &AtlasManager,
        batch: &mut StageBatch,
        parent: &Matrix,
        parent_alpha: f32,
    ) {
        if let Self::Container(c) = self {
            c.push_quads_in(atlases, batch, parent, parent_alpha);
            return;
        }
        let transform = self.transform();
        if !transform.visible {
            return;
        }
        let (matrix, alpha) = transform.concat(parent, parent_alpha);
        match self {
            Self::Sprite(s) => s.push_quad(atlases, batch, &matrix, alpha),
            Self::Label(l) => l.push_quads(batch, &matrix, alpha),
            Self::Container(_) => {}
        }
    }
}

/// An `FContainer` (or the `FStage` itself): children drawn in order, later on top.
#[derive(Debug, Clone, Default)]
pub struct Container {
    pub transform: NodeTransform,
    pub children: Vec<Node>,
}

impl Container {
    /// `AddChild`; returns the child's index.
    pub fn add(&mut self, node: Node) -> usize {
        self.children.push(node);
        self.children.len() - 1
    }

    /// Appends this tree's quads, in draw order, to `batch`.
    pub fn push_quads(&self, atlases: &AtlasManager, batch: &mut StageBatch) {
        self.push_quads_in(atlases, batch, &Matrix::IDENTITY, 1.0);
    }

    fn push_quads_in(
        &self,
        atlases: &AtlasManager,
        batch: &mut StageBatch,
        parent: &Matrix,
        parent_alpha: f32,
    ) {
        if !self.transform.visible {
            return;
        }
        let (matrix, alpha) = self.transform.concat(parent, parent_alpha);
        for child in &self.children {
            child.push_quads(atlases, batch, &matrix, alpha);
        }
    }
}
