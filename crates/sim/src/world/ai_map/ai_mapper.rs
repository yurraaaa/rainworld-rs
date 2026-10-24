//! `AImapper`: builds a room's [`AiMap`] in the game's passes. First every tile's
//! accessibility class (`FindAccessibilityOfCurrentTile`), then per tile its
//! connections, narrow space and floor altitudes (`FindPassagesOfCurrentTile`) and fall
//! risk and smoothed altitude (`FindFallRiskOfCurrentTile`), rows bottom-up and left to
//! right, then the terrain proximity (`TerrainProximityMapper`). The room's terrain
//! curves (`TerrainManager`, DLC only) are not ported. No random draws.
//!
//! Mirrors: AImapper.cs

use std::collections::VecDeque;

use rw_assets::{Room, ShortcutType, Terrain};

use super::movement::{Accessibility, MovementConnection, MovementType};
use crate::world::AiMap;
use crate::world::ai_map::{AiTile, EIGHT_DIRECTIONS, FOUR_DIRECTIONS};

/// An `AImapper` run over a realized room (a shelter whose door opened): without Remix
/// `Update` takes one step per tile in each of the first two passes, one to start the
/// terrain proximity, one per tile it queues plus two to finish, and one to end, so
/// `2 * width * height + queued + 4` steps. The room does not change while it runs, so
/// the map is built at the first step and handed over once the steps are done.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiMapper {
    pub steps: i32,
    /// The steps the run takes, known from the first step.
    pub total: Option<i32>,
    map: Option<AiMap>,
}

impl AiMapper {
    #[must_use]
    pub fn done(&self) -> bool {
        self.total.is_some_and(|t| self.steps >= t)
    }

    /// `n` calls of `Update`, mapping `room` over a copy of its current map `old`.
    pub(crate) fn update(&mut self, n: i32, old: &AiMap, room: &Room) {
        if self.done() {
            return;
        }
        let total = match self.total {
            Some(t) => t,
            None => {
                let (map, queued) = old.remapped(room);
                let t = 2 * map.width * map.height + i32::try_from(queued).unwrap_or(0) + 4;
                self.map = Some(map);
                self.total = Some(t);
                t
            }
        };
        self.steps = (self.steps + n).min(total);
    }

    /// `ReturnAIMap()` once done.
    pub(crate) fn take_map(&mut self) -> Option<AiMap> {
        if self.done() { self.map.take() } else { None }
    }
}

impl AiMap {
    /// `room.aimap.map = new AImapper(room).ReturnAIMap().map`: this map with its tiles
    /// mapped again from `room` (their baked visibility gone; the terrain proximity,
    /// creature layers and exits kept), and how many tiles the terrain proximity pass
    /// queued.
    pub(crate) fn remapped(&self, room: &Room) -> (Self, usize) {
        let mut fresh = self.clone();
        fresh.tiles = Vec::with_capacity(self.tiles.len());
        fresh.terrain_proximity = vec![-1; self.terrain_proximity.len()];
        fresh.map_room(room);
        let queued = fresh.terrain_proximity.iter().filter(|&&p| p > 0).count();
        fresh.terrain_proximity.clone_from(&self.terrain_proximity);
        (fresh, queued)
    }

    /// The mapper's passes over `room`.
    pub(crate) fn map_room(&mut self, room: &Room) {
        for x in 0..self.width {
            for y in 0..self.height {
                let tile = AiTile::new(Self::accessibility_of(room, x, y), room.water_int(x, y));
                self.tiles.push(tile);
            }
        }
        for t in 0..self.tiles.len() {
            let (x, y) = self.tile_of(t);
            self.tiles[t].fall_risk_tile = (x, y);
        }
        for y in 0..self.height {
            for x in 0..self.width {
                self.find_passages(room, x, y);
                self.find_fall_risk(room, x, y);
            }
        }
        self.map_terrain_proximity(room);
    }

    fn tile_of(&self, i: usize) -> (i32, i32) {
        let i = i32::try_from(i).unwrap_or(0);
        (i / self.height, i % self.height)
    }

    fn acc(&self, x: i32, y: i32) -> Accessibility {
        self.tile(x, y).acc
    }

    fn acc_at(&self, p: (i32, i32), d: (i32, i32)) -> Accessibility {
        self.acc(p.0 + d.0, p.1 + d.1)
    }

    fn walkable_at(&self, p: (i32, i32), d: (i32, i32)) -> bool {
        self.tile(p.0 + d.0, p.1 + d.1).walkable
    }

