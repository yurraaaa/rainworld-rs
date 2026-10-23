//! The sprites of a `SuperStructureProjector`'s parts, all in `Shadows`: a scan line is two
//! black `pixel` bars 15 px apart across the whole screen, a cursor four bars boxing a
//! square of the 15 px grid with a `glyphs` sprite in it, a single glyph one grid cell of
//! `glyphs`, a matrix a block of cells. The glyph sprites use the `GlyphProjection` shader,
//! which reads the screen position, not the sprite's own, and the projector's
//! `_gridOffset` ([`rw_sim::SuperStructureProjector::grid_offset`]).
//!
//! The bars are placed as the game does, in stage pixels (the sprite's position is
//! relative to the stage, not the room): the unset coordinate of a bar is 0, so a
//! vertical bar spans the stage from its bottom edge up 768 px, a horizontal bar from its
//! left edge 1366 px (not the chosen screen size: the game hard-codes both).
//!
//! The sprite of a `ProjectedScanLines` line: one black `pixel` bar across the whole screen
//! in `Shadows`, placed in stage pixels as the projector's (the unset coordinate is 0, the
//! size hard-coded 768 and 1366).
//!
//! Mirrors: SuperStructureProjector.cs (ScanLine, Cursor, SingleGlyph, GlyphMatrix: InitiateSprites, DrawSprites, AddToContainer), ProjectedScanLines.cs (ScanLine: InitiateSprites, DrawSprites, AddToContainer)

use rw_sim::{
    Cursor, FloatExt, GlyphMatrix, PartKind, ProjectedScanLines, ScanLine, SingleGlyph,
    SuperStructureProjector, Vec2,
};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// What the draw reads of the camera: `camPos` (the frame's origin) and
/// `room.cameraPositions[rCam.currentCameraPosition]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectionView {
    pub cam_pos: Vec2,
    pub camera_position: Vec2,
}

/// The sprites of a projector's parts.
pub trait ProjectorSprites {
    /// `DrawSprites` of the part at `order` at `t` between ticks (empty while the
    /// projector is not visible, or for a glyph gone inactive).
    fn draw_part(&self, order: u64, t: f32, view: &ProjectionView) -> DrawList;
}

impl ProjectorSprites for SuperStructureProjector {
    fn draw_part(&self, order: u64, t: f32, view: &ProjectionView) -> DrawList {
        let Some(part) = self.part_index(order).map(|i| &self.parts[i]) else {
            return DrawList::default();
        };
        if !self.visible || part.slated {
            return DrawList::default();
        }
        let draw = PartDraw {
            projector: self,
            t,
            view,
        };
        let items = match part.kind {
            PartKind::ScanLine(line) => draw.scan_line(&line),
            PartKind::Cursor(cursor) => draw.cursor(&cursor),
            PartKind::Single(single) => draw.single(&single),
            PartKind::Matrix(matrix) => draw.matrix(&matrix),
        };
        DrawList { items }
    }
}

/// One part's draw: the projector, the time between ticks and the camera.
struct PartDraw<'a> {
    projector: &'a SuperStructureProjector,
    t: f32,
    view: &'a ProjectionView,
}

