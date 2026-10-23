//! What `get_state` reports of the realized rooms' coral: each `CoralNeuronSystem`
//! (wind, age, whether it is frozen, its registry of strands and their owners), each
//! `WallMycelia` and the `NeuronSpark`s strands struck.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{CosmeticKind, MyceliumPool, OwnerKey, WallMycelia};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// Strands in a system's registry that belong to one owner.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryOwnerState {
    pub owner: u32,
    /// `wall_mycelia` for the room's `WallMycelia`, else an overseer's graphics module
    /// (the other owners are not ported yet).
    pub kind: &'static str,
    pub strands: usize,
    /// How many of them are linked to another strand.
    pub connected: usize,
}

/// One `CoralNeuronSystem`.
#[derive(Debug, Clone, Serialize)]
pub struct CoralSystemState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub wind: [f32; 2],
    pub age: i32,
    pub not_viewed_counter: i32,
    pub frozen: bool,
    /// `AIMapReady` has built its owners.
    pub built: bool,
    /// The strands in its `mycelia` registry.
    pub mycelia: usize,
    pub owners: Vec<RegistryOwnerState>,
}

/// One `WallMycelia`.
#[derive(Debug, Clone, Serialize)]
pub struct WallMyceliaState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub owner: u32,
    /// The placed objects it was made from: position and radius.
    pub places: Vec<([f32; 2], f32)>,
    pub strands: usize,
    pub culled: usize,
    /// Pairs of strands linked to each other.
    pub pairs_connected: usize,
    /// Strands linked to a strand of another owner (an overseer's).
    pub linked_to_others: usize,
    /// The first strand: root, direction, length, points and random movement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<WallStrandState>,
}

/// A strand of a `WallMycelia`.
#[derive(Debug, Clone, Serialize)]
pub struct WallStrandState {
    pub root: [f32; 2],
    pub direction: [f32; 2],
    pub length: f32,
    pub points: usize,
    pub tip: [f32; 2],
    pub random_movement: [f32; 2],
    pub culled: bool,
    pub rest: i32,
    pub connected: bool,
}

/// One `NeuronSpark`.
#[derive(Debug, Clone, Serialize)]
pub struct NeuronSparkState {
    pub room: String,
    /// The cosmetic's id.
    pub id: u64,
    pub pos: [f32; 2],
    pub life: f32,
    pub life_time: f32,
}

impl CoralSystemState {
    /// Every realized room's system.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((i, world.realized(i)?)))
            .filter_map(|(i, r)| {
                let s = r.coral.as_ref()?;
                let pool = world.mycelia();
                let walls: Vec<OwnerKey> = s.wall_mycelia.iter().map(|w| w.owner).collect();
                Some(Self {
                    room: RoomName::of(world, i),
                    id: s.update_order,
                    wind: s.wind.to_array(),
                    age: s.age,
                    not_viewed_counter: s.not_viewed_counter,
                    frozen: s.frozen(),
                    built: s.built,
                    mycelia: pool.registry(i).len(),
                    owners: Self::owners(pool, i, &walls),
                })
            })
            .collect()
    }

    fn owners(pool: &MyceliumPool, room: i32, walls: &[OwnerKey]) -> Vec<RegistryOwnerState> {
        let mut owners: Vec<RegistryOwnerState> = Vec::new();
        for m in pool.registry(room).iter().filter_map(|&id| pool.get(id)) {
            let i = match owners.iter().position(|o| o.owner == m.owner.0) {
                Some(i) => i,
                None => {
                    owners.push(RegistryOwnerState {
                        owner: m.owner.0,
                        kind: if walls.contains(&m.owner) {
                            "wall_mycelia"
                        } else {
                            "overseer"
                        },
                        strands: 0,
                        connected: 0,
                    });
                    owners.len() - 1
                }
            };
            owners[i].strands += 1;
            owners[i].connected += usize::from(m.connection.is_some());
        }
        owners
    }
}

impl WallMyceliaState {
    /// Every realized room's wall mycelia.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.coral
                    .iter()
                    .flat_map(|s| &s.wall_mycelia)
                    .map(move |w| (room.clone(), w))
            })
            .map(|(room, w)| Self::of(room, w, world.mycelia()))
            .collect()
    }

    fn of(room: String, w: &WallMycelia, pool: &MyceliumPool) -> Self {
        let strands: Vec<_> = w
            .mycelia
            .iter()
            .filter_map(|&id| Some((id, pool.get(id)?)))
            .collect();
        let connected = strands
            .iter()
            .filter(|(_, m)| m.connection.is_some())
            .count();
        let own = strands
            .iter()
            .filter(|(id, m)| {
                m.connection
                    .is_some_and(|c| pool.get(c.other(*id)).is_some_and(|o| o.owner == w.owner))
            })
            .count();
        Self {
            room,
            id: w.update_order,
            owner: w.owner.0,
            places: w.places.iter().map(|p| (p.pos.to_array(), p.rad)).collect(),
            strands: strands.len(),
            culled: strands.iter().filter(|(_, m)| m.culled).count(),
            pairs_connected: own / 2,
            linked_to_others: connected - own,
            first: strands.first().map(|(_, m)| WallStrandState {
                root: w.roots.root_positions[0].to_array(),
                direction: w.roots.directions[0].to_array(),
                length: m.length,
                points: m.points.len(),
                tip: m.tip().to_array(),
                random_movement: w.random_movements[0].to_array(),
                culled: m.culled,
                rest: m.rest,
                connected: m.connection.is_some(),
            }),
        }
    }
}

impl NeuronSparkState {
    /// Every realized room's sparks.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .map(|i| (RoomName::of(world, i), world.room_cosmetics(i)))
            .flat_map(|(room, cosmetics)| {
                cosmetics.iter().filter_map(move |c| match &c.kind {
                    CosmeticKind::NeuronSpark(s) => Some(Self {
                        room: room.clone(),
                        id: c.id,
                        pos: s.pos.to_array(),
                        life: s.life,
                        life_time: s.life_time,
                    }),
                    _ => None,
                })
            })
            .collect()
    }
}