    fn push(&mut self, kind: MovementType, from: (i32, i32), to: (i32, i32), distance: i32) {
        let c = MovementConnection::new(
            kind,
            self.world_coordinate(from),
            self.world_coordinate(to),
            distance,
        );
        self.tile_mut(from.0, from.1).outgoing_paths.push(c);
    }

    /// `FindAccessibilityOfCurrentTile`: Solid; else Floor over Solid or a Floor tile
    /// (or by a diagonal Solid with the side open), Corridor between Solid sides, Climb
    /// on a pole, Wall by a wall (or the background wall), Ceiling under Solid, else Air.
    fn accessibility_of(room: &Room, x: i32, y: i32) -> Accessibility {
        let solid = [-1, 0, 1].map(|dy| [-1, 0, 1].map(|dx| room.tile(x + dx, y + dy).is_solid()));
        // solid[dy + 1][dx + 1]
        if solid[1][1] {
            return Accessibility::Solid;
        }
        let below = room.tile(x, y - 1).terrain;
        let here = room.tile(x, y);
        if below == Terrain::Solid
            || below == Terrain::Floor
            || (solid[0][0] && !solid[1][0])
            || (solid[0][2] && !solid[1][2])
        {
            Accessibility::Floor
        } else if solid[1][0] && solid[1][2] {
            Accessibility::Corridor
        } else if here.any_pole() {
            Accessibility::Climb
        } else if here.wall_behind()
            || solid[1][0]
            || solid[1][2]
            || (solid[2][0] && !solid[2][1])
            || (solid[2][2] && !solid[2][1])
        {
            Accessibility::Wall
        } else if solid[2][1] {
            Accessibility::Ceiling
        } else {
            Accessibility::Air
        }
    }

    /// `FindPassagesOfCurrentTile`.
    fn find_passages(&mut self, room: &Room, x: i32, y: i32) {
        let p = (x, y);
        let here = room.tile(x, y).terrain;
        if here == Terrain::Floor
            || (here == Terrain::Solid && room.tile(x, y + 1).terrain != Terrain::Solid)
        {
            let mut i = y + 1;
            while i < self.height && room.tile(x, i).terrain != Terrain::Solid {
                self.tile_mut(x, i).floor_altitude = i - y;
                i += 1;
            }
        }
        for j in 0..4 {
            let (dx, dy) = FOUR_DIRECTIONS[j];
            if self.index(x + dx, y + dy).is_some() {
                self.push(MovementType::Standard, p, (x + dx, y + dy), 1);
            }
            let (ex, ey) = EIGHT_DIRECTIONS[j];
            if room.tile(x + ex, y + ey).terrain == Terrain::Solid
                && room.tile(x - ex, y - ey).terrain == Terrain::Solid
            {
                self.tile_mut(x, y).narrow_space = true;
            }
        }
        self.find_vertical_reaches(p);
        self.find_drops(room, p);
        for dx in [-1, 1] {
            self.find_sideways_passages(room, p, dx);
        }
        if room.get(x, y).map(|t| t.terrain) == Some(Terrain::ShortcutEntrance) {
            self.find_shortcut_passages(room, p);
        }
        let outgoing = self.tile(x, y).outgoing_paths.clone();
        for c in outgoing {
            let (dx, dy) = c.dest_tile();
            if let Some(i) = self.index(dx, dy) {
                self.tiles[i].incoming_paths.push(c);
            }
        }
    }

    /// ReachUp, DoubleReachUp and ReachDown: past a more open tile to a less open one.
    fn find_vertical_reaches(&mut self, p: (i32, i32)) {
        let here = self.acc_at(p, (0, 0));
        let (up1, up2, up3) = (
            self.acc_at(p, (0, 1)),
            self.acc_at(p, (0, 2)),
            self.acc_at(p, (0, 3)),
        );
        if up1 != Accessibility::Solid && self.walkable_at(p, (0, 2)) && up1 > here && up2 < up1 {
            self.push(MovementType::ReachUp, p, (p.0, p.1 + 2), 2);
        }
        if self.walkable_at(p, (0, 3))
            && up1 != Accessibility::Solid
            && up2 != Accessibility::Solid
            && up1 > here
            && up1 > up3
            && up2 > here
            && up2 > up3
        {
            self.push(MovementType::DoubleReachUp, p, (p.0, p.1 + 3), 3);
        }
        let down1 = self.acc_at(p, (0, -1));
        // The game compares the landing tile with the tile above, not the one between.
        if down1 != Accessibility::Solid
            && self.walkable_at(p, (0, -2))
            && down1 > here
            && self.acc_at(p, (0, -2)) < up1
        {
            self.push(MovementType::ReachDown, p, (p.0, p.1 - 2), 2);
        }
    }

