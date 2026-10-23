//! What `get_state` reports of the realized rooms' coral: each `CoralNeuronSystem`
//! (wind, age, whether it is frozen, its registry of strands and their owners), each
//! `WallMycelia`, `CoralNeuron` and `ProjectedCircle`, and the `NeuronSpark`s strands struck.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{
    CoralNeuron, CosmeticKind, MyceliumPool, OwnerKey, ProjectedCircle, VineRef, WallMycelia,
};
use serde::Serialize;

use super::placed_cosmetics_reply::RoomName;

/// Strands in a system's registry that belong to one owner.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryOwnerState {
    pub owner: u32,
    /// `wall_mycelia` for the room's `WallMycelia`, `coral_neuron` for a `CoralNeuron`,
    /// else an overseer's graphics module (the other owners are not ported yet).
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

/// One `CoralNeuron`.
#[derive(Debug, Clone, Serialize)]
pub struct CoralNeuronState {
    pub room: String,
    /// Its place in the update list (its `VineRef`).
    pub id: u64,
    /// The constructor's `length`, recovered from `conRad * segments / 1.5`.
    pub length: f32,
    pub segments: usize,
    pub con_rad: f32,
    /// The first and the last segment's position.
    pub first: [f32; 2],
    pub last: [f32; 2],
    /// The ends' places (snapped to the walls) and whether each is rooted on one.
    pub pos_a: Option<[f32; 2]>,
    pub pos_b: Option<[f32; 2]>,
    pub rooted_a: bool,
    pub rooted_b: bool,
    /// Its system is frozen: it does not update.
    pub frozen: bool,
    pub bumps: usize,
    /// Bumps pinging now (ping above 0).
    pub pinging: usize,
    pub owner: u32,
    /// The strands' ids in the pool, in `mycelia[l, m]` order.
    pub mycelia: Vec<u32>,
    pub culled_mycelia: usize,
    /// The player holds it (`VineGrab` on it).
    pub being_climbed: bool,
}

/// One `ProjectedCircle`.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectedCircleState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    /// The owner's place in the update list (a `CoralNeuron`) and the circle's `index`.
    pub owner: u64,
    pub index: usize,
    pub rad: f32,
    pub base_rad: f32,
    pub spokes: i32,
    pub blink: f32,
    pub depth_offset: f32,
    pub connected_circles: Vec<u64>,
    pub off_screen_connections: Vec<[f32; 2]>,
    pub connections_blink: Vec<i32>,
    pub rotation: f32,
    pub slated: bool,
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
                let neurons: Vec<OwnerKey> = s.neurons.iter().map(|n| n.owner).collect();
                Some(Self {
                    room: RoomName::of(world, i),
                    id: s.update_order,
                    wind: s.wind.to_array(),
                    age: s.age,
                    not_viewed_counter: s.not_viewed_counter,
                    frozen: s.frozen(),
                    built: s.built,
                    mycelia: pool.registry(s.id).len(),
                    owners: Self::owners(pool, s.id, &walls, &neurons),
                })
            })
            .collect()
    }

    fn owners(
        pool: &MyceliumPool,
        system: rw_sim::SystemId,
        walls: &[OwnerKey],
        neurons: &[OwnerKey],
    ) -> Vec<RegistryOwnerState> {
        let mut owners: Vec<RegistryOwnerState> = Vec::new();
        for m in pool.registry(system).iter().filter_map(|&id| pool.get(id)) {
            let i = match owners.iter().position(|o| o.owner == m.owner.0) {
                Some(i) => i,
                None => {
                    owners.push(RegistryOwnerState {
                        owner: m.owner.0,
                        kind: if walls.contains(&m.owner) {
                            "wall_mycelia"
                        } else if neurons.contains(&m.owner) {
                            "coral_neuron"
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

impl CoralNeuronState {
    /// Every realized room's neurons; `climbed` is the vine the player holds.
    #[must_use]
    pub fn all(world: &World, climbed: Option<VineRef>) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((i, world.realized(i)?)))
            .flat_map(|(i, r)| {
                let room = RoomName::of(world, i);
                let frozen = r.coral.as_ref().is_some_and(|s| s.frozen());
                r.coral
                    .iter()
                    .flat_map(|s| &s.neurons)
                    .map(move |n| Self::of(room.clone(), n, i, frozen, climbed, world.mycelia()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn of(
        room: String,
        n: &CoralNeuron,
        index: i32,
        frozen: bool,
        climbed: Option<VineRef>,
        pool: &MyceliumPool,
    ) -> Self {
        let g = &n.geometry;
        Self {
            room,
            id: n.update_order,
            length: n.con_rad * g.segments.len() as f32 / 1.5,
            segments: g.segments.len(),
            con_rad: n.con_rad,
            first: g.segments[0][0].to_array(),
            last: g.segments[g.segments.len() - 1][0].to_array(),
            pos_a: n.pos_a.map(rw_sim::Vec2::to_array),
            pos_b: n.pos_b.map(rw_sim::Vec2::to_array),
            rooted_a: g.root_dir_a.is_some(),
            rooted_b: g.root_dir_b.is_some(),
            frozen,
            bumps: n.bumps.len(),
            pinging: n.bump_pings.iter().filter(|p| p[0] > 0.0).count(),
            owner: n.owner.0,
            mycelia: n.mycelia.iter().map(|id| id.0).collect(),
            culled_mycelia: n
                .mycelia
                .iter()
                .filter(|&&id| pool.get(id).is_some_and(|m| m.culled))
                .count(),
            being_climbed: climbed
                == Some(VineRef::CoralNeuron {
                    room: index,
                    order: n.update_order,
                }),
        }
    }
}

impl ProjectedCircleState {
    /// Every realized room's circles.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| {
                r.projections
                    .circles
                    .iter()
                    .map(|c| Self::of(room.clone(), c))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn of(room: String, c: &ProjectedCircle) -> Self {
        let rw_sim::CircleOwner::Neuron { order } = c.owner;
        Self {
            room,
            id: c.update_order,
            owner: order,
            index: c.index,
            rad: c.rad,
            base_rad: c.base_rad,
            spokes: c.spokes,
            blink: c.blink,
            depth_offset: c.depth_offset,
            connected_circles: c.connected_circles.clone(),
            off_screen_connections: c
                .off_screen_connections
                .iter()
                .map(|p| p.to_array())
                .collect(),
            connections_blink: c.connections_blink.clone(),
            rotation: c.rotation,
            slated: c.slated,
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
