//! Shortcut travel: the game's `ShortcutHandler` tick divider and the vessel a creature
//! rides in while inside a tube.
//!
//! Behaviour: `rainworld-re/notes/shortcuts.md` section 3. The map itself is
//! [`rw_assets::ShortcutMap`].

use rw_assets::Room;

use crate::Player;

/// `RainWorldGame.updateShortCut`: [`ShortcutHandler::update`] moves vessels only on
/// every third game tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShortcutHandler {
    update_counter: u8,
}

impl ShortcutHandler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One game tick, called before the room's objects update (as the game does):
    /// every third call moves `player`'s vessel one tile, spitting the player out when
    /// it reaches an entrance.
    pub fn update(&mut self, room: &Room, player: &mut Player) {
        self.update_counter += 1;
        if self.update_counter > 2 {
            self.update_counter = 0;
            player.shortcut_vessel_update(room);
        }
    }
}

/// `ShortcutHandler.ShortCutVessel` for a one-segment creature (the slugcat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShortcutVessel {
    pub(crate) pos: (i32, i32),
    /// `lastPositions[0]`.
    pub(crate) last_pos: (i32, i32),
    /// Updates to sit out; always 0 without Jolly co-op.
    pub(crate) wait: i32,
}

impl ShortcutVessel {
    pub(crate) fn new(pos: (i32, i32)) -> Self {
        Self {
            pos,
            last_pos: pos,
            wait: 0,
        }
    }

    /// One `ShortcutHandler.Update` step: counts `wait` down, or moves one tile along
    /// the tube. Returns whether it moved.
    pub(crate) fn step(&mut self, room: &Room) -> bool {
        if self.wait > 0 {
            self.wait -= 1;
            return false;
        }
        let prev = self.pos;
        self.pos = room.next_shortcut_position(self.pos, self.last_pos);
        self.last_pos = prev;
        true
    }
}
