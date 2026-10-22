//! The room's level graphic: settings, palettes and the shown camera's image (the
//! scene's room camera chooses which), the palette's rain darkening (`DarkPalette`,
//! `ApplyFade`) and the level shader's light (`_light`).
//!
//! Mirrors: RoomCamera.cs (DarkPalette, ApplyFade, ChangeRoom, MoveCamera2, DrawUpdate, ApplyPalette)

use std::sync::Arc;

use rw_assets::{
    AssetError, DangerType, NoiseTexture, PaletteTexture, PngImage, Room, RoomEffectType,
    RoomVisuals, StreamingAssets, UnityResources,
};
use rw_render::{LevelGraphics, LevelParams, LevelView, Renderer, RoomCamera};
use rw_sim::FloatExt;
use rw_sim::world::RainCycle;

use crate::full_screen_effect::FullScreenEffect;

/// Level graphic state of one room.
pub struct Visuals {
    room: RoomVisuals,
    noise: NoiseTexture,
    /// `room.lightAngle`.
    light_angle: [f32; 2],
    /// The camera whose image is shown.
    shown: usize,
    /// Every camera's image, decoded once at load so switching never stalls.
    images: Vec<Arc<PngImage>>,
    palette: PaletteTexture,
    /// The `DarkPalette` the palette was last built with (`fadeCoord.y`).
    dark: f32,
    /// Counts camera switches, so a renderer knows when its upload is stale.
    generation: u32,
    /// Counts palette rebuilds without a camera switch (`ApplyFade` on a new dark).
    palette_generation: u32,
    /// The room camera's `fullScreenEffect` (`ApplyPalette`).
    effect: Option<FullScreenEffect>,
}

