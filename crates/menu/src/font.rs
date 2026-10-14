//! `FFont`: a BMFont config placed on an atlas element, and its text layout
//! (`GetQuadInfoForText`). Behaviour: `rainworld-re/notes/menu.md` ("Fonts").
//!
//! Futile's display and resource scales are 1 in Rain World (one resolution level,
//! 1366 wide), so they are left out. Text is walked in UTF-16 units, as C# `char`s.
//! The Thai-only placement rules of `GetQuadInfoForText` are not ported (English only).

use std::collections::HashMap;

use rw_assets::FontConfig;
use rw_render::StageTextureId;

use crate::atlas::Element;
use crate::geometry::Rect;

/// `FCharInfo`, in stage pixels after the element's and the font's scale.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharInfo {
    pub id: i32,
    /// Position in the element's untrimmed image, before `font_scale`.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Texture coordinates, `y` from the bottom.
    pub uv_rect: Rect,
    pub offset_x: f32,
    pub offset_y: f32,
    pub xadvance: f32,
    pub page: i32,
    pub letter: Option<String>,
    /// Set on characters added by [`Font::combine_with`]: a line ending after one of
    /// them uses this line height.
    pub specific_line_height: f32,
}

impl CharInfo {
    /// Texture coordinates of the corners TL, TR, BR, BL.
    #[must_use]
    pub fn uv_corners(&self) -> [[f32; 2]; 4] {
        let r = &self.uv_rect;
        [
            [r.x_min(), r.y_max()],
            [r.x_max(), r.y_max()],
            [r.x_max(), r.y_min()],
            [r.x_min(), r.y_min()],
        ]
    }
}

/// `FTextParams`: extra spacing added to every kerning and line height.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextParams {
    pub kerning_offset: f32,
    pub line_height_offset: f32,
}

/// `FLetterQuad`: one character's rectangle, then its corners once placed.
#[derive(Debug, Clone, PartialEq)]
pub struct LetterQuad {
    /// The character's [`CharInfo`] key (0, the empty character, when the font lacks it).
    pub char_id: u32,
    pub rect: Rect,
    /// TL, TR, BR, BL in the label's local space, set by the label.
    pub corners: [[f32; 2]; 4],
}

/// `FLetterQuadLine`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LetterQuadLine {
    pub bounds: Rect,
    pub quads: Vec<LetterQuad>,
}

/// One loaded font.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub name: String,
    /// The atlas texture all its characters are on.
    pub texture: StageTextureId,
    chars: HashMap<u32, CharInfo>,
    /// `(first, second)` to the amount; the last line of a pair wins, as the game's
    /// search keeps the last match.
    kernings: HashMap<(i32, i32), f32>,
    line_height: f32,
    max_char_width: f32,
    text_params: TextParams,
    offset: [f32; 2],
}

impl Font {
    /// `Custom.GetFont()` for English.
    pub const FONT: &'static str = "font";
    /// `Custom.GetDisplayFont()` for English (`bigText` labels).
    pub const DISPLAY_FONT: &'static str = "DisplayFont";

    /// `new FFont(name, element, config, offsetX, offsetY, new FTextParams(), fontScale)`
    /// (`LoadAndParseConfigFile`).
    #[must_use]
    pub fn new(
        name: &str,
        element: &Element,
        config: &FontConfig,
        offset: [f32; 2],
        font_scale: f32,
    ) -> Self {
        let ratio = element.source_pixel_size[0] / config.config_width as f32;
        let [tex_w, tex_h] = element.texture_size;
        let mut chars = HashMap::new();
        chars.insert(0, CharInfo::default());
        let mut max_char_width = 0.0_f32;
        for c in &config.chars {
            let x = c.x as f32 * ratio - element.source_rect.x;
            let y = c.y as f32 * ratio - element.source_rect.y;
            let width = c.width as f32 * ratio;
            let height = c.height as f32 * ratio;
            let uv_rect = Rect::new(
                element.uv_rect.x + x / tex_w,
                (tex_h - y - height) / tex_h - (1.0 - element.uv_rect.y_max()),
                width / tex_w,
                height / tex_h,
            );
            let info = CharInfo {
                id: c.id,
                x,
                y,
                width: width * font_scale,
                height: height * font_scale,
                uv_rect,
                offset_x: c.xoffset as f32 * ratio * font_scale,
                offset_y: c.yoffset as f32 * ratio * font_scale,
                xadvance: c.xadvance as f32 * ratio * font_scale,
                page: c.page,
                letter: c.letter.clone(),
                specific_line_height: 0.0,
            };
            max_char_width = max_char_width.max(info.width);
            chars.insert(c.id as u32, info);
        }
        if let Some(space) = chars.get_mut(&32) {
            space.offset_x = 0.0;
            space.offset_y = 0.0;
        }
        let kernings = if config.kernings_count.is_some() {
            config
                .kernings
                .iter()
                .filter(|k| k.first != -1)
                .map(|k| ((k.first, k.second), k.amount as f32 * ratio))
                .collect()
        } else {
            HashMap::new()
        };
        Self {
            name: name.to_owned(),
            texture: element.texture,
            chars,
            kernings,
            line_height: config.line_height as f32 * ratio,
            max_char_width,
            text_params: TextParams::default(),
            offset,
        }
    }

    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    #[must_use]
    pub fn max_char_width(&self) -> f32 {
        self.max_char_width
    }

