//! The flies' swarm room map (`SwarmRoomMapper`, run to completion by `WorldLoader`'s
//! `MappingSwarmRooms` and handed to `FliesWorldAI.LoadDijkstraMaps`), the swarm room
//! queries `FliesWorldAI` answers (`GetSwarmRoomDistance`, `ActiveSwarmRoom`) and
//! `AbstractRoom.NumberOfQuantifiedCreatureInRoom`. None of it draws a random number.
//!
//! Behaviour: `rainworld-re/notes/creature-model/overseers-and-flies.md` §7.
//!
//! Mirrors: SwarmRoomMapper.cs, FliesWorldAI.cs, WorldLoader.cs, AbstractRoom.cs

use std::collections::VecDeque;

use rw_assets::world::WorldData;
use rw_assets::{ActiveMods, CreatureType};

use crate::creature::{CreatureTemplate, WorldCoordinate};
use crate::world::World;

/// `SwarmRoomMapper`: for each swarm room, a breadth-first spread from its dry nodes
/// over every node of the region, adding the fly's connection costs; a step through a
/// room exit costs the two tubes' length times the fly's `ShortCut` resistance plus its
/// `BetweenRooms` resistance. Submerged nodes are never entered.
#[derive(Debug, Clone, PartialEq)]
pub struct SwarmRoomMapper {
    current_swarm_room: i32,
    check_next: VecDeque<WorldCoordinate>,
    pub done: bool,
    map: SwarmRoomMap,
}

/// `[room][node][swarm room]`: the flies' path cost to each swarm room, -1 where none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SwarmRoomMap(Vec<Vec<Vec<f32>>>);

impl SwarmRoomMap {
    /// The cell of local room `room`, node `node`, swarm room `swarm`.
    #[must_use]
    pub fn get(&self, room: usize, node: usize, swarm: usize) -> Option<f32> {
        self.0.get(room)?.get(node)?.get(swarm).copied()
    }

    /// The cell at a coordinate of the world (`None` outside, where the game throws).
    fn cell_mut(
        &mut self,
        data: &WorldData,
        at: WorldCoordinate,
        swarm: usize,
    ) -> Option<&mut f32> {
        let room = usize::try_from(at.room - data.first_room_index()).ok()?;
        let node = usize::try_from(at.node).ok()?;
        self.0.get_mut(room)?.get_mut(node)?.get_mut(swarm)
    }
}

impl SwarmRoomMapper {
    /// The Fly template's `ShortCut` connection resistance (`InitStaticWorld` lists 1).
    const FLY_SHORTCUT_RESISTANCE: f32 = 1.0;
    /// The Fly template's `BetweenRooms` resistance: not listed, so the constructor's
    /// default (100, illegal).
    const FLY_BETWEEN_ROOMS_RESISTANCE: f32 = 100.0;
    /// `WorldLoader` stops mapping after this many updates.
    const MAX_UPDATES: u32 = 50_000;

    /// The constructor: every cell -1, the first swarm room seeded.
    #[must_use]
    pub fn new(data: &WorldData) -> Self {
        let swarms = data.swarm_rooms.len();
        let map = data
            .rooms
            .iter()
            .map(|r| vec![vec![-1.0; swarms]; r.nodes.len()])
            .collect();
        let mut mapper = Self {
            current_swarm_room: -1,
            check_next: VecDeque::new(),
            done: false,
            map: SwarmRoomMap(map),
        };
        mapper.init_new_mapping(data);
        mapper
    }

    /// `MappingSwarmRooms`: updates until done, or past [`Self::MAX_UPDATES`].
    #[must_use]
    pub fn run(data: &WorldData) -> SwarmRoomMap {
        let mut mapper = Self::new(data);
        let mut updates = 0;
        loop {
            updates += 1;
            mapper.update(data);
            if mapper.done || updates > Self::MAX_UPDATES {
                return mapper.map;
            }
        }
    }

    fn swarm(&self) -> usize {
        usize::try_from(self.current_swarm_room).unwrap_or(0)
    }

