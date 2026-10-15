//! `FliesWorldAI`, the world process that spreads, gathers and migrates the region's
//! bat flies between rooms while they are counted per node (quantified), and the
//! quantified-creature bookkeeping of `AbstractRoom` and `World` it uses. Every random
//! draw is the game's, in its order (`rainworld-re/notes/creature-model.md` §8).
//!
//! Base game: the echo and Watcher spinning-top checks of `AddFlyToSwarmRoom` never
//! apply (no echo in a new game, Watcher off). The realized flies (`FliesRoomAI`, the
//! `Fly` body) are not ported yet (docs/DIVERGENCES.md D14).

use rw_assets::{CreatureType, NodeType};
use rw_random::UnityRandom;

use super::World;
use crate::FloatExt;
use crate::creature::{CreatureTemplate, IdIssuer, WorldCoordinate};
use crate::room_script::StorySession;

/// `FliesWorldAI.Behavior`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwarmBehavior {
    Inactive,
    Spread,
    Gather,
    Migrate,
}

impl SwarmBehavior {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Inactive => "Inactive",
            Self::Spread => "Spread",
            Self::Gather => "Gather",
            Self::Migrate => "Migrate",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [Self::Inactive, Self::Spread, Self::Gather, Self::Migrate]
            .into_iter()
            .find(|b| b.value() == value)
    }
}

/// `FliesWorldAI`'s state (its swarm room map is [`super::WorldProcesses::swarm_room_map`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FliesWorldAi {
    pub migration_targets: Vec<i32>,
    pub swarm_room_behaviors: Vec<SwarmBehavior>,
    pub flies_to_spawn: Vec<i32>,
    pub flies_to_respawn: i32,
    pub all_initial_flies_spawned: bool,
}

/// What the flies' world process reads of the game besides the world.
pub struct FliesTick<'a> {
    pub rng: &'a mut UnityRandom,
    pub ids: &'a mut IdIssuer,
    pub session: &'a StorySession,
}

impl World {
    #[must_use]
    pub fn flies_world_ai(&self) -> Option<&FliesWorldAi> {
        self.processes.flies.as_ref()
    }

    /// `RainCycle.RainApproaching` (story: the last 2400 ticks; else the last 400).
    #[must_use]
    pub fn rain_approaching(&self) -> f32 {
        let until = self.rain_cycle.time_until_rain() as f32;
        if self.story {
            until.inverse_lerp(0.0, 2400.0)
        } else {
            until.inverse_lerp(0.0, 400.0)
        }
    }

    /// `FliesWorldAI.Update`.
    pub fn update_flies_world_ai(&mut self, cx: &mut FliesTick<'_>) {
        if self.processes.flies.is_none() {
            return;
        }
        if !self.single_room() && cx.rng.value() < 1.0 {
            let n = i32::try_from(self.number_of_rooms()).unwrap_or(i32::MAX);
            let room = cx.rng.range_i32(0, n) + self.first_room_index();
            let nodes = self.abstract_room(room).map_or(0, |r| r.nodes.len());
            if nodes != 0 {
                let node = cx
                    .rng
                    .range_i32(0, i32::try_from(nodes).unwrap_or(i32::MAX));
                let to = self.migration_direction(WorldCoordinate::new(room, -1, -1, node), cx.rng);
                if to > -1 {
                    self.move_fly(room, to, cx);
                }
            }
        }
        if self
            .processes
            .flies
            .as_ref()
            .is_some_and(|f| !f.all_initial_flies_spawned)
        {
            let fac = (self.rain_cycle.timer as f32).inverse_lerp(600.0, 2800.0);
            self.start_up_update(fac, cx);
        }
        let respawn = self
            .processes
            .flies
            .as_ref()
            .map_or(0, |f| f.flies_to_respawn);
        if respawn > 0 && self.rain_cycle.time_until_rain() > 800 {
            self.respawn_fly_in_world(cx);
            if let Some(f) = &mut self.processes.flies {
                f.flies_to_respawn -= 1;
            }
        }
        if cx.rng.value() >= 0.0025 {
            return;
        }
        self.reroll_swarm_behaviors(cx.rng);
    }

