//! The parts of a room's `AImap` the overseer and the Fly read: terrain proximity
//! (`AImapper.TerrainProximityMapper`), `narrowSpace` and `floorAltitude`
//! (`FindPassagesOfCurrentTile`), the tiles' water, the baked `visibility` (room file line
//! 10), the Fly's accessibility, outgoing paths and baked exit distances. The other
//! accessibility classes, passages, fall risk and the other creatures' maps are not
//! ported (`rainworld-re/notes/ai-map.md`).

use std::collections::VecDeque;

use rw_assets::{CreatureType, Room, ShortcutType, Terrain};

use super::CreatureSpecificAiMap;
use crate::creature::CreatureTemplate;

/// `Custom.fourDirections`.
const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// `Custom.eightDirections`.
pub(crate) const EIGHT_DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// A room's AI map (x-major grids, `ExtraExtentions.ind(x, y, height)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiMap {
    width: i32,
    height: i32,
    terrain_proximity: Vec<i32>,
    narrow_space: Vec<bool>,
    visibility: Vec<i32>,
    /// `AItile.waterInt`: 1 deep water, 2 the surface.
    water: Vec<u8>,
    /// Whether the tile's accessibility is Solid (the only class the Fly tells apart).
    solid: Vec<bool>,
    floor_altitude: Vec<i32>,
    smoothed_floor_altitude: Vec<i32>,
    /// `creatureSpecificAImaps` of the Fly.
    fly: Option<CreatureSpecificAiMap>,
}

/// `MovementConnection.MovementType`, the types the Fly may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementType {
    Standard,
    ShortCut,
    NpcTransportation,
}

/// `MovementConnection` inside one room (tiles; the room is the map's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementConnection {
    pub kind: MovementType,
    pub start: (i32, i32),
    pub dest: (i32, i32),
    pub distance: i32,
}

impl AiMap {
    /// The mapper's result for `room` (made on the room preparer's thread before
    /// `ReadyForAI`).
    #[must_use]
    pub fn new(room: &Room) -> Self {
        let (width, height) = (room.width(), room.height());
        let cells = usize::try_from(width * height).unwrap_or(0);
        let mut map = Self {
            width,
            height,
            terrain_proximity: vec![-1; cells],
            narrow_space: vec![false; cells],
            visibility: vec![0; cells],
            water: vec![0; cells],
            solid: vec![false; cells],
            floor_altitude: vec![100_000; cells],
            smoothed_floor_altitude: vec![100_000; cells],
            fly: None,
        };
        map.map_accessibility(room);
        map.map_terrain_proximity(room);
        map.map_narrow_spaces(room);
        map.map_floor_altitudes(room);
        map.smooth_floor_altitudes(room);
        map.set_visibility(room);
        let fly = CreatureTemplate::of(CreatureType::Fly, rw_assets::ActiveMods::NONE);
        map.fly = Some(CreatureSpecificAiMap::new(&map, room, &fly));
        map
    }

    /// `FindAccessibilityOfCurrentTile`'s Solid class and water (no terrain manager).
    fn map_accessibility(&mut self, room: &Room) {
        for x in 0..self.width {
            for y in 0..self.height {
                let i = x as usize * self.height as usize + y as usize;
                self.solid[i] = room.tile(x, y).is_solid();
                self.water[i] = room.water_int(x, y);
            }
        }
    }

    /// `floorAltitude`: from a Floor tile, or a Solid tile under a non-Solid one, up the
    /// column to the next Solid tile; rows bottom-up, so the nearest floor below wins.
    fn map_floor_altitudes(&mut self, room: &Room) {
        for y in 0..self.height {
            for x in 0..self.width {
                let tile = room.tile(x, y);
                let floor = tile.terrain == Terrain::Floor
                    || (tile.is_solid() && !room.tile(x, y + 1).is_solid());
                if !floor {
                    continue;
                }
                let mut above = y + 1;
                while above < self.height && !room.tile(x, above).is_solid() {
                    let i = x as usize * self.height as usize + above as usize;
                    self.floor_altitude[i] = above - y;
                    above += 1;
                }
            }
        }
    }

