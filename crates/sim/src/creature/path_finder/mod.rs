//! `PathFinder` (`PathFinder.cs`): a realized creature's search over its room's tiles and
//! the world's nodes, back from the destination toward the creature, a few cells a
//! frame under the game's `PathfinderResourceDivider`. The cells' reachability comes
//! from the [`mapper`] (`AccessibilityMapper`, shared between creatures with the same
//! pre-baked pathing); the breed-specific parts (`LizardPather`'s heuristic and
//! `FollowPath`, `LizardAI.TravelPreference`) are [`PatherKind`]'s. Every `PathingCell`
//! makes two draws when it is made. Behaviour: `rainworld-re/notes/creature-model.md`
//! section 14.

mod lizard;
pub(crate) mod mapper;

use std::sync::Arc;

use rw_assets::{NodeType, ShortcutType};
use rw_random::UnityRandom;

use super::{CreatureId, CreatureTemplate, WorldCoordinate};
use crate::world::{
    AiMap, Legality, MovementConnection, MovementType, PathCost, QuickPathFinder, World,
};

pub use lizard::PatherKind;
pub use mapper::{AccessibilityMapper, PathFinders, PathfinderResourceDivider};

/// `PathFinder.PathingCell`.
#[derive(Debug, Clone, PartialEq)]
pub struct PathingCell {
    pub generation: i32,
    pub heuristic: PathCost,
    pub cost_to_goal: PathCost,
    pub in_check_next_list: bool,
    pub reachable: bool,
    pub possible_to_get_back_from: bool,
    pub coord: WorldCoordinate,
}

impl PathingCell {
    /// The constructor: both costs start at `123456 * value` (two draws).
    fn new(coord: WorldCoordinate, rng: &mut UnityRandom) -> Self {
        let heuristic = PathCost::new(123_456.0 * rng.value(), Legality::Allowed);
        let cost_to_goal = PathCost::new(123_456.0 * rng.value(), Legality::Allowed);
        Self {
            generation: -1,
            heuristic,
            cost_to_goal,
            in_check_next_list: false,
            reachable: false,
            possible_to_get_back_from: false,
            coord,
        }
    }
}

/// Which cell a coordinate names (the game's object references).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellRef {
    Fallback,
    /// Local room index and node.
    World(usize, usize),
    /// Index into the current room's cells.
    Room(usize),
}

/// `WorldAITiles`: a node's off-screen connections.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeTile {
    pub outgoing: Vec<MovementConnection>,
    pub incoming: Vec<MovementConnection>,
}

/// `IntRect` (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoveredArea {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl CoveredArea {
    /// `Custom.InsideRect`.
    #[must_use]
    pub fn contains(&self, t: (i32, i32)) -> bool {
        t.0 >= self.left && t.0 <= self.right && t.1 >= self.bottom && t.1 <= self.top
    }

    #[must_use]
    pub fn width(&self) -> i32 {
        self.right - self.left + 1
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.top - self.bottom + 1
    }
}

/// `InitiatePath`'s entrance and quick path.
pub(crate) type InitiatedPath = (WorldCoordinate, Option<Vec<(i32, i32)>>);

/// What the path finder reads of its creature.
#[derive(Clone, Copy)]
pub struct PathEnv<'a> {
    pub world: &'a World,
    /// `creature.pos`.
    pub pos: WorldCoordinate,
}

/// `PathFinder` with its `LizardPather` (or other breed) parts.
#[derive(Debug, Clone, PartialEq)]
pub struct PathFinder {
    pub creature: CreatureId,
    pub template: CreatureTemplate,
    pub kind: PatherKind,
    pub destination: WorldCoordinate,
    pub next_destination: Option<WorldCoordinate>,
    pub currently_following_destination: WorldCoordinate,
    pub path_generation: i32,
    pub creature_following_generation: i32,
    check_next: Vec<CellRef>,
    pub covered_area: CoveredArea,
    pub(crate) acceptable_path_age: i32,
    minimum_possible_path_cost: PathCost,
    fallback: PathingCell,
    pub time_until_path_certain: i32,
    time_in_generation: i32,
    last_following_generation: i32,
    pub re_assign_destination_once_accessibility_mapping_is_done: bool,
    pub forbidden_node: Option<WorldCoordinate>,
    world_cells: Vec<Vec<PathingCell>>,
    world_tiles: Vec<Vec<NodeTile>>,
    room_cells: Vec<PathingCell>,
    /// `realizedRoom` (global index) and its AI map as the last `Reset` saw them.
    pub realized_room: Option<i32>,
    map: Option<Arc<AiMap>>,
    /// `room`.
    pub room: i32,
    first_room_index: i32,
    pub accessibility_center: WorldCoordinate,
    /// `accessibilityMapper`: an index into [`PathFinders`]' mappers.
    pub mapper: Option<usize>,
    pub steps_per_frame: i32,
    pub accessibility_steps_per_frame: i32,
    pub walk_past_point_of_no_return: bool,
    pub forbidden_entrance: WorldCoordinate,
    pub forbidden_entrance_counter: i32,
    pub looking_for_impossible_path: bool,
}