    /// The rare re-roll: every swarm room gathers, the fullest spreads and the second
    /// fullest migrates to a random (active, five tries) swarm room.
    fn reroll_swarm_behaviors(&mut self, rng: &mut UnityRandom) {
        let swarms = self.data.swarm_rooms.clone();
        let len = i32::try_from(swarms.len()).unwrap_or(i32::MAX);
        let Some(flies) = &mut self.processes.flies else {
            return;
        };
        flies.swarm_room_behaviors.fill(SwarmBehavior::Gather);
        let mut skip = -1;
        for pass in 0..2 {
            let (mut most, mut pick) = (0, -1);
            for (k, &room) in swarms.iter().enumerate() {
                let k = i32::try_from(k).unwrap_or(-1);
                let count = self.number_of_quantified_creature_in_room(room, CreatureType::Fly);
                if k != skip && count > most {
                    most = count;
                    pick = k;
                }
            }
            skip = pick;
            let Ok(p) = usize::try_from(pick) else {
                continue;
            };
            let Some(flies) = &mut self.processes.flies else {
                return;
            };
            if pass == 0 {
                flies.swarm_room_behaviors[p] = SwarmBehavior::Spread;
                continue;
            }
            flies.swarm_room_behaviors[p] = SwarmBehavior::Migrate;
            flies.migration_targets[p] = rng.range_i32(0, len);
            let Some(state) = &self.region_state else {
                continue;
            };
            for _ in 0..5 {
                let target = usize::try_from(flies.migration_targets[p]).unwrap_or(0);
                if state.swarm_room_active(&self.data, target) {
                    break;
                }
                flies.migration_targets[p] = rng.range_i32(0, len);
            }
        }
    }

    /// `SwarmRoomBehavior`: Inactive for no swarm room or an inactive one, Gather while
    /// the rain approaches, else the room's own.
    fn swarm_room_behavior(&self, swarm: i32) -> SwarmBehavior {
        let room = usize::try_from(swarm)
            .ok()
            .and_then(|s| self.data.swarm_rooms.get(s).copied());
        if !room.is_some_and(|r| self.active_swarm_room(r)) {
            return SwarmBehavior::Inactive;
        }
        if self.rain_approaching() < 0.75 {
            return SwarmBehavior::Gather;
        }
        self.processes
            .flies
            .as_ref()
            .and_then(|f| {
                f.swarm_room_behaviors
                    .get(usize::try_from(swarm).ok()?)
                    .copied()
            })
            .unwrap_or(SwarmBehavior::Inactive)
    }

    /// `ClosestSwarmRoom`: the room's own when it is an active swarm room, else the swarm
    /// room with the lowest distance from the node whose behaviour is not Inactive.
    fn closest_swarm_room(&self, coord: WorldCoordinate) -> i32 {
        if self.active_swarm_room(coord.room) {
            return self
                .abstract_room(coord.room)
                .map_or(-1, |r| r.swarm_room_index);
        }
        if !coord.node_defined() {
            log::warn!("QuickConnectivity.DefineNodeOfLocalCoordinate is not ported");
        }
        let (mut best, mut result) = (f32::MAX, -1);
        for i in 0..self.data.swarm_rooms.len() {
            let k = i32::try_from(i).unwrap_or(-1);
            let d = self.swarm_room_distance(coord, i);
            if self.swarm_room_behavior(k) != SwarmBehavior::Inactive && d > -1.0 && d < best {
                best = d;
                result = k;
            }
        }
        result
    }

    /// `MigrationDirection`: the neighbour room a fly at `coord` should move to, or -1.
    fn migration_direction(&self, coord: WorldCoordinate, rng: &mut UnityRandom) -> i32 {
        let Some(room) = self.abstract_room(coord.room) else {
            return -1;
        };
        let closest = self.closest_swarm_room(coord);
        let here = self.number_of_quantified_creature_in_room(coord.room, CreatureType::Fly);
        if self.active_swarm_room(coord.room) && here < 6 {
            return -1;
        }
        let swarm = usize::try_from(closest).unwrap_or(usize::MAX);
        let mut best = f32::MAX;
        let mut result = -1;
        match self.swarm_room_behavior(closest) {
            SwarmBehavior::Gather if room.swarm_room_index != closest && closest != -1 => {
                for &c in room.connections.iter().filter(|&&c| c > -1) {
                    let at = Self::node_in_a_leading_to_b(&self.data, c, room.index);
                    let d = self.swarm_room_distance(at, swarm);
                    if d < best && d > -1.0 {
                        best = d;
                        result = c;
                    }
                }
            }
            SwarmBehavior::Spread => {
                for &c in room.connections.iter().filter(|&&c| c > -1) {
                    let there = self.number_of_quantified_creature_in_room(c, CreatureType::Fly);
                    let at = Self::node_in_a_leading_to_b(&self.data, c, room.index);
                    let score =
                        there as f32 * 20.0 + rng.value() - self.swarm_room_distance(at, swarm);
                    if score <= here as f32 && score < best {
                        best = score;
                        result = c;
                    }
                }
            }
            SwarmBehavior::Migrate => {
                let target = self
                    .processes
                    .flies
                    .as_ref()
                    .and_then(|f| f.migration_targets.get(swarm).copied())
                    .unwrap_or(-1);
                if room.swarm_room_index != target {
                    let target = usize::try_from(target).unwrap_or(usize::MAX);
                    for &c in room.connections.iter().filter(|&&c| c > -1) {
                        let at = Self::node_in_a_leading_to_b(&self.data, c, room.index);
                        let d = self.swarm_room_distance(at, target);
                        if d != -1.0 && d < best {
                            best = d;
                            result = c;
                        }
                    }
                }
            }
            _ => {}
        }
        result
    }

