//! `AbstractSpacePathFinder` and `AbstractSpaceNodeFinder` (16334, 15982): searches over
//! the abstract rooms' nodes, on the room files' baked connectivity, the shortcuts
//! between rooms and the border highways.

use rw_assets::NodeType;
use rw_random::UnityRandom;

use super::World;
use crate::creature::{AbstractMovement, CreatureTemplate, WorldCoordinate};

/// A search node: its position, the node it was reached from and its cost.
#[derive(Debug, Clone, Copy)]
struct SearchNode {
    pos: WorldCoordinate,
    parent: Option<usize>,
    cost: i32,
}

/// The cells already checked (`alreadyChecked[room - firstRoomIndex, node]`).
struct Checked {
    first_room: i32,
    cells: Vec<Vec<bool>>,
}

impl Checked {
    fn new(world: &World) -> Self {
        let most = world
            .data
            .rooms
            .iter()
            .map(|r| r.nodes.len())
            .max()
            .unwrap_or(0);
        Self {
            first_room: world.first_room_index(),
            cells: vec![vec![false; most]; world.number_of_rooms()],
        }
    }

    fn cell(&mut self, pos: WorldCoordinate) -> Option<&mut bool> {
        let room = usize::try_from(pos.room - self.first_room).ok()?;
        let node = usize::try_from(pos.node).ok()?;
        self.cells.get_mut(room)?.get_mut(node)
    }

    /// Whether `pos` is checked; out of range (where the game throws) counts as checked.
    fn get(&mut self, pos: WorldCoordinate) -> bool {
        self.cell(pos).is_none_or(|c| *c)
    }

    fn set(&mut self, pos: WorldCoordinate) {
        if let Some(c) = self.cell(pos) {
            *c = true;
        }
    }
}

/// `AbstractSpacePathFinder.Path` (no `IOwnAnAbstractSpacePathFinder` owner: only the
/// scavengers' AI is one).
pub(crate) struct AbstractSpacePathFinder<'a> {
    pub world: &'a World,
    pub template: &'a CreatureTemplate,
}

