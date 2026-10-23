//! `Room.Loaded`'s `CustomDecal`s and `Rainbow`s joining the room's update list, and the
//! rainbows' turns in it. Types: [`crate::CustomDecal`], [`crate::Rainbow`].
//!
//! Mirrors: Room.cs (Loaded: CustomDecal, Rainbow, RainbowNoFade)

use rw_assets::{CustomDecalData, RainbowData};

use super::{RainWorldGame, RoomObject};
use crate::creature::ai::SeededRandom;
use crate::{CustomDecal, Rainbow, Vec2};

impl RainWorldGame {
    /// `AddObject(new CustomDecal(placedObject))`.
    pub(super) fn add_custom_decal(&mut self, index: i32, pos: Vec2, data: CustomDecalData) {
        let id = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.decals.push(CustomDecal::new(id, pos, data));
        }
    }

    /// `Room.Loaded`'s condition for a placed `Rainbow` in room `index`:
    /// `SeededRandom(rainbowSeed + abstractRoom.index)` against the chance while the
    /// cycle starts up, or an arena session.
    pub(super) fn rainbow_spawns(&mut self, index: i32, data: &RainbowData) -> bool {
        let cycle = &self.world.rain_cycle;
        let (start_up, seed) = (cycle.cycle_start_up(), cycle.rainbow_seed);
        let arena = !self.world.story;
        // `SeededRandom` is evaluated only past the start-up check.
        let seeded = if start_up < 1.0 {
            self.rng.seeded_value(seed.wrapping_add(index))
        } else {
            1.0
        };
        Rainbow::spawns(start_up, seeded, data.chance(), arena)
    }

    /// `AddObject(new Rainbow(room, placedObject))` (`RainbowNoFade` when `no_fade`).
    pub(super) fn add_rainbow(&mut self, index: i32, pos: Vec2, data: &RainbowData, no_fade: bool) {
        let id = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.rainbows.push(Rainbow::new(id, pos, data, no_fade));
        }
    }

    pub(super) fn room_rainbow_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.rainbows
                .iter()
                .map(|b| (b.update_order, RoomObject::Rainbow(b.update_order)))
                .collect()
        })
    }

    /// The rainbow's `Update` in its slot of room `index`'s update list; a destroyed one
    /// leaves the room.
    pub(super) fn run_rainbow(&mut self, index: i32, order: u64) {
        let start_up = self.world.rain_cycle.cycle_start_up();
        let arena = !self.world.story;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        if let Some(b) = r.rainbows.iter_mut().find(|b| b.update_order == order) {
            b.update(start_up, arena);
        }
        r.rainbows.retain(|b| !b.slated);
    }
}