    /// `MoveFly`: a quantified fly of an unrealized room moves; from a realized room a
    /// realized fly would be sent out (`FliesRoomAI.GetRandomFly().AI.LeaveRoom`), and
    /// the port realizes none yet.
    fn move_fly(&mut self, from: i32, to: i32, cx: &mut FliesTick<'_>) {
        if self.realized(from).is_none() {
            self.move_quantified_creature(CreatureType::Fly, from, to, cx);
        } else {
            log::trace!("no realized fly leaves room {from}: realized flies are not ported");
        }
    }

    /// `StartUpUpdate`: while flies wait to be spawned, now and then one goes to the
    /// swarm room with the most of them left (ties by a draw per room).
    fn start_up_update(&mut self, fac: f32, cx: &mut FliesTick<'_>) {
        if (fac < 1.0 && cx.rng.value() > 0.1) || cx.rng.value() > fac {
            return;
        }
        let to_spawn = self
            .processes
            .flies
            .as_ref()
            .map(|f| f.flies_to_spawn.clone())
            .unwrap_or_default();
        let (mut best, mut pick, mut none_left) = (0.0, -1, true);
        for (i, &n) in to_spawn.iter().enumerate() {
            let score = n as f32 + cx.rng.value();
            if n > 0 {
                none_left = false;
                if score > best {
                    best = score;
                    pick = i32::try_from(i).unwrap_or(-1);
                }
            }
        }
        if let Ok(p) = usize::try_from(pick) {
            self.add_fly_to_swarm_room(p, cx);
            if let Some(f) = &mut self.processes.flies {
                f.flies_to_spawn[p] -= 1;
            }
        }
        if none_left && let Some(f) = &mut self.processes.flies {
            f.all_initial_flies_spawned = true;
        }
    }

    /// `RespawnFlyInWorld`: an active swarm room, weighted towards the emptier ones.
    fn respawn_fly_in_world(&mut self, cx: &mut FliesTick<'_>) {
        let (mut best, mut pick) = (0.0, -1);
        for (i, &room) in self.data.swarm_rooms.clone().iter().enumerate() {
            let active = self
                .region_state
                .as_ref()
                .is_none_or(|s| s.swarm_room_active(&self.data, i));
            if active {
                let count = self.number_of_quantified_creature_in_room(room, CreatureType::Fly);
                let score = cx.rng.value() / count as f32;
                if score > best {
                    best = score;
                    pick = i32::try_from(i).unwrap_or(-1);
                }
            }
        }
        if let Ok(p) = usize::try_from(pick) {
            self.add_fly_to_swarm_room(p, cx);
        }
    }

    /// `RespawnOneFly`.
    pub fn respawn_one_fly(&mut self) {
        if let Some(f) = &mut self.processes.flies {
            f.flies_to_respawn += 1;
        }
    }

