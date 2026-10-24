//! The room's shortcut map as the game's `ShortcutMapper` builds it, plus the tube walk
//! (`ShortcutHandler.NextShortcutPosition`) and hole direction it relies on.
//!
//! Behaviour: `rainworld-re/notes/shortcuts.md` sections 1 and 3, `room-format.md`
//! section 5. Lives here, derived once per room, because both the simulation (entering,
//! travelling, spawning) and the renderer need it.
//!
//! Mirrors: ShortcutMapper.cs, ShortcutHandler.cs (NextShortcutPosition)

use crate::{Room, Shortcut, Terrain};

/// The game's `Custom.fourDirections`: left, down, right, up.
pub const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// `MapShortcut` gives up after this many steps (it checks `steps <= 1000` after each).
const MAX_STEPS: u32 = 1000;

/// How a tube ends (`ShortcutData.Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutType {
    /// Ends at another entrance in the same room.
    Normal,
    /// Ends at a room-exit node tile.
    RoomExit,
    /// Ends at a creature den node tile.
    CreatureHole,
    /// Ends at an NPC transportation tile (not a node).
    NpcTransportation,
    /// Ends at a region transportation node tile.
    RegionTransportation,
    /// The walk stalls, or runs past the step limit.
    DeadEnd,
}

impl ShortcutType {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::RoomExit => "RoomExit",
            Self::CreatureHole => "CreatureHole",
            Self::NpcTransportation => "NPCTransportation",
            Self::RegionTransportation => "RegionTransportation",
            Self::DeadEnd => "DeadEnd",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::Normal,
            Self::RoomExit,
            Self::CreatureHole,
            Self::NpcTransportation,
            Self::RegionTransportation,
            Self::DeadEnd,
        ]
        .into_iter()
        .find(|t| t.value() == value)
    }
}

/// One shortcut (`ShortcutData`): the tube walked from one entrance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutData {
    pub kind: ShortcutType,
    /// The entrance tile.
    pub start: (i32, i32),
    /// The last tile of the walk.
    pub dest: (i32, i32),
    /// Steps walked (`connection.distance`).
    pub length: u32,
    /// Index into [`ShortcutMap::nodes`] for room exits, dens and region transports.
    pub dest_node: Option<usize>,
    /// Every visited tile from `start` to `dest`. A stalled walk repeats its last tile.
    pub path: Vec<(i32, i32)>,
}

/// A room's nodes and shortcuts (`Room.exitAndDenIndex`, `Room.shortcuts`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShortcutMap {
    nodes: Vec<(i32, i32)>,
    shortcuts: Vec<ShortcutData>,
}

impl ShortcutMap {
    /// Maps `room` as `ShortcutMapper`: nodes are the room-exit tiles, then the den
    /// tiles, then the region-transport tiles, each pass top row first, left to right;
    /// one shortcut per entrance tile in the same scan order.
    #[must_use]
    pub fn new(room: &Room) -> Self {
        let nodes: Vec<(i32, i32)> = [
            Shortcut::RoomExit,
            Shortcut::CreatureDen,
            Shortcut::RegionTransportation,
        ]
        .into_iter()
        .flat_map(|kind| {
            room.scan_top_down()
                .filter(move |&(x, y)| room.tile(x, y).shortcut == kind)
        })
        .collect();
        let shortcuts = room
            .scan_top_down()
            .filter(|&(x, y)| room.tile(x, y).terrain == Terrain::ShortcutEntrance)
            .map(|start| ShortcutData::walk(room, start, &nodes))
            .collect();
        Self { nodes, shortcuts }
    }

    /// Node tiles (`exitAndDenIndex`).
    #[must_use]
    pub fn nodes(&self) -> &[(i32, i32)] {
        &self.nodes
    }

    /// Shortcuts in entrance scan order (`Room.shortcuts`).
    #[must_use]
    pub fn shortcuts(&self) -> &[ShortcutData] {
        &self.shortcuts
    }

