//! What the background scene shaders (`background_scene.wgsl`) read besides the sprite
//! batch: the globals the scenes set (`_AboveCloudsAtmosphereColor`, `_MultiplyColor`,
//! `_WorldCamPos`) and the textures `RainWorld.SetGlobalTextures` sets (`_CloudsTex`,
//! `_ApartmentsTex`, `_CityPalette`).
//!
//! Differences from the game: D31.
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
    /// The anisotropy level `_CloudsTex` is sampled with (1 is off).
    pub clouds_aniso: u16,
    /// `_ApartmentsTex` (point, clamp).
    pub apartments: PngImage,
    /// `_CityPalette` (point, clamp).
    pub city_palette: PngImage,
}

impl SceneTextures {
    /// The sampler's `anisotropy_clamp`: wgpu takes 1 for off, and the clouds texture's
    /// filters are linear, as wgpu requires.
    #[must_use]
    pub fn clouds_anisotropy_clamp(&self) -> u16 {
        self.clouds_aniso.clamp(1, 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textures(aniso: u16) -> SceneTextures {
        let image = PngImage::from_pixels(1, 1, vec![[0; 4]]);
        SceneTextures {
            clouds: vec![image.clone()],
            clouds_aniso: aniso,
            apartments: image.clone(),
            city_palette: image,
        }
    }

    #[test]
    fn the_clouds_sampler_is_anisotropic_only_above_level_one() {
        assert_eq!(textures(1).clouds_anisotropy_clamp(), 1);
        assert_eq!(textures(0).clouds_anisotropy_clamp(), 1);
        assert_eq!(textures(4).clouds_anisotropy_clamp(), 4);
    }
}