    /// `AddFlyToSwarmRoom`: on a random fly-relevant node of an unrealized swarm room;
    /// in a realized one whose quantified creatures are placed, a fly is made in a hive
    /// (`FliesRoomAI.CreateFlyInHive`: the hive node's draw and the abstract creature;
    /// the realized fly is not ported); nothing while it loads.
    pub(crate) fn add_fly_to_swarm_room(&mut self, swarm: usize, cx: &mut FliesTick<'_>) {
        let Some(&room) = self.data.swarm_rooms.get(swarm) else {
            return;
        };
        let fly = CreatureTemplate::of(CreatureType::Fly, self.mods);
        if let Some(realized) = self.realized(room) {
            if realized.fully_loaded() {
                self.create_fly_in_hive(room, cx);
            }
            return;
        }
        let Some(data) = self.abstract_room(room) else {
            return;
        };
        let relevant: Vec<usize> = data
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| fly.maps_node(n.kind))
            .map(|(i, _)| i)
            .collect();
        let count = i32::try_from(relevant.len()).unwrap_or(i32::MAX);
        let specific = cx.rng.range_i32(0, count);
        let node = usize::try_from(specific)
            .ok()
            .and_then(|s| relevant.get(s))
            .map_or(-1, |&i| i32::try_from(i).unwrap_or(-1));
        self.add_quantified(room, node, CreatureType::Fly, 1, cx.rng);
    }

    /// `FliesRoomAI.CreateFlyInHive` up to the realization: a new fly in a random hive
    /// node of the room (the nodes after the exits, dens and border exits).
    fn create_fly_in_hive(&mut self, room: i32, cx: &mut FliesTick<'_>) {
        let Some(realized) = self.realized(room) else {
            return;
        };
        let hives = i32::try_from(realized.room.hives().len()).unwrap_or(0);
        if hives == 0 {
            return;
        }
        let Some(data) = self.abstract_room(room) else {
            return;
        };
        let first = data
            .nodes
            .iter()
            .position(|n| n.kind == NodeType::BatHive)
            .map_or(0, |i| i32::try_from(i).unwrap_or(0));
        let node = cx.rng.range_i32(first, first + hives);
        let id = cx.ids.new_id(-1);
        let template = CreatureTemplate::of(CreatureType::Fly, self.mods);
        let c = self.new_creature(
            template,
            WorldCoordinate::new(room, -1, -1, node),
            id,
            cx.session,
            cx.ids,
            cx.rng,
        );
        let cid = self.insert_creature(c);
        self.add_entity(cid);
        log::debug!("fly made in a hive of room {room}; realized flies are not ported");
    }

    /// `World.MoveQuantifiedCreatureFromAbstractRoom` from unrealized room `from` to `to`:
    /// one fly leaves by the exit to `to` (or, if none waits there, from the fullest node
    /// connected to it) and arrives at `to`'s exit back. Into a realized room it also
    /// becomes an `AbstractCreature` moved in (whose realization, `ChangeRooms`, is not
    /// ported: it stays abstract).
    fn move_quantified_creature(
        &mut self,
        kind: CreatureType,
        from: i32,
        to: i32,
        cx: &mut FliesTick<'_>,
    ) {
        if self.number_of_quantified_creature_in_room(from, kind) == 0 {
            return;
        }
        let exit = Self::node_in_a_leading_to_b(&self.data, from, to).node;
        if self.number_of_quantified_creature_in_node(from, kind, exit) > 0 {
            self.remove_quantified(from, exit, kind);
        } else {
            let fly = CreatureTemplate::of(CreatureType::Fly, self.mods);
            let pbi = fly.pre_baked_pathing_index();
            let Some(data) = self.abstract_room(from) else {
                return;
            };
            let (mut most, mut pick) = (0, -1);
            for i in 0..data.nodes.len() {
                let i = i32::try_from(i).unwrap_or(-1);
                let possible = exit >= 0
                    && usize::try_from(exit).is_ok_and(|e| e < data.nodes.len())
                    && data.nodes[i as usize].connection_cost(exit, pbi) > -1;
                let count = self.number_of_quantified_creature_in_node(from, kind, i);
                if i != exit && possible && count > most {
                    most = count;
                    pick = i;
                }
            }
            if pick == -1 {
                return;
            }
            self.remove_quantified(from, pick, kind);
        }
        let back = Self::node_in_a_leading_to_b(&self.data, to, from).node;
        self.add_quantified(to, back, kind, 1, cx.rng);
        if self.realized(to).is_some() {
            let id = cx.ids.new_id(-1);
            let template = CreatureTemplate::of(kind, self.mods);
            let c = self.new_creature(
                template,
                WorldCoordinate::new(from, -1, -1, exit),
                id,
                cx.session,
                cx.ids,
                cx.rng,
            );
            let cid = self.insert_creature(c);
            log::debug!("{kind} moved into realized room {to} stays abstract");
            self.move_creature(cid, WorldCoordinate::new(to, -1, -1, back));
        }
    }
}
