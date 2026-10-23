//! The game's parts for the rocks and spears: their slots in a room's update list, their
//! sound loops' emitters, the sparks and spikes they and the throws make, and the room's
//! collision pass after the update list.
//!
//! Mirrors: Weapon.cs, Rock.cs, Spear.cs, Room.cs (Update, InGameNoise, ScreenMovement)

use std::sync::Arc;

use crate::RoomCall;
use crate::game::{RainWorldGame, RoomObject};
use crate::item::ObjectId;
use crate::world::{ItemTick, World};

impl RainWorldGame {
    /// The realized items and their loop emitters in room `index`'s update list.
    pub(crate) fn room_weapon_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
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
                .map(|(order, owner)| (order, RoomObject::WeaponLoop { owner, order })),
        );
        list
    }

    /// The sparks and spikes in room `index`'s update list.
    pub(crate) fn room_cosmetic_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .room_cosmetics(index)
            .iter()
            .filter(|c| !c.slated)
            .map(|c| (c.update_order, RoomObject::Cosmetic(c.id)))
            .collect()
    }

    /// The `ChunkSoundEmitter` of item `owner`'s sound loop that joined room `index` at
    /// `order`.
    pub(crate) fn run_weapon_loop(&mut self, index: i32, owner: ObjectId, order: u64) {
        self.with_item_tick(index, |world, cx| {
            world.update_weapon_loop(index, owner, order, cx);
        });
    }

    /// The `ChunkSoundEmitter` of a swallowed item's loop that joined room `index` at
    /// `order`.
    pub(crate) fn run_orphan_loop(&mut self, index: i32, order: u64) {
        self.with_item_tick(index, |world, cx| {
            world.update_orphan_loop(index, order, cx);
        });
    }

    /// `f` with the world and what an item's turn in room `index` reads of the game;
    /// then the room calls `f` made (noises, screen movements).
    pub(crate) fn with_item_tick<R>(
        &mut self,
        index: i32,
        f: impl FnOnce(&mut World, &mut ItemTick<'_>) -> R,
    ) -> R {
        let triggers = Arc::clone(self.sounds.table());
        let eu = self.even_update();
        let player = self
            .player
            .as_ref()
            .map(|p| (self.player_id, p.throw_direction()));
        let shelter_door_moving = self
            .shelter_doors
            .iter()
            .any(|d| d.room == index && (d.is_closing() || d.is_opening()));
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = ItemTick {
            rng: &mut self.rng,
            sounds,
            triggers: &triggers,
            camera_room: self.camera_room,
            tick: self.tick,
            eu,
            player,
            shelter_door_moving,
            calls: Vec::new(),
        };
        let r = f(&mut self.world, &mut cx);
        let calls = std::mem::take(&mut cx.calls);
        self.make_room_calls(index, calls);
        r
    }

    /// The noises and screen movements an item's update made in room `index`, in order.
    fn make_room_calls(&mut self, index: i32, calls: Vec<RoomCall>) {
        for call in calls {
            match call {
                RoomCall::Noise(n) => self.room_noise(index, n),
                RoomCall::ScreenMovement(m) => self.room_screen_movement(index, m),
            }
        }
    }
}
