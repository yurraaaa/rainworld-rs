//! The game's side of a room's `RoomBorderPushBack`: `Room.Loaded` making it and its turn
//! in the update list, pushing the player's chunks and those of what it holds sideways.
//! Type: [`crate::BorderPushBack`].
//!
//! Mirrors: Room.cs (Loaded: BorderPushBack), RoomBorderPushBack.cs (Update, PushAtCreature)

use rw_assets::RoomEffectType;

use super::{RainWorldGame, RoomObject};
use crate::BorderPushBack;
use crate::item::Entity;

impl RainWorldGame {
    /// `Room.Loaded`'s `RoomBorderPushBack` for each such effect, joining the update list
    /// (one player).
    pub(super) fn add_border_push_backs(&mut self, index: i32) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let wanted = r
            .settings
            .effects
            .iter()
            .filter(|e| e.kind == RoomEffectType::BorderPushBack.value())
            .count();
        let cameras = r.room.cameras().to_vec();
        for _ in 0..wanted {
            let order = self.world.next_update_seq();
            if let Some(r) = self.world.realized_mut(index) {
                r.border_push_backs
                    .push(BorderPushBack::new(order, &cameras, 1));
            }
        }
    }

    /// Room `index`'s `RoomBorderPushBack`s in its update list.
    pub(super) fn room_border_push_back_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.border_push_backs
                .iter()
                .map(|b| (b.update_order, RoomObject::BorderPushBack(b.update_order)))
                .collect()
        })
    }

    /// `RoomBorderPushBack.Update` of the object at `order` in room `index`.
    pub(super) fn run_border_push_back(&mut self, index: i32, order: u64) {
        // The player's main chunk, while its body is in the room and nothing holds it.
        let held_by_none = self
            .world
            .grabbed_by(Entity::Creature(self.player_id))
            .is_empty();
        let in_room = self.pos().room == index;
        let testable = self
            .player
            .as_ref()
            .filter(|p| in_room && held_by_none && !p.in_shortcut())
            .map(|p| p.body.chunks[0].pos.x);
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(push) = r
            .border_push_backs
            .iter_mut()
            .find(|b| b.update_order == order)
        else {
            return;
        };
        push.grow_to(1);
        if let Some(psh) = push.update_player(0, testable) {
            self.push_player_and_held(psh);
        }
    }

    /// `PushAtCreature`: every chunk of the player and of what it holds gets `psh` sideways.
    fn push_player_and_held(&mut self, psh: f32) {
        if let Some(p) = self.player.as_mut() {
            for c in &mut p.body.chunks {
                c.vel.x += psh;
            }
        }
        let held: Vec<Entity> = self
            .world
            .creature(self.player_id)
            .grasps
            .iter()
            .flatten()
            .map(|g| g.grabbed)
            .collect();
        for e in held {
            if let Some(chunks) = self.world.surface_chunks_mut(e) {
                for c in chunks {
                    c.vel.x += psh;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "game_border_push_back_tests.rs"]
mod tests;
