//! What `get_state` reports of the realized rooms' `CoralCircuit`s (`coral_circuits`: per
//! cluster the places, tile box, bit counts, displaced and repairing bits, links, connectors
//! and crabs) and of the player's circuit swimming (`circuit_swim_resistance`,
//! `circuit_jump_meter`, `swim_bits`).
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{CoralCircuit, MyceliumPool, SwimBit};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// One `CircuitBit` worth showing: the first displaced one of its circuit.
#[derive(Debug, Clone, Serialize)]
pub struct CircuitBitState {
    pub index: usize,
    pub pos: [f32; 2],
    pub stuck_pos: [f32; 2],
    pub vel: [f32; 2],
    pub size: [f32; 2],
    pub rotat: [f32; 2],
    pub d_rotat: [f32; 2],
    pub rotat_vel: f32,
    pub d_rotat_vel: f32,
    pub repair_counter: i32,
    pub no_repair_needed_counter: i32,
    pub lonely: bool,
    pub shader: bool,
    pub neighbors: usize,
    pub active_connections: usize,
}

/// A `CircuitConnector`.
#[derive(Debug, Clone, Serialize)]
pub struct CircuitConnectorState {
    /// Its place in the update list.
    pub id: u64,
    pub bit: usize,
    pub c_pos: [f32; 2],
    pub owner: u32,
    pub strands: usize,
    pub culled: usize,
}

/// A `CircuitCrab`.
#[derive(Debug, Clone, Serialize)]
pub struct CircuitCrabState {
    /// Its place in the update list.
    pub id: u64,
    pub bit: usize,
    pub next_bit: usize,
    pub c_pos: [f32; 2],
    pub prog: f32,
    pub legs: usize,
    /// Legs reaching for a spot (`legBits` set).
    pub legs_reaching: usize,
}

/// One `CoralCircuit`.
#[derive(Debug, Clone, Serialize)]
pub struct CoralCircuitState {
    pub room: String,
    /// Its place in the update list, and its index in the system's `circuits`.
    pub id: u64,
    pub circuit: usize,
    /// The placed objects of its cluster: position and radius.
    pub places: Vec<([f32; 2], f32)>,
    pub tiles: usize,
    pub bottom_left: [i32; 2],
    pub top_right: [i32; 2],
    pub bits: usize,
    /// Bits not in position (`InPosition()` false).
    pub displaced: usize,
    /// Displaced bits that have waited over 100 ticks, so are being pulled home.
    pub repairing: usize,
    /// Neighbour links, and those still active.
    pub links: usize,
    pub active_links: usize,
    pub displaced_counter: i32,
    pub disconnect_chance: f32,
    pub connectors: Vec<CircuitConnectorState>,
    pub crabs: Vec<CircuitCrabState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_displaced: Option<CircuitBitState>,
}

/// A remembered `swimBits` entry.
#[derive(Debug, Clone, Serialize)]
pub struct SwimBitState {
    pub room: i32,
    pub circuit: u32,
    pub bit: u32,
    pub pos: [f32; 2],
}

impl SwimBitState {
    pub fn of(s: SwimBit) -> Self {
        Self {
            room: s.bit.room,
            circuit: s.bit.circuit,
            bit: s.bit.bit,
            pos: s.pos.to_array(),
        }
    }
}

impl CoralCircuitState {
    /// Every realized room's circuits.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        let mut out = Vec::new();
        for i in world.active_rooms() {
            let Some(r) = world.realized(i) else {
                continue;
            };
            let room = RoomName::of(world, i);
            for (n, circuit) in r.coral.iter().flat_map(|s| &s.circuits).enumerate() {
                out.push(Self::of(room.clone(), n, circuit, world.mycelia()));
            }
        }
        out
    }

    fn of(room: String, n: usize, c: &CoralCircuit, pool: &MyceliumPool) -> Self {
        let displaced: Vec<usize> = (0..c.bits.len())
            .filter(|&i| !c.bits[i].in_position())
            .collect();
        let links = c.bits.iter().map(|b| b.neighbors.len()).sum();
        let active_links = c
            .bits
            .iter()
            .map(|b| b.active_connections.iter().filter(|&&a| a).count())
            .sum();
        Self {
            room,
            id: c.update_order,
            circuit: n,
            places: c.places.iter().map(|p| (p.pos.to_array(), p.rad)).collect(),
            tiles: c.tiles.len(),
            bottom_left: [c.bottom_left.0, c.bottom_left.1],
            top_right: [c.top_right.0, c.top_right.1],
            bits: c.bits.len(),
            displaced: displaced.len(),
            repairing: displaced
                .iter()
                .filter(|&&i| c.bits[i].repair_counter > 100)
                .count(),
            links,
            active_links,
            displaced_counter: c.displaced_counter,
            disconnect_chance: c.disconnect_chance,
            connectors: c
                .connectors
                .iter()
                .map(|x| CircuitConnectorState {
                    id: x.update_order,
                    bit: x.bit,
                    c_pos: x.c_pos.to_array(),
                    owner: x.owner.0,
                    strands: x.connections.len(),
                    culled: x
                        .connections
                        .iter()
                        .filter(|&&id| pool.get(id).is_some_and(|m| m.culled))
                        .count(),
                })
                .collect(),
            crabs: c
                .crabs
                .iter()
                .map(|x| CircuitCrabState {
                    id: x.update_order,
                    bit: x.bit,
                    next_bit: x.next_bit,
                    c_pos: x.c_pos.to_array(),
                    prog: x.prog,
                    legs: x.legs.len(),
                    legs_reaching: x.leg_bits.iter().flatten().count(),
                })
                .collect(),
            first_displaced: displaced.first().map(|&i| {
                let b = &c.bits[i];
                CircuitBitState {
                    index: i,
                    pos: b.pos.to_array(),
                    stuck_pos: b.stuck_pos.to_array(),
                    vel: b.vel.to_array(),
                    size: b.size.to_array(),
                    rotat: b.rotat.to_array(),
                    d_rotat: b.d_rotat.to_array(),
                    rotat_vel: b.rotat_vel,
                    d_rotat_vel: b.d_rotat_vel,
                    repair_counter: b.repair_counter,
                    no_repair_needed_counter: b.no_repair_needed_counter,
                    lonely: b.lonely,
                    shader: b.shader,
                    neighbors: b.neighbors.len(),
                    active_connections: b.active_connections.iter().filter(|&&a| a).count(),
                }
            }),
        }
    }
}
