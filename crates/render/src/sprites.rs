//! Futile sprites on the CPU: atlas texture coordinates, `FSprite` quads and
//! `TriangleMesh`es built from an `rw_graphics::DrawList`, and the frame's draw queue
//! that keeps shapes, sprites and the level in call order. GPU-free.
//!
//! Behaviour: `rainworld-re/notes/sprite-atlases.md` section 3 (`FSprite.UpdateLocalVertices`,
//! `FMatrix.SetScaleThenRotate`, `Futile/Basic`).

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use rw_graphics::{Atlas, AtlasFrame, DrawItem, DrawList, Layer, Mesh as SpriteMesh, Sprite};
use rw_sim::Vec2;

use crate::Mesh;

/// One textured vertex: stage position (room pixels minus the camera position),
/// texture coordinate (`v` = 0 at the image's bottom row, as Unity) and colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct SpriteVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// An atlas's frames with the image size, for texture coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteAtlas {
    atlas: Atlas,
    /// Where `Futile_White`-textured meshes sample: an opaque white texel.
    white_uv: [f32; 2],
}

/// `FMatrix` as `FNode` sets it: scale, then rotate clockwise, then translate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeMatrix {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    tx: f32,
    ty: f32,
}

/// Sprites and shapes queued this frame, in call order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpriteBatch {
    pub vertices: Vec<SpriteVertex>,
    pub indices: Vec<u32>,
}

/// One draw call of the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Batch {
    /// Debug-shape vertices.
    Shapes(Range<u32>),
    /// Sprite indices.
    Sprites(Range<u32>),
    /// The level graphic: batches before it are drawn behind it.
    Level,
}

/// Things that interleave with the debug shapes.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    Sprites {
        shapes_before: u32,
        indices: Range<u32>,
    },
    Level {
        shapes_before: u32,
    },
}

/// One frame's debug shapes and sprites with their order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrawQueue {
    pub shapes: Mesh,
    pub sprites: SpriteBatch,
    events: Vec<Event>,
}

impl SpriteAtlas {
    /// The frame `Futile_White` meshes sample: 16x16 opaque white in `rainWorld`.
    pub const WHITE_ELEMENT: &'static str = "RainWorld_White";

    /// Wraps `atlas`; `None` without the white element.
    #[must_use]
    pub fn new(atlas: Atlas) -> Option<Self> {
        let white = atlas.get(Self::WHITE_ELEMENT)?;
        let [min, max] = Self::uv_rect(&atlas, white);
        let white_uv = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        Some(Self { atlas, white_uv })
    }

    #[must_use]
    pub fn atlas(&self) -> &Atlas {
        &self.atlas
    }

    /// Futile's `uvRect` of a frame: `[min, max]` with `v` from the bottom
    /// (`(x / W, (H - y - h) / H)` to `((x + w) / W, (H - y) / H)`).
    #[must_use]
    pub fn uv_rect(atlas: &Atlas, frame: &AtlasFrame) -> [[f32; 2]; 2] {
        let (w, h) = (atlas.size.w as f32, atlas.size.h as f32);
        let f = frame.frame;
        [
            [f.x as f32 / w, (h - f.y as f32 - f.h as f32) / h],
            [(f.x + f.w) as f32 / w, (h - f.y as f32) / h],
        ]
    }

    /// `FSprite.UpdateLocalVertices`: the trimmed frame's corners TL, TR, BR, BL in
    /// the sprite's local space, the untrimmed source box anchored at `anchor`.
    #[must_use]
    pub fn local_corners(frame: &AtlasFrame, anchor: Vec2) -> [Vec2; 4] {
        let (sw, sh) = (frame.source_size.w as f32, frame.source_size.h as f32);
        let trim = frame.sprite_source_size;
        let (w, h) = (trim.w as f32, trim.h as f32);
        let x = -anchor.x * sw + trim.x as f32;
        let y = -anchor.y * sh + (sh - trim.y as f32 - h);
        [
            Vec2::new(x, y + h),
            Vec2::new(x + w, y + h),
            Vec2::new(x + w, y),
            Vec2::new(x, y),
        ]
    }

    /// The four vertices of `sprite` (TL, TR, BR, BL) at stage position
    /// `sprite.pos - cam_pos`; `None` if its element is not in the atlas.
    #[must_use]
    pub fn quad(&self, sprite: &Sprite, cam_pos: Vec2) -> Option<[SpriteVertex; 4]> {
        let frame = self.atlas.get(sprite.element)?;
        let [min, max] = Self::uv_rect(&self.atlas, frame);
        let uvs = [
            [min[0], max[1]],
            [max[0], max[1]],
            [max[0], min[1]],
            [min[0], min[1]],
        ];
        let matrix =
            NodeMatrix::scale_then_rotate(sprite.pos - cam_pos, sprite.scale, sprite.rotation);
        let corners = Self::local_corners(frame, sprite.anchor);
        let c = sprite.color;
        Some(std::array::from_fn(|i| SpriteVertex {
            pos: matrix.apply(corners[i]).to_array(),
            uv: uvs[i],
            color: [c.r, c.g, c.b, c.a],
        }))
    }
}

