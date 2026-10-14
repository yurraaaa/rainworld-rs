//! Futile's stage on the CPU: textured quads in screen pixels (y up, origin at the
//! bottom-left, as `Futile.stage` with origin (0, 0)), each drawn with one texture and
//! one of the game's shaders. Consecutive quads with the same texture and shader share
//! a draw, as Futile's render layers batch facets of one atlas and shader.

use std::ops::Range;

use crate::SpriteVertex;

/// A texture uploaded with [`Renderer::set_stage_texture`](crate::Renderer::set_stage_texture).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StageTextureId(pub u32);

/// The game shader a stage quad is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StageShader {
    /// `Futile/Basic`: texel times vertex colour, alpha blended.
    Basic,
    /// `Futile/EdgeFade`: the vertex colour with an alpha that rises from the quad's
    /// centre to its edges, the process-switch fade (`ProcessManager.fadeSprite`).
    EdgeFade,
}

/// Unity's `filterMode` for a stage texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFilter {
    Point,
    Bilinear,
}

/// How a stage texture is sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureSampling {
    pub filter: TextureFilter,
    /// `TextureWrapMode.Clamp`, else repeat.
    pub clamp: bool,
}

/// One draw call: `indices` of the batch with one texture and shader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDraw {
    pub texture: StageTextureId,
    pub shader: StageShader,
    pub indices: Range<u32>,
}

/// A frame's stage quads in draw order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StageBatch {
    /// Positions in screen pixels.
    pub vertices: Vec<SpriteVertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<StageDraw>,
}

impl StageBatch {
    /// Appends a quad (TL, TR, BR, BL), extending the last draw when it has the same
    /// texture and shader.
    pub fn push_quad(
        &mut self,
        texture: StageTextureId,
        shader: StageShader,
        quad: [SpriteVertex; 4],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.extend(quad);
        let start = self.indices.len() as u32;
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
        let end = self.indices.len() as u32;
        match self.draws.last_mut() {
            Some(last) if last.texture == texture && last.shader == shader => {
                last.indices.end = end;
            }
            _ => self.draws.push(StageDraw {
                texture,
                shader,
                indices: start..end,
            }),
        }
    }

    /// Appends `other`'s quads after these.
    pub fn append(&mut self, other: &Self) {
        let base = self.vertices.len() as u32;
        let offset = self.indices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base));
        self.draws.extend(other.draws.iter().map(|d| StageDraw {
            indices: d.indices.start + offset..d.indices.end + offset,
            ..d.clone()
        }));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.draws.is_empty()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.draws.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_texture_and_shader_share_a_draw() {
        let mut b = StageBatch::default();
        let q = [SpriteVertex::default(); 4];
        b.push_quad(StageTextureId(1), StageShader::Basic, q);
        b.push_quad(StageTextureId(1), StageShader::Basic, q);
        b.push_quad(StageTextureId(2), StageShader::Basic, q);
        b.push_quad(StageTextureId(2), StageShader::EdgeFade, q);
        let ranges: Vec<_> = b.draws.iter().map(|d| d.indices.clone()).collect();
        assert_eq!(ranges, [0..12, 12..18, 18..24]);
        assert_eq!(b.indices[6..12], [4, 5, 6, 4, 6, 7]);
        let mut all = b.clone();
        all.append(&b);
        assert_eq!(all.draws.len(), 6);
        assert_eq!(all.draws[3].indices, 24..36);
        assert_eq!(all.indices[24], 16);
    }
}