/// The level shader's inputs for `get_state`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelState {
    pub params: LevelParams,
    /// `_light`.
    pub light: f32,
    /// `num4`: the room's light level before clouds.
    pub shader_light: f32,
    /// `DarkPalette`.
    pub dark_palette: f32,
    /// `paletteBlend`.
    pub palette_blend: f32,
    /// `SunBlocker`s in the room.
    pub sun_blockers: usize,
    /// The full-screen effect and its alpha this frame.
    pub full_screen: Option<(FullScreenEffect, f32)>,
    /// `_waterLevel` this frame.
    pub water_level: f32,
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
            .map(|i| visuals.camera_image(i).map(Arc::new))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            effect: FullScreenEffect::select(visuals.settings()),
            noise: NoiseTexture::load(resources)?,
            light_angle: room.light_angle(),
            images,
            palette: visuals.palette_for_camera(0, 0.0),
            room: visuals,
            shown: 0,
            dark: 0.0,
            generation: 0,
            palette_generation: 0,
        })
    }

    /// Shows camera `i`'s image and palette fade (a cut, or `MoveCamera`).
    pub fn show(&mut self, i: usize) {
        let i = i.min(self.images.len().saturating_sub(1));
        self.shown = i;
        self.palette = self.room.palette_for_camera(i, self.dark);
        self.generation += 1;
    }

    /// `RoomCamera.Update`'s `ApplyFade` when `DarkPalette` changed under `rain`;
    /// returns whether the palette was rebuilt.
    pub fn fade(&mut self, rain: &RainCycle) -> bool {
        let dark = self.dark_palette(rain);
        if dark == self.dark {
            return false;
        }
        self.dark = dark;
        self.palette = self.room.palette_for_camera(self.shown, dark);
        self.palette_generation += 1;
        true
    }

    /// `DarkPalette`: 0 in rooms without rain danger, else the rain cycle's
    /// `RainDarkPalette` times the room's `RainIntensity`. A start without a cycle
    /// (`cycle_length` 0) keeps 0 (D22).
    #[must_use]
    pub fn dark_palette(&self, rain: &RainCycle) -> f32 {
        if rain.cycle_length == 0 || !self.rain_danger() {
            // A `BrokenZeroG` room would take `1 - CurrentLightsOn` of the broken
            // anti-gravity here; zero gravity is not ported.
            return 0.0;
        }
        rain.rain_dark_palette() * self.room.settings().rain_intensity
    }

    /// `num4` of `DrawUpdate`: the rain cycle's `ShaderLight` in rooms with rain danger,
    /// else 1 (lightning is not ported). A start without a cycle keeps 1 (D22).
    #[must_use]
    pub fn shader_light(&self, rain: &RainCycle) -> f32 {
        if rain.cycle_length == 0 || !self.rain_danger() {
            return 1.0;
        }
        rain.shader_light()
    }

    /// `DangerType != None` (an unregistered value counts as rain, as in the game).
    fn rain_danger(&self) -> bool {
        self.room.settings().danger_type != Some(DangerType::None)
    }

    /// `_light` (no ghost).
    #[must_use]
    pub fn light(&self, rain: &RainCycle) -> f32 {
        LevelParams::light(self.room.settings().clouds, self.shader_light(rain), 0.0)
    }

    /// The level shader's room-constant uniforms.
    #[must_use]
    pub fn params(&self) -> LevelParams {
        LevelParams::new(self.room.settings(), &self.palette, self.light_angle)
    }

    /// `Room.Loaded` adds a `SunBlocker` per `SunBlock` effect, whatever its amount.
    #[must_use]
    pub fn sun_blockers(&self) -> usize {
        self.room
            .settings()
            .effects
            .iter()
            .filter(|e| e.kind == RoomEffectType::SunBlock.value())
            .count()
    }

    /// The level shader's inputs under `rain` with `_waterLevel` `water_level`, for
    /// `get_state`.
    #[must_use]
    pub fn state(&self, rain: &RainCycle, water_level: f32) -> LevelState {
        LevelState {
            params: self.params(),
            light: self.light(rain),
            shader_light: self.shader_light(rain),
            dark_palette: self.dark,
            palette_blend: self.room.palette_blend(self.shown),
            sun_blockers: self.sun_blockers(),
            full_screen: self.effect.map(|e| (e, self.effect_alpha(&e, rain))),
            water_level,
        }
    }

    /// The room's full-screen effect.
    #[must_use]
    pub fn full_screen_effect(&self) -> Option<FullScreenEffect> {
        self.effect
    }

    /// `DrawUpdate`'s `fullScreenEffect.alpha` under `rain` (`num4` without lightning).
    #[must_use]
    pub fn effect_alpha(&self, effect: &FullScreenEffect, rain: &RainCycle) -> f32 {
        effect.alpha(self.room.settings(), self.shader_light(rain))
    }

    /// `_waterLevel` (`DrawUpdate`): `InverseLerp(sSize.y, 0, line - targetPoint.y)`
    /// for the room's water line (`line`: water + 100, the death fall + 180, else 0);
    /// `None` in gates and shelters, which set 0.
    #[must_use]
    pub fn water_level(line: Option<f32>, target_y: f32, screen_height: f32) -> f32 {
        line.map_or(0.0, |h| (h - target_y).inverse_lerp(screen_height, 0.0))
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub fn palette_generation(&self) -> u32 {
        self.palette_generation
    }

    /// Camera 0's picture as the sim's lights read it: the shown camera's image (its
    /// bottom-left at `cam_pos`) and palette, in global room `room`.
    #[must_use]
    pub fn camera_image(&self, room: i32, cam_pos: [f32; 2]) -> rw_sim::CameraImage {
        rw_sim::CameraImage {
            room,
            cam_pos: cam_pos.into(),
            image: Arc::clone(&self.images[self.shown]),
            palette: self.palette.clone(),
        }
    }

    /// Uploads the shown camera's graphic.
    pub fn upload(&self, renderer: &mut impl Renderer, camera: &RoomCamera) {
        let current = self.shown;
        renderer.set_level(&LevelGraphics {
            camera_pos: camera.position(current),
            image: &self.images[current],
            palette: &self.palette,
            noise: &self.noise,
            params: self.params(),
        });
    }

    /// Uploads the palette alone (after [`fade`](Self::fade) rebuilt it).
    pub fn upload_palette(&self, renderer: &mut impl Renderer) {
        renderer.set_level_palette(&self.palette);
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

    /// The camera's palette texture (`palette.texture`).
    #[must_use]
    pub fn palette_texture(&self) -> &PaletteTexture {
        &self.palette
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

    /// The game's window at `alpha` between ticks, `seconds` of shader time in, lit
    /// under `rain`, with the room's water line `water_line` ([`Self::water_level`]).
    #[must_use]
    pub fn view(
        &self,
        camera: &RoomCamera,
        alpha: f32,
        seconds: f32,
        rain: &RainCycle,
        water_line: Option<f32>,
    ) -> LevelView {
        let origin = camera.view_origin(alpha);
        let size = camera.screen_size();
        LevelView {
            origin,
            size,
            rain: seconds / 5.0,
            light: self.light(rain),
            water_level: Self::water_level(water_line, origin[1], size[1]),
        }
    }
}