impl AbstractSpacePathFinder<'_> {
    /// The path from `start` to `goal`, goal first and the step after `start` last;
    /// `None` where the game returns null. Draws for an undefined start or goal node and
    /// for a goal node the creature does not map.
    pub fn path(
        &self,
        mut start: WorldCoordinate,
        mut goal: WorldCoordinate,
        rng: &mut UnityRandom,
    ) -> Option<Vec<WorldCoordinate>> {
        let world = self.world;
        if start == goal {
            return Some(Vec::new());
        }
        if !world.room_in_region(start.room) || !world.room_in_region(goal.room) {
            return None;
        }
        let a = world.abstract_room(start.room)?;
        let b = world.abstract_room(goal.room)?;
        if a.nodes.is_empty() || b.nodes.is_empty() {
            return None;
        }
        let a_nodes = i32::try_from(a.nodes.len()).unwrap_or(i32::MAX);
        let b_nodes = i32::try_from(b.nodes.len()).unwrap_or(i32::MAX);
        if !start.node_defined() || start.node >= a_nodes {
            start.node = rng.range_i32(0, a_nodes);
        }
        if !goal.node_defined() || goal.node >= b_nodes {
            goal.node = rng.range_i32(0, b_nodes);
        }
        let t = self.template;
        if !t.maps_room_node(a, start.node) && !a.offscreen_den {
            log::warn!(
                "Looking for path from inside node that's inaccessible to {}",
                t.kind
            );
            return None;
        }
        if !t.maps_room_node(b, goal.node) {
            let list: Vec<i32> = (0..b_nodes).filter(|&i| t.maps_room_node(b, i)).collect();
            if list.is_empty() {
                return None;
            }
            let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
            let k = usize::try_from(rng.range_i32(0, len)).unwrap_or(0);
            goal = WorldCoordinate::new(goal.room, -1, -1, list[k]);
            if start == goal {
                return Some(Vec::new());
            }
        }
        if start.room == goal.room
            && world.connectivity_cost(start.room, start.node, goal.node, t) >= 0
        {
            return Some(vec![goal]);
        }
        if let Some(path) = self.neighbour_path(start, goal) {
            return Some(path);
        }
        self.flood(start, goal)
    }

    /// The direct path into a connected room; `None` to flood instead (the game breaks
    /// out of its loop at the first exit to the goal room that fails).
    fn neighbour_path(
        &self,
        start: WorldCoordinate,
        goal: WorldCoordinate,
    ) -> Option<Vec<WorldCoordinate>> {
        let world = self.world;
        let t = self.template;
        let a = world.abstract_room(start.room)?;
        let b = world.abstract_room(goal.room)?;
        let exit = i32::try_from(a.connections.iter().position(|&c| c == goal.room)?).ok()?;
        let entrance = b.exit_index(start.room);
        let possible = entrance != -1
            && (start.node == exit
                || world.connectivity_cost(start.room, start.node, exit, t) >= 0)
            && (entrance == goal.node
                || world.connectivity_cost(goal.room, entrance, goal.node, t) >= 0);
        if !possible {
            return None;
        }
        let mut path = Vec::new();
        if goal.node != entrance {
            path.push(WorldCoordinate::new(goal.room, -1, -1, goal.node));
        }
        path.push(WorldCoordinate::new(goal.room, -1, -1, entrance));
        if start.node != exit {
            path.push(WorldCoordinate::new(start.room, -1, -1, exit));
        }
        Some(path)
    }

    /// The flood from the goal: the cheapest queued node (first on ties) each
    /// generation, until the queue is empty; more than 500 generations gives up.
    fn flood(
        &self,
        mut start: WorldCoordinate,
        mut goal: WorldCoordinate,
    ) -> Option<Vec<WorldCoordinate>> {
        let world = self.world;
        let t = self.template;
        start.tile = (-1, -1);
        goal.tile = (-1, -1);
        let mut nodes = vec![SearchNode {
            pos: goal,
            parent: None,
            cost: 0,
        }];
        let mut queue = vec![0usize];
        let mut checked = Checked::new(world);
        let mut found = None;
        let mut generation = 0;
        while !queue.is_empty() {
            let mut best = 0;
            for (k, &n) in queue.iter().enumerate() {
                if nodes[n].cost < nodes[queue[best]].cost {
                    best = k;
                }
            }
            let current = queue.remove(best);
            let pos = nodes[current].pos;
            let room = world.abstract_room(pos.room)?;
            let mut adds: Vec<(WorldCoordinate, f32)> = Vec::new();
            let offscreen = t
                .abstract_resistance(AbstractMovement::OffScreenMovement)
                .resistance;
            for k in 0..i32::try_from(room.nodes.len()).unwrap_or(0) {
                if k == pos.node {
                    continue;
                }
                let to = WorldCoordinate::new(room.index, -1, -1, k);
                let cost = world.connectivity_cost(room.index, k, pos.node, t) as f32 * offscreen;
                if cost >= 0.0 && !checked.get(to) {
                    adds.push((to, cost));
                }
            }
            let kind = usize::try_from(pos.node)
                .ok()
                .and_then(|n| room.nodes.get(n))
                .map(|n| n.kind);
            match kind {
                Some(NodeType::Exit) if t.maps_node(NodeType::Exit) => {
                    let conn = usize::try_from(pos.node)
                        .ok()
                        .and_then(|n| room.connections.get(n))
                        .copied()
                        .unwrap_or(-1);
                    if conn > -1 {
                        let entrance = world
                            .abstract_room(conn)
                            .map_or(-1, |r| r.exit_index(room.index));
                        let to = WorldCoordinate::new(conn, -1, -1, entrance);
                        let mut cost = world.data.total_shortcut_length_between(room.index, conn)
                            as f32
                            * t.abstract_resistance(AbstractMovement::ShortCut).resistance;
                        if cost > 0.0 {
                            cost += t
                                .abstract_resistance(AbstractMovement::BetweenRooms)
                                .resistance;
                            if !checked.get(to) {
                                adds.push((to, cost));
                            }
                        }
                    }
                }
                Some(
                    k @ (NodeType::SideExit
                    | NodeType::SkyExit
                    | NodeType::SeaExit
                    | NodeType::RegionTransportation),
                ) => {
                    let movement = World::highway_of(k);
                    let cost = t.abstract_resistance(movement);
                    if t.maps_node(k) && cost.allowed {
                        // The game's region transportation costs read the side highway's
                        // resistance.
                        let resistance = if k == NodeType::RegionTransportation {
                            t.abstract_resistance(AbstractMovement::SideHighway)
                                .resistance
                        } else {
                            cost.resistance
                        };
                        for to in world.border_access_nodes(k) {
                            if to.same_room_and_node(pos) {
                                continue;
                            }
                            let mut c = world.highway_distance(k, pos, to) as f32 * resistance;
                            if c > 0.0 {
                                c += t
                                    .abstract_resistance(AbstractMovement::BetweenRooms)
                                    .resistance;
                                if !checked.get(to) {
                                    adds.push((to, c));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            for (to, cost) in adds {
                // `AddNode`: checked when queued, so a later, cheaper route is not taken.
                if checked.get(to) {
                    continue;
                }
                checked.set(to);
                nodes.push(SearchNode {
                    pos: to,
                    parent: Some(current),
                    cost: nodes[current].cost + cost as i32,
                });
                queue.push(nodes.len() - 1);
                if to.node == start.node && to.room == start.room {
                    found = Some(nodes.len() - 1);
                }
            }
            generation += 1;
            if generation > 500 {
                log::warn!(
                    "EMERGENCY EXIT AbstractSpacePathFinder after {generation} generations!"
                );
                return None;
            }
        }
        let Some(mut n) = found else {
            log::warn!("Abstract path finder has given up after {generation} generations");
            return None;
        };
        let mut path = vec![nodes[n].pos];
        while nodes[n].pos != goal {
            n = nodes[n].parent?;
            path.push(nodes[n].pos);
        }
        path.reverse();
        Some(path)
    }
}

/// `AbstractSpaceNodeFinder.SearchingFor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchingFor {
    Den,
    SwarmRoom,
    AttractiveRoom,
}

impl SearchingFor {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Den => "Den",
            Self::SwarmRoom => "SwarmRoom",
            Self::AttractiveRoom => "AttractiveRoom",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [Self::Den, Self::SwarmRoom, Self::AttractiveRoom]
            .into_iter()
            .find(|s| s.value() == value)
    }
}

/// `AbstractSpaceNodeFinder.FloodMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloodMethod {
    Cost,
    Random,
}

impl FloodMethod {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Cost => "Cost",
            Self::Random => "Random",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [Self::Cost, Self::Random]
            .into_iter()
            .find(|s| s.value() == value)
    }
}

/// `AbstractSpaceNodeFinder`, run to its end (`GoToDen` updates it until finished).
/// `maxGenerations` only scales the attractive-room goal, which is not ported.
pub(crate) struct AbstractSpaceNodeFinder<'a> {
    pub world: &'a World,
    pub template: &'a CreatureTemplate,
    pub searching_for: SearchingFor,
    pub flood_method: FloodMethod,
}

