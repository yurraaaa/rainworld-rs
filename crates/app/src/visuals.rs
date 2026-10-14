//! The room's level graphic: settings, palettes, the shown camera's image, and the
//! game's room camera that chooses it.

use rw_assets::{
    AssetError, NoiseTexture, PaletteTexture, PngImage, Room, RoomVisuals, StreamingAssets,
};
use rw_render::{Follow, LevelGraphics, LevelParams, LevelView, Renderer, RoomCamera};

/// Campaign timeline whose `_settings-<timeline>.txt` variants apply (Survivor).
const TIMELINE: &str = "white";

/// Level graphic state of one room.
pub struct Visuals {
    room: RoomVisuals,
    noise: NoiseTexture,
    camera: RoomCamera,
    image: PngImage,
    palette: PaletteTexture,
    /// The image or palette changed since the last upload.
    changed: bool,
}

impl Visuals {
    /// Loads the room's settings, palettes, noise texture and the image of camera 0.
    ///
    /// # Errors
    /// When any of those cannot be loaded.
    pub fn load(assets: &StreamingAssets, room: &Room) -> Result<Self, AssetError> {
        let visuals = RoomVisuals::load(assets, room.name(), room.cameras().len(), Some(TIMELINE))?;
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
        let camera = RoomCamera::new(room.cameras(), RoomCamera::SCREEN_SIZE);
        Ok(Self {
            noise: NoiseTexture::load(assets)?,
            image: visuals.camera_image(camera.current())?,
            palette: visuals.palette_for_camera(camera.current()),
            room: visuals,
            camera,
            changed: true,
        })
    }

    /// One 40 Hz tick of the room camera; on a camera switch loads that camera's image
    /// and palette fade.
    ///
    /// # Errors
    /// When the new camera's image cannot be loaded.
    pub fn tick(&mut self, follow: Option<&Follow>) -> Result<(), AssetError> {
        if self.camera.update(follow) {
            let current = self.camera.current();
            log::info!("camera {}", current + 1);
            self.image = self.room.camera_image(current)?;
            self.palette = self.room.palette_for_camera(current);
            self.changed = true;
        }
        Ok(())
    }

    /// Uploads the shown camera's graphic if it changed.
    pub fn upload(&mut self, renderer: &mut impl Renderer) {
        if !self.changed {
            return;
        }
        let current = self.camera.current();
        renderer.set_level(&LevelGraphics {
            camera_pos: self.camera.position(current),
            image: &self.image,
            palette: &self.palette,
            noise: &self.noise,
            params: LevelParams::new(self.room.settings(), &self.palette),
        });
        self.changed = false;
    }

    /// The game's window at `alpha` between ticks, `seconds` of shader time in.
    #[must_use]
    pub fn view(&self, alpha: f32, seconds: f32) -> LevelView {
        LevelView {
            origin: self.camera.view_origin(alpha),
            size: self.camera.screen_size(),
            rain: seconds / 5.0,
        }
    }
}
