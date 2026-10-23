//! `Room.Loaded`'s placed `LightBeam`s and `SpotLight`s joining the room's update list,
//! a beam's turn in it, and camera 0's palette for the beams (`ApplyPalette`). Types:
//! [`crate::LightBeam`], [`crate::SpotLight`].
//!
//! Mirrors: Room.cs (Loaded: LightBeam, SpotLight, SetLightBeamBlink)

use rw_assets::{LightBeamData, QuadData};

use super::{RainWorldGame, RoomObject};
use crate::{LightBeam, SpotLight, Vec2};

impl RainWorldGame {
    /// `AddObject(new LightBeam(placedObject))` in room `index`; the beam takes the
    /// camera's palette now if that room is shown (`InitiateSprites` calls
    /// `ApplyPalette`).
    pub(super) fn add_light_beam(&mut self, index: i32, pos: Vec2, data: LightBeamData) {
        let id = self.world.next_update_seq();
        let mut beam = LightBeam::new(id, pos, data);
        if let Some(c) = self.camera_image.as_ref().filter(|c| c.room == index) {
            beam.apply_palette(c);
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.light_beams.push(beam);
        }
    }

    /// `AddObject(new SpotLight(placedObject))` in room `index`.
    pub(super) fn add_spot_light(&mut self, index: i32, pos: Vec2, data: &QuadData) {
        let id = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.spot_lights.push(SpotLight::new(id, pos, data));
        }
    }

    pub(super) fn room_light_beam_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.light_beams
                .iter()
                .map(|b| (b.update_order, RoomObject::LightBeam(b.update_order)))
                .collect()
        })
    }

    /// The beam's `Update` in its slot of room `index`'s update list.
    pub(super) fn run_light_beam(&mut self, index: i32, order: u64) {
        let environment = self
            .camera_image
            .as_ref()
            .filter(|c| c.room == index && index == self.camera_room)
            .and_then(|c| {
                let beam = self
                    .world
                    .realized(index)?
                    .light_beams
                    .iter()
                    .find(|b| b.update_order == order)?;
                beam.wants_environment_color()
                    .then(|| c.pixel_color_at_coordinate(beam.quad[1]))
            });
        if let Some(b) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.light_beams.iter_mut().find(|b| b.update_order == order))
        {
            b.update(environment);
        }
    }

    /// `ApplyPalette` for the beams of the room camera 0 shows.
    pub(super) fn apply_palette_to_light_beams(&mut self) {
        if let Some(c) = &self.camera_image
            && let Some(r) = self.world.realized_mut(c.room)
        {
            for b in &mut r.light_beams {
                b.apply_palette(c);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/light_beam_turn.rs"]
mod tests;
