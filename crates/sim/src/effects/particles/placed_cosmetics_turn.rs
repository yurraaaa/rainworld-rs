//! `Room.Loaded`'s cosmetic slime molds and dead token stalks joining the room's update
//! list, and the stalks' turns in it. Types: [`crate::CosmeticSlimeMold`],
//! [`crate::TokenStalk`].
//!
//! Mirrors: Room.cs (Loaded: CosmeticSlimeMold, CosmeticSlimeMold2, SlimeMold, DeadTokenStalk)

#[cfg(test)]
#[path = "tests/placed_cosmetics_turn.rs"]
mod tests;

use crate::game::{RainWorldGame, RoomObject};
use crate::{CosmeticSlimeMold, TokenStalk, Vec2};

impl RainWorldGame {
    /// `AddObject(new SlimeMold.CosmeticSlimeMold(this, pos, rad, throughWalls))`.
    pub(crate) fn add_slime_mold(&mut self, index: i32, pos: Vec2, rad: f32, through_walls: bool) {
        let id = self.world.next_update_seq();
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let mold = CosmeticSlimeMold::new(id, &r.room, pos, rad, through_walls, &mut self.rng);
        r.slime_molds.push(mold);
    }

    /// `AddObject(new CollectToken.TokenStalk(this, pos, pos + handlePos, null, false))`.
    pub(crate) fn add_token_stalk(&mut self, index: i32, hover: Vec2, base: Vec2) {
        let id = self.world.next_update_seq();
        let stalk = TokenStalk::new(id, hover, base, &mut self.rng);
        if let Some(r) = self.world.realized_mut(index) {
            r.token_stalks.push(stalk);
        }
    }

    pub(crate) fn room_token_stalk_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.token_stalks
                .iter()
                .map(|s| (s.update_order, RoomObject::TokenStalk(s.update_order)))
                .collect()
        })
    }

    /// The stalk's `Update` in its slot of room `index`'s update list.
    pub(crate) fn run_token_stalk(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let room = r.room.clone();
        if let Some(s) = r.token_stalks.iter_mut().find(|s| s.update_order == order) {
            s.update(&room, &mut self.rng);
        }
    }
}