    /// One queued node spreads to its room's nodes and, through its own exit, to the
    /// room behind it.
    pub fn update(&mut self, data: &WorldData) {
        if self.done {
            return;
        }
        let Some(here) = self.check_next.pop_front() else {
            self.init_new_mapping(data);
            return;
        };
        let swarm = self.swarm();
        let Some(room) = data.room(here.room) else {
            return;
        };
        let fly = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
        let Some(&mut here_value) = self.map.cell_mut(data, here, swarm) else {
            return;
        };
        for (i, node) in room.nodes.iter().enumerate() {
            let index = i32::try_from(i).unwrap_or(-1);
            let mut to = WorldCoordinate::new(here.room, -1, -1, index);
            let mut cost = node.connection_cost(here.node, fly.pre_baked_pathing_index()) as f32;
            let connection = room.connections.get(i).copied().unwrap_or(-1);
            if index == here.node && connection > -1 {
                to = World::node_in_a_leading_to_b(data, connection, room.index);
                cost = data.total_shortcut_length_between(connection, room.index) as f32
                    * Self::FLY_SHORTCUT_RESISTANCE
                    + Self::FLY_BETWEEN_ROOMS_RESISTANCE;
                if Self::migration_blocked(data, to, here) {
                    cost = -1.0;
                }
            }
            if to.node < 0 || Self::submerged(data, to) {
                cost = -1.0;
            }
            if cost > -1.0
                && let Some(cell) = self.map.cell_mut(data, to, swarm)
                && *cell == -1.0
            {
                *cell = here_value + cost;
                self.check_next.push_back(to);
            }
        }
    }

    fn submerged(data: &WorldData, at: WorldCoordinate) -> bool {
        data.room(at.room)
            .and_then(|r| r.nodes.get(usize::try_from(at.node).ok()?))
            .is_some_and(|n| n.submerged)
    }

    /// `InitNewMapping`: the next swarm room's dry nodes start at 0.
    fn init_new_mapping(&mut self, data: &WorldData) {
        self.current_swarm_room += 1;
        let swarm = self.swarm();
        let Some(room) = data.swarm_rooms.get(swarm).and_then(|&r| data.room(r)) else {
            log::info!("mapped all {} swarm rooms", data.swarm_rooms.len());
            self.done = true;
            return;
        };
        self.check_next.clear();
        for (i, node) in room.nodes.iter().enumerate() {
            if !node.submerged {
                let at = WorldCoordinate::new(room.index, -1, -1, i32::try_from(i).unwrap_or(-1));
                if let Some(cell) = self.map.cell_mut(data, at, swarm) {
                    *cell = 0.0;
                    self.check_next.push_back(at);
                }
            }
        }
    }

    /// `MigrationBlocked`: flies may not go from `from` into `to`'s room when a
    /// blockage names it (from `from`'s room, or from anywhere); shelters and gates are
    /// never blocked.
    fn migration_blocked(data: &WorldData, from: WorldCoordinate, to: WorldCoordinate) -> bool {
        if data.room(to.room).is_some_and(|r| r.shelter() || r.gate()) {
            return false;
        }
        data.bat_migration_blockages
            .iter()
            .any(|&[f, t]| t == to.room && (f == from.room || f == -1))
    }
}

impl World {
    /// `World.NodeInALeadingToB`: the exit of room `a` leading to room `b`, or an
    /// all -1 coordinate.
    #[must_use]
    pub fn node_in_a_leading_to_b(data: &WorldData, a: i32, b: i32) -> WorldCoordinate {
        match data.room(a) {
            Some(room) if data.room(b).is_some() && room.exit_index(b) != -1 => {
                WorldCoordinate::new(a, -1, -1, room.exit_index(b))
            }
            _ => WorldCoordinate::new(-1, -1, -1, -1),
        }
    }

    /// `WorldLoader`'s `MappingSwarmRooms` and `FliesWorldAI.LoadDijkstraMaps` (not for
    /// a single-room world, which gets no map).
    pub fn map_swarm_rooms(&mut self) {
        if !self.single_room() {
            self.processes.swarm_room_map = Some(SwarmRoomMapper::run(&self.data));
        }
    }

