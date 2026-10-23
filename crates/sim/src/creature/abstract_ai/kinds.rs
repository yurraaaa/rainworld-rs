//! The fields and constructors of the `AbstractCreatureAI` subclasses the
//! `AbstractCreature` constructor picks (300531..300570) besides the overseer's and the
//! scavenger's: `VultureAbstractAI` (286524), `CicadaAbstractAI` (150275),
//! `BigEelAbstractAI` (139532), `DeerAbstractAI` (174999), `MirosBirdAbstractAI`
//! (209826), `NeedleWormAbstractAI` (215520). Their behaviour is
//! `creature/abstract_ai/kinds_world.rs`.
//!
//! Mirrors: AbstractCreatureAI.cs, VultureAbstractAI.cs, CicadaAbstractAI.cs, BigEelAbstractAI.cs, DeerAbstractAI.cs,
//!   MirosBirdAbstractAI.cs, NeedleWormAbstractAI.cs

use rw_assets::{CreatureType, NodeType, RoomAttraction};
use rw_random::UnityRandom;

use crate::creature::{AbstractCreature, CreatureId, WorldCoordinate};
use crate::world::World;

/// `VultureAbstractAI` (base game: never a Miros vulture). `lostMask` and
/// `checkedForLostMask` are left out: a vulture's mask is only lost by a realized
/// vulture, and its mask object is never in the port's world, so the search for it
/// finds nothing and the behaviour goes on as with the mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VultureAbstractAi {
    pub check_rooms: Vec<WorldCoordinate>,
    pub time_in_room: i32,
    pub dont_go_to_this_room: i32,
}

/// `CicadaAbstractAI`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CicadaAbstractAi {
    pub swarm_room: Option<WorldCoordinate>,
    pub cant_get_to_swarm_room: bool,
}

/// `BigEelAbstractAI`; `last_room` is its own field, hiding the base one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigEelAbstractAi {
    pub go_to: Option<WorldCoordinate>,
    pub last_room: i32,
    pub time_in_room: i32,
}

/// `DeerAbstractAI` (Remix's deer behaviour off). `sporePos` and `damageGoHome` are set
/// by the realized deer only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeerAbstractAi {
    pub go_to: Option<WorldCoordinate>,
    pub spore_pos: Option<WorldCoordinate>,
    pub last_room: i32,
    pub time_in_room: i32,
    pub allowed_nodes: Vec<WorldCoordinate>,
    pub damage_go_home: bool,
}

/// `MirosBirdAbstractAI`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirosBirdAbstractAi {
    pub allowed_nodes: Vec<WorldCoordinate>,
    pub wait: i32,
}

/// `NeedleWormAbstractAI`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeedleWormAbstractAi {
    pub mother: Option<CreatureId>,
    pub migrate_active: bool,
    pub migrate_active_counter: i32,
    pub temp_suppress_follow_creature: bool,
}

/// `DeerAbstractAI.UGLYHARDCODEDALLOWEDROOMS` and `...NODES`.
const DEER_ROOMS: [(&str, &[i32]); 6] = [
    ("LF_E04", &[6, 7]),
    ("LF_H01", &[5, 6]),
    ("LF_J01", &[2]),
    ("LF_E05", &[10]),
    ("LF_H02", &[5]),
    ("LF_D07", &[8, 9]),
];

/// `MirosBirdAbstractAI.UGLYHARDCODEDALLOWEDROOMS`.
const MIROS_ROOMS: [&str; 7] = [
    "SH_E01", "SH_E03", "SH_D03", "SH_H01", "SH_E04", "SB_G03", "SB_J01",
];

impl DeerAbstractAi {
    /// The constructor: `PopulateAllowedNodes` (no draws). The game reads the six rooms
    /// without a null check (a deer outside LF would throw); a missing room is skipped.
    pub(crate) fn new(parent: &AbstractCreature, world: &World) -> Self {
        let mut allowed_nodes = Vec::new();
        if !world.single_room() {
            for (name, nodes) in DEER_ROOMS {
                let Some(index) = world.room_index(name) else {
                    log::warn!("deer allowed room {name} is not in this world");
                    continue;
                };
                allowed_nodes.extend(
                    nodes
                        .iter()
                        .map(|&n| WorldCoordinate::new(index, -1, -1, n)),
                );
            }
        }
        Self {
            go_to: None,
            spore_pos: None,
            last_room: parent.pos.room,
            time_in_room: 0,
            allowed_nodes,
            damage_go_home: false,
        }
    }
}

impl MirosBirdAbstractAi {
    /// The constructor: `PopulateAllowedNodes` (no draws): the side exits at least 5 wide
    /// of the single room, else of the hard-coded rooms in this world.
    pub(crate) fn new(world: &World) -> Self {
        let rooms: Vec<i32> = if world.single_room() {
            vec![0]
        } else {
            MIROS_ROOMS
                .iter()
                .filter_map(|name| world.room_index(name))
                .collect()
        };
        let mut allowed_nodes = Vec::new();
        for room in rooms {
            let Some(data) = world.abstract_room(room) else {
                continue;
            };
            for (k, n) in data.nodes.iter().enumerate() {
                if n.kind == NodeType::SideExit && n.entrance_width >= 5 {
                    let k = i32::try_from(k).unwrap_or(-1);
                    allowed_nodes.push(WorldCoordinate::new(room, -1, -1, k));
                }
            }
        }
        Self {
            allowed_nodes,
            wait: 0,
        }
    }
}