impl PathFinder {
    /// The constructor: the destination from the abstract AI (its position when the
    /// destination has no tile and the position a node), the fallback cell, then
    /// `SetUpWorld`'s cells and connections for every node of the world.
    pub fn new(
        world: &World,
        creature: CreatureId,
        kind: PatherKind,
        rng: &mut UnityRandom,
    ) -> Self {
        let c = world.creature(creature);
        let mut destination = c.ai.as_ref().map_or(c.pos, |a| a.destination);
        if !destination.tile_defined() && c.pos.node_defined() {
            destination = c.pos;
        }
        let fallback_coord = WorldCoordinate::new(-1, -1, -1, -1);
        let mut fallback = PathingCell::new(fallback_coord, rng);
        fallback.possible_to_get_back_from = false;
        fallback.reachable = false;
        let mut pf = Self {
            creature,
            template: c.template,
            kind,
            destination,
            next_destination: Some(destination),
            currently_following_destination: destination,
            path_generation: 0,
            creature_following_generation: -2,
            check_next: Vec::new(),
            covered_area: CoveredArea::default(),
            acceptable_path_age: 5,
            minimum_possible_path_cost: PathCost::new(0.0, Legality::Allowed),
            fallback,
            time_until_path_certain: -1,
            time_in_generation: 0,
            last_following_generation: 0,
            re_assign_destination_once_accessibility_mapping_is_done: false,
            forbidden_node: None,
            world_cells: Vec::new(),
            world_tiles: Vec::new(),
            room_cells: Vec::new(),
            realized_room: None,
            map: None,
            room: 0,
            first_room_index: world.first_room_index(),
            accessibility_center: WorldCoordinate::new(-1, -1, -1, -1),
            mapper: None,
            steps_per_frame: 10,
            accessibility_steps_per_frame: 10,
            walk_past_point_of_no_return: false,
            forbidden_entrance: WorldCoordinate::new(-1, -1, -1, -1),
            forbidden_entrance_counter: 0,
            looking_for_impossible_path: false,
        };
        pf.set_up_world(world, rng);
        pf.check_next.clear();
        pf.path_generation = 1;
        pf
    }

    /// `SetUpWorld`: a cell and an off-screen tile per node of every room, the nodes'
    /// shortcut and off-screen connections (`ConnectAITile`, not in rooms disabled on
    /// the map), then the highways between the border nodes the template may use.
    fn set_up_world(&mut self, world: &World, rng: &mut UnityRandom) {
        let first = self.first_room_index;
        let rooms = world.number_of_rooms();
        for i in 0..rooms {
            let index = first + i32::try_from(i).unwrap_or(0);
            let nodes = world.abstract_room(index).map_or(0, |r| r.nodes.len());
            let cells = (0..nodes)
                .map(|j| {
                    let node = i32::try_from(j).unwrap_or(-1);
                    PathingCell::new(WorldCoordinate::new(index, -1, -1, node), rng)
                })
                .collect();
            self.world_cells.push(cells);
            self.world_tiles.push(vec![NodeTile::default(); nodes]);
        }
        let highways = [
            (MovementType::SideHighway, NodeType::SideExit),
            (MovementType::SkyHighway, NodeType::SkyExit),
            (MovementType::SeaHighway, NodeType::SeaExit),
            (
                MovementType::RegionTransportation,
                NodeType::RegionTransportation,
            ),
        ];
        let mut lists: [Vec<WorldCoordinate>; 4] = Default::default();
        for i in 0..rooms {
            let index = first + i32::try_from(i).unwrap_or(0);
            let Some(data) = world.abstract_room(index) else {
                continue;
            };
            if world.data().disabled_map_rooms.contains(&data.name) {
                continue;
            }
            for (l, node) in data.nodes.iter().enumerate() {
                let node_index = i32::try_from(l).unwrap_or(-1);
                self.connect_ai_tile(world, index, node_index);
                for (k, &(movement, kind)) in highways.iter().enumerate() {
                    if self.template.connection_resistance(movement).allowed() && node.kind == kind
                    {
                        lists[k].push(WorldCoordinate::new(index, -1, -1, node_index));
                    }
                }
            }
        }
        for (k, &(movement, kind)) in highways.iter().enumerate() {
            if !self.template.connection_resistance(movement).allowed() {
                continue;
            }
            let list = &lists[k];
            for m in 0..list.len() {
                for n in 0..list.len() {
                    if m == n {
                        continue;
                    }
                    let out = MovementConnection::new(
                        movement,
                        list[m],
                        list[n],
                        world.highway_distance(kind, list[m], list[n]),
                    );
                    let inc = MovementConnection::new(
                        movement,
                        list[n],
                        list[m],
                        world.highway_distance(kind, list[n], list[m]),
                    );
                    if let Some(t) = self.node_tile_mut(list[m]) {
                        t.outgoing.push(out);
                        t.incoming.push(inc);
                    }
                }
            }
        }
    }

    /// `ConnectAITile`: an exit's shortcut to the room behind, then an off-screen move
    /// (or an unallowed one) to every other node of the room; each mirrored as the
    /// destination's incoming connection.
    fn connect_ai_tile(&mut self, world: &World, room: i32, node: i32) {
        let Some(data) = world.abstract_room(room) else {
            return;
        };
        let start = WorldCoordinate::new(room, -1, -1, node);
        let n = usize::try_from(node).unwrap_or(0);
        let mut out = Vec::new();
        if data.nodes[n].kind == NodeType::Exit
            && !world.single_room()
            && n < data.connections.len()
        {
            let other = data.connections[n];
            if other > -1 {
                let back = world
                    .abstract_room(other)
                    .map_or(-1, |r| r.exit_index(room));
                out.push(MovementConnection::new(
                    MovementType::ShortCut,
                    start,
                    WorldCoordinate::new(other, -1, -1, back),
                    world.data().total_shortcut_length_between(room, other),
                ));
            }
        }
        let index = self.template.pre_baked_pathing_index();
        for i in 0..data.nodes.len() {
            if i == n {
                continue;
            }
            let other = i32::try_from(i).unwrap_or(-1);
            let cost = data.nodes[n].connection_cost(other, index);
            let dest = WorldCoordinate::new(room, -1, -1, other);
            out.push(if cost > -1 {
                MovementConnection::new(MovementType::OffScreenMovement, start, dest, cost)
            } else {
                MovementConnection::new(MovementType::OffScreenUnallowed, start, dest, 1)
            });
        }
        for c in &out {
            if let Some(t) = self.node_tile_mut(c.dest) {
                t.incoming.push(*c);
            }
        }
        if let Some(t) = self.node_tile_mut(start) {
            t.outgoing.extend(out);
        }
    }

