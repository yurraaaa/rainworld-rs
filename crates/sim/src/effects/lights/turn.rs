//! The game's side of light sources: `Room.Loaded`'s placed lights, each light's slot
//! in its room's update list, the glowing player's light (`PlayerGraphics.Update`),
//! camera 0's picture the placed lights read (`PixelColorAtCoordinate`,
//! `ApplyPalette`), and the room's light queries (`Darkness`, `LightSourceExposure`,
//! `LightSourceColor`, `DarknessOfPoint`). No draws.
//!
//! Mirrors: Room.cs (Loaded, Darkness, LightSourceExposure, LightSourceColor, DarknessOfPoint), LightSource.cs (Update), PlayerGraphics.cs (Update)

use rw_assets::{LightColorType, LightSourceData, SlugcatName};

use super::{RainWorldGame, RoomObject};
use crate::color::Hsl;
use crate::world::RealizedRoom;
use crate::{CameraImage, FloatExt, LightSource, LightSourceTick, LightTie, Vec2};

/// `PlayerGraphics.lightSource`'s radius and alpha.
const PLAYER_LIGHT_RAD: f32 = 300.0;

/// The player's side of its light: `Player.glowing` (set when the player is built) and
/// its graphics' `lightSource`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerGlow {
    pub glowing: bool,
    pub light: Option<u64>,
}

impl PlayerGlow {
    /// `PlayerGraphics.DefaultSlugcatColor` (the More Slugcats and Watcher colours come
    /// with their DLCs).
    #[must_use]
    pub fn default_slugcat_color(name: SlugcatName) -> [f32; 3] {
        match name {
            SlugcatName::Yellow => [1.0, 1.0, 115.0 / 255.0],
            SlugcatName::Red => [1.0, 115.0 / 255.0, 115.0 / 255.0],
            SlugcatName::Night => Hsl::new(0.630_555_57, 0.54, 0.2).rgb(),
            _ => [1.0; 3],
        }
    }

    /// The light's colour: `Lerp(white, SlugcatColor(CharacterForColor), 0.5)`.
    #[must_use]
    pub fn light_color(name: SlugcatName) -> [f32; 3] {
        Self::default_slugcat_color(name).map(|c| 1.0.lerp(c, 0.5))
    }
}

impl RainWorldGame {
    /// Camera 0's level image and palette, as the app shows them before the tick;
    /// `ApplyPalette` gives the effect-coloured lights of its room their colour.
    pub fn set_camera_image(&mut self, image: Option<CameraImage>) {
        self.camera_image = image;
        if let Some(c) = &self.camera_image
            && let Some(r) = self.world.realized_mut(c.room)
        {
            for l in r.light_sources.iter_mut() {
                l.apply_palette(c);
            }
        }
        self.apply_palette_to_light_beams();
    }

    /// `Room.Darkness(pos)` of room `index`: camera 0's palette darkness in the room it
    /// shows, else 0.
    #[must_use]
    pub fn darkness(&self, index: i32) -> f32 {
        if index == self.camera_room {
            self.camera_darkness
        } else {
            0.0
        }
    }

    /// `Room.DarknessOfPoint(cam, pos)`: how dark `pos` looks, `InverseLerp(0.2, 1,
    /// Darkness)` less the lights' exposure.
    #[must_use]
    pub fn darkness_of_point(&self, index: i32, pos: Vec2) -> f32 {
        let dark = self.darkness(index).inverse_lerp(0.2, 1.0);
        if dark <= 0.0 {
            return 0.0;
        }
        let exposure = self
            .world
            .realized(index)
            .map_or(0.0, |r| r.light_source_exposure(pos));
        dark * (1.0 - exposure)
    }

    /// `Room.Loaded`'s placed `LightSource` (`index` its room): environmental, white,
    /// rad and strength set for its first update, the data's flags and colour type
    /// (`SetLightSourceBlink`'s blink and night light come with the DLCs, D24).
    pub(super) fn add_placed_light(&mut self, index: i32, pos: Vec2, data: &LightSourceData) {
        let id = self.world.next_update_seq();
        let mut light = LightSource::new(id, index, pos, true, [1.0; 3], None);
        light.set_rad = Some(data.rad());
        light.set_alpha = Some(data.strength);
        light.fade_with_sun = data.fade_with_sun;
        light.color_from_environment = data.color_type == Some(LightColorType::Environment);
        light.flat = data.flat;
        light.effect_color = data.effect_color();
        if let Some(c) = self.camera_image.as_ref().filter(|c| c.room == index) {
            light.apply_palette(c);
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.light_sources.push(light);
        }
    }

