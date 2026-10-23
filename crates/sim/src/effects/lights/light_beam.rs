//! `LightBeam`: a placed sunbeam, a quad of light in its room's update list. Its
//! `Update` only reads the camera's picture for a beam coloured by the environment;
//! the mesh and the per-frame alpha are `rw-graphics`'. Blinking and night beams (no
//! base beam has either) come with the DLCs that use them (D24). No draws.
//!
//! `SpotLight`: a placed quad drawn as a black disc in the `Shadows` container
//! (`VectorCircle`). It has no per-tick behaviour in play (`Update` only flags the mesh
//! dirty when the quad changes, which the editor does); its mesh is `rw-graphics`'. No
//! draws.
//!
//! Mirrors: LightBeam.cs (LightBeam, Update, GetIdealGridDiv's quad), SpotLight.cs (SpotLight), Room.cs (Loaded: LightBeam, SetLightBeamBlink)

use rw_assets::{LightBeamData, QuadData};

use crate::Vec2;

/// `LightBeam` (its placed data and what `Update` and `ApplyPalette` keep).
#[derive(Debug, Clone, PartialEq)]
pub struct LightBeam {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    pub data: LightBeamData,
    /// `quad`: the placed position and the three handles added to it.
    pub quad: [Vec2; 4],
    /// `environmentColor` (clear black until `Update` reads the camera).
    pub environment_color: [f32; 4],
    /// `paletteLitColor` (clear black until `ApplyPalette`).
    pub palette_lit_color: [f32; 4],
}

impl LightBeam {
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, data: LightBeamData) -> Self {
        Self {
            update_order,
            data,
            quad: Self::quad_of(pos, &data.quad.handles),
            environment_color: [0.0; 4],
            palette_lit_color: [0.0; 4],
        }
    }

    /// `quad[0]` is the position, `quad[i + 1]` the position plus handle `i`.
    #[must_use]
    pub fn quad_of(pos: Vec2, handles: &[[f32; 2]; 3]) -> [Vec2; 4] {
        [
            pos,
            pos + Vec2::from(handles[0]),
            pos + Vec2::from(handles[1]),
            pos + Vec2::from(handles[2]),
        ]
    }

    /// `Update`'s read of camera 0: with `colorB` above 0 and the camera showing the
    /// room, `environmentColor = PixelColorAtCoordinate(quad[1])`.
    #[must_use]
    pub fn wants_environment_color(&self) -> bool {
        self.data.color_b > 0.0
    }

    /// `Update` given the camera's colour at `quad[1]` (when it shows the room).
    pub fn update(&mut self, environment: Option<[f32; 4]>) {
        if self.wants_environment_color()
            && let Some(c) = environment
        {
            self.environment_color = c;
        }
    }

    /// `ApplyPalette`: `palette.texture.GetPixel(8, 5)`.
    pub fn apply_palette(&mut self, c: &crate::CameraImage) {
        self.palette_lit_color = c.palette_texel(8, 5);
    }
}

/// `SpotLight`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpotLight {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    /// `quad`: the placed position and the three handles added to it.
    pub quad: [Vec2; 4],
}

impl SpotLight {
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, data: &QuadData) -> Self {
        Self {
            update_order,
            quad: LightBeam::quad_of(pos, &data.handles),
        }
    }
}

#[cfg(test)]
#[path = "tests/light_beam.rs"]
mod tests;