    /// `FliesWorldAI.GetSwarmRoomDistance`: -1 for an undefined node, and where the game
    /// throws (no map, a coordinate outside it).
    #[must_use]
    pub fn swarm_room_distance(&self, coord: WorldCoordinate, swarm_room: usize) -> f32 {
        let (Ok(room), Ok(node)) = (
            usize::try_from(coord.room - self.first_room_index()),
            usize::try_from(coord.node),
        ) else {
            return -1.0;
        };
        self.processes
            .swarm_room_map
            .as_ref()
            .and_then(|m| m.get(room, node, swarm_room))
            .unwrap_or(-1.0)
    }

    /// `FliesWorldAI.ActiveSwarmRoom`: a swarm room whose region state counter is 0;
    /// every swarm room without a region state (arena, single-room worlds).
    #[must_use]
    pub fn active_swarm_room(&self, room: i32) -> bool {
        let Some(index) = self
            .abstract_room(room)
            .and_then(|r| usize::try_from(r.swarm_room_index).ok())
        else {
            return false;
        };
        self.region_state
            .as_ref()
            .is_none_or(|s| s.swarm_room_active(&self.data, index))
    }

    /// `Player.DepleteSwarmRoom` for a fly first caught in room `room` (story sessions):
    /// the room's own active swarm room, else the active one nearest a fly node of the
    /// room, becomes a candidate for depletion. Returns its name.
    pub fn deplete_swarm_room(&mut self, room: i32) -> Option<String> {
        let state = self.region_state.as_ref()?;
        let own = self
            .abstract_room(room)
            .and_then(|r| usize::try_from(r.swarm_room_index).ok())
            .filter(|&i| state.swarm_room_active(&self.data, i));
        let mut pick = own;
        if pick.is_none() {
            let mut best = f32::MAX;
            for i in 0..self.data.swarm_rooms.len() {
                if !state.swarm_room_active(&self.data, i) {
                    continue;
                }
                for (node, _) in self.fly_nodes(room) {
                    let d = self.swarm_room_distance(WorldCoordinate::new(room, -1, -1, node), i);
                    if d >= 0.0 && d < best {
                        pick = Some(i);
                        best = d;
                    }
                }
            }
        }
        let name = self
            .data
            .swarm_rooms
            .get(pick?)
            .and_then(|&r| self.data.room(r))?
            .name
            .clone();
        if let Some(state) = &mut self.region_state {
            state.candidates_for_deplete_swarm_rooms.push(name.clone());
        }
        Some(name)
    }

    /// `AbstractRoom.NumberOfQuantifiedCreatureInRoom`: the counts of the room's nodes,
    /// plus its creatures of that type without a node that `Quantify`.
    #[must_use]
    pub fn number_of_quantified_creature_in_room(&self, room: i32, kind: CreatureType) -> i32 {
        let nodes = self.abstract_room(room).map_or(0, |r| r.nodes.len());
        let counted: i32 = (0..nodes)
            .map(|n| {
                self.number_of_quantified_creature_in_node(
                    room,
                    kind,
                    i32::try_from(n).unwrap_or(-1),
                )
            })
            .sum();
        let loose = self
            .room_creatures(room)
            .iter()
            .map(|&id| self.creature(id))
            .filter(|c| !c.pos.node_defined() && c.kind() == kind && c.quantify())
            .count();
        counted + i32::try_from(loose).unwrap_or(0)
    }

    /// `AbstractRoom.NumberOfQuantifiedCreatureInNode`: in a realized room whose
    /// quantified creatures are placed (fully loaded), its creatures at that node that
    /// `Quantify`; else the node's count.
    #[must_use]
    pub fn number_of_quantified_creature_in_node(
        &self,
        room: i32,
        kind: CreatureType,
        node: i32,
    ) -> i32 {
        if self
            .realized(room)
            .is_some_and(crate::world::RealizedRoom::fully_loaded)
        {
            let n = self
                .room_creatures(room)
                .iter()
                .map(|&id| self.creature(id))
                .filter(|c| c.pos.node == node && c.kind() == kind && c.quantify())
                .count();
            return i32::try_from(n).unwrap_or(0);
        }
        self.quantified_count(room, kind, node)
    }

    /// `quantifiedCreatures[node, type]`.
    #[must_use]
    pub fn quantified_count(&self, room: i32, kind: CreatureType, node: i32) -> i32 {
        self.room_quantified(room)
            .iter()
            .find(|q| q.kind == kind && q.node == node)
            .map_or(0, |q| q.count)
    }
}