impl AbstractSpaceNodeFinder<'_> {
    /// `ReturnPathToClosest` after running from `pos`: the goal first, `pos`'s node last.
    /// The constructor's `DefineNodeOfLocalCoordinate` is the caller's.
    pub fn run(&self, pos: WorldCoordinate, rng: &mut UnityRandom) -> Option<Vec<WorldCoordinate>> {
        let world = self.world;
        let t = self.template;
        if self.is_goal(pos) {
            return Some(vec![WorldCoordinate::new(pos.room, -1, -1, pos.node)]);
        }
        let mut nodes = vec![SearchNode {
            pos,
            parent: None,
            cost: 0,
        }];
        let mut queue = vec![0usize];
        let mut checked = Checked::new(world);
        loop {
            if queue.is_empty() {
                return None;
            }
            let pick = match self.flood_method {
                FloodMethod::Cost => {
                    let mut best = 0;
                    for (k, &n) in queue.iter().enumerate() {
                        if nodes[n].cost < nodes[queue[best]].cost {
                            best = k;
                        }
                    }
                    best
                }
                FloodMethod::Random => {
                    let len = i32::try_from(queue.len()).unwrap_or(i32::MAX);
                    usize::try_from(rng.range_i32(0, len)).unwrap_or(0)
                }
            };
            let current = queue.remove(pick);
            let pos = nodes[current].pos;
            if checked.cell(pos).is_none() {
                continue;
            }
            checked.set(pos);
            let Some(room) = world.abstract_room(pos.room) else {
                continue;
            };
            let Some(node) = usize::try_from(pos.node)
                .ok()
                .and_then(|n| room.nodes.get(n))
            else {
                continue;
            };
            let mut adds: Vec<(WorldCoordinate, f32)> = Vec::new();
            let offscreen = t
                .abstract_resistance(AbstractMovement::OffScreenMovement)
                .resistance;
            for k in 0..i32::try_from(room.nodes.len()).unwrap_or(0) {
                if k == pos.node {
                    continue;
                }
                let to = WorldCoordinate::new(room.index, -1, -1, k);
                let cost = world.connectivity_cost(room.index, k, pos.node, t) as f32 * offscreen;
                if cost >= 0.0 && !checked.get(to) {
                    adds.push((to, cost));
                }
            }
            // The game crosses a border only for node types the creature does NOT map
            // (`!mappedNodeTypes[...]`; for exits it reads index 0, Exit).
            match node.kind {
                NodeType::Exit if !t.maps_node(NodeType::Exit) => {
                    let conn = room
                        .connections
                        .get(usize::try_from(pos.node).unwrap_or(usize::MAX))
                        .copied()
                        .unwrap_or(-1);
                    if conn > -1 {
                        let entrance = world
                            .abstract_room(conn)
                            .map_or(-1, |r| r.exit_index(room.index));
                        let to = WorldCoordinate::new(conn, -1, -1, entrance);
                        let mut cost = world.data.total_shortcut_length_between(room.index, conn)
                            as f32
                            * t.abstract_resistance(AbstractMovement::ShortCut).resistance;
                        if cost > 0.0 && !world.forbidden_for(to.room, t) {
                            cost += t
                                .abstract_resistance(AbstractMovement::BetweenRooms)
                                .resistance;
                            if !checked.get(to) {
                                adds.push((to, cost));
                            }
                        }
                    }
                }
                k @ (NodeType::SideExit
                | NodeType::SkyExit
                | NodeType::SeaExit
                | NodeType::RegionTransportation)
                    if !t.maps_node(k) =>
                {
                    let resistance = t.abstract_resistance(World::highway_of(k)).resistance;
                    for to in world.border_access_nodes(k) {
                        if to.same_room_and_node(pos) {
                            continue;
                        }
                        let mut c = world.highway_distance(k, pos, to) as f32 * resistance;
                        if c > 0.0 && !world.forbidden_for(to.room, t) {
                            c += t
                                .abstract_resistance(AbstractMovement::BetweenRooms)
                                .resistance;
                            if !checked.get(to) {
                                adds.push((to, c));
                            }
                        }
                    }
                }
                _ => {}
            }
            for (to, cost) in adds {
                nodes.push(SearchNode {
                    pos: to,
                    parent: Some(current),
                    cost: nodes[current].cost + cost as i32,
                });
                queue.push(nodes.len() - 1);
            }
            if self.is_goal(pos) {
                let mut path = vec![pos];
                let mut n = current;
                while let Some(p) = nodes[n].parent {
                    n = p;
                    path.push(nodes[n].pos);
                }
                return Some(path);
            }
        }
    }

    /// `IsNodeGoal`. The attractive-room goal reads `startPos`, which the game never
    /// sets; it is not ported (no caller in the base game's plain AI).
    fn is_goal(&self, pos: WorldCoordinate) -> bool {
        let world = self.world;
        match self.searching_for {
            SearchingFor::Den => pos.node_defined() && world.node_kind(pos) == Some(NodeType::Den),
            SearchingFor::SwarmRoom => world
                .abstract_room(pos.room)
                .is_some_and(|r| r.swarm_room_index > -1 && world.active_swarm_room(pos.room)),
            SearchingFor::AttractiveRoom => {
                log::warn!("AbstractSpaceNodeFinder's AttractiveRoom goal is not ported");
                false
            }
        }
    }
}