    /// The drops off a tile with a more open tile under it: straight down to the first
    /// floor, onto climbable tiles and the water surface on the way, and after a fall of
    /// four or more, diagonally onto the columns either side.
    fn find_drops(&mut self, room: &Room, p: (i32, i32)) {
        let (x, y) = p;
        let below = self.acc(x, y - 1);
        if below == Accessibility::Solid || below <= self.acc(x, y) {
            return;
        }
        let mut fallen = 0;
        let mut landed: Vec<i32> = Vec::new();
        let mut row = y - 1;
        while row > 0 {
            let acc = self.acc(x, row);
            let under = room.tile(x, row - 1).terrain;
            if (acc == Accessibility::Floor || acc == Accessibility::CurvedFloor)
                && (under == Terrain::Solid || under == Terrain::Floor)
            {
                self.push(MovementType::DropToFloor, p, (x, row), y - row);
                landed.extend([row, row - 1, row + 1]);
                break;
            }
            if acc == Accessibility::Climb && self.acc(x, row + 1) > acc {
                self.push(MovementType::DropToClimb, p, (x, row), y - row);
                landed.push(row);
            }
            if room.water_int(x, row) == 2 {
                self.push(MovementType::DropToWater, p, (x, row), y - row);
                landed.push(row);
            }
            fallen += 1;
            row -= 1;
        }
        if fallen < 4 {
            return;
        }
        for side in [1, -1] {
            let sx = x + side;
            let mut row = y - 4;
            while row > 0
                && self.acc(sx, row) != Accessibility::Solid
                && self.acc(x, row) != Accessibility::Solid
            {
                let acc = self.acc(sx, row);
                let clear_above = self.acc(sx, row + 1) != Accessibility::Solid
                    && self.acc(sx, row + 2) != Accessibility::Solid;
                let not_straight =
                    !self.is_there_a_straight_drop_between_tiles(room, (x - 1, y), (sx, row))
                        && !self.is_there_a_straight_drop_between_tiles(
                            room,
                            (x + 1, y),
                            (sx, row),
                        );
                let under = room.tile(sx, row - 1).terrain;
                if (acc == Accessibility::Floor || acc == Accessibility::CurvedFloor)
                    && clear_above
                    && (under == Terrain::Solid || under == Terrain::Floor)
                    && not_straight
                {
                    if !landed.contains(&row) {
                        self.push(MovementType::DropToFloor, p, (sx, row), y - row);
                    }
                    break;
                }
                if acc == Accessibility::Climb
                    && clear_above
                    && self.acc(sx, row + 1) > acc
                    && not_straight
                    && !landed.contains(&row)
                {
                    self.push(MovementType::DropToClimb, p, (sx, row), y - row);
                }
                row -= 1;
            }
        }
    }

    /// `IsThereAStraightDropBetweenTiles`: whether a straight drop from `a` lands on `b`
    /// (its first floor, or a climbable tile on the way).
    fn is_there_a_straight_drop_between_tiles(
        &self,
        room: &Room,
        a: (i32, i32),
        b: (i32, i32),
    ) -> bool {
        let below = self.acc(a.0, a.1 - 1);
        if below == Accessibility::Solid || below <= self.acc(a.0, a.1) {
            return false;
        }
        let mut row = a.1 - 1;
        while row > 0 {
            let acc = self.acc(a.0, row);
            let under = room.tile(a.0, row - 1).terrain;
            if acc == Accessibility::Floor && (under == Terrain::Solid || under == Terrain::Floor) {
                return (a.0, row) == b;
            }
            if acc == Accessibility::Climb && self.acc(a.0, row + 1) > acc && (a.0, row) == b {
                return true;
            }
            row -= 1;
        }
        false
    }

