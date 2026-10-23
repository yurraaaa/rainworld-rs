//! Futile's stage on the CPU: textured quads in screen pixels (y up, origin at the
//! bottom-left, as `Futile.stage` with origin (0, 0)), each drawn with one texture and
//! one of the game's shaders. Consecutive quads with the same texture and shader share
//! a draw, as Futile's render layers batch facets of one atlas and shader.
//!
//! Mirrors: FStage.cs, Futile.cs

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
    /// `Futile/FlatLight`: a soft disc of the vertex colour (`DarkGradient`).
    FlatLight,
    /// `Futile/VectorCircle`: an opaque ring whose width is the vertex alpha.
    VectorCircle,
    /// `Futile/VectorCircleFadable`: a ring in a palette colour picked by red, faded by
    /// blue.
    VectorCircleFadable,
    /// `Futile/HoldButtonCircle`: the hold button's fill arc, swept to the vertex alpha.
    HoldButtonCircle,
    /// `Futile/MenuText`: title lettering coloured from the text gradient.
    MenuText,
    /// `Futile/FlatLightNoisy`: `FlatLight` with a falloff bent by `_NoiseTex2` read at
    /// the screen position within `_spriteRect` (`RainEffect`'s light source).
    FlatLightNoisy,
    /// `Futile/Map`: a layer of the region map, read from the map texture at the pan
    /// position and revealed by `_mapFogTexture`.
    Map,
    /// `Futile/MapAerial`: a sky layer of the region map.
    MapAerial,
    /// `Futile/MapShortcut`: the dashed line of a room connection on the map.
    MapShortcut,
    /// `Futile/LocalBloom`: a disc that brightens the frame drawn before it with a blur
    /// of itself (its `GrabPass`), by the vertex alpha (the karma ladder's bloom).
    LocalBloom,
    /// `Futile/SceneBlur`: a menu depth illustration (colour over a depth map), shifted
    /// per pixel by its depth and blurred by its distance from the focus; the vertex
    /// colour carries the sprite's size and depth ([`MenuSceneGlobals`]).
    SceneBlur,
    /// `Futile/SceneBlurLightEdges`: `SceneBlur` with the blur taps not weighted by
    /// their alpha.
    SceneBlurLightEdges,
    /// `Futile/SceneLighten`: a screen blend of the unblurred illustration over its
    /// `GrabPass`.
    SceneLighten,
    /// `Futile/SceneOverlay`: the `SceneBlurLightEdges` blur, overlay-blended over its
    /// `GrabPass`.
    SceneOverlay,
    /// `Futile/SceneSoftLight`: as `SceneOverlay` with a soft-light blend.
    SceneSoftLight,
}

impl StageShader {
    /// Whether the shader has a `GrabPass` (reads the frame drawn before it).
    #[must_use]
    pub fn grabs(self) -> bool {
        matches!(
            self,
            Self::LocalBloom | Self::SceneLighten | Self::SceneOverlay | Self::SceneSoftLight
        )
    }
}

/// The menu scenes' shader globals, as `MenuScene.GrafUpdate` sets them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MenuSceneGlobals {
    /// `_BlurDepth`: the depth in focus, 1 to 10.
    pub blur_depth: f32,
    /// `_BlurRange`: blur per unit of depth away from the focus.
    pub blur_range: f32,
    /// `_MenuCamPos`: the depth camera, 0..1 on each axis.
    pub cam_pos: [f32; 2],
}

/// The region map's shader globals (`_mapSize`, `_mapPan`, `_MapCol`, `_MapWaterCol`,
/// `_mapFogTexture`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MapGlobals {
    pub size: [f32; 2],
    /// The pan position as a fraction of `size`.
    pub pan: [f32; 2],
    pub color: [f32; 4],
    pub water_color: [f32; 4],
    pub fog: Option<StageTextureId>,
}

/// Shader globals of the stage (`Shader.SetGlobal*`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StageGlobals {
    /// `_RAIN`: `ProcessManager.shadersTime / 5`.
    pub rain: f32,
    /// `_TextGradientTex`, a stage texture.
    pub text_gradient: Option<StageTextureId>,
    /// `_NoiseTex2` (`Palettes/noise2`).
    pub noise2: Option<StageTextureId>,
    /// `_spriteRect`: set only by a room camera; (0, 0, 0, 0), Unity's value for an unset
    /// global, in the menus.
    pub sprite_rect: [f32; 4],
    pub map: MapGlobals,
    pub menu_scene: MenuSceneGlobals,
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

impl TextureSampling {
    /// A crisp, clamped texture (`crispPixels`, `clampWrapMode`).
    pub const POINT_CLAMP: Self = Self {
        filter: TextureFilter::Point,
        clamp: true,
    };

    /// A Texture2D's import settings: `filter_mode` 0 is point, 1 bilinear, 2 trilinear
    /// (sampled bilinear: no mips are uploaded); `wrap` is U and V, 0 repeat, 1 clamp.
    #[must_use]
    pub fn from_import(filter_mode: i32, wrap: [i32; 2]) -> Self {
        Self {
            filter: if filter_mode == 0 {
                TextureFilter::Point
            } else {
                TextureFilter::Bilinear
            },
            clamp: wrap == [1, 1],
        }
    }
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