    /// `smoothedFloorAltitude` (`FindFallRiskOfCurrentTile`, after every floor altitude
    /// of the row is final): the lowest of the tile's floor altitude and, up to
    /// `min(altitude, 5) - 1` open tiles to each side, theirs plus half the distance.
    /// Solid tiles keep 100000.
    fn smooth_floor_altitudes(&mut self, room: &Room) {
        for y in 0..self.height {
            for x in 0..self.width {
                if room.tile(x, y).is_solid() {
                    continue;
                }
                let mut num = self.floor_altitude(x, y);
                let reach = num.clamp(0, 5);
                for j in [-1, 1] {
                    let mut k = 1;
                    while k < reach
                        && (0..self.width).contains(&(x + j * k))
                        && !room.tile(x + j * k, y).is_solid()
                    {
                        let side = self.floor_altitude(x + j * k, y) + k / 2;
                        if side < num {
                            num = side;
                        }
                        k += 1;
                    }
                }
                let i = x as usize * self.height as usize + y as usize;
                self.smoothed_floor_altitude[i] = num;
            }
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.height).contains(&y))
            .then(|| usize::try_from(x * self.height + y).unwrap_or(0))
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

    /// `narrowSpace`: a tile with Solid on both sides along one of the first four
    /// `eightDirections` (clamped lookups).
    fn map_narrow_spaces(&mut self, room: &Room) {
        for x in 0..self.width {
            for y in 0..self.height {
                let narrow = EIGHT_DIRECTIONS[..4].iter().any(|(dx, dy)| {
                    room.tile(x + dx, y + dy).terrain == Terrain::Solid
                        && room.tile(x - dx, y - dy).terrain == Terrain::Solid
                });
                let i = x as usize * self.height as usize + y as usize;
                self.narrow_space[i] = narrow;
            }
        }
    }

    /// `SetVisibilityMapFromCompressedArray`: the baked values over the non-Solid tiles.
    fn set_visibility(&mut self, room: &Room) {
        let mut values = room.visibility().iter();
        for x in 0..self.width {
            for y in 0..self.height {
                if room.tile(x, y).is_solid() {
                    continue;
                }
                let Some(&v) = values.next() else {
                    return;
                };
                let i = x as usize * self.height as usize + y as usize;
                self.visibility[i] = v;
            }
        }
    }

    /// `getTerrainProximity`: -1 outside the room.
    #[must_use]
    pub fn terrain_proximity(&self, x: i32, y: i32) -> i32 {
        self.index(x, y).map_or(-1, |i| self.terrain_proximity[i])
    }

    /// `getAItile(x, y).narrowSpace` (false for the standard tile outside the room).
    #[must_use]
    pub fn narrow_space(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.narrow_space[i])
    }

    /// `getAItile(x, y).visibility` (0 outside the room and on Solid tiles).
    #[must_use]
    pub fn visibility(&self, x: i32, y: i32) -> i32 {
        self.index(x, y).map_or(0, |i| self.visibility[i])
    }
}

impl AiMap {
    /// `getAItile(x, y).floorAltitude` (the standard tile's 100000 outside the room).
    #[must_use]
    pub fn floor_altitude(&self, x: i32, y: i32) -> i32 {
        self.index(x, y).map_or(100_000, |i| self.floor_altitude[i])
    }

    /// `getAItile(x, y).smoothedFloorAltitude` (100000 outside the room).
    #[must_use]
    pub fn smoothed_floor_altitude(&self, x: i32, y: i32) -> i32 {
        self.index(x, y)
            .map_or(100_000, |i| self.smoothed_floor_altitude[i])
    }