    /// The shortcut starting at `tile`, as `Room.shortcutData`. `None` where the game
    /// returns its empty `DeadEnd` placeholder: tiles without a shortcut and, a quirk
    /// of the game, tile `(0, 0)` even if it is an entrance.
    #[must_use]
    pub fn data_at(&self, tile: (i32, i32)) -> Option<&ShortcutData> {
        if tile == (0, 0) {
            return None;
        }
        self.shortcuts.iter().find(|s| s.start == tile)
    }

    /// `Array.IndexOf(shortcutsIndex, tile)`: the shortcut whose entrance is `tile`.
    #[must_use]
    pub fn index_of(&self, tile: (i32, i32)) -> Option<usize> {
        self.shortcuts.iter().position(|s| s.start == tile)
    }

    /// Type of the shortcut at `tile` (`Room.shortcutData(tile).shortCutType`).
    #[must_use]
    pub fn type_at(&self, tile: (i32, i32)) -> ShortcutType {
        self.data_at(tile).map_or(ShortcutType::DeadEnd, |s| s.kind)
    }

    /// `Room.ShortcutLeadingToNode`: the first shortcut ending in `node`, else the first
    /// shortcut; `None` only without shortcuts (where the game throws).
    #[must_use]
    pub fn leading_to_node(&self, node: usize) -> Option<&ShortcutData> {
        self.shortcuts
            .iter()
            .find(|s| s.dest_node == Some(node))
            .or_else(|| self.shortcuts.first())
    }
}

impl ShortcutData {
    /// `ShortcutMapper.MapShortcut` from the entrance `start`; `nodes` give the
    /// `dest_node` of an end at a node (the type does not depend on them).
    #[must_use]
    pub fn walk(room: &Room, start: (i32, i32), nodes: &[(i32, i32)]) -> Self {
        let mut path = vec![start];
        let (mut last, mut pos) = (start, start);
        let mut steps = 0;
        let kind = loop {
            steps += 1;
            let prev = pos;
            pos = room.next_shortcut_position(pos, last);
            path.push(pos);
            if pos == prev {
                break ShortcutType::DeadEnd;
            }
            last = prev;
            let tile = room.tile(pos.0, pos.1);
            if tile.terrain == Terrain::ShortcutEntrance {
                break ShortcutType::Normal;
            }
            match tile.shortcut {
                Shortcut::RoomExit => break ShortcutType::RoomExit,
                Shortcut::CreatureDen => break ShortcutType::CreatureHole,
                Shortcut::NpcTransportation => break ShortcutType::NpcTransportation,
                Shortcut::RegionTransportation => break ShortcutType::RegionTransportation,
                Shortcut::None | Shortcut::Path => {}
            }
            if steps > MAX_STEPS {
                break ShortcutType::DeadEnd;
            }
        };
        let dest_node = match kind {
            ShortcutType::RoomExit
            | ShortcutType::CreatureHole
            | ShortcutType::RegionTransportation => nodes.iter().position(|&n| n == pos),
            _ => None,
        };
        Self {
            kind,
            start,
            dest: pos,
            length: steps,
            dest_node,
            path,
        }
    }
}

/// What an unmapped room reads: no nodes, no shortcuts.
static UNMAPPED: ShortcutMap = ShortcutMap {
    nodes: Vec::new(),
    shortcuts: Vec::new(),
};

impl Room {
    /// The shortcut map, empty until [`Self::map_shortcuts`] has run (the game's
    /// `shortcutData` gives its `DeadEnd` placeholder there).
    #[must_use]
    pub fn shortcut_map(&self) -> &ShortcutMap {
        self.shortcut_map.get().unwrap_or(&UNMAPPED)
    }

    /// `ShortcutMapper.Update` run to its end: maps the room once, then gives the map.
    pub fn map_shortcuts(&self) -> &ShortcutMap {
        self.shortcut_map.get_or_init(|| ShortcutMap::new(self))
    }

    /// The mapper's result handed over from the loading thread; a room already mapped
    /// keeps its map (the same, from the same tiles).
    pub fn install_shortcut_map(&self, map: ShortcutMap) {
        let _ = self.shortcut_map.set(map);
    }

