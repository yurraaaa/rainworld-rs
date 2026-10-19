//! The room's level graphic: settings, palettes and the shown camera's image (the
//! scene's room camera chooses which).
//!
//! Mirrors: RoomCamera.cs

use rw_assets::{
    AssetError, NoiseTexture, PaletteTexture, PngImage, Room, RoomVisuals, StreamingAssets,
    UnityResources,
};
use rw_render::{LevelGraphics, LevelParams, LevelView, Renderer, RoomCamera};

/// Level graphic state of one room.
pub struct Visuals {
    room: RoomVisuals,
    noise: NoiseTexture,
    /// The camera whose image is shown.
    shown: usize,
    /// Every camera's image, decoded once at load so switching never stalls.
    images: Vec<PngImage>,
    palette: PaletteTexture,
    /// Counts camera switches, so a renderer knows when its upload is stale.
    generation: u32,
}

impl Visuals {
    /// Loads the room's settings (the `timeline`'s `_settings-<timeline>.txt` variant
    /// where there is one), palettes and every camera's image, and the noise texture
    /// from the game's Unity resources, showing camera 0.
    ///
    /// # Errors
    /// When any of those cannot be loaded.
    pub fn load(
        assets: &StreamingAssets,
        resources: &UnityResources,
        room: &Room,
        timeline: &str,
    ) -> Result<Self, AssetError> {
        let timeline = timeline.to_lowercase();
        let visuals =
            RoomVisuals::load(assets, room.name(), room.cameras().len(), Some(&timeline))?;
        let s = visuals.settings();
        log::info!(
            "room settings: palette {} fade {:?}, effect colours {}/{}, clouds {}, grime {}",
            s.palette,
            s.fade_palette,
            s.effect_color_a,
            s.effect_color_b,
            s.clouds,
            s.grime
        );
        let images = (0..room.cameras().len())
            .map(|i| visuals.camera_image(i))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            noise: NoiseTexture::load(resources)?,
            images,
            palette: visuals.palette_for_camera(0),
            room: visuals,
            shown: 0,
            generation: 0,
        })
    }

    /// Shows camera `i`'s image and palette fade (a cut, or `MoveCamera`).
    pub fn show(&mut self, i: usize) {
        let i = i.min(self.images.len().saturating_sub(1));
        self.shown = i;
        self.palette = self.room.palette_for_camera(i);
        self.generation += 1;
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Uploads the shown camera's graphic.
    pub fn upload(&self, renderer: &mut impl Renderer, camera: &RoomCamera) {
        let current = self.shown;
        renderer.set_level(&LevelGraphics {
            camera_pos: camera.position(current),
            image: &self.images[current],
            palette: &self.palette,
            noise: &self.noise,
            params: LevelParams::new(self.room.settings(), &self.palette),
        });
    }

    /// The palette's `blackColor` (the slugcat's eye colour), as stored.
    #[must_use]
    pub fn black_color(&self) -> rw_graphics::Color {
        let [r, g, b, a] = self.palette.black_color().map(|c| f32::from(c) / 255.0);
        rw_graphics::Color { r, g, b, a }
    }

    /// The palette's `fogColor` (the malnourished slugcat's eyes are tinted toward it).
    #[must_use]
    pub fn fog_color(&self) -> rw_graphics::Color {
        let [r, g, b, a] = self.palette.fog_color().map(|c| f32::from(c) / 255.0);
        rw_graphics::Color { r, g, b, a }
    }

    /// The camera's `RoomPalette` (water colours among others).
    #[must_use]
    pub fn room_palette(&self) -> rw_assets::RoomPalette {
        self.palette.room_palette()
    }

    /// The palette colours of the shortcut graphics.
    #[must_use]
    pub fn shortcut_palette(&self) -> rw_graphics::ShortcutPalette {
        let p = self.room_palette();
        rw_graphics::ShortcutPalette {
            colors: p.shortcut_colors.map(rw_graphics::Color::from),
            symbol: p.shortcut_symbol.into(),
        }
    }

    /// The palette's earth colour (`GetPixel(0, 2)`, the overseers' graphics).
    #[must_use]
    pub fn earth_color(&self) -> rw_graphics::Color {
        let [r, g, b, a] = self.palette.texel(0, 2).map(|c| f32::from(c) / 255.0);
        rw_graphics::Color { r, g, b, a }
    }

    /// The game's window at `alpha` between ticks, `seconds` of shader time in.
    #[must_use]
    pub fn view(camera: &RoomCamera, alpha: f32, seconds: f32) -> LevelView {
        LevelView {
            origin: camera.view_origin(alpha),
            size: camera.screen_size(),
            rain: seconds / 5.0,
        }
    }
}