    fn node_tile_mut(&mut self, coord: WorldCoordinate) -> Option<&mut NodeTile> {
        let (r, n) = self.world_index(coord)?;
        self.world_tiles.get_mut(r)?.get_mut(n)
    }

    /// The local room and node of a node coordinate inside the world.
    fn world_index(&self, coord: WorldCoordinate) -> Option<(usize, usize)> {
        if !coord.node_defined() {
            return None;
        }
        let r = usize::try_from(coord.room - self.first_room_index).ok()?;
        let n = usize::try_from(coord.node).ok()?;
        (r < self.world_cells.len() && n < self.world_cells[r].len()).then_some((r, n))
    }

    /// The realized room's AI map as the last `Reset` saw it.
    #[must_use]
    pub fn map(&self) -> Option<&Arc<AiMap>> {
        self.map.as_ref()
    }

    /// `DoneMappingAccessibility`.
    #[must_use]
    pub fn done_mapping_accessibility(&self) -> bool {
        self.mapper.is_none()
    }

    /// `GetDestination`.
    #[must_use]
    pub fn get_destination(&self) -> WorldCoordinate {
        self.next_destination.unwrap_or(self.destination)
    }

    /// `GetEffectualDestination`.
    #[must_use]
    pub fn get_effectual_destination(&self) -> WorldCoordinate {
        self.currently_following_destination
    }

    /// `InThisRealizedRoom`.
    #[must_use]
    pub fn in_this_realized_room(&self, coord: WorldCoordinate) -> bool {
        coord.room == self.room && coord.tile_defined()
    }

    /// `DestInRoom`.
    #[must_use]
    pub fn dest_in_room(&self) -> bool {
        self.in_this_realized_room(self.get_destination())
    }

    /// `PathingCellAtWorldCoordinate`.
    #[must_use]
    pub fn cell_ref(&self, coord: WorldCoordinate) -> CellRef {
        if self.in_this_realized_room(coord) {
            if self.covered_area.contains(coord.tile) {
                let x = coord.tile.0 - self.covered_area.left;
                let y = coord.tile.1 - self.covered_area.bottom;
                let i = x * self.covered_area.height() + y;
                return CellRef::Room(usize::try_from(i).unwrap_or(0));
            }
        } else if let Some((r, n)) = self.world_index(coord) {
            return CellRef::World(r, n);
        }
        CellRef::Fallback
    }

    #[must_use]
    pub fn cell(&self, r: CellRef) -> &PathingCell {
        match r {
            CellRef::Fallback => &self.fallback,
            CellRef::World(a, b) => &self.world_cells[a][b],
            CellRef::Room(i) => &self.room_cells[i],
        }
    }

    pub fn cell_mut(&mut self, r: CellRef) -> &mut PathingCell {
        match r {
            CellRef::Fallback => &mut self.fallback,
            CellRef::World(a, b) => &mut self.world_cells[a][b],
            CellRef::Room(i) => &mut self.room_cells[i],
        }
    }

    #[must_use]
    pub fn cell_at(&self, coord: WorldCoordinate) -> &PathingCell {
        self.cell(self.cell_ref(coord))
    }

    /// `CoordinateReachable`.
    #[must_use]
    pub fn coordinate_reachable(&self, coord: WorldCoordinate) -> bool {
        self.cell_at(coord).reachable
    }

    /// `CoordinatePossibleToGetBackFrom`.
    #[must_use]
    pub fn coordinate_possible_to_get_back_from(&self, coord: WorldCoordinate) -> bool {
        self.cell_at(coord).possible_to_get_back_from
    }

    /// `CoordinateReachableAndGetbackable`.
    #[must_use]
    pub fn coordinate_reachable_and_getbackable(&self, coord: WorldCoordinate) -> bool {
        let c = self.cell_at(coord);
        c.reachable && c.possible_to_get_back_from
    }

    /// `CoordinateViable`.
    #[must_use]
    pub fn coordinate_viable(&self, coord: WorldCoordinate) -> bool {
        if self.walk_past_point_of_no_return {
            self.coordinate_reachable(coord)
        } else {
            self.coordinate_reachable_and_getbackable(coord)
        }
    }

    /// `CoordinateAtCurrentPathingGeneration`.
    #[must_use]
    pub fn coordinate_at_current_pathing_generation(&self, coord: WorldCoordinate) -> bool {
        self.cell_at(coord).generation == self.path_generation
    }

    /// The world cells and the current room's cells, for the mapper's merges.
    pub(crate) fn cells_mut(&mut self) -> (&mut Vec<Vec<PathingCell>>, &mut Vec<PathingCell>) {
        (&mut self.world_cells, &mut self.room_cells)
    }

    pub(crate) fn cells(&self) -> (&Vec<Vec<PathingCell>>, &Vec<PathingCell>) {
        (&self.world_cells, &self.room_cells)
    }

    /// Every cell's reachability flags cleared (the mapper's first pass).
    pub(crate) fn clear_reachability(&mut self, room_too: bool) {
        for c in self.world_cells.iter_mut().flatten() {
            c.reachable = false;
            c.possible_to_get_back_from = false;
        }
        if room_too {
            for c in &mut self.room_cells {
                c.reachable = false;
                c.possible_to_get_back_from = false;
            }
        }
    }

    /// The paths of `AITileAtWorldCoordinate(coord)`: the map's tile in this room, a
    /// node's off-screen tile, else a Solid tile without connections.
    fn paths(&self, coord: WorldCoordinate, outgoing: bool) -> &[MovementConnection] {
        if self.in_this_realized_room(coord)
            && let Some(map) = &self.map
        {
            let t = map.tile(coord.tile.0, coord.tile.1);
            return if outgoing {
                &t.outgoing_paths
            } else {
                &t.incoming_paths
            };
        }
        if let Some((r, n)) = self.world_index(coord) {
            let t = &self.world_tiles[r][n];
            return if outgoing { &t.outgoing } else { &t.incoming };
        }
        &[]
    }