    #[must_use]
    pub fn shortcuts_mapped(&self) -> bool {
        self.shortcut_map.get().is_some()
    }

    /// The room as `new Room` reads it, before its `ShortcutMapper` ran.
    pub fn unmap_shortcuts(&mut self) {
        self.shortcut_map.take();
    }

    /// Every tile coordinate, top row first, left to right (the mapper's scan order).
    pub fn scan_top_down(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..self.height())
            .rev()
            .flat_map(move |y| (0..self.width()).map(move |x| (x, y)))
    }

    /// `ShortcutHandler.NextShortcutPosition`: the tube tile after `pos` coming from
    /// `last`. Straight on if that tile is in the room with any shortcut value, else
    /// the first such neighbour (left, down, right, up) that is not straight back
    /// (nothing is skipped when `pos == last`); `pos` itself when there is none.
    #[must_use]
    pub fn next_shortcut_position(&self, pos: (i32, i32), last: (i32, i32)) -> (i32, i32) {
        let dir = (pos.0 - last.0, pos.1 - last.1);
        let straight = (pos.0 + dir.0, pos.1 + dir.1);
        if dir != (0, 0) && self.is_tube_tile(straight) {
            return straight;
        }
        let next = FOUR_DIRECTIONS
            .into_iter()
            .filter(|&d| d != (-dir.0, -dir.1))
            .map(|(dx, dy)| (pos.0 + dx, pos.1 + dy))
            .find(|&p| self.is_tube_tile(p))
            .unwrap_or(pos);
        // The game steps back by `dir` if this lands on `last`; the reverse is skipped
        // above, so that only happens with `dir == (0, 0)`, where it changes nothing.
        if next == last {
            (next.0 - dir.0, next.1 - dir.1)
        } else {
            next
        }
    }

    /// `Room.ShorcutEntranceHoleDirection`: the first neighbour of `(x, y)`, in the
    /// order left, down, right, up, whose terrain is not Solid (clamped lookups);
    /// `(0, 0)` if all four are Solid.
    #[must_use]
    pub fn shortcut_hole_direction(&self, x: i32, y: i32) -> (i32, i32) {
        FOUR_DIRECTIONS
            .into_iter()
            .find(|&(dx, dy)| !self.tile(x + dx, y + dy).is_solid())
            .unwrap_or((0, 0))
    }

    /// In the room with any shortcut value (entrances included through flag 3).
    fn is_tube_tile(&self, (x, y): (i32, i32)) -> bool {
        self.get(x, y).is_some_and(|t| t.shortcut != Shortcut::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a room from ASCII rows (top row first): `.` Air, `#` Solid, `-` Floor,
    /// `E` entrance (flag 3), `e` entrance without flags, `x` Solid path, `o` Air path,
    /// `R` Solid room exit, `D` Solid den, `N` Solid NPC transport, `T` Solid region
    /// transport.
    fn ascii(rows: &[&str]) -> Room {
        let (w, h) = (rows[0].len(), rows.len());
        let mut grid = String::new();
        for x in 0..w {
            for row in rows {
                grid.push_str(match row.as_bytes()[x] {
                    b'#' => "1",
                    b'-' => "3",
                    b'E' => "4,3",
                    b'e' => "4",
                    b'x' => "1,3",
                    b'o' => "0,3",
                    b'R' => "1,4",
                    b'D' => "1,5",
                    b'N' => "1,9",
                    b'T' => "1,12",
                    _ => "0",
                });
                grid.push('|');
            }
        }
        Room::parse(&format!(
            "T\n{w}*{h}|-1|0\n0*0|0|0\n0,0\n\n\n\n\n\n\n\n{grid}\n"
        ))
        .expect("test room parses")
    }

    #[test]
    fn hole_direction_prefers_left_down_right_up() {
        // (2,1): left is Air. (4,1): left and down Solid, right Air.
        let r = ascii(&["######", "#.E#E.", "######"]);
        assert_eq!(r.shortcut_hole_direction(2, 1), (-1, 0));
        assert_eq!(r.shortcut_hole_direction(4, 1), (1, 0));
        // Enclosed.
        let r = ascii(&["###", "#E#", "###"]);
        assert_eq!(r.shortcut_hole_direction(1, 1), (0, 0));
        // Air path tiles and Floors are not Solid: (1,1)'s left is an `o` path tile.
        let r = ascii(&["#-#", "oE#", "###"]);
        assert_eq!(r.shortcut_hole_direction(1, 1), (-1, 0));
        // Down is a Floor (not Solid) and comes before up.
        let r = ascii(&["#.#", "#E#", "#-#"]);
        assert_eq!(r.shortcut_hole_direction(1, 1), (0, -1));
        // Out of the room the edge tile repeats: (0,1)'s left clamps to itself, an
        // entrance, which reads as Solid; down is Air.
        let r = ascii(&["##", "E#", ".#"]);
        assert_eq!(r.shortcut_hole_direction(0, 1), (0, -1));
    }

    #[test]
    fn normal_pipe_between_two_entrances() {
        // From (1,3) with no direction yet: left (0,3) has no shortcut value, down
        // (1,2) has. Straight to (1,1); straight (1,0) has none, so the order: left
        // (0,1) no, down no, right (2,1) yes. Straight to (3,1); (4,1) has none: left is
        // the reverse, down no, right no, up (3,2). Straight to (3,3), an entrance:
        // Normal after 6 steps.
        let r = ascii(&[".....", "#E#E#", "#x#x#", "#xxx#", "#####"]);
        let map = r.map_shortcuts();
        assert_eq!(map.shortcuts().len(), 2);
        let s = &map.shortcuts()[0];
        assert_eq!(s.kind, ShortcutType::Normal);
        assert_eq!(s.start, (1, 3));
        assert_eq!(s.dest, (3, 3));
        assert_eq!(s.length, 6);
        assert_eq!(s.dest_node, None);
        assert_eq!(
            s.path,
            vec![(1, 3), (1, 2), (1, 1), (2, 1), (3, 1), (3, 2), (3, 3)]
        );
        assert_eq!(map.shortcuts()[1].dest, (1, 3));
        assert_eq!(map.type_at((1, 3)), ShortcutType::Normal);
        assert!(map.nodes().is_empty());
    }

    #[test]
    fn walk_prefers_straight_over_the_direction_order() {
        // Down from the entrance (1,4): at (1,3) the left neighbour (0,3) is a path
        // tile and comes first in the order, but straight on (1,2) wins; at (1,2)
        // straight (1,1) is the room exit.
        let r = ascii(&["..#", "#E#", "xx#", "#x#", "#R#", "###"]);
        let s = &r.map_shortcuts().shortcuts()[0];
        assert_eq!(s.path, vec![(1, 4), (1, 3), (1, 2), (1, 1)]);
        assert_eq!(
            (s.kind, s.dest_node, s.length),
            (ShortcutType::RoomExit, Some(0), 3)
        );
    }

    #[test]
    fn walk_turns_by_the_direction_order_when_straight_is_blocked() {
        // From (2,2) down to (2,1); straight (2,0) has nothing, so left (1,1) beats
        // right (3,1). From (1,1) straight on (0,1) is the den.
        let r = ascii(&["#####", "##E##", "DxxxR", "#####"]);
        let s = &r.map_shortcuts().shortcuts()[0];
        assert_eq!(s.path, vec![(2, 2), (2, 1), (1, 1), (0, 1)]);
        assert_eq!(s.kind, ShortcutType::CreatureHole);
        // Nodes: room exits first, then dens. The exit (4,1) is node 0, the den node 1.
        assert_eq!(r.map_shortcuts().nodes(), &[(4, 1), (0, 1)]);
        assert_eq!(s.dest_node, Some(1));
    }

    #[test]
    fn dead_ends() {
        // No neighbour with a shortcut value: stalls on the first step, and the path
        // repeats the stalled tile.
        let r = ascii(&["...", "#E#", "###"]);
        let s = &r.map_shortcuts().shortcuts()[0];
        assert_eq!(s.kind, ShortcutType::DeadEnd);
        assert_eq!(s.path, vec![(1, 1), (1, 1)]);
        assert_eq!((s.dest, s.length), ((1, 1), 1));
        // A tube that stops at a plain path tile: stalls at (1,1) after one step.
        let r = ascii(&["...", "#E#", "#x#", "###"]);
        let s = &r.map_shortcuts().shortcuts()[0];
        assert_eq!(s.kind, ShortcutType::DeadEnd);
        assert_eq!(s.path, vec![(1, 2), (1, 1), (1, 1)]);
        assert_eq!(s.length, 2);
        // An entrance tile without a shortcut value is not a tube tile: the walk from
        // (1,2) does not step into the bare entrance (3,2).
        let r = ascii(&[".....", "#E#e#", "#xxx#", "#####"]);
        let s = &r.map_shortcuts().shortcuts()[0];
        assert_eq!(s.kind, ShortcutType::DeadEnd);
        assert_eq!(s.dest, (3, 1));
    }

    #[test]
    fn npc_and_region_transport_ends() {
        let r = ascii(&[".....", "#E#E#", "#N#T#", "#####"]);
        let map = r.map_shortcuts();
        let [npc, region] = map.shortcuts() else {
            panic!("two shortcuts");
        };
        assert_eq!(
            (npc.kind, npc.dest_node),
            (ShortcutType::NpcTransportation, None)
        );
        // NPC transport tiles are not nodes; the region transport is the only node.
        assert_eq!(map.nodes(), &[(3, 1)]);
        assert_eq!(
            (region.kind, region.dest_node),
            (ShortcutType::RegionTransportation, Some(0))
        );
    }

    #[test]
    fn node_and_shortcut_scan_order() {
        // Two exits and two dens. Nodes: exits top row first ((3,3) above (1,1)), then
        // dens ((1,3) above (3,1)). Entrances scan top row first, left to right.
        let r = ascii(&[
            ".......", //
            "#E#E#..", // y=4
            "#D#R#..", // y=3
            "#E#E#..", // y=2
            "#R#D#..", // y=1
            "#######",
        ]);
        let map = r.map_shortcuts();
        assert_eq!(map.nodes(), &[(3, 3), (1, 1), (1, 3), (3, 1)]);
        let starts: Vec<_> = map.shortcuts().iter().map(|s| s.start).collect();
        assert_eq!(starts, vec![(1, 4), (3, 4), (1, 2), (3, 2)]);
        let dests: Vec<_> = map.shortcuts().iter().map(|s| s.dest_node).collect();
        assert_eq!(dests, vec![Some(2), Some(0), Some(1), Some(3)]);
        assert_eq!(map.leading_to_node(1).map(|s| s.start), Some((1, 2)));
        // Unknown node: the first shortcut.
        assert_eq!(map.leading_to_node(9).map(|s| s.start), Some((1, 4)));
    }

    #[test]
    fn data_at_tile_zero_is_the_dead_end_placeholder() {
        // A Normal pipe from (0,0) to (0,2): the game's `shortcutData` never finds the
        // entry at (0,0).
        let r = ascii(&["E", "x", "E"]);
        let map = r.map_shortcuts();
        assert_eq!(map.shortcuts()[1].start, (0, 0));
        assert_eq!(map.shortcuts()[1].kind, ShortcutType::Normal);
        assert_eq!(map.type_at((0, 0)), ShortcutType::DeadEnd);
        assert_eq!(map.type_at((0, 2)), ShortcutType::Normal);
        assert_eq!(map.type_at((0, 1)), ShortcutType::DeadEnd);
    }

    #[test]
    fn next_position_at_the_start_tries_every_direction() {
        // From a tile with `pos == last`, no direction is skipped: (1,1) goes up to
        // (1,2) when that is the only tube neighbour.
        let r = ascii(&["#x#", "#x#", "###"]);
        assert_eq!(r.next_shortcut_position((1, 1), (1, 1)), (1, 2));
        // Coming from (1,2) the reverse is skipped: nothing else, so it stays.
        assert_eq!(r.next_shortcut_position((1, 1), (1, 2)), (1, 1));
    }
}
