//! Futile sprites on the CPU: atlas texture coordinates, `FSprite` quads and
//! `TriangleMesh`es built from an `rw_graphics::DrawList`, split into draws per game
//! shader, and the frame's draw queue that keeps shapes, sprites and the level in call
//! order. GPU-free.
//!
//! Behaviour: `rainworld-re/notes/sprite-atlases.md` section 3 (`FSprite.UpdateLocalVertices`,
//! `FMatrix.SetScaleThenRotate`, the room sprite shaders).
//!
//! Mirrors: FSprite.cs, FMatrix.cs, TriangleMesh.cs

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use rw_graphics::{
    Atlas, AtlasFrame, Color as SpriteColor, DrawItem, DrawList, Layer, Mesh as SpriteMesh, Rect,
    Shader, Size, Sprite,
};
use rw_sim::Vec2;

use crate::{LevelView, Mesh, StageBatch, StageDraw, WaterSprites, WaterVertex};

/// One textured vertex: stage position (room pixels minus the camera position),
/// texture coordinate (`v` = 0 at the image's bottom row, as Unity) and colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct SpriteVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// One room sprite vertex: stage position, where the port's atlas is sampled (`uv`),
/// the texture coordinate the game's shader reads as `i.uv` (`tex_coord`) and colour.
///
/// The two coordinates differ only for `Futile_White`, which in the game is its own
/// texture (`uvRect` (0, 0, 1, 1)) and here the atlas's white element.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct RoomVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub tex_coord: [f32; 2],
    pub color: [f32; 4],
}

/// The loaded atlases' frames (Futile's atlas manager): one page per texture, an
/// element found in the first page holding it.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteAtlas {
    pages: Vec<Atlas>,
    /// The white element's `[min, max]` texture coordinates (on page 0) inset by half a
    /// texel: where `Futile_White`'s 0..1 is sampled.
    white: [[f32; 2]; 2],
}

/// The globals the room sprite shaders read, as `RoomCamera.DrawUpdate` sets them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoomGlobals {
    /// `_spriteRect`: the level image in screen coordinates (0..1 over the screen, y up).
    pub sprite_rect: [f32; 4],
    /// `_screenSize`.
    pub screen_size: [f32; 2],
    /// `_RAIN`.
    pub rain: f32,
    /// `_fogAmount` (`RoomCamera.ApplyPalette`).
    pub fog_amount: f32,
    /// `_waterLevel` (see [`LevelView::water_level`]).
    pub water_level: f32,
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

/// One draw call of room sprites: `indices` of the batch with one shader and one
/// atlas page's texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteDraw {
    pub shader: Shader,
    pub page: usize,
    pub indices: Range<u32>,
}

/// Sprites queued this frame, in call order, and their draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpriteBatch {
    pub vertices: Vec<RoomVertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<SpriteDraw>,
}

/// One draw call of the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Batch {
    /// Debug-shape vertices.
    Shapes(Range<u32>),
    /// Sprite indices drawn with one shader.
    Sprites(SpriteDraw),
    /// The level graphic: batches before it are drawn behind it.
    Level,
    /// Water triangles drawn with one of the game's water shaders.
    Water(WaterShader, Range<u32>),
    /// Stage quads (screen pixels) of one texture and shader.
    Stage(StageDraw),
}

/// Which water shader a [`Batch::Water`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterShader {
    /// `WaterSurface`: the surface band.
    Surface,
    /// `DeepWater`: the water body.
    Deep,
}

/// Things that interleave with the debug shapes.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    Sprites {
        shapes_before: u32,
        draw: SpriteDraw,
    },
    Level {
        shapes_before: u32,
    },
    Water {
        shapes_before: u32,
        shader: WaterShader,
        vertices: Range<u32>,
    },
    Stage {
        shapes_before: u32,
        draw: StageDraw,
    },
}

