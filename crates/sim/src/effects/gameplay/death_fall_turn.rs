//! The game's side of a room's `DeathFallGraphic`: `Room.Loaded` making it and its turn in
//! the update list, which counts for as long as the room stays realized, viewed or not.
//! Type: [`crate::DeathFall`].
//!
//! Mirrors: Room.cs (Loaded: DeathFallGraphic), DeathFallGraphic.cs (Update)

#[cfg(test)]
#[path = "tests/death_fall_turn.rs"]
mod tests;

use crate::DeathFall;
use crate::game::{RainWorldGame, RoomObject};

impl RainWorldGame {
    /// `Room.Loaded`'s `AddObject(deathFallGraphic = new DeathFallGraphic())` when
    /// `RoomRequiresDeathFall`.
    pub(crate) fn add_death_fall(&mut self, index: i32) {
        let world = &self.world;
        let Some(data) = world.abstract_room(index) else {
            return;
        };
        let unbroken_shelter = data.shelter()
            && !usize::try_from(data.shelter_index)
                .ok()
                .and_then(|i| world.data().broken_shelters.get(i).copied())
                .unwrap_or(false);
        let Some(realized) = world.realized(index) else {
            return;
        };
        let placed = realized
            .settings
            .placed_objects_for(world.timeline(), world.mods());
        let Some(mut fall) = DeathFall::new(
            &realized.room,
            realized.water.as_ref(),
            unbroken_shelter,
            &placed,
        ) else {
            return;
        };
        fall.update_order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.death_fall = Some(fall);
        }
    }

    pub(crate) fn room_death_fall_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.death_fall.as_ref())
            .map(|f| (f.update_order, RoomObject::DeathFall))
            .into_iter()
            .collect()
    }

    /// `DeathFallGraphic.Update` of room `index`.
    pub(crate) fn run_death_fall(&mut self, index: i32) {
        let view = self.camera_view();
        if let Some(r) = self.world.realized_mut(index)
            && let Some(fall) = &mut r.death_fall
        {
            fall.update(&r.room, index, &view);
        }
    }
}