    /// `AITileAtWorldCoordinate(coord).acc`.
    fn tile_acc(&self, coord: WorldCoordinate) -> crate::world::Accessibility {
        use crate::world::Accessibility;
        if self.in_this_realized_room(coord)
            && let Some(map) = &self.map
        {
            return map.tile(coord.tile.0, coord.tile.1).acc;
        }
        if self.world_index(coord).is_some() {
            return Accessibility::OffScreen;
        }
        Accessibility::Solid
    }

    /// `ConnectionAtCoordinate`: the `index`th connection out of (or into) `coord`, `None`
    /// past the last (`default(MovementConnection)`).
    #[must_use]
    pub fn connection_at(
        &self,
        world: &World,
        outgoing: bool,
        coord: WorldCoordinate,
        index: usize,
    ) -> Option<MovementConnection> {
        let list = self.paths(coord, outgoing);
        if coord.tile_defined() {
            if index < list.len() {
                return Some(list[index]);
            }
            if index == list.len() {
                return self.realized_to_world_connection(outgoing, coord);
            }
        } else if coord.node_defined() {
            if coord.room == self.room {
                return self
                    .connections_of_abstract_node_in_realized_room(world, outgoing, coord, index);
            }
            if index < list.len() {
                return Some(list[index]);
            }
        }
        None
    }

    /// Every connection out of (or into) `coord`, in order.
    #[must_use]
    pub fn connections(
        &self,
        world: &World,
        outgoing: bool,
        coord: WorldCoordinate,
    ) -> Vec<MovementConnection> {
        (0..)
            .map_while(|i| self.connection_at(world, outgoing, coord, i))
            .collect()
    }

    /// `ConnectionsOfAbstractNodeInRealizedRoom`.
    fn connections_of_abstract_node_in_realized_room(
        &self,
        world: &World,
        outgoing: bool,
        coord: WorldCoordinate,
        index: usize,
    ) -> Option<MovementConnection> {
        let map = self.map.as_ref()?;
        let data = world.abstract_room(self.room)?;
        let node = usize::try_from(coord.node).ok()?;
        let exits = map.shortcuts().nodes().len();
        let kind = data.nodes.get(node)?.kind;
        if node < exits {
            if index == 0 {
                let s = map.shortcuts().leading_to_node(node)?;
                let tile = WorldCoordinate::new(self.room, s.start.0, s.start.1, -1);
                return Some(MovementConnection::directed(
                    outgoing,
                    MovementType::ShortCut,
                    coord,
                    tile,
                    1,
                ));
            }
            if kind == NodeType::Exit {
                if index != 1 {
                    return None;
                }
                let other = *data.connections.get(node)?;
                if other == -1 {
                    return None;
                }
                let back = world
                    .abstract_room(other)
                    .map_or(-1, |r| r.exit_index(self.room));
                let there = WorldCoordinate::new(other, -1, -1, back);
                return Some(MovementConnection::directed(
                    outgoing,
                    MovementType::ShortCut,
                    coord,
                    there,
                    1,
                ));
            }
            if kind == NodeType::RegionTransportation {
                let c = *self.paths(coord, outgoing).get(index - 1)?;
                if c.kind != MovementType::RegionTransportation {
                    return Some(MovementConnection::new(
                        MovementType::OffScreenUnallowed,
                        c.start,
                        c.dest,
                        1,
                    ));
                }
                return Some(c);
            }
            return None;
        }
        if kind.is_border_exit() {
            let exit = map.border_exits().get(node - exits)?;
            if index < exit.tiles.len() {
                let t = exit.tiles[index];
                let tile = WorldCoordinate::new(self.room, t.0, t.1, -1);
                return Some(MovementConnection::directed(
                    outgoing,
                    MovementType::OutsideRoom,
                    coord,
                    tile,
                    10,
                ));
            }
            let c = *self.paths(coord, outgoing).get(index - exit.tiles.len())?;
            if !matches!(
                c.kind,
                MovementType::SideHighway | MovementType::SkyHighway | MovementType::SeaHighway
            ) {
                return Some(MovementConnection::new(
                    MovementType::OffScreenUnallowed,
                    c.start,
                    c.dest,
                    1,
                ));
            }
            return Some(c);
        }
        None
    }

    /// `FromRealizedToWorldConnection`: a room exit, den or region transport entrance
    /// leads to its node; a border tile to its border exit for a template that maps one.
    fn realized_to_world_connection(
        &self,
        outgoing: bool,
        coord: WorldCoordinate,
    ) -> Option<MovementConnection> {
        if !self.in_this_realized_room(coord) {
            return None;
        }
        let map = self.map.as_ref()?;
        if map.is_shortcut_entrance(coord.tile) {
            let data = map.shortcuts().data_at(coord.tile);
            let kind = data.map_or(ShortcutType::DeadEnd, |d| d.kind);
            if matches!(
                kind,
                ShortcutType::RoomExit
                    | ShortcutType::CreatureHole
                    | ShortcutType::RegionTransportation
            ) {
                let dest = data.map_or((0, 0), |d| d.dest);
                let node = map
                    .shortcuts()
                    .nodes()
                    .iter()
                    .position(|&n| n == dest)
                    .map_or(-1, |n| i32::try_from(n).unwrap_or(-1));
                return Some(MovementConnection::directed(
                    outgoing,
                    MovementType::ShortCut,
                    coord,
                    WorldCoordinate::new(coord.room, -1, -1, node),
                    1,
                ));
            }
        } else if self.template.use_any_room_border_exit()
            && (coord.tile.0 == 0
                || coord.tile.1 == 0
                || coord.tile.0 == map.width() - 1
                || coord.tile.1 == map.height() - 1)
        {
            let exits = map.shortcuts().nodes().len();
            for (i, exit) in map.border_exits().iter().enumerate() {
                if !self.template.maps_node(exit.kind) {
                    continue;
                }
                if exit.tiles.contains(&coord.tile) {
                    let node = i32::try_from(exits + i).unwrap_or(-1);
                    return Some(MovementConnection::directed(
                        outgoing,
                        MovementType::OutsideRoom,
                        coord,
                        WorldCoordinate::new(coord.room, -1, -1, node),
                        10,
                    ));
                }
            }
        }
        None
    }

