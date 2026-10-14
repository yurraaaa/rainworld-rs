//! The room's level graphic: settings, palettes, the shown camera's image, and the
//! game's room camera that chooses it.

use rw_assets::{
    AssetCache, AssetError, NoiseTexture, PaletteTexture, PngImage, Room, RoomVisuals,
    StreamingAssets,
};
use rw_render::{Follow, LevelGraphics, LevelParams, LevelView, Renderer, RoomCamera};

/// Campaign timeline whose `_settings-<timeline>.txt` variants apply (Survivor).
const TIMELINE: &str = "white";

/// Level graphic state of one room.
pub struct Visuals {
    room: RoomVisuals,
    noise: NoiseTexture,
    camera: RoomCamera,
    /// Every camera's image, decoded once at load so switching never stalls.
    images: Vec<PngImage>,
    palette: PaletteTexture,
    /// Counts camera switches, so a renderer knows when its upload is stale.
    generation: u32,
}

impl Visuals {
    /// Loads the room's settings, palettes and every camera's image, and the noise
    /// texture from the asset cache; the room camera shows `screen` pixels (`sSize`).
    ///
    /// # Errors
    /// When any of those cannot be loaded.
    pub fn load(
        assets: &StreamingAssets,
        cache: &AssetCache,
        room: &Room,
        screen: [f32; 2],
    ) -> Result<Self, AssetError> {
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
        let camera = RoomCamera::new(room.cameras(), screen);
        let images = (0..room.cameras().len())
            .map(|i| visuals.camera_image(i))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            noise: NoiseTexture::load(cache)?,
            images,
            palette: visuals.palette_for_camera(camera.current()),
            room: visuals,
            camera,
            generation: 0,
        })
    }

    /// One 40 Hz tick of the room camera; on a camera switch selects that camera's
    /// image and palette fade.
    pub fn tick(&mut self, follow: Option<&Follow>) {
        if self.camera.update(follow) {
            let current = self.camera.current();
            log::info!("camera {}", current + 1);
            self.palette = self.room.palette_for_camera(current);
            self.generation += 1;
        }
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// The camera shown (0-based).
    #[must_use]
    pub fn current_camera(&self) -> usize {
        self.camera.current()
    }

    /// Uploads the shown camera's graphic.
    pub fn upload(&self, renderer: &mut impl Renderer) {
        let current = self.camera.current();
        renderer.set_level(&LevelGraphics {
            camera_pos: self.camera.position(current),
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

    /// The room camera's window size (`sSize`, the screen resolution).
    #[must_use]
    pub fn screen_size(&self) -> [f32; 2] {
        self.camera.screen_size()
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