impl PartDraw<'_> {
    /// A black `pixel` bar at `stage` (relative to the stage origin).
    fn bar(&self, index: usize, stage: Vec2, scale: Vec2, anchor: Vec2) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "pixel",
            pos: stage + self.view.cam_pos,
            rotation: 0.0,
            scale,
            anchor,
            color: Color::BLACK,
            shader: Shader::Basic,
            layer: Layer::Shadows,
        })
    }

    /// A `glyphs` sprite under the `GlyphProjection` shader.
    fn glyphs(index: usize, pos: Vec2, scale: Vec2, anchor: Vec2, color: Color) -> DrawItem {
        DrawItem::Sprite(Sprite {
            index,
            element: "glyphs",
            pos,
            rotation: 0.0,
            scale,
            anchor,
            color,
            shader: Shader::GlyphProjection,
            layer: Layer::Shadows,
        })
    }

    /// `ScanLine.DrawSprites`: two bars 15 px apart.
    fn scan_line(&self, line: &ScanLine) -> Vec<DrawItem> {
        let cam = self.view.cam_pos;
        let along = line.last_pos.lerp(line.pos, self.t);
        (0..2)
            .map(|j| {
                let offset = 15.0 * j as f32;
                if line.alignment {
                    let stage = Vec2::new(along - cam.x + offset, 0.0);
                    self.bar(j, stage, Vec2::new(2.0, 768.0), Vec2::new(0.5, 0.0))
                } else {
                    let stage = Vec2::new(0.0, along - cam.y + offset);
                    self.bar(j, stage, Vec2::new(1366.0, 2.0), Vec2::new(0.0, 0.5))
                }
            })
            .collect()
    }

    /// `Cursor.DrawSprites`: four bars round the cell block and the glyph sprite.
    fn cursor(&self, cursor: &Cursor) -> Vec<DrawItem> {
        let room = cursor.last_pos.lerp(cursor.pos, self.t);
        let v = room - self.view.cam_pos;
        let span = 15.0 * cursor.size as f32;
        let corner = Vec2::new(0.0, 0.0);
        let tall = Vec2::new(2.0, 768.0);
        let wide = Vec2::new(1366.0, 2.0);
        vec![
            self.bar(0, Vec2::new(v.x, 0.0), tall, corner),
            self.bar(1, Vec2::new(v.x + span, 0.0), tall, corner),
            self.bar(2, Vec2::new(0.0, v.y), wide, corner),
            self.bar(3, Vec2::new(0.0, v.y + span), wide, corner),
            Self::glyphs(
                4,
                room,
                Vec2::new(span / 750.0, span / 15.0),
                corner,
                Color::WHITE,
            ),
        ]
    }

    /// `SingleGlyph.DrawSprites`: one cell, while it is not inactive.
    fn single(&self, single: &SingleGlyph) -> Vec<DrawItem> {
        if single.inactive {
            return Vec::new();
        }
        let pos = self.projector.grid_pos_at(single.glyph.pos, self.t) + self.view.camera_position;
        let selected = if single.sprite_selected { 1.0 } else { 0.0 };
        vec![Self::glyphs(
            0,
            pos,
            Vec2::new(0.02, 1.0),
            Vec2::new(0.0, 0.0),
            Color::rgb(selected, 0.0, 0.0),
        )]
    }

    /// `GlyphMatrix.DrawSprites`: a block of cells about its cell, odd sizes shifted half
    /// a cell down and left; faded with its life.
    fn matrix(&self, matrix: &GlyphMatrix) -> Vec<DrawItem> {
        let mut pos =
            self.projector.grid_pos_at(matrix.glyph.pos, self.t) + self.view.camera_position;
        if matrix.size.0 % 2 == 1 {
            pos.x -= 7.5;
        }
        if matrix.size.1 % 2 == 1 {
            pos.y -= 7.5;
        }
        let alpha = (matrix.glyph.counter as f32).inverse_lerp(matrix.glyph.life as f32, 0.0) * 0.5;
        vec![Self::glyphs(
            0,
            pos,
            Vec2::new(
                matrix.size.0 as f32 * 15.0 / 750.0,
                matrix.size.1 as f32 * 15.0 / 15.0,
            ),
            Vec2::new(0.5, 0.5),
            Color::rgb(0.0, 0.0, matrix.glyph.glyph_col).with_alpha(alpha),
        )]
    }
}

/// A scan line's sprite.
pub trait ProjectedScanLinesSprites {
    /// `DrawSprites` of the line at `order` at `t` between ticks with the frame's
    /// `cam_pos` (empty while the effect is not visible).
    fn draw_line(&self, order: u64, t: f32, cam_pos: Vec2) -> DrawList;
}

impl ProjectedScanLinesSprites for ProjectedScanLines {
    fn draw_line(&self, order: u64, t: f32, cam_pos: Vec2) -> DrawList {
        let Some(line) = self.line_index(order).map(|i| &self.lines[i]) else {
            return DrawList::default();
        };
        if !self.visible || line.slated {
            return DrawList::default();
        }
        let along = line.last_pos.lerp(line.pos, t);
        let (stage, scale, anchor) = if line.alignment {
            (
                Vec2::new(along - cam_pos.x, 0.0),
                Vec2::new(2.0, 768.0),
                Vec2::new(0.5, 0.0),
            )
        } else {
            (
                Vec2::new(0.0, along - cam_pos.y),
                Vec2::new(1366.0, 2.0),
                Vec2::new(0.0, 0.5),
            )
        };
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "pixel",
                pos: stage + cam_pos,
                rotation: 0.0,
                scale,
                anchor,
                color: Color::BLACK,
                shader: Shader::Basic,
                layer: Layer::Shadows,
            })],
        }
    }
}

#[cfg(test)]
#[path = "tests/projector.rs"]
pub(crate) mod tests;