    /// `CoordinateCost`: Unallowed on the forbidden node, the map's tile cost in this
    /// room, else the template's off-screen resistance.
    #[must_use]
    pub fn coordinate_cost(&self, coord: WorldCoordinate) -> PathCost {
        if let Some(f) = self.forbidden_node
            && coord.room == f.room
            && coord.node == f.node
        {
            return PathCost::new(0.0, Legality::Unallowed);
        }
        if self.in_this_realized_room(coord)
            && let Some(map) = &self.map
        {
            return map.tile_cost_for_creature(coord.tile, &self.template);
        }
        self.template.accessibility_resistance(self.tile_acc(coord))
    }

    /// `CheckConnectionCost`: illegal unless the map allows the connection, else the
    /// type's resistance times the distance plus the goal's cost (and the start's
    /// legality); between rooms the `BetweenRooms` resistance, an NPC transport the
    /// travel aversion (100), a shortcut the aversion (0); in the room the AI's
    /// `TravelPreference`.
    #[must_use]
    pub fn check_connection_cost(
        &self,
        start: WorldCoordinate,
        goal: WorldCoordinate,
        connection: &MovementConnection,
    ) -> PathCost {
        let mut cost = PathCost::new(
            100.0 * connection.distance as f32,
            Legality::IllegalConnection,
        );
        if self
            .map
            .as_ref()
            .is_some_and(|m| !m.is_connection_allowed_for_creature(connection, &self.template))
        {
            cost = cost + PathCost::new(0.0, Legality::IllegalConnection);
        } else {
            let mut r = self.template.connection_resistance(connection.kind);
            if r.considerable() {
                let goal_cost = self.coordinate_cost(goal);
                let start_cost = self.coordinate_cost(start);
                if goal_cost.considerable() && start_cost.considerable() {
                    r.resistance *= connection.distance as f32;
                    cost = r + goal_cost + PathCost::new(0.0, start_cost.legality);
                }
            }
        }
        if start.room != goal.room {
            cost = cost
                + self
                    .template
                    .connection_resistance(MovementType::BetweenRooms);
        } else if connection.kind == MovementType::NpcTransportation {
            cost = cost + PathCost::new(100.0, Legality::Allowed);
        } else if connection.kind == MovementType::ShortCut {
            cost = cost + PathCost::new(0.0, Legality::Allowed);
        }
        if cost.considerable()
            && self.in_this_realized_room(connection.dest)
            && self.in_this_realized_room(connection.start)
            && let Some(map) = &self.map
        {
            cost = self.kind.travel_preference(map, connection, cost);
        }
        cost
    }

    /// The `Update` before its steps: the generation clock and the destination's cost
    /// to goal zeroed. Returns whether the accessibility mapper runs this frame.
    pub(crate) fn update_start(&mut self) {
        self.time_in_generation += 1;
        if self.last_following_generation != self.creature_following_generation {
            self.time_in_generation = 0;
            self.last_following_generation = self.creature_following_generation;
        }
        let d = self.cell_ref(self.destination);
        self.cell_mut(d).cost_to_goal.resistance = 0.0;
    }

    /// One of `Update`'s path steps: a cell off the check list has its neighbours
    /// checked (and the path toward the destination becomes the followed one once the
    /// creature's cell or the following generation is reached), else a waiting next
    /// destination, else a stale generation restarts.
    pub(crate) fn path_step(&mut self, env: &PathEnv<'_>, rng: &mut UnityRandom) {
        if !self.check_next.is_empty() {
            let r = self.check_next.remove(0);
            self.cell_mut(r).in_check_next_list = false;
            self.check_neighbours(env, r);
            let cost = self.cell(r).cost_to_goal;
            if self.minimum_possible_path_cost.less_than(cost) {
                self.minimum_possible_path_cost = cost;
            }
            if self.creature_following_generation == self.path_generation {
                if self.time_in_generation > self.time_until_path_certain {
                    self.check_next.clear();
                }
                self.currently_following_destination = self.destination;
            } else if self.cell(r).coord.room == env.pos.room
                && self.cell(r).coord.tile == env.pos.tile
            {
                self.currently_following_destination = self.destination;
            }
        } else if let Some(next) = self.next_destination {
            self.assign_new_destination(env, next, rng);
            self.next_destination = None;
        } else if self.creature_following_generation < self.path_generation - 1
            && !self.re_assign_destination_once_accessibility_mapping_is_done
        {
            let d = self.destination;
            self.assign_new_destination(env, d, rng);
        }
    }

    /// `CheckNeighbours`: each connection into the cell, from a reachable start (or any
    /// considerable one while looking for an impossible path), offers its start the
    /// cost through this cell; a start already in this generation keeps the cheaper.
    fn check_neighbours(&mut self, env: &PathEnv<'_>, now: CellRef) {
        let now_coord = self.cell(now).coord;
        let now_cost = self.cell(now).cost_to_goal;
        let mut i = 0;
        while let Some(c) = self.connection_at(env.world, false, now_coord, i) {
            i += 1;
            let r = self.cell_ref(c.start);
            let start = self.cell(r).coord;
            if !start.tile_defined() && !start.node_defined() {
                continue;
            }
            let cost = self.check_connection_cost(start, now_coord, &c);
            if (!cost.allowed() || !self.cell(r).reachable)
                && (!self.looking_for_impossible_path || !cost.considerable())
            {
                continue;
            }
            let to_goal = now_cost + cost;
            let h = self.heuristic_for_cell(env, r, to_goal);
            let generation = self.path_generation;
            let cell = self.cell(r);
            if cell.generation == generation {
                if cell.in_check_next_list {
                    if cell.heuristic.greater_than(h) {
                        if let Some(p) = self.check_next.iter().position(|&x| x == r) {
                            self.check_next.remove(p);
                        }
                        self.cell_mut(r).heuristic = h;
                        self.add_to_check_next_list(r);
                    }
                    if self.cell(r).cost_to_goal.greater_than(to_goal) {
                        self.cell_mut(r).cost_to_goal = to_goal;
                    }
                }
            } else if cell.generation < generation {
                let cell = self.cell_mut(r);
                cell.cost_to_goal = to_goal;
                cell.heuristic = h;
                cell.in_check_next_list = true;
                cell.generation = generation;
                self.add_to_check_next_list(r);
            }
        }
    }