    pub(super) fn room_light_source_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.light_sources
                .iter()
                .filter(|l| !l.slated)
                .map(|l| (l.id, RoomObject::LightSource(l.id)))
                .collect()
        })
    }

    /// The light source `id`'s `Update` in its slot of room `index`'s update list.
    pub(super) fn run_light_source(&mut self, index: i32, id: u64) {
        let Some(light) = self
            .world
            .realized(index)
            .and_then(|r| r.light_sources.iter().find(|l| l.id == id && !l.slated))
        else {
            return;
        };
        let pos = light.set_pos.unwrap_or(light.pos);
        let tie_gone = light.tied_to.is_some_and(|t| self.light_tie_gone(index, t));
        let environment_color = self
            .camera_image
            .as_ref()
            .filter(|c| {
                light.color_from_environment && c.room == index && index == self.camera_room
            })
            .map(|c| c.pixel_color_at_coordinate(pos));
        let shader_light = self.world.rain_cycle.shader_light();
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let tick = LightSourceTick {
            tie_gone,
            shader_light,
            environment_color,
            water_level: r.water.as_ref().map_or(f32::MIN, |w| w.level_at(pos.x)),
        };
        if let Some(light) = r.light_sources.iter_mut().find(|l| l.id == id) {
            light.update(&tick);
        }
    }

    /// `tiedToObject.slatedForDeletetion || tiedToObject.room != room`.
    fn light_tie_gone(&self, index: i32, tie: LightTie) -> bool {
        match tie {
            LightTie::HoloLight(h) => !self
                .world
                .realized(index)
                .is_some_and(|r| r.holo_lights.iter().any(|l| l.id == h && !l.slated)),
            LightTie::Lizard(c) => !self
                .world
                .creature(c)
                .lizard()
                .is_some_and(|l| !l.slated && l.room == Some(index)),
            LightTie::Insect(order) => self.insect_tie_gone(index, order),
            LightTie::GreenSpark(order) => self.green_spark_tie_gone(index, order),
            LightTie::Fixture(order) => self.fixture_tie_gone(index, order),
            // In a shortcut the player has no room (`RemoveFromRoom`).
            LightTie::Player => !self
                .player
                .as_ref()
                .is_some_and(|p| !p.is_destroyed() && !p.in_shortcut() && self.pos().room == index),
        }
    }

    /// `PlayerGraphics.Update`'s light, for the player in room `index` with a graphics
    /// module: kept alive and moved to the main chunk while the room is dark, dropped
    /// (left to die without upkeep) when it is not, made when a glowing player is in a
    /// dark room.
    pub(super) fn player_glow_update(&mut self, index: i32) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let pos = p.body.chunks[0].pos;
        let darkness = self.darkness(index);
        if let Some(id) = self.player_glow.light {
            if let Some(l) = self.world.light_source_mut(id) {
                l.stay_alive = true;
                l.set_pos = Some(pos);
            }
            let gone = self
                .world
                .light_source(id)
                .is_none_or(|l| l.slated || l.room != index);
            if gone || darkness == 0.0 {
                self.player_glow.light = None;
            }
        } else if darkness > 0.0 && self.player_glow.glowing {
            let id = self.world.next_update_seq();
            let color = PlayerGlow::light_color(self.campaign.name);
            let mut light = LightSource::new(id, index, pos, false, color, Some(LightTie::Player));
            light.require_up_keep = true;
            light.set_rad = Some(PLAYER_LIGHT_RAD);
            light.set_alpha = Some(1.0);
            if let Some(r) = self.world.realized_mut(index) {
                r.light_sources.push(light);
                self.player_glow.light = Some(id);
            }
        }
    }
}

impl RealizedRoom {
    /// `Room.LightSourceExposure(pos)`: the lights' summed exposure terms, clamped.
    #[must_use]
    pub fn light_source_exposure(&self, pos: Vec2) -> f32 {
        self.light_sources
            .iter()
            .filter(|l| !l.slated)
            .filter_map(|l| l.exposure_at(pos))
            .sum::<f32>()
            .clamp(0.0, 1.0)
    }

    /// `Room.LightSourceColor(pos)`: per channel the largest of colour times exposure.
    #[must_use]
    pub fn light_source_color(&self, pos: Vec2) -> [f32; 3] {
        LightSource::color_over(&self.light_sources, pos)
    }
}

#[cfg(test)]
#[path = "tests/turn.rs"]
pub(super) mod tests;
