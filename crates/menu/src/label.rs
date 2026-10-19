//! `FLabel`: text in one font, laid out by [`Font::quad_lines`] and placed around the
//! label's position by its anchor (`UpdateLocalPosition`). Behaviour:
//! `rainworld-re/notes/menu.md` ("Fonts").
//!
//! Mirrors: FLabel.cs

use std::sync::Arc;

use rw_render::{SpriteVertex, StageBatch, StageShader};

use crate::font::{Font, LetterQuadLine, TextParams};
use crate::geometry::{Matrix, Rect};
use crate::stage::{NodeTransform, StageColor};

/// `FLabelAlignment`: sets the horizontal anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Center,
    Left,
    Right,
}

/// One `FLabel`.
#[derive(Debug, Clone)]
pub struct Label {
    font: Arc<Font>,
    text: String,
    params: TextParams,
    anchor: [f32; 2],
    lines: Vec<LetterQuadLine>,
    text_rect: Rect,
    pub transform: NodeTransform,
    /// `label.color`; its alpha multiplies the node alpha.
    pub color: StageColor,
    pub shader: StageShader,
}

impl Label {
    /// `FLabel.defaultAnchorX/Y`.
    pub const DEFAULT_ANCHOR: [f32; 2] = [0.5, 0.5];

    #[must_use]
    pub fn new(font: Arc<Font>, text: &str) -> Self {
        Self::with_params(font, text, TextParams::default())
    }

    #[must_use]
    pub fn with_params(font: Arc<Font>, text: &str, params: TextParams) -> Self {
        let mut label = Self {
            font,
            text: text.to_owned(),
            params,
            anchor: Self::DEFAULT_ANCHOR,
            lines: Vec::new(),
            text_rect: Rect::default(),
            transform: NodeTransform::default(),
            color: StageColor::WHITE,
            shader: StageShader::Basic,
        };
        label.create_text_quads();
        label
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: &str) {
        if self.text != text {
            text.clone_into(&mut self.text);
            self.create_text_quads();
        }
    }

    #[must_use]
    pub fn anchor(&self) -> [f32; 2] {
        self.anchor
    }

    pub fn set_anchor(&mut self, anchor: [f32; 2]) {
        if self.anchor != anchor {
            self.anchor = anchor;
            self.update_local_position();
        }
    }

    pub fn set_alignment(&mut self, alignment: Alignment) {
        let x = match alignment {
            Alignment::Center => 0.5,
            Alignment::Left => 0.0,
            Alignment::Right => 1.0,
        };
        self.set_anchor([x, self.anchor[1]]);
    }

    /// The text's bounds around the label's position, before its transform.
    #[must_use]
    pub fn text_rect(&self) -> Rect {
        self.text_rect
    }

    #[must_use]
    pub fn lines(&self) -> &[LetterQuadLine] {
        &self.lines
    }

    #[must_use]
    pub fn font(&self) -> &Arc<Font> {
        &self.font
    }

    fn create_text_quads(&mut self) {
        self.lines = self.font.quad_lines(&self.text, &self.params);
        self.update_local_position();
    }

    /// `UpdateLocalPosition`: shifts the lines so the anchor point of the whole text
    /// (vertically) and of each line (horizontally) is at the label's origin.
    fn update_local_position(&mut self) {
        let y_min = self
            .lines
            .iter()
            .map(|l| l.bounds.y_min())
            .fold(f32::MAX, f32::min);
        let y_max = self
            .lines
            .iter()
            .map(|l| l.bounds.y_max())
            .fold(f32::MIN, f32::max);
        let dy = -(y_min + (y_max - y_min) * self.anchor[1]);
        let mut x_min = f32::MAX;
        let mut x_max = f32::MIN;
        let [font_x, font_y] = self.font.offset();
        for line in &mut self.lines {
            let dx = -line.bounds.width * self.anchor[0];
            x_min = x_min.min(dx);
            x_max = x_max.max(dx + line.bounds.width);
            for quad in &mut line.quads {
                let (ox, oy) = (dx + font_x, dy + font_y);
                let r = &quad.rect;
                quad.corners = [
                    [r.x_min() + ox, r.y_max() + oy],
                    [r.x_max() + ox, r.y_max() + oy],
                    [r.x_max() + ox, r.y_min() + oy],
                    [r.x_min() + ox, r.y_min() + oy],
                ];
            }
        }
        self.text_rect = Rect::new(x_min, y_min + dy, x_max - x_min, y_max - y_min);
    }

    /// `PopulateRenderLayer`: every character's quad (TL, TR, BR, BL) through the
    /// label's concatenated `matrix`, coloured with `color` at `alpha` (the node's
    /// concatenated alpha).
    pub fn push_quads(&self, batch: &mut StageBatch, matrix: &Matrix, alpha: f32) {
        let color = self.color.with_alpha_multiplied(alpha).to_array();
        for line in &self.lines {
            for quad in &line.quads {
                let uvs = self
                    .font
                    .char_info(quad.char_id)
                    .map_or([[0.0; 2]; 4], |c| c.uv_corners());
                let vertices = std::array::from_fn(|i| SpriteVertex {
                    pos: matrix.apply(quad.corners[i]),
                    uv: uvs[i],
                    color,
                });
                batch.push_quad(self.font.texture, self.shader, vertices);
            }
        }
    }
}