    /// One side's ReachOverGap, LizardTurn, Slope or OpenDiagonal, CeilingSlope and the
    /// two SemiDiagonalReaches. The paired connections are also added, reversed, to the
    /// other tile; where that tile was mapped already its reverse is copied into this
    /// tile's list too, so that it reaches this tile's incoming paths.
    fn find_sideways_passages(&mut self, room: &Room, p: (i32, i32), dx: i32) {
        let (x, y) = p;
        let here = self.acc_at(p, (0, 0));
        let side = self.acc_at(p, (dx, 0));
        let up = self.acc_at(p, (0, 1));
        let side_up = self.acc_at(p, (dx, 1));
        if side != Accessibility::Solid
            && self.walkable_at(p, (2 * dx, 0))
            && side > here
            && self.acc_at(p, (2 * dx, 0)) < side
        {
            self.push(MovementType::ReachOverGap, p, (x + 2 * dx, y), 2);
        }
        if here == Accessibility::Floor
            && side == Accessibility::Floor
            && up != Accessibility::Solid
            && side_up != Accessibility::Solid
        {
            self.push(MovementType::LizardTurn, p, (x + dx, y), 1);
        }
        let side_terrain = room.tile(x + dx, y).terrain;
        if here == Accessibility::Floor
            && side_terrain == Terrain::Slope
            && side_up == Accessibility::Floor
        {
            self.push(MovementType::Slope, p, (x + dx, y + 1), 1);
            self.push(MovementType::Slope, (x + dx, y + 1), p, 1);
        } else if side != Accessibility::Solid && up != Accessibility::Solid {
            let num = here.max(side_up);
            if side > num && up > num {
                self.push(MovementType::OpenDiagonal, p, (x + dx, y + 1), 1);
                self.push(MovementType::OpenDiagonal, (x + dx, y + 1), p, 1);
            }
        }
        let side_down = self.acc_at(p, (dx, -1));
        if here.wall_or_ceiling() && side_terrain == Terrain::Slope && side_down.wall_or_ceiling() {
            self.push(MovementType::CeilingSlope, p, (x + dx, y - 1), 1);
            self.push_paired(MovementType::CeilingSlope, p, (x + dx, y - 1), 1);
        }
        if here == Accessibility::CurvedFloor && side_up == Accessibility::CurvedFloor {
            self.push(MovementType::Slope, p, (x + dx, y + 1), 1);
            self.push(MovementType::Slope, (x + dx, y + 1), p, 1);
        }
        if self.semi_diagonal_open(p, (dx, 0), (0, -1), (dx, -1), (0, -2), (dx, -2)) {
            self.push(MovementType::SemiDiagonalReach, p, (x + dx, y - 2), 3);
            self.push_paired(MovementType::SemiDiagonalReach, p, (x + dx, y - 2), 3);
        }
        // The game reuses the side's sign as the vertical offset, always reaching left.
        if self.semi_diagonal_open(p, (0, dx), (-1, 0), (-1, dx), (-2, 0), (-2, dx)) {
            self.push(MovementType::SemiDiagonalReach, p, (x - 2, y + dx), 3);
            self.push_paired(MovementType::SemiDiagonalReach, p, (x - 2, y + dx), 3);
        }
    }

    /// Adds the reverse connection to `other`, then copies it into `p`'s list.
    fn push_paired(&mut self, kind: MovementType, p: (i32, i32), other: (i32, i32), distance: i32) {
        self.push(kind, other, p, distance);
        if let Some(&reverse) = self.tile(other.0, other.1).outgoing_paths.last() {
            self.tile_mut(p.0, p.1).outgoing_paths.push(reverse);
        }
    }

    /// The SemiDiagonalReach test: from `p` to `p + dest`, the tiles `a` (beside),
    /// `b` (under or beside), `c` (between) and `d` (the far side) all more open than
    /// both ends, `b` and `c` not Solid and `a` or `d` not Solid, and the destination
    /// walkable.
    #[allow(clippy::too_many_arguments)]
    fn semi_diagonal_open(
        &self,
        p: (i32, i32),
        a: (i32, i32),
        b: (i32, i32),
        c: (i32, i32),
        d: (i32, i32),
        dest: (i32, i32),
    ) -> bool {
        let here = self.acc_at(p, (0, 0));
        let end = self.acc_at(p, dest);
        let (a, b, c, d) = (
            self.acc_at(p, a),
            self.acc_at(p, b),
            self.acc_at(p, c),
            self.acc_at(p, d),
        );
        self.walkable_at(p, dest)
            && b != Accessibility::Solid
            && c != Accessibility::Solid
            && (a != Accessibility::Solid || d != Accessibility::Solid)
            && a > here
            && a > end
            && d > here
            && d > end
            && b > here
            && b > end
            && c > here
            && c > end
    }