    /// `HeuristicForCell` (the breed's).
    fn heuristic_for_cell(&self, env: &PathEnv<'_>, r: CellRef, to_goal: PathCost) -> PathCost {
        self.kind.heuristic_for_cell(self, env, r, to_goal)
    }

    /// `AddToCheckNextList`: before the first cell with a heuristic not below it; never
    /// the fallback cell.
    fn add_to_check_next_list(&mut self, r: CellRef) {
        if r == CellRef::Fallback {
            return;
        }
        let h = self.cell(r).heuristic;
        let at = self
            .check_next
            .iter()
            .position(|&x| h.less_or_equal(self.cell(x).heuristic));
        match at {
            Some(i) => self.check_next.insert(i, r),
            None => self.check_next.push(r),
        }
    }

    /// The check list's length.
    #[must_use]
    pub fn check_next_len(&self) -> usize {
        self.check_next.len()
    }

    /// `AssignNewDestination`: a new generation from the destination (its node defined
    /// when it is in another room without one).
    pub(crate) fn assign_new_destination(
        &mut self,
        env: &PathEnv<'_>,
        dest: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        self.destination = dest;
        self.path_generation += 1;
        self.forbidden_node = None;
        self.check_next.clear();
        if self.destination.room != self.room && !self.destination.node_defined() {
            self.destination = env.world.define_node(self.destination, &self.template, rng);
        }
        self.minimum_possible_path_cost = self.coordinate_cost(self.destination);
        let r = self.cell_ref(self.destination);
        self.looking_for_impossible_path = !self.cell(r).reachable;
        self.add_to_check_next_list(r);
        let start = PathCost::new(0.0, self.minimum_possible_path_cost.legality);
        let cell = self.cell_mut(r);
        cell.heuristic = start;
        cell.cost_to_goal = start;
    }

    /// `LeavingRoom`: an undefined node of the (next) destination is defined.
    pub(crate) fn leaving_room(&mut self, env: &PathEnv<'_>, rng: &mut UnityRandom) {
        if let Some(next) = self.next_destination
            && !next.node_defined()
        {
            self.destination = env.world.define_node(next, &self.template, rng);
        } else if !self.destination.node_defined() {
            self.destination = env.world.define_node(self.destination, &self.template, rng);
        }
        self.next_destination = None;
    }

    /// `Reset`'s cells: the covered area (the bounding box of the template's accessible
    /// tiles, `(0, 0, 1, 1)` without any) and a new cell for each of its tiles, x-major
    /// (two draws each).
    pub(crate) fn reset_cells(
        &mut self,
        room: i32,
        map: Arc<AiMap>,
        pos: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        self.currently_following_destination = pos;
        self.realized_room = Some(room);
        self.room = room;
        self.check_next.clear();
        let (w, h) = (map.width(), map.height());
        let (mut left, mut bottom, mut right, mut top) = (w, h, 0, 0);
        let mut any = false;
        for x in 0..w {
            for y in 0..h {
                if map.tile_accessible_to_creature((x, y), &self.template) {
                    any = true;
                    left = left.min(x);
                    bottom = bottom.min(y);
                    right = right.max(x);
                    top = top.max(y);
                }
            }
        }
        if !any {
            (left, bottom, right, top) = (0, 0, 1, 1);
        }
        self.covered_area = CoveredArea {
            left,
            bottom,
            right,
            top,
        };
        let mut cells = Vec::new();
        for k in 0..self.covered_area.width() {
            for l in 0..self.covered_area.height() {
                let c = WorldCoordinate::new(room, k + left, l + bottom, -1);
                cells.push(PathingCell::new(c, rng));
            }
        }
        self.room_cells = cells;
        self.map = Some(map);
    }

    /// `InitiatePath`'s search: the forbidden entrance and node are the door it came in
    /// by; the entrance tile (a border exit's nearest accessible border tile) and a quick
    /// path from it to the destination in this room, or the exit it will leave by.
    /// Returns the entrance and the path's tiles; `None` for a node out of range.
    pub(crate) fn initiate_path(
        &mut self,
        env: &PathEnv<'_>,
        door: i32,
        rng: &mut UnityRandom,
    ) -> Option<InitiatedPath> {
        let data = env.world.abstract_room(self.room)?;
        if door < 0 || door >= i32::try_from(data.nodes.len()).unwrap_or(0) {
            return None;
        }
        let map = Arc::clone(self.map.as_ref()?);
        self.forbidden_entrance = WorldCoordinate::new(self.room, -1, -1, door);
        self.forbidden_entrance_counter = 40;
        self.forbidden_node = Some(WorldCoordinate::new(self.room, -1, -1, door));
        let tile = env.world.local_coordinate_of_node(self.room, door, rng);
        let mut entrance = WorldCoordinate::new(self.room, tile.0, tile.1, -1);
        let node = &data.nodes[usize::try_from(door).unwrap_or(0)];
        if node.kind.is_border_exit()
            && !map.tile_accessible_to_creature(entrance.tile, &self.template)
        {
            let exits = map.shortcuts().nodes().len();
            let index = usize::try_from(door).unwrap_or(0).saturating_sub(exits);
            if let Some(exit) = map.border_exits().get(index) {
                let mut best = f32::MAX;
                let mut pick = entrance.tile;
                for &t in &exit.tiles {
                    let (dx, dy) = (
                        (t.0 - entrance.tile.0) as f32,
                        (t.1 - entrance.tile.1) as f32,
                    );
                    let d = (dx * dx + dy * dy).sqrt();
                    if map.tile_accessible_to_creature(t, &self.template) && d < best {
                        best = d;
                        pick = t;
                    }
                }
                entrance.tile = pick;
            }
        }
        let goal = if self.in_this_realized_room(self.destination) {
            self.destination.tile
        } else {
            let mut exit = self.destination_exit(env, self.room, door);
            if exit < 0 {
                exit = door;
            }
            env.world.local_coordinate_of_node(self.room, exit, rng)
        };
        let path = QuickPathFinder::new(entrance.tile, goal, &map, &self.template)
            .run()
            .map(|p| p.tiles);
        self.re_assign_destination_once_accessibility_mapping_is_done = true;
        Some((entrance, path))
    }