/// One frame's debug shapes and sprites with their order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrawQueue {
    pub shapes: Mesh,
    pub sprites: SpriteBatch,
    /// Water triangles (`WaterSprites`), in stage pixels.
    pub water: Vec<WaterVertex>,
    /// Stage quads, in screen pixels.
    pub stage: StageBatch,
    events: Vec<Event>,
}

/// A draw-list colour as vertex data.
trait VertexColor {
    fn to_array(self) -> [f32; 4];
}

impl VertexColor for SpriteColor {
    fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl SpriteAtlas {
    /// The frame `Futile_White` is drawn from: 16x16 opaque white in `rainWorld`.
    pub const WHITE_ELEMENT: &'static str = "RainWorld_White";

    /// Futile's runtime 16x16 white texture, an element of its own one-image atlas.
    pub const FUTILE_WHITE: &'static str = "Futile_White";

    const FUTILE_WHITE_FRAME: AtlasFrame = AtlasFrame {
        frame: Rect {
            x: 0,
            y: 0,
            w: 16,
            h: 16,
        },
        rotated: false,
        trimmed: false,
        sprite_source_size: Rect {
            x: 0,
            y: 0,
            w: 16,
            h: 16,
        },
        source_size: Size { w: 16, h: 16 },
    };

    /// `uvRect` corners of a whole texture in vertex order TL, TR, BR, BL.
    const TEXTURE_CORNERS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    /// Wraps `atlas` as page 0; `None` without the white element.
    #[must_use]
    pub fn new(atlas: Atlas) -> Option<Self> {
        let white = atlas.get(Self::WHITE_ELEMENT)?;
        let [min, max] = Self::uv_rect(&atlas, white);
        let half = [0.5 / atlas.size.w as f32, 0.5 / atlas.size.h as f32];
        let white = [
            [min[0] + half[0], min[1] + half[1]],
            [max[0] - half[0], max[1] - half[1]],
        ];
        Some(Self {
            pages: vec![atlas],
            white,
        })
    }

    /// Adds `atlas` as the next page (its texture is the next image uploaded).
    #[must_use]
    pub fn with_page(mut self, atlas: Atlas) -> Self {
        self.pages.push(atlas);
        self
    }

    /// Page 0.
    #[must_use]
    pub fn atlas(&self) -> &Atlas {
        &self.pages[0]
    }

    #[must_use]
    pub fn pages(&self) -> &[Atlas] {
        &self.pages
    }

    /// The page holding element `name` (the first: Futile's element table holds each
    /// name once) and its frame.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<(usize, &AtlasFrame)> {
        self.pages
            .iter()
            .enumerate()
            .find_map(|(i, a)| a.get(name).map(|f| (i, f)))
    }