impl NodeMatrix {
    /// `SetScaleThenRotate(x, y, scaleX, scaleY, -rotation in radians)`.
    #[must_use]
    pub fn scale_then_rotate(pos: Vec2, scale: Vec2, rotation_deg: f32) -> Self {
        let r = -rotation_deg.to_radians();
        let (sin, cos) = r.sin_cos();
        Self {
            a: scale.x * cos,
            b: scale.x * sin,
            c: -scale.y * sin,
            d: scale.y * cos,
            tx: pos.x,
            ty: pos.y,
        }
    }

    #[must_use]
    pub fn apply(&self, v: Vec2) -> Vec2 {
        Vec2::new(
            v.x * self.a + v.y * self.c + self.tx,
            v.x * self.b + v.y * self.d + self.ty,
        )
    }
}

impl SpriteBatch {
    /// Futile's quad triangles over vertices TL, TR, BR, BL.
    pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

    /// Appends `sprite`; false (nothing added) if its element is missing.
    pub fn push_sprite(&mut self, atlas: &SpriteAtlas, sprite: &Sprite, cam_pos: Vec2) -> bool {
        let Some(quad) = atlas.quad(sprite, cam_pos) else {
            return false;
        };
        let base = self.vertices.len() as u32;
        self.vertices.extend(quad);
        self.indices.extend(Self::QUAD_INDICES.map(|i| base + i));
        true
    }

    /// Appends a flat-coloured `TriangleMesh` (every UV on the white texel), its
    /// vertices at stage positions `v - cam_pos`.
    pub fn push_mesh(&mut self, atlas: &SpriteAtlas, mesh: &SpriteMesh, cam_pos: Vec2) {
        let base = self.vertices.len() as u32;
        let c = mesh.color;
        self.vertices
            .extend(mesh.vertices.iter().map(|&v| SpriteVertex {
                pos: (v - cam_pos).to_array(),
                uv: atlas.white_uv,
                color: [c.r, c.g, c.b, c.a],
            }));
        self.indices.extend(
            mesh.triangles
                .iter()
                .flatten()
                .map(|&i| base + u32::from(i)),
        );
    }

    /// Appends the items of `list` in `layer`, in order. Returns the names of
    /// elements missing from the atlas (those sprites are skipped).
    pub fn push_list(
        &mut self,
        atlas: &SpriteAtlas,
        list: &DrawList,
        layer: Layer,
        cam_pos: Vec2,
    ) -> Vec<&'static str> {
        let mut missing = Vec::new();
        for item in list.layer(layer) {
            match item {
                DrawItem::Sprite(s) => {
                    if !self.push_sprite(atlas, s, cam_pos) {
                        missing.push(s.element);
                    }
                }
                DrawItem::Mesh(m) => self.push_mesh(atlas, m, cam_pos),
            }
        }
        missing
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

impl DrawQueue {
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.sprites.clear();
        self.events.clear();
    }

    /// Queues `list`'s items of `layer` after everything queued so far. Returns the
    /// names of elements missing from the atlas.
    pub fn push_sprites(
        &mut self,
        atlas: &SpriteAtlas,
        list: &DrawList,
        layer: Layer,
        cam_pos: Vec2,
    ) -> Vec<&'static str> {
        let start = self.sprites.indices.len() as u32;
        let missing = self.sprites.push_list(atlas, list, layer, cam_pos);
        let end = self.sprites.indices.len() as u32;
        if end > start {
            self.events.push(Event::Sprites {
                shapes_before: self.shapes.vertex_count(),
                indices: start..end,
            });
        }
        missing
    }

    /// Marks where the level graphic is drawn (the last mark wins).
    pub fn mark_level(&mut self) {
        self.events.retain(|e| !matches!(e, Event::Level { .. }));
        self.events.push(Event::Level {
            shapes_before: self.shapes.vertex_count(),
        });
    }

    /// The frame's draw calls in order; consecutive shapes merge into one batch.
    #[must_use]
    pub fn batches(&self) -> Vec<Batch> {
        let mut out = Vec::new();
        let mut shapes_done = 0;
        for event in &self.events {
            let shapes_before = match event {
                Event::Sprites { shapes_before, .. } | Event::Level { shapes_before } => {
                    *shapes_before
                }
            };
            if shapes_before > shapes_done {
                out.push(Batch::Shapes(shapes_done..shapes_before));
                shapes_done = shapes_before;
            }
            out.push(match event {
                Event::Sprites { indices, .. } => Batch::Sprites(indices.clone()),
                Event::Level { .. } => Batch::Level,
            });
        }
        let total = self.shapes.vertex_count();
        if total > shapes_done {
            out.push(Batch::Shapes(shapes_done..total));
        }
        out
    }
}

#[cfg(test)]
mod tests;
