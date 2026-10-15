//! The parts of the save's `RegionState` that the world processes read. A story world
//! that is not a single room has one (`World.regionState`); a new game's has every swarm
//! room's counter at 0 and no room visited.

use std::collections::{HashMap, HashSet};

use rw_assets::world::WorldData;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionState {
    /// `swarmRoomCounters` by room name: cycles until the swarm room's flies return
    /// (0: active).
    pub swarm_room_counters: HashMap<String, i32>,
    /// `roomsVisited`: room names. The port does not record visits; the caller fills it.
    pub rooms_visited: HashSet<String>,
    /// `candidatesForDepleteSwarmRooms`: swarm rooms a caught fly came from (read when
    /// the cycle is saved, which the port does not do yet).
    pub candidates_for_deplete_swarm_rooms: Vec<String>,
}

impl RegionState {
    /// `new RegionState` for a region without saved state.
    #[must_use]
    pub fn new_game(data: &WorldData) -> Self {
        Self {
            swarm_room_counters: data
                .swarm_rooms
                .iter()
                .filter_map(|&r| data.room(r))
                .map(|r| (r.name.clone(), 0))
                .collect(),
            rooms_visited: HashSet::new(),
            candidates_for_deplete_swarm_rooms: Vec::new(),
        }
    }

    /// `SwarmRoomActive(swarmRoomIndex)`.
    #[must_use]
    pub fn swarm_room_active(&self, data: &WorldData, swarm_room: usize) -> bool {
        data.swarm_rooms
            .get(swarm_room)
            .and_then(|&r| data.room(r))
            .and_then(|r| self.swarm_room_counters.get(&r.name))
            .is_some_and(|&c| c == 0)
    }
}