    /// `getAItile(x, y).AnyWater`.
    #[must_use]
    pub fn any_water(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.water[i] != 0)
    }

    /// `getAItile(x, y).DeepWater`.
    #[must_use]
    pub fn deep_water(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.water[i] == 1)
    }

    /// `getAItile(x, y).WaterSurface`.
    #[must_use]
    pub fn water_surface(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.water[i] == 2)
    }

    /// `TileAccessibleToCreature` for a template whose highest allowed accessibility is
    /// Air (the Fly: the constructor allows every class up to it, so all but Solid): its
    /// water rule, a pre-baked layer, and not Solid (the standard tile outside the room
    /// is). The base game's water access modifier allows every tile; worm grass is not
    /// ported (docs/DIVERGENCES.md D14).
    #[must_use]
    pub fn tile_accessible_to_creature(&self, x: i32, y: i32, template: &CreatureTemplate) -> bool {
        debug_assert_eq!(
            template.kind,
            CreatureType::Fly,
            "only the Fly's rule is ported"
        );
        if !template
            .water_relationship
            .movement_legal(self.deep_water(x, y), self.water_surface(x, y))
        {
            return false;
        }
        if template.pre_baked_pathing_index() == -1 {
            return false;
        }
        self.index(x, y).is_some_and(|i| !self.solid[i])
    }

    /// The Fly's `CreatureSpecificAImap`.
    #[must_use]
    pub fn fly(&self) -> Option<&CreatureSpecificAiMap> {
        self.fly.as_ref()
    }

    /// `ExitDistanceForCreatureAndCheckNeighbours(tile, node, Fly)`: the first of the
    /// tile and its left, down, right and up neighbours with a distance, else -1.
    #[must_use]
    pub fn fly_exit_distance_and_check_neighbours(&self, tile: (i32, i32), node: i32) -> i32 {
        let Some(map) = &self.fly else { return -1 };
        if node < 0 {
            return -1;
        }
        [(0, 0), (-1, 0), (0, -1), (1, 0), (0, 1)]
            .into_iter()
            .map(|(dx, dy)| map.distance_to_exit(tile.0 + dx, tile.1 + dy, node))
            .find(|&d| d > -1)
            .unwrap_or(-1)
    }

    /// `ExitDistanceForCreature(tile, node, Fly)`: -1 on a tile the Fly cannot reach.
    #[must_use]
    pub fn fly_exit_distance(&self, tile: (i32, i32), node: i32) -> i32 {
        let fly = CreatureTemplate::of(CreatureType::Fly, rw_assets::ActiveMods::NONE);
        match &self.fly {
            Some(map) if self.tile_accessible_to_creature(tile.0, tile.1, &fly) => {
                map.distance_to_exit(tile.0, tile.1, node)
            }
            _ => -1,
        }
    }

    /// `getAItile(x, y).outgoingPaths` of the types the Fly may use, in the list's order:
    /// the four Standard links inside the room, then on a shortcut entrance the ShortCut
    /// link of a Normal shortcut or the links to every other NPC transportation entrance.
    #[must_use]
    pub fn fly_outgoing_paths(&self, room: &Room, x: i32, y: i32) -> Vec<MovementConnection> {
        if self.index(x, y).is_none() {
            return Vec::new();
        }
        let mut paths: Vec<MovementConnection> = FOUR_DIRECTIONS
            .iter()
            .filter(|&&(dx, dy)| self.index(x + dx, y + dy).is_some())
            .map(|&(dx, dy)| MovementConnection {
                kind: MovementType::Standard,
                start: (x, y),
                dest: (x + dx, y + dy),
                distance: 1,
            })
            .collect();
        if room.get(x, y).map(|t| t.terrain) != Some(Terrain::ShortcutEntrance) {
            return paths;
        }
        let map = room.shortcut_map();
        let Some(here) = map.data_at((x, y)) else {
            return paths;
        };
        match here.kind {
            ShortcutType::Normal => paths.push(MovementConnection {
                kind: MovementType::ShortCut,
                start: (x, y),
                dest: here.dest,
                distance: i32::try_from(here.length).unwrap_or(i32::MAX),
            }),
            ShortcutType::NpcTransportation => {
                for other in map.shortcuts() {
                    if other.kind != ShortcutType::NpcTransportation || other.start == (x, y) {
                        continue;
                    }
                    let (dx, dy) = ((other.start.0 - x) as f32, (other.start.1 - y) as f32);
                    let length = i32::try_from(here.length + other.length).unwrap_or(i32::MAX);
                    paths.push(MovementConnection {
                        kind: MovementType::NpcTransportation,
                        start: (x, y),
                        dest: other.start,
                        distance: (dx * dx + dy * dy).sqrt() as i32 + length,
                    });
                }
            }
            _ => {}
        }
        paths
    }

    /// `IsConnectionAllowedForCreature(connection, Fly)`: both ends accessible (the
    /// types listed are all allowed).
    #[must_use]
    pub fn fly_connection_allowed(&self, c: &MovementConnection) -> bool {
        let fly = CreatureTemplate::of(CreatureType::Fly, rw_assets::ActiveMods::NONE);
        self.tile_accessible_to_creature(c.start.0, c.start.1, &fly)
            && self.tile_accessible_to_creature(c.dest.0, c.dest.1, &fly)
    }
}