    #[must_use]
    pub fn offset(&self) -> [f32; 2] {
        self.offset
    }

    /// The character info of `id` (0 is the empty character every font has).
    #[must_use]
    pub fn char_info(&self, id: u32) -> Option<&CharInfo> {
        self.chars.get(&id)
    }

    /// Number of character infos, the empty character included.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.chars.len()
    }

    #[must_use]
    pub fn kerning(&self, first: i32, second: i32) -> Option<f32> {
        self.kernings.get(&(first, second)).copied()
    }

    #[must_use]
    pub fn kerning_count(&self) -> usize {
        self.kernings.len()
    }

    /// `CombineWithFont`: takes `add`'s characters this font lacks, centring them on
    /// this font's line (`offset_y` more) and giving them `add`'s line height.
    pub fn combine_with(&mut self, add: &Self, offset_y: f32) {
        for (id, info) in &add.chars {
            if self.chars.contains_key(id) {
                continue;
            }
            let mut info = info.clone();
            info.specific_line_height = add.line_height;
            info.offset_y += (self.line_height - add.line_height) / 2.0 + offset_y;
            self.chars.insert(*id, info);
        }
        self.max_char_width = self.max_char_width.max(add.max_char_width);
    }

    /// `GetQuadInfoForText`: one line per `\n`, each character's rectangle (`y` up,
    /// the first line's top at 0) and each line's bounds.
    #[must_use]
    pub fn quad_lines(&self, text: &str, label_params: &TextParams) -> Vec<LetterQuadLine> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut lines = Vec::new();
        let mut line = LetterQuadLine::default();
        let mut x = 0.0_f32;
        let mut y = 0.0_f32;
        let mut last_found: Option<&CharInfo> = None;
        let mut previous = 0u16;
        let mut bounds = LineBounds::new();
        let mut line_height = self.line_height
            + label_params.line_height_offset
            + self.text_params.line_height_offset;
        for &unit in &units {
            if unit == u16::from(b'\n') {
                if let Some(info) = last_found.filter(|i| i.specific_line_height != 0.0) {
                    line_height = info.specific_line_height
                        + label_params.line_height_offset
                        + self.text_params.line_height_offset;
                }
                line.bounds = bounds.rect(&line, y, line_height);
                lines.push(std::mem::take(&mut line));
                bounds = LineBounds::new();
                x = 0.0;
                y -= line_height;
            } else {
                let kerning = self
                    .kerning(i32::from(previous), i32::from(unit))
                    .unwrap_or(0.0);
                let found = self.chars.get(&u32::from(unit));
                let (char_id, info) = match found {
                    Some(info) => (u32::from(unit), info),
                    None => (0, &self.chars[&0]),
                };
                let spacing =
                    kerning + label_params.kerning_offset + self.text_params.kerning_offset;
                x = if line.quads.is_empty() {
                    -info.offset_x
                } else {
                    x + spacing
                };
                let rect = Rect::new(
                    x + info.offset_x,
                    y - info.offset_y - info.height,
                    info.width,
                    info.height,
                );
                bounds.add(&rect, y, line_height);
                line.quads.push(LetterQuad {
                    char_id,
                    rect,
                    corners: [[0.0; 2]; 4],
                });
                x += info.xadvance;
                if found.is_some() {
                    last_found = Some(info);
                }
            }
            previous = unit;
        }
        line.bounds = bounds.rect(&line, y, line_height);
        lines.push(line);
        lines
    }
}

/// The running min/max of one line in `GetQuadInfoForText`.
struct LineBounds {
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
}

impl LineBounds {
    fn new() -> Self {
        Self {
            x_min: f32::MAX,
            x_max: f32::MIN,
            y_min: f32::MAX,
            y_max: f32::MIN,
        }
    }

    fn add(&mut self, rect: &Rect, y: f32, line_height: f32) {
        self.x_min = self.x_min.min(rect.x_min());
        self.x_max = self.x_max.max(rect.x_max());
        self.y_min = self.y_min.min(y - line_height);
        self.y_max = self.y_max.max(y);
    }

    /// The line's bounds; an empty line gets the game's `Rect(0, 0, y, y - lineHeight)`.
    fn rect(&self, line: &LetterQuadLine, y: f32, line_height: f32) -> Rect {
        if line.quads.is_empty() {
            Rect::new(0.0, 0.0, y, y - line_height)
        } else {
            Rect::new(
                self.x_min,
                self.y_min,
                self.x_max - self.x_min,
                self.y_max - self.y_min,
            )
        }
    }
}