impl World {
    /// `VultureAbstractAI.AddRandomCheckRoom` for a vulture at `pos` (base game: sky
    /// exits only): half the time, by its room's attraction, the cluster of its own room;
    /// else a sky access node weighted by size-dependent attraction squared.
    pub(crate) fn vulture_add_random_check_room(
        &self,
        pos: WorldCoordinate,
        kind: CreatureType,
        v: &mut VultureAbstractAi,
        rng: &mut UnityRandom,
    ) {
        let here = pos.room;
        let own_room = rng.value() > 0.5
            && here != v.dont_go_to_this_room
            && rng.value() < self.attraction_value(here, kind)
            && self
                .abstract_room(here)
                .is_some_and(|r| r.nodes.iter().any(|n| n.kind == NodeType::SkyExit));
        if own_room {
            v.dont_go_to_this_room = here;
            self.vulture_add_room_cluster(here, pos, kind, v, rng);
            return;
        }
        let sky = self.sky_access_nodes().to_vec();
        let total: f32 = sky
            .iter()
            .filter(|c| self.vulture_viable(c.room, kind))
            .map(|c| self.vulture_weight(c.room, kind))
            .sum();
        let mut pick = rng.value() * total;
        for c in sky.iter().filter(|c| self.vulture_viable(c.room, kind)) {
            let w = self.vulture_weight(c.room, kind);
            if pick < w {
                self.vulture_add_room_cluster(c.room, pos, kind, v, rng);
                break;
            }
            pick -= w;
        }
    }

    /// `AddRoomClusterToCheckList`: the room itself (unless the vulture is there) and,
    /// by attraction, its neighbours with sky access, each at a random sky node, ordered
    /// by a drawn priority.
    fn vulture_add_room_cluster(
        &self,
        original: i32,
        pos: WorldCoordinate,
        kind: CreatureType,
        v: &mut VultureAbstractAi,
        rng: &mut UnityRandom,
    ) {
        let Some(room) = self.abstract_room(original) else {
            return;
        };
        let template = crate::creature::CreatureTemplate::of(kind, self.mods);
        let mut list: Vec<(WorldCoordinate, i32)> = Vec::new();
        if original != pos.room
            && room.nodes.iter().any(|n| n.kind == NodeType::SkyExit)
            && self.vulture_viable(original, kind)
        {
            let node = self.random_relevant_node_by_width(original, &template, 5, rng);
            let w = 100_000 + rng.range_i32(0, 50);
            list.push((WorldCoordinate::new(original, -1, -1, node), w));
        }
        for &c in &room.connections {
            if c <= -1 || !self.vulture_viable(c, kind) {
                continue;
            }
            if rng.value() >= self.attraction_value(c, kind) * 1.5 {
                continue;
            }
            let Some(other) = self.abstract_room(c) else {
                continue;
            };
            if !other.nodes.iter().any(|n| n.kind == NodeType::SkyExit) {
                continue;
            }
            let num = rng.range_i32(
                50,
                500 + 2000 * self.size_dependent_attraction(c, kind) as i32,
            );
            let node = self.random_relevant_node_by_width(c, &template, 5, rng);
            let w = num + rng.range_i32(0, 50);
            list.push((WorldCoordinate::new(c, -1, -1, node), w));
        }
        while !list.is_empty() {
            let best = list
                .iter()
                .enumerate()
                .filter(|(_, (c, _))| c.node_defined())
                .fold(
                    None,
                    |best: Option<(usize, i32)>, (i, &(_, w))| match best {
                        Some((_, bw)) if w <= bw => best,
                        _ => Some((i, w)),
                    },
                );
            match best {
                Some((i, _)) => {
                    v.check_rooms.push(list[i].0);
                    list.remove(i);
                }
                // `list.RemoveAt(-1)` throws when no entry has a node; stop instead.
                None => break,
            }
        }
    }

    fn vulture_weight(&self, room: i32, kind: CreatureType) -> f32 {
        crate::FloatExt::mathf_pow(self.size_dependent_attraction(room, kind), 2.0)
    }

    /// `VultureAbstractAI.RoomViableRoamDestination`: not Forbidden, with a sky exit
    /// wider than 4.
    pub(crate) fn vulture_viable(&self, room: i32, kind: CreatureType) -> bool {
        if self.attraction(room, kind) == Some(RoomAttraction::Forbidden) {
            return false;
        }
        self.abstract_room(room).is_some_and(|r| {
            r.nodes
                .iter()
                .any(|n| n.kind == NodeType::SkyExit && n.entrance_width > 4)
        })
    }
}
