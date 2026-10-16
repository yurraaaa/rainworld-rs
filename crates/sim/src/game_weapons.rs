//! The game's parts for the rocks and spears: their slots in a room's update list, their
//! sound loops' emitters, the sparks and spikes they and the throws make, and the room's
//! collision pass after the update list.

use std::sync::Arc;

use super::{RainWorldGame, RoomObject};
use crate::item::ObjectId;
use crate::world::ItemTick;

impl RainWorldGame {
    /// The realized items and their loop emitters in room `index`'s update list.
    pub(super) fn room_weapon_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let mut list: Vec<(u64, RoomObject)> = self
            .world
            .room_weapons(index)
            .into_iter()
            .map(|(seq, id)| (seq, RoomObject::Weapon(id)))
            .collect();
        list.extend(
            self.world
                .room_weapon_loops(index)
                .into_iter()
                .map(|(seq, id)| (seq, RoomObject::WeaponLoop(id))),
        );
        list
    }

    /// The sparks and spikes in room `index`'s update list.
    pub(super) fn room_cosmetic_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .room_cosmetics(index)
            .iter()
            .filter(|c| !c.slated)
            .map(|c| (c.update_order, RoomObject::Cosmetic(c.id)))
            .collect()
    }

    /// Item `id`'s `Update` in room `index`, or with `emitter` the `ChunkSoundEmitter`
    /// of its sound loop.
    pub(super) fn run_weapon(&mut self, index: i32, id: ObjectId, emitter: bool) {
        let triggers = Arc::clone(self.sounds.table());
        let eu = self.even_update();
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let player = self
            .player
            .as_ref()
            .map(|p| (self.player_id, p.throw_direction()));
        let shelter_door_moving = self
            .shelter_doors
            .iter()
            .any(|d| d.room == index && (d.is_closing() || d.is_opening()));
        let mut cx = ItemTick {
            rng: &mut self.rng,
            sounds,
            triggers: &triggers,
            camera_room: self.camera_room,
            tick: self.tick,
            eu,
            player,
            shelter_door_moving,
        };
        if emitter {
            self.world.update_weapon_loop(index, id, &mut cx);
        } else {
            self.world.update_weapon(index, id, &mut cx);
        }
    }
}
