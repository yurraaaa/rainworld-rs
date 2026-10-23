//! `SpotLight`: a placed quad drawn as a black disc in the `Shadows` container
//! (`VectorCircle`). It has no per-tick behaviour in play (`Update` only flags the mesh
//! dirty when the quad changes, which the editor does); its mesh is `rw-graphics`'. No
//! draws.
//!
//! Mirrors: SpotLight.cs (SpotLight)

use rw_assets::QuadData;

use crate::{LightBeam, Vec2};

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
