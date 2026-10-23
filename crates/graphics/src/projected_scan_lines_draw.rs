//! The sprite of a `ProjectedScanLines` line: one black `pixel` bar across the whole screen
//! in `Shadows`, placed in stage pixels as [`crate::projector_draw`]'s (the unset
//! coordinate is 0, the size hard-coded 768 and 1366).
//!
//! Mirrors: ProjectedScanLines.cs (ScanLine: InitiateSprites, DrawSprites, AddToContainer)

use rw_sim::{FloatExt, ProjectedScanLines, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

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
