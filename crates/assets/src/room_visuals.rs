//! Everything the level graphic of one room needs from disk: settings, palettes with
//! effect colours, and the per-camera images (`<room>_<N>.png`, loaded on demand like
//! the game's camera texture).
//!
//! Mirrors: -

use std::path::PathBuf;

use crate::{
    AssetError, EffectColors, PaletteImage, PaletteTexture, PngImage, RoomSettings, StreamingAssets,
};

/// Static visual data of a room, built as `RoomCamera.ChangeRoom` does.
#[derive(Debug, Clone)]
pub struct RoomVisuals {
    settings: RoomSettings,
    palette_a: PaletteImage,
    /// The `FadePalette` palette, absent without one or when it is -1.
    palette_b: Option<PaletteImage>,
    /// `<room>_<N>.png` per camera, where found.
    images: Vec<Option<PathBuf>>,
}

impl RoomVisuals {
    /// Loads the settings (see [`RoomSettings::load`], region from the room's location),
    /// the palettes with `EffectColorA/B` applied, and locates `cameras` images.
    ///
    /// # Errors
    /// When the settings, a palette or `effectcolors.png` cannot be loaded.
    pub fn load(
        assets: &StreamingAssets,
        room: &str,
        cameras: usize,
        timeline: Option<&str>,
    ) -> Result<Self, AssetError> {
        let room = room.to_lowercase();
        let region = assets.region_of_room(&room);
        let settings = RoomSettings::load(assets, &room, region.as_deref(), timeline)?;
        let effects = EffectColors::load(assets)?;
        let palette_a = Self::palette(assets, &effects, &settings, settings.palette)?;
        let palette_b = match &settings.fade_palette {
            Some(fade) if fade.palette != -1 => {
                Some(Self::palette(assets, &effects, &settings, fade.palette)?)
            }
            _ => None,
        };
        let images = (0..cameras)
            .map(|i| assets.find_room_file(&room, &format!("_{}.png", i + 1)))
            .collect();
        Ok(Self {
            settings,
            palette_a,
            palette_b,
            images,
        })
    }

    /// `LoadPalette` with the room's effect colours.
    fn palette(
        assets: &StreamingAssets,
        effects: &EffectColors,
        settings: &RoomSettings,
        index: i32,
    ) -> Result<PaletteImage, AssetError> {
        let mut palette = PaletteImage::load(assets, index)?;
        palette
            .apply_effect_colors(effects, settings.effect_color_a, settings.effect_color_b)
            .map_err(|m| AssetError::format(&assets.file("palettes/effectcolors.png"), m))?;
        Ok(palette)
    }

    #[must_use]
    pub fn settings(&self) -> &RoomSettings {
        &self.settings
    }

    /// The palette texture while `camera` is shown: palette A faded toward the
    /// `FadePalette` palette by that camera's amount (0 past the list). No rain darkening.
    #[must_use]
    pub fn palette_for_camera(&self, camera: usize) -> PaletteTexture {
        let fade = self
            .settings
            .fade_palette
            .as_ref()
            .zip(self.palette_b.as_ref())
            .map(|(f, b)| (b, f.fade(camera)));
        PaletteTexture::blend(&self.palette_a, fade, 0.0)
    }

    /// Decodes the image of `camera`.
    ///
    /// # Errors
    /// [`AssetError::Missing`] when the room has no image for it, else as
    /// [`PngImage::load`].
    pub fn camera_image(&self, camera: usize) -> Result<PngImage, AssetError> {
        let path = self
            .images
            .get(camera)
            .and_then(Option::as_ref)
            .ok_or_else(|| AssetError::Missing {
                what: format!("image of camera {}", camera + 1),
            })?;
        PngImage::load(path)
    }
}