    /// `DestinationExit`: of the room's other nodes connected to the door, the reachable
    /// one of the newest generation and lowest cost; else the abstract path's node in
    /// this room; else the abstract destination's node or an exit toward the path.
    #[must_use]
    pub fn destination_exit(&self, env: &PathEnv<'_>, room: i32, door: i32) -> i32 {
        let Some(data) = env.world.abstract_room(room) else {
            return -1;
        };
        let index = self.template.pre_baked_pathing_index();
        let mut generation = -1;
        let mut best = PathCost::new(0.0, Legality::Unallowed);
        let mut pick = -1;
        let door_node = usize::try_from(door).ok().and_then(|d| data.nodes.get(d));
        for i in 0..data.nodes.len() {
            let n = i32::try_from(i).unwrap_or(-1);
            if n == door || door_node.is_none_or(|d| d.connection_cost(n, index) <= -1) {
                continue;
            }
            let cell = self.cell_at(WorldCoordinate::new(room, -1, -1, n));
            if !cell.reachable {
                continue;
            }
            if cell.generation > generation
                || (cell.generation == generation && cell.cost_to_goal.less_than(best))
            {
                generation = cell.generation;
                best = cell.cost_to_goal;
                pick = n;
            }
        }
        let ai = env.world.creature(self.creature).ai.as_ref();
        if pick == -1
            && let Some(ai) = ai
        {
            for p in &ai.path {
                if p.room == self.room {
                    pick = p.node;
                }
            }
        }
        if pick == -1
            && let Some(ai) = ai
        {
            for j in 0..data.nodes.len() {
                let n = i32::try_from(j).unwrap_or(-1);
                let toward_path = data.connections.get(j).is_some_and(|&other| {
                    other > -1
                        && env.world.abstract_room(other).is_some_and(|o| {
                            ai.path.contains(&WorldCoordinate::new(
                                other,
                                -1,
                                -1,
                                o.exit_index(room),
                            ))
                        })
                });
                if (n == ai.destination.node && ai.destination.room == room) || toward_path {
                    pick = n;
                    break;
                }
            }
        }
        pick
    }

    /// `SetDestination`'s part after the node is defined: a destination in this room
    /// is assigned now when it is reachable and the old one is not, queued as the next
    /// destination when the old one connects to it within 100 generations, else
    /// assigned; one in another room with a node is assigned.
    pub(crate) fn set_destination(
        &mut self,
        env: &PathEnv<'_>,
        new: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        if self.destination == new || Some(self.destination) == self.next_destination {
            return;
        }
        let mut new = new;
        if self.in_this_realized_room(new) {
            new.tile.0 = new
                .tile
                .0
                .clamp(self.covered_area.left, self.covered_area.right);
            new.tile.1 = new
                .tile
                .1
                .clamp(self.covered_area.bottom, self.covered_area.top);
            new = self.find_reachable_neighbour_if_possible(new);
        }
        if new.room != self.room && !new.node_defined() {
            let room_ready = env
                .world
                .realized(self.room)
                .is_some_and(crate::world::RealizedRoom::ready_for_ai);
            let keep = !room_ready
                && self.destination.room == new.room
                && self.destination.node_defined()
                && env
                    .world
                    .node_kind(self.destination)
                    .is_some_and(|k| self.template.maps_node(k));
            if keep {
                new.node = self.destination.node;
            } else {
                new = env.world.define_node(new, &self.template, rng);
            }
        }
        if self.destination == new || Some(self.destination) == self.next_destination {
            return;
        }
        if self.in_this_realized_room(new) {
            if !self.coordinate_cost(new).considerable() {
                return;
            }
            let map = self.map.clone();
            if self.cell_at(new).reachable && !self.cell_at(self.destination).reachable {
                self.assign_new_destination(env, new, rng);
            } else if map.is_some_and(|m| {
                crate::world::QuickConnectivity::check(
                    &m,
                    &self.template,
                    self.destination.tile,
                    new.tile,
                    100,
                ) > -1
            }) {
                self.next_destination = Some(new);
            } else {
                self.assign_new_destination(env, new, rng);
            }
        } else if new.node_defined() {
            self.assign_new_destination(env, new, rng);
        }
    }

    /// `FindReachableNeighbourIfPossible`: the first reachable tile of the coordinate
    /// and its eight neighbours inside the covered area.
    fn find_reachable_neighbour_if_possible(&self, coord: WorldCoordinate) -> WorldCoordinate {
        for (dx, dy) in EIGHT_DIRECTIONS_AND_ZERO {
            let c =
                WorldCoordinate::new(coord.room, coord.tile.0 + dx, coord.tile.1 + dy, coord.node);
            if self.cell_at(c).reachable && self.covered_area.contains(c.tile) {
                return c;
            }
        }
        coord
    }

