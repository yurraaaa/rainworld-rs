//! What the background scene shaders (`background_scene.wgsl`) read besides the sprite
//! batch: the globals the scenes set (`_AboveCloudsAtmosphereColor`, `_MultiplyColor`,
//! `_WorldCamPos`) and the textures `RainWorld.SetGlobalTextures` sets (`_CloudsTex`,
//! `_ApartmentsTex`, `_CityPalette`).
//!
//! Mirrors: RainWorld.cs (SetGlobalTextures)

use rw_assets::PngImage;

/// The scenes' shader globals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneGlobals {
    /// `_AboveCloudsAtmosphereColor` (RGB).
    pub atmosphere: [f32; 3],
    /// `_MultiplyColor` (RGB; alpha is 1).
    pub multiply: [f32; 3],
    /// `_WorldCamPos`.
    pub world_cam_pos: [f32; 2],
}

impl Default for SceneGlobals {
    fn default() -> Self {
        Self {
            atmosphere: [41.0 / 255.0, 59.0 / 255.0, 81.0 / 255.0],
            multiply: [1.0; 3],
            world_cam_pos: [0.0; 2],
        }
    }
}

/// The global textures of the scene shaders.
#[derive(Debug, Clone)]
pub struct SceneTextures {
    /// `_CloudsTex`: every mip level, largest first (bilinear, repeat; the mip is the
    /// nearest one, as Unity's bilinear filter picks).
    pub clouds: Vec<PngImage>,
    /// `_ApartmentsTex` (point, clamp).
    pub apartments: PngImage,
    /// `_CityPalette` (point, clamp).
    pub city_palette: PngImage,
}