    /// Where `Futile_White`'s texture coordinate `t` (0..1) is sampled in the atlas.
    #[must_use]
    pub fn white_uv(&self, t: [f32; 2]) -> [f32; 2] {
        let [min, max] = self.white;
        [
            min[0] + (max[0] - min[0]) * t[0],
            min[1] + (max[1] - min[1]) * t[1],
        ]
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
    /// `sprite.pos - cam_pos`, with the page they sample; `None` if its element is in no
    /// page.
    #[must_use]
    pub fn quad(&self, sprite: &Sprite, cam_pos: Vec2) -> Option<(usize, [RoomVertex; 4])> {
        let (page, frame, uvs, tex_coords) = if sprite.element == Self::FUTILE_WHITE {
            let corners = Self::TEXTURE_CORNERS;
            (
                0,
                &Self::FUTILE_WHITE_FRAME,
                corners.map(|t| self.white_uv(t)),
                corners,
            )
        } else {
            let (page, frame) = self.find(sprite.element)?;
            let [min, max] = Self::uv_rect(&self.pages[page], frame);
            let uvs = [
                [min[0], max[1]],
                [max[0], max[1]],
                [max[0], min[1]],
                [min[0], min[1]],
            ];
            (page, frame, uvs, uvs)
        };
        let matrix =
            NodeMatrix::scale_then_rotate(sprite.pos - cam_pos, sprite.scale, sprite.rotation);
        let corners = Self::local_corners(frame, sprite.anchor);
        Some((
            page,
            std::array::from_fn(|i| RoomVertex {
                pos: matrix.apply(corners[i]).to_array(),
                uv: uvs[i],
                tex_coord: tex_coords[i],
                color: sprite.color.to_array(),
            }),
        ))
    }
}

impl RoomGlobals {
    /// The globals while the level image sits at room rect `image` (position, size) and
    /// the game's window is `view`. `_spriteRect` (decompile 130393) is the image's
    /// offset from the window shifted by (-0.5, +0.5) px: the image's rect on screen
    /// (`Camera::SCREEN_PIXEL_OFFSET`), so the shaders' `floor(screen pixel)` reads the
    /// level texel shown at that pixel.
    #[must_use]
    pub fn new(view: &LevelView, image: [f32; 4]) -> Self {
        let [sw, sh] = view.size;
        let x = image[0] - view.origin[0] - 0.5;
        let y = image[1] - view.origin[1] + 0.5;
        Self {
            sprite_rect: [x / sw, y / sh, (x + image[2]) / sw, (y + image[3]) / sh],
            screen_size: view.size,
            rain: view.rain,
            fog_amount: 0.0,
            water_level: view.water_level,
        }
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

    /// Appends `sprite`, extending the last draw when it has the same shader; false
    /// (nothing added) if its element is missing.
    pub fn push_sprite(&mut self, atlas: &SpriteAtlas, sprite: &Sprite, cam_pos: Vec2) -> bool {
        let start = self.indices.len() as u32;
        let page = self.append_sprite(atlas, sprite, cam_pos);
        self.record(sprite.shader, page.unwrap_or(0), start, true);
        page.is_some()
    }

    /// Appends a `TriangleMesh` over `Futile_White`, its vertices at stage positions
    /// `v - cam_pos`, extending the last draw when it has the same shader.
    pub fn push_mesh(&mut self, atlas: &SpriteAtlas, mesh: &SpriteMesh, cam_pos: Vec2) {
        let start = self.indices.len() as u32;
        self.append_mesh(atlas, mesh, cam_pos);
        self.record(mesh.shader, 0, start, true);
    }

    /// Appends `sprite`'s quad; returns its page, `None` (nothing added) if its element
    /// is missing.
    fn append_sprite(
        &mut self,
        atlas: &SpriteAtlas,
        sprite: &Sprite,
        cam_pos: Vec2,
    ) -> Option<usize> {
        let (page, quad) = atlas.quad(sprite, cam_pos)?;
        let base = self.vertices.len() as u32;
        self.vertices.extend(quad);
        self.indices.extend(Self::QUAD_INDICES.map(|i| base + i));
        Some(page)
    }

    /// Each vertex samples `Futile_White` at its UV (the centre without UVs), coloured
    /// by its vertex colour (`mesh.color` without them).
    fn append_mesh(&mut self, atlas: &SpriteAtlas, mesh: &SpriteMesh, cam_pos: Vec2) {
        let base = self.vertices.len() as u32;
        self.vertices
            .extend(mesh.vertices.iter().enumerate().map(|(i, &v)| {
                let tex_coord = mesh.uvs.get(i).map_or([0.5, 0.5], |t| t.to_array());
                RoomVertex {
                    pos: (v - cam_pos).to_array(),
                    uv: atlas.white_uv(tex_coord),
                    tex_coord,
                    color: mesh.vertex_colors.get(i).unwrap_or(&mesh.color).to_array(),
                }
            }));
        self.indices.extend(
            mesh.triangles
                .iter()
                .flatten()
                .map(|&i| base + u32::from(i)),
        );
    }

    /// Records indices `start..` as drawn with `shader` from `page`, merged into the
    /// last draw when `merge` and it has the same shader and page.
    fn record(&mut self, shader: Shader, page: usize, start: u32, merge: bool) {
        let end = self.indices.len() as u32;
        if end == start {
            return;
        }
        match self.draws.last_mut() {
            Some(last)
                if merge
                    && last.shader == shader
                    && last.page == page
                    && last.indices.end == start =>
            {
                last.indices.end = end;
            }
            _ => self.draws.push(SpriteDraw {
                shader,
                page,
                indices: start..end,
            }),
        }
    }

    /// Appends the items of `list` in `layer`, in order, in new draws split where the
    /// shader changes. Returns the names of elements missing from the atlas (those
    /// sprites are skipped).
    pub fn push_list(
        &mut self,
        atlas: &SpriteAtlas,
        list: &DrawList,
        layer: Layer,
        cam_pos: Vec2,
    ) -> Vec<&'static str> {
        let mut missing = Vec::new();
        let first_draw = self.draws.len();
        for item in list.layer(layer) {
            let start = self.indices.len() as u32;
            let (shader, page) = match item {
                DrawItem::Sprite(s) => {
                    let page = self.append_sprite(atlas, s, cam_pos);
                    if page.is_none() {
                        missing.push(s.element);
                    }
                    (s.shader, page.unwrap_or(0))
                }
                DrawItem::Mesh(m) => {
                    self.append_mesh(atlas, m, cam_pos);
                    (m.shader, 0)
                }
            };
            self.record(shader, page, start, self.draws.len() > first_draw);
        }
        missing
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.draws.clear();
    }
}

impl DrawQueue {
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.sprites.clear();
        self.water.clear();
        self.stage.clear();
        self.events.clear();
    }

