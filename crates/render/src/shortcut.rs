//! Shortcut tubes reconstructed from tile flags, for drawing.
//!
//! Mirrors the game's `ShortcutMapper` walk and `Room.ShorcutEntranceHoleDirection`
//! (`rainworld-re/notes/shortcuts.md` sections 1 and 3). This is a drawing aid only;
//! the simulation owns the authoritative shortcut map.

use rw_assets::{Room, Shortcut, Terrain};

/// Neighbour order of the game's shortcut searches: left, down, right, up.
const DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// The game's step limit for tracing one tube.
const MAX_STEPS: usize = 1000;

/// Shortcut tubes of a room.
pub trait ShortcutPaths {
    /// Inside the room with a shortcut value (entrances included).
    fn is_tube_tile(&self, pos: (i32, i32)) -> bool;

    /// Open side of the entrance at `(x, y)`: the first neighbour, in the order left,
    /// down, right, up, that is not Solid (clamped lookups); `(0, 0)` if none.
    fn hole_direction(&self, x: i32, y: i32) -> (i32, i32);

    /// Next tube tile after `pos` coming from `last`: straight on if possible, else the
    /// first shortcut neighbour (left, down, right, up) that is not straight back;
    /// `pos` itself when stuck.
    fn next_shortcut_tile(&self, pos: (i32, i32), last: (i32, i32)) -> (i32, i32);

    /// The tube walked from the entrance at `start`, including both ends. Stops at
    /// another entrance, a node tile (room exit, den, transport), a dead end or after
    /// the game's 1000 steps.
    fn trace_shortcut(&self, start: (i32, i32)) -> Vec<(i32, i32)>;

    /// One traced tube per entrance, in the game's order (top row first, left to right).
    fn shortcut_paths(&self) -> Vec<Vec<(i32, i32)>>;
}

impl ShortcutPaths for Room {
    fn is_tube_tile(&self, (x, y): (i32, i32)) -> bool {
        self.get(x, y).is_some_and(|t| t.shortcut != Shortcut::None)
    }

    fn hole_direction(&self, x: i32, y: i32) -> (i32, i32) {
        DIRECTIONS
            .into_iter()
            .find(|&(dx, dy)| !self.tile(x + dx, y + dy).is_solid())
            .unwrap_or((0, 0))
    }

    fn next_shortcut_tile(&self, pos: (i32, i32), last: (i32, i32)) -> (i32, i32) {
        let dir = (pos.0 - last.0, pos.1 - last.1);
        let straight = (pos.0 + dir.0, pos.1 + dir.1);
        if dir != (0, 0) && self.is_tube_tile(straight) {
            return straight;
        }
        DIRECTIONS
            .into_iter()
            .filter(|&d| dir == (0, 0) || d != (-dir.0, -dir.1))
            .map(|(dx, dy)| (pos.0 + dx, pos.1 + dy))
            .find(|&p| self.is_tube_tile(p))
            .unwrap_or(pos)
    }

    fn trace_shortcut(&self, start: (i32, i32)) -> Vec<(i32, i32)> {
        let mut path = vec![start];
        let (mut last, mut cur) = (start, start);
        for _ in 0..MAX_STEPS {
            let next = self.next_shortcut_tile(cur, last);
            if next == cur {
                break;
            }
            path.push(next);
            (last, cur) = (cur, next);
            let tile = self.tile(next.0, next.1);
            if tile.terrain == Terrain::ShortcutEntrance
                || !matches!(tile.shortcut, Shortcut::None | Shortcut::Path)
            {
                break;
            }
        }
        path
    }

    fn shortcut_paths(&self) -> Vec<Vec<(i32, i32)>> {
        let mut out = Vec::new();
        for y in (0..self.height()).rev() {
            for x in 0..self.width() {
                if self.tile(x, y).terrain == Terrain::ShortcutEntrance {
                    out.push(self.trace_shortcut((x, y)));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_room::FromAscii;

    #[test]
    fn hole_direction_prefers_left_down_right_up() {
        // (2,1): left is Air. (4,1): left and down Solid, right Air.
        let r = Room::from_ascii(&["######", "#.E#E.", "######"]);
        assert_eq!(r.hole_direction(2, 1), (-1, 0));
        assert_eq!(r.hole_direction(4, 1), (1, 0));
        let r = Room::from_ascii(&["###", "#E#", "###"]);
        assert_eq!(r.hole_direction(1, 1), (0, 0));
    }

    #[test]
    fn traces_a_pipe_between_two_entrances() {
        // Tube: (1,3) down to (1,1), right to (3,1), up to (3,3).
        let r = Room::from_ascii(&[".....", "#E#E#", "#x#x#", "#xxx#", "#####"]);
        assert_eq!(
            r.trace_shortcut((1, 3)),
            vec![(1, 3), (1, 2), (1, 1), (2, 1), (3, 1), (3, 2), (3, 3)]
        );
        let paths = r.shortcut_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0][0], (1, 3));
        assert_eq!(paths[1].first(), paths[0].last());
        assert_eq!(r.hole_direction(1, 3), (0, 1));
    }

    #[test]
    fn trace_prefers_straight_and_stops_at_a_node() {
        // The side branch at (0,3) is ignored while straight on is possible.
        let r = Room::from_ascii(&["..#", "#E#", "xx#", "#x#", "#R#", "###"]);
        assert_eq!(
            r.trace_shortcut((1, 4)),
            vec![(1, 4), (1, 3), (1, 2), (1, 1)]
        );
    }

    #[test]
    fn dead_end_entrance_traces_only_itself() {
        let r = Room::from_ascii(&["...", "#E#", "###"]);
        assert_eq!(r.trace_shortcut((1, 1)), vec![(1, 1)]);
    }
}