    /// `ForceNextDestination`.
    pub fn force_next_destination(&mut self, env: &PathEnv<'_>, rng: &mut UnityRandom) {
        if let Some(next) = self.next_destination
            && self.destination != next
        {
            self.assign_new_destination(env, next, rng);
        }
    }

    /// `CreatePathForAbstractreature`: from the creature's node, up to 100 steps along
    /// the best outgoing connection (allowed, newest generation, cheapest), ending at
    /// the destination's room and node; reversed, destination first. `None` when it
    /// loops, stalls or meets an unallowed step.
    #[must_use]
    pub fn create_path_for_abstract_creature(
        &self,
        env: &PathEnv<'_>,
        dest: WorldCoordinate,
    ) -> Option<Vec<WorldCoordinate>> {
        self.realized_room?;
        if !self.template.can_auto_abstract_path() {
            return None;
        }
        let mut at = env.pos;
        at.tile = (-1, -1);
        let mut list = vec![at];
        for _ in 0..100 {
            let mut pick: Option<MovementConnection> = None;
            let mut cost = PathCost::new(0.0, Legality::Unallowed);
            let mut generation = -self.acceptable_path_age;
            let mut legality = Legality::Unallowed;
            let start = self.cell_at(at).coord;
            for c in self.paths(at, true) {
                let goal = self.cell_at(c.dest);
                let c_cost = self.check_connection_cost(start, goal.coord, c);
                let total = goal.cost_to_goal + c_cost;
                if c_cost.legality < legality
                    || (c_cost.legality == legality
                        && (goal.generation > generation
                            || (goal.generation == generation && total.less_than(cost))))
                {
                    pick = Some(*c);
                    legality = c_cost.legality;
                    generation = goal.generation;
                    cost = total;
                }
                if c_cost.allowed() && c.dest.room == dest.room && c.dest.node == dest.node {
                    list.push(c.dest);
                    list.reverse();
                    return Some(list);
                }
            }
            let c = pick?;
            at = c.dest;
            if list.contains(&at) || !cost.allowed() {
                break;
            }
            list.push(at);
        }
        None
    }

    /// `ForbiddenEntranceDist`: the exit distance to the forbidden entrance.
    #[must_use]
    pub fn forbidden_entrance_dist(&self, world: &World, origin: WorldCoordinate) -> i32 {
        let Some(node) = self.forbidden_entrance_specific(world) else {
            return i32::MAX;
        };
        self.map.as_ref().map_or(i32::MAX, |m| {
            m.exit_distance_for_creature(origin.tile, node, &self.template)
        })
    }

    /// The forbidden entrance's creature-specific node, if the template maps it.
    fn forbidden_entrance_specific(&self, world: &World) -> Option<i32> {
        if self.forbidden_entrance.node < 0 {
            return None;
        }
        let data = world.abstract_room(self.room)?;
        let kind = data
            .nodes
            .get(usize::try_from(self.forbidden_entrance.node).ok()?)?
            .kind;
        if !self.template.maps_node(kind) {
            return None;
        }
        let n = self
            .template
            .specific_node_index(data, self.forbidden_entrance.node);
        (n >= 0 && self.map.is_some()).then_some(n)
    }

    /// `AwayFromForbiddenEntrance`: of the moves out of `origin` (no shortcuts, allowed,
    /// onto accessible tiles) the one farthest from the forbidden entrance; a move that
    /// gets farther scores a random 0-100 (+100 out of narrow spaces), a reachable one
    /// +1000.
    pub fn away_from_forbidden_entrance(
        &self,
        world: &World,
        origin: WorldCoordinate,
        rng: &mut UnityRandom,
    ) -> Option<MovementConnection> {
        let node = self.forbidden_entrance_specific(world)?;
        let map = self.map.as_ref()?;
        let here = map.exit_distance_for_creature(origin.tile, node, &self.template) as f32;
        let mut best = f32::MIN;
        let mut pick = None;
        let mut i = 0;
        while let Some(c) = self.connection_at(world, true, origin, i) {
            i += 1;
            if c.kind == MovementType::ShortCut
                || !self.template.connection_resistance(c.kind).allowed()
                || !map.tile_accessible_to_creature(c.dest_tile(), &self.template)
            {
                continue;
            }
            let mut score =
                map.exit_distance_for_creature(c.dest_tile(), node, &self.template) as f32;
            if score > 0.0 && score > here {
                let narrow = map.tile(c.dest_tile().0, c.dest_tile().1).narrow_space;
                score = rng.value() * 100.0 + if narrow { 0.0 } else { 100.0 };
            }
            if self.cell_at(c.dest).reachable {
                score += 1000.0;
            }
            if score > best {
                best = score;
                pick = Some(c);
            }
        }
        pick
    }

    /// `RayTraceInAccessibleTiles`: every tile of the line reachable and getbackable.
    #[must_use]
    pub fn ray_trace_in_accessible_tiles(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (mut x, mut y) = a;
        let mut dx = (b.0 - a.0).abs();
        let mut dy = (b.1 - a.1).abs();
        let mut n = 1 + dx + dy;
        let sx = if b.0 > a.0 { 1 } else { -1 };
        let sy = if b.1 > a.1 { 1 } else { -1 };
        let mut err = dx - dy;
        dx *= 2;
        dy *= 2;
        while n > 0 {
            if !self.coordinate_reachable_and_getbackable(WorldCoordinate::new(self.room, x, y, -1))
            {
                return false;
            }
            if err > 0 {
                x += sx;
                err -= dy;
            } else {
                y += sy;
                err += dx;
            }
            n -= 1;
        }
        true
    }
}

/// `Custom.eightDirectionsAndZero`.
const EIGHT_DIRECTIONS_AND_ZERO: [(i32, i32); 9] = [
    (0, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

#[cfg(test)]
mod tests;
