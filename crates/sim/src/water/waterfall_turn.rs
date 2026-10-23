//! A waterfall's turn in its room's update list: its drips and bubbles joining the room, and
//! the bodies it pushes (every physical object of the room except a player holding up).
//! Types: [`crate::water::waterfall`]. Made in [`super::light_fixtures`].
//!
//! Mirrors: WaterFall.cs (Update)

use super::RainWorldGame;

impl RainWorldGame {
    /// `WaterFall.Update` of the fall at `order` in room `index`.
    pub(super) fn run_waterfall(&mut self, index: i32, order: u64) {
        let player_room = self.pos().room;
        let player_seq = self.player_seq;
        let here = self
            .player
            .as_mut()
            .filter(|p| player_room == index && !p.in_shortcut() && !p.input_up())
            .map(|p| (player_seq, &mut p.body.chunks[..]));
        self.world
            .update_waterfall(index, order, here, &mut self.rng);
    }
}
