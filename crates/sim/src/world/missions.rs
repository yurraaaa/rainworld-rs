//! `MissionTracker.LeaveRoom.Act`: the way out of the room for a lizard that submitted
//! to another, chosen once (the threat tracker's most attractive exit while it fears
//! something, else the nearest open exit, else one its node connects to, else a random
//! connection) and then walked to, through the next room to the one beyond it where
//! that room's cheapest way on is an exit.
//!
//! Mirrors: MissionTracker.cs (LeaveRoom.Act)

use rw_assets::NodeType;
use rw_random::UnityRandom;

use super::World;
use crate::creature::{CreatureId, LeaveRoom, WorldCoordinate};

impl World {
    /// `LeaveRoom.Act` of AI `ai` with mission state `m`; returns whether it abandons
    /// the mission (at its destination's room and node, or 1000 ticks unable to reach
    /// or come back from it).
    pub(crate) fn leave_room_act(
        &mut self,
        ai: CreatureId,
        m: &mut LeaveRoom,
        rng: &mut UnityRandom,
    ) -> bool {
        let c = self.creature(ai);
        let t = c.template;
        let mut coord = c.pos;
        if !coord.node_defined() {
            coord = self.define_node(coord, &t, rng);
        }
        if let Some(dest) = m.destination {
            let mut abandon = coord.same_room_and_node(dest);
            if !abandon {
                self.set_destination(ai, dest, rng);
            }
            let lost = self.path_finder(ai).is_some_and(|p| {
                !p.coordinate_possible_to_get_back_from(dest) || !p.coordinate_reachable(dest)
            });
            if lost {
                m.dest_unreachable += 1;
                abandon |= m.dest_unreachable > 1000;
            } else {
                m.dest_unreachable = 0;
            }
            return abandon;
        }
        let Some(exit) = self.leave_room_exit(ai, coord, rng) else {
            return false;
        };
        m.destination = self.leave_room_destination(ai, coord.room, exit);
        false
    }

    /// The exit `Act` leaves by: in a room ready for AI the threat tracker's most
    /// attractive exit while its utility is above 0 (an exit node only), else the open
    /// exit nearest by the template's exit distance; failing those the node it is at
    /// when that is an open exit, the first open connection there and back from it, or
    /// a random connection (one draw).
    fn leave_room_exit(
        &self,
        ai: CreatureId,
        coord: WorldCoordinate,
        rng: &mut UnityRandom,
    ) -> Option<usize> {
        let t = self.creature(ai).template;
        let room = self.abstract_room(coord.room)?;
        let mut exit: Option<usize> = None;
        if let Some(map) = self
            .realized(coord.room)
            .filter(|r| r.ready_for_ai())
            .and_then(|r| r.ai_map.as_deref())
        {
            if self
                .creature(ai)
                .lizard()
                .is_some_and(|l| l.ai.threat.utility() > 0.0)
            {
                let e = self.threat_find_most_attractive_exit(ai, rng);
                exit = usize::try_from(e)
                    .ok()
                    .filter(|&e| room.nodes.get(e).is_some_and(|n| n.kind == NodeType::Exit));
            }
            if exit.is_none() {
                let mut best = i32::MAX;
                for i in 0..room.connections.len() {
                    let specific = t.specific_node_index(room, i32::try_from(i).ok()?);
                    let d = map
                        .exit_distance_for_creature_and_check_neighbours(coord.tile, specific, &t);
                    if d > -1 && d < best && self.open_exit(coord.room, i) {
                        best = d;
                        exit = Some(i);
                    }
                }
            }
        }
        if exit.is_some() {
            return exit;
        }
        let here = usize::try_from(coord.node).ok()?;
        if self.open_exit(coord.room, here) {
            return Some(here);
        }
        for j in 0..room.connections.len() {
            if room.connections[j] > -1
                && self.connection_and_back_possible(
                    coord.room,
                    coord.node,
                    i32::try_from(j).ok()?,
                    &t,
                )
            {
                return Some(j);
            }
        }
        if room.connections.is_empty() {
            // D14: the game reads `connections[-1]` and throws (IndexOutOfRange).
            log::warn!("LeaveRoom.Act in a room without connections: the game throws here");
            return None;
        }
        let n = i32::try_from(room.connections.len()).ok()?;
        usize::try_from(rng.range_i32(0, n)).ok()
    }

    /// Whether node `i` of `room` is an exit with a connection.
    fn open_exit(&self, room: i32, i: usize) -> bool {
        self.abstract_room(room).is_some_and(|r| {
            r.nodes.get(i).is_some_and(|n| n.kind == NodeType::Exit)
                && r.connections.get(i).is_some_and(|&c| c > -1)
        })
    }

    /// Where `Act` heads through exit `exit` of room `here`: in the next room the
    /// node cheapest to reach from its entrance there and back (exits at a tenth the
    /// cost); an open exit there leads on to the entrance of the room beyond, another
    /// node is the destination itself, and with none the entrance is.
    fn leave_room_destination(
        &self,
        ai: CreatureId,
        here: i32,
        exit: usize,
    ) -> Option<WorldCoordinate> {
        let t = self.creature(ai).template;
        let next = *self.abstract_room(here)?.connections.get(exit)?;
        if next < 0 {
            // D14: the random connection is unconnected; the game's
            // `GetAbstractRoom(-1)` is null and `.index` throws (NullReference).
            log::warn!("LeaveRoom.Act chose connection -1: the game throws here");
            return None;
        }
        let other = self.abstract_room(next)?;
        let entrance = other.exit_index(here);
        let value = WorldCoordinate::new(next, -1, -1, entrance);
        let mut best: Option<usize> = None;
        let mut cheapest = f32::MAX;
        for (k, node) in other.nodes.iter().enumerate() {
            let ki = i32::try_from(k).ok()?;
            if entrance != ki && self.connection_and_back_possible(next, entrance, ki, &t) {
                let mut cost = if node.kind == NodeType::Exit {
                    1.0
                } else {
                    10.0
                };
                cost *= self.connectivity_cost(next, entrance, ki, &t) as f32;
                if cost < cheapest {
                    cheapest = cost;
                    best = Some(k);
                }
            }
        }
        let Some(k) = best else {
            return Some(value);
        };
        let onward = other.connections.get(k).copied().unwrap_or(-1);
        if other.nodes[k].kind == NodeType::Exit && onward > -1 {
            let beyond = self.abstract_room(onward)?;
            return Some(WorldCoordinate::new(
                onward,
                -1,
                -1,
                beyond.exit_index(next),
            ));
        }
        Some(WorldCoordinate::new(next, -1, -1, i32::try_from(k).ok()?))
    }
}

#[cfg(test)]
#[path = "tests/missions.rs"]
mod tests;