    /// A shortcut entrance's BigCreatureShortCutSqueezes (both ways, to the open tiles
    /// two and three out of the hole), then its ShortCut (Normal) or NPCTransportation
    /// links. A squeeze back from an earlier tile goes straight into the incoming paths.
    fn find_shortcut_passages(&mut self, room: &Room, p: (i32, i32)) {
        let (x, y) = p;
        let hole = FOUR_DIRECTIONS
            .iter()
            .copied()
            .find(|&(dx, dy)| room.tile(x + dx, y + dy).terrain != Terrain::Solid)
            .unwrap_or((0, 0));
        let perp = (hole.1, -hole.0);
        for (l, m) in [(2, 0), (3, -1), (3, 0), (3, 1)] {
            let pos = (x + hole.0 * l + perp.0 * m, y + hole.1 * l + perp.1 * m);
            if room.tile(pos.0, pos.1).is_solid() || !room.contains(pos.0, pos.1) {
                continue;
            }
            self.push(MovementType::BigCreatureShortCutSqueeze, p, pos, l);
            let back = MovementConnection::new(
                MovementType::BigCreatureShortCutSqueeze,
                self.world_coordinate(pos),
                self.world_coordinate(p),
                l,
            );
            self.tile_mut(pos.0, pos.1).outgoing_paths.push(back);
            if pos.0 < x || pos.1 < y {
                self.tile_mut(x, y).incoming_paths.push(back);
            }
        }
        let map = &self.shortcuts;
        let Some(here) = map.data_at(p) else { return };
        let mut pushes = Vec::new();
        match here.kind {
            ShortcutType::Normal => {
                let length = i32::try_from(here.length).unwrap_or(i32::MAX);
                pushes.push((MovementType::ShortCut, here.dest, length));
            }
            ShortcutType::NpcTransportation => {
                for other in map.shortcuts() {
                    if other.kind != ShortcutType::NpcTransportation || other.start == p {
                        continue;
                    }
                    let (dx, dy) = (
                        (here.start.0 - other.start.0) as f32,
                        (here.start.1 - other.start.1) as f32,
                    );
                    let length = i32::try_from(here.length + other.length).unwrap_or(i32::MAX);
                    pushes.push((
                        MovementType::NpcTransportation,
                        other.start,
                        (dx * dx + dy * dy).sqrt() as i32 + length,
                    ));
                }
            }
            _ => {}
        }
        for (kind, to, cost) in pushes {
            self.push(kind, p, to, cost);
        }
    }

    /// `FindFallRiskOfCurrentTile`: an open tile on a floor (or the room's bottom row)
    /// marks the column up to the next Solid tile, and the open tiles beside it, as
    /// falling to it ((-1, -1) when the bottom row is open, a fall out of the room);
    /// then `smoothedFloorAltitude`.
    fn find_fall_risk(&mut self, room: &Room, x: i32, y: i32) {
        if room.tile(x, y).is_solid() {
            return;
        }
        if y == 0 || room.tile(x, y - 1).is_solid() {
            let land = if y != 0 || room.tile(x, -1).is_solid() {
                (x, y)
            } else {
                (-1, -1)
            };
            let mut i = y;
            while i < self.height && !room.tile(x, i).is_solid() {
                if self.tile(x, i).fall_risk_tile.1 >= land.1 {
                    self.tile_mut(x, i).fall_risk_tile = land;
                }
                for sx in [x - 1, x + 1] {
                    if !room.tile(sx, i).is_solid() && self.tile(sx, i).fall_risk_tile.1 > land.1 {
                        self.tile_mut(sx, i).fall_risk_tile = land;
                    }
                }
                i += 1;
            }
        }
        let mut num = self.tile(x, y).floor_altitude;
        let reach = num.clamp(0, 5);
        for j in [-1, 1] {
            let mut k = 1;
            while k < reach
                && (0..self.width).contains(&(x + j * k))
                && !room.tile(x + j * k, y).is_solid()
            {
                let side = self.tile(x + j * k, y).floor_altitude + k / 2;
                if side < num {
                    num = side;
                }
                k += 1;
            }
        }
        self.tile_mut(x, y).smoothed_floor_altitude = num;
    }

    /// `TerrainProximityMapper`: Solid tiles 0, open tiles touching a Solid one (four
    /// directions, clamped at the edges) 1, the rest by breadth-first steps from those,
    /// queued in x-major order.
    fn map_terrain_proximity(&mut self, room: &Room) {
        let mut queue = VecDeque::new();
        for x in 0..self.width {
            for y in 0..self.height {
                let i = x as usize * self.height as usize + y as usize;
                if room.tile(x, y).is_solid() {
                    self.terrain_proximity[i] = 0;
                } else if FOUR_DIRECTIONS
                    .iter()
                    .any(|(dx, dy)| room.tile(x + dx, y + dy).is_solid())
                {
                    self.terrain_proximity[i] = 1;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let here = self.terrain_proximity(x, y);
            for (dx, dy) in FOUR_DIRECTIONS {
                if let Some(i) = self.index(x + dx, y + dy)
                    && self.terrain_proximity[i] < 0
                {
                    self.terrain_proximity[i] = here + 1;
                    queue.push_back((x + dx, y + dy));
                }
            }
        }
    }
}