    /// Queues `batch`'s stage draws after everything queued so far.
    pub fn push_stage(&mut self, batch: &StageBatch) {
        let first = self.stage.draws.len();
        self.stage.append(batch);
        let shapes_before = self.shapes.vertex_count();
        for draw in &self.stage.draws[first..] {
            self.events.push(Event::Stage {
                shapes_before,
                draw: draw.clone(),
            });
        }
    }

    /// Queues the water's surface band, then its body, after everything so far.
    pub fn push_water(&mut self, sprites: &WaterSprites) {
        let start = self.water.len() as u32;
        let split = sprites.push_triangles(&mut self.water) as u32;
        let end = self.water.len() as u32;
        let shapes_before = self.shapes.vertex_count();
        for (shader, vertices) in [
            (WaterShader::Surface, start..split),
            (WaterShader::Deep, split..end),
        ] {
            self.events.push(Event::Water {
                shapes_before,
                shader,
                vertices,
            });
        }
    }

    /// Queues `list`'s items of `layer` after everything queued so far, one draw per
    /// run of items with the same shader. Returns the names of elements missing from
    /// the atlas.
    pub fn push_sprites(
        &mut self,
        atlas: &SpriteAtlas,
        list: &DrawList,
        layer: Layer,
        cam_pos: Vec2,
    ) -> Vec<&'static str> {
        let first = self.sprites.draws.len();
        let missing = self.sprites.push_list(atlas, list, layer, cam_pos);
        let shapes_before = self.shapes.vertex_count();
        for draw in &self.sprites.draws[first..] {
            self.events.push(Event::Sprites {
                shapes_before,
                draw: draw.clone(),
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
                Event::Sprites { shapes_before, .. }
                | Event::Level { shapes_before }
                | Event::Water { shapes_before, .. }
                | Event::Stage { shapes_before, .. } => *shapes_before,
            };
            if shapes_before > shapes_done {
                out.push(Batch::Shapes(shapes_done..shapes_before));
                shapes_done = shapes_before;
            }
            out.push(match event {
                Event::Sprites { draw, .. } => Batch::Sprites(draw.clone()),
                Event::Level { .. } => Batch::Level,
                Event::Water {
                    shader, vertices, ..
                } => Batch::Water(*shader, vertices.clone()),
                Event::Stage { draw, .. } => Batch::Stage(draw.clone()),
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
