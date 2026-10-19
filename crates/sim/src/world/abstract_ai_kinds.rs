//! The `AbstractBehavior`s of the `AbstractCreatureAI` subclasses (their state is
//! `creature/ai_kinds.rs`): vultures (286569), cicadas (150292), big eels (139554),
//! deer (175084), Miros birds (209900) and needle worms (215613). Drop bugs and every
//! other type with an AI run the plain behaviour (`abstract_ai.rs`); the overseer's is
//! `creature/overseer`, the scavenger's is not ported (they stay in their dens).
//! A creature is never realized here (D14), so each `realizedCreature == null` test
//! passes.
//!
//! Mirrors: VultureAbstractAI.cs, CicadaAbstractAI.cs, BigEelAbstractAI.cs, DeerAbstractAI.cs,
//!   MirosBirdAbstractAI.cs, NeedleWormAbstractAI.cs, DropBugAbstractAI.cs

use rw_assets::{CreatureType, NodeType, RoomAttraction};
use rw_random::UnityRandom;

use super::abstract_path::{AbstractSpaceNodeFinder, FloodMethod, SearchingFor};
use super::{AbstractTick, World};
use crate::creature::ai_kinds::{
    BigEelAbstractAi, CicadaAbstractAi, DeerAbstractAi, MirosBirdAbstractAi, NeedleWormAbstractAi,
    VultureAbstractAi,
};
use crate::creature::{AbstractCreatureAi, AiKind, CreatureId, CreatureTemplate, WorldCoordinate};
use crate::item::Entity;

impl World {
    /// `abstractAI.AbstractBehavior(time)` by the AI's class.
    pub(super) fn kind_abstract_behavior(
        &mut self,
        id: CreatureId,
        time: i32,
        cx: &mut AbstractTick<'_>,
    ) {
        match self.ai_of(id).map(|a| &a.kind) {
            Some(AiKind::Vulture(_)) => self.vulture_behavior(id, time, cx),
            Some(AiKind::Cicada(_)) => self.cicada_behavior(id, time, cx),
            Some(AiKind::BigEel(_)) => self.big_eel_behavior(id, time, cx),
            Some(AiKind::Deer(_)) => self.deer_behavior(id, time, cx),
            Some(AiKind::MirosBird(_)) => self.miros_behavior(id, time, cx),
            Some(AiKind::NeedleWorm(_)) => self.needle_worm_behavior(id, time, cx),
            Some(AiKind::Plain | AiKind::DropBug) => self.abstract_behavior(id, time, cx),
            Some(AiKind::Overseer(_) | AiKind::Scavenger { .. }) | None => {}
        }
    }

    fn has_path(&self, id: CreatureId) -> bool {
        self.ai_of(id).is_some_and(|a| !a.path.is_empty())
    }

    fn kind_mut(&mut self, id: CreatureId) -> Option<&mut AiKind> {
        self.ai_of_mut(id).map(|a| &mut a.kind)
    }

    /// `denPosition` of creature `id`.
    fn den_of(&self, id: CreatureId) -> Option<WorldCoordinate> {
        let c = self.creature(id);
        c.ai.as_ref()
            .and_then(|a| a.den_position(c, self.offscreen_den_index()))
    }

    /// `HavePrey`: a stick from this creature to a creature it eats.
    pub(crate) fn have_prey(&self, id: CreatureId) -> bool {
        let c = self.creature(id);
        c.sticks.iter().any(|s| {
            s.a == id
                && matches!(s.b, Entity::Creature(b) if c.template.eats(self.creature(b).kind()))
        })
    }

    /// `VultureAbstractAI.AbstractBehavior`: a path is followed; else, with sky access in
    /// the world, the check rooms are visited in turn, 620 ticks each.
    fn vulture_behavior(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        if self.has_path(id) {
            self.follow_path(id, time, cx);
            return;
        }
        if self.sky_access_nodes().is_empty() {
            return;
        }
        let c = self.creature(id);
        let (pos, kind) = (c.pos, c.kind());
        let Some(ai) = &c.ai else { return };
        let destination = ai.destination;
        let AiKind::Vulture(v) = &ai.kind else { return };
        let mut v: VultureAbstractAi = v.clone();
        if v.check_rooms.is_empty() {
            self.vulture_add_random_check_room(pos, kind, &mut v, cx.rng);
        } else if destination.same_room_and_node(v.check_rooms[0]) {
            v.time_in_room += time;
            if v.time_in_room > 620 {
                v.check_rooms.remove(0);
                v.time_in_room -= 620;
            }
        } else {
            let to = v.check_rooms[0];
            self.set_destination(id, to, cx.rng);
        }
        if let Some(AiKind::Vulture(slot)) = self.kind_mut(id) {
            *slot = v;
        }
    }

    /// `CicadaAbstractAI.AbstractBehavior`: a path; prey or (a cicada A) being away from
    /// its den's room sends it home; else a 1 in 10 roll may catch a fly of a node; else,
    /// with the rain far off in a story, it heads for an active swarm room.
    fn cicada_behavior(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        if self.has_path(id) {
            self.follow_path(id, time, cx);
            return;
        }
        let c = self.creature(id);
        let (pos, kind, template) = (c.pos, c.kind(), c.template);
        let away =
            kind == CreatureType::CicadaA && self.den_of(id).is_some_and(|d| pos.room != d.room);
        if self.have_prey(id) || away {
            self.go_to_den(id, cx.rng);
            return;
        }
        if cx.rng.value() < 0.1 && cx.rng.value() > 1.0 / time as f32 {
            self.cicada_catch_fly(id, cx);
            return;
        }
        let Some(AiKind::Cicada(state)) = self.ai_of(id).map(|a| &a.kind) else {
            return;
        };
        let mut state: CicadaAbstractAi = *state;
        if state.cant_get_to_swarm_room || self.rain_cycle.time_until_rain() <= 4800 || !self.story
        {
            return;
        }
        if let Some(swarm) = state.swarm_room {
            if pos.room != swarm.room {
                self.set_destination(id, swarm, cx.rng);
            }
            return;
        }
        let start = self.define_node(pos, &template, cx.rng);
        let finder = AbstractSpaceNodeFinder {
            world: self,
            template: &template,
            searching_for: SearchingFor::SwarmRoom,
            flood_method: FloodMethod::Random,
        };
        match finder.run(start, cx.rng) {
            Some(path) => {
                let swarm = path[0];
                if let Some(ai) = self.ai_of_mut(id) {
                    ai.path = path;
                }
                state.swarm_room = Some(swarm);
                self.set_destination(id, swarm, cx.rng);
                state.cant_get_to_swarm_room = false;
            }
            None => state.cant_get_to_swarm_room = true,
        }
        if let Some(AiKind::Cicada(slot)) = self.kind_mut(id) {
            *slot = state;
        }
    }

    /// The cicada's catch: a random node of its room; when flies are counted there, one
    /// in that many, and the node is its own or reachable and back for a fly, a new
    /// abstract fly at the cicada's position is taken from the count and carried home.
    fn cicada_catch_fly(&mut self, id: CreatureId, cx: &mut AbstractTick<'_>) {
        let pos = self.creature(id).pos;
        let Some(room) = self.abstract_room(pos.room) else {
            return;
        };
        let nodes = i32::try_from(room.nodes.len()).unwrap_or(i32::MAX);
        let num = cx.rng.range_i32(0, nodes);
        let count = self.number_of_quantified_creature_in_node(pos.room, CreatureType::Fly, num);
        if count <= 0 || cx.rng.value() >= 1.0 / count as f32 {
            return;
        }
        let fly = CreatureTemplate::of(CreatureType::Fly, self.mods);
        if num != pos.node && !self.connection_and_back_possible(pos.room, pos.node, num, &fly) {
            return;
        }
        let new_id = cx.ids.new_id(-1);
        let c = self.new_creature(fly, pos, new_id, cx.session, cx.ids, cx.rng);
        let fly_id = self.insert_creature(c);
        self.add_entity(fly_id);
        self.remove_quantified(pos.room, num, CreatureType::Fly);
        self.add_grip_stick(id, Entity::Creature(fly_id), 0, true);
        self.go_to_den(id, cx.rng);
    }

    /// `BigEelAbstractAI.AbstractBehavior`: a path; else, with sea access, a sea exit to
    /// go to, kept for 1200 ticks in a room.
    fn big_eel_behavior(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        if self.has_path(id) {
            self.follow_path(id, time, cx);
            return;
        }
        if self.border_access_nodes(NodeType::SeaExit).is_empty() {
            return;
        }
        let pos = self.creature(id).pos;
        let Some(AiKind::BigEel(e)) = self.ai_of(id).map(|a| &a.kind) else {
            return;
        };
        let mut e: BigEelAbstractAi = *e;
        if e.go_to.is_none() {
            self.big_eel_add_random_check_room(id, &mut e, cx.rng);
        }
        if pos.room != e.last_room {
            e.last_room = pos.room;
            e.time_in_room = 0;
        }
        e.time_in_room += time;
        let den = self.offscreen_den_index();
        let done_single =
            self.single_room() && e.go_to.is_some_and(|g| pos.room == g.room) && pos.room == den;
        let go = if e.time_in_room > 1200 || done_single {
            e.go_to = None;
            e.time_in_room -= 1200;
            None
        } else {
            e.go_to.filter(|g| pos.room != g.room)
        };
        if let Some(AiKind::BigEel(slot)) = self.kind_mut(id) {
            *slot = e;
        }
        if let Some(g) = go {
            self.set_destination(id, g, cx.rng);
        }
    }

    /// `BigEelAbstractAI.AddRandomCheckRoom`.
    fn big_eel_add_random_check_room(
        &self,
        id: CreatureId,
        e: &mut BigEelAbstractAi,
        rng: &mut UnityRandom,
    ) {
        let pos = self.creature(id).pos;
        if self.single_room() {
            let first = self.first_room_index();
            let mut list: Vec<WorldCoordinate> = self.den_of(id).into_iter().collect();
            if let Some(room) = self.abstract_room(first) {
                for (i, n) in room.nodes.iter().enumerate() {
                    if n.kind == NodeType::SeaExit {
                        let i = i32::try_from(i).unwrap_or(-1);
                        list.push(WorldCoordinate::new(first, -1, -1, i));
                    }
                }
            }
            if !list.is_empty() {
                let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
                e.go_to = Some(list[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)]);
            }
        } else if rng.value() > 0.2 && pos.room != self.offscreen_den_index() {
            self.big_eel_add_room_cluster(id, pos.room, false, e, rng);
        } else {
            let sea = self.border_access_nodes(NodeType::SeaExit);
            if !sea.is_empty() {
                let len = i32::try_from(sea.len()).unwrap_or(i32::MAX);
                let k = usize::try_from(rng.range_i32(0, len)).unwrap_or(0);
                self.big_eel_add_room_cluster(id, sea[k].room, true, e, rng);
            }
        }
    }

    /// `BigEelAbstractAI.AddRoomClusterToCheckList`: the wide sea exits (over 20) of the
    /// room's neighbours (and the room, or the room again for an unconnected exit) that
    /// are not Forbidden and hold no big eel; one is picked.
    fn big_eel_add_room_cluster(
        &self,
        id: CreatureId,
        original: i32,
        include_original: bool,
        e: &mut BigEelAbstractAi,
        rng: &mut UnityRandom,
    ) {
        let kind = self.creature(id).kind();
        let Some(room) = self.abstract_room(original) else {
            return;
        };
        let mut list: Vec<WorldCoordinate> = Vec::new();
        let first = if include_original { -1 } else { 0 };
        let count = i32::try_from(room.connections.len()).unwrap_or(0);
        for i in first..count {
            let conn = usize::try_from(i).ok().map_or(-1, |i| room.connections[i]);
            let index = if i > -1 && conn > -1 { conn } else { original };
            if self.attraction(index, kind) == Some(RoomAttraction::Forbidden) {
                continue;
            }
            let has_eel = self
                .room_creatures(index)
                .iter()
                .any(|&c| self.creature(c).kind() == CreatureType::BigEel);
            if has_eel {
                continue;
            }
            let Some(data) = self.abstract_room(index) else {
                continue;
            };
            for (k, n) in data.nodes.iter().enumerate() {
                if n.kind == NodeType::SeaExit && n.entrance_width > 20 {
                    let k = i32::try_from(k).unwrap_or(-1);
                    list.push(WorldCoordinate::new(index, -1, -1, k));
                }
            }
        }
        if !list.is_empty() {
            let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
            e.go_to = Some(list[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)]);
        }
    }

    /// `DeerAbstractAI.AbstractBehavior` (Remix's deer behaviour off): a path; home at
    /// the rain; else to one of the allowed LF side exits, kept for 1200 ticks.
    fn deer_behavior(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        if self.has_path(id) {
            self.follow_path(id, time, cx);
            return;
        }
        let c = self.creature(id);
        let (pos, night, ignore) = (c.pos, c.night_creature, c.ignore_cycle);
        let Some(AiKind::Deer(d)) = self.ai_of(id).map(|a| &a.kind) else {
            return;
        };
        let mut d: DeerAbstractAi = d.clone();
        if self.rain_cycle.time_until_rain() < 800 && !night && !ignore {
            if self
                .den_of(id)
                .is_none_or(|den| !pos.same_room_and_node(den))
            {
                self.go_to_den(id, cx.rng);
            }
            return;
        }
        let mut destination = None;
        if d.damage_go_home {
            let den = WorldCoordinate::new(self.offscreen_den_index(), -1, -1, 0);
            if let Some(ai) = self.ai_of_mut(id) {
                ai.den_position = Some(den);
            }
            if pos.room == den.room {
                d.damage_go_home = false;
            }
            self.store_deer(id, d);
            self.go_to_den(id, cx.rng);
            return;
        }
        if let Some(spore) = d.spore_pos {
            self.set_destination(id, spore, cx.rng);
            if spore.same_room_and_node(pos) {
                d.spore_pos = None;
            }
            self.store_deer(id, d);
            return;
        }
        if d.allowed_nodes.is_empty() {
            return;
        }
        if d.go_to.is_none() {
            self.deer_roam_to_random_room(&mut d, cx.rng);
        }
        if pos.room != d.last_room {
            d.last_room = pos.room;
            d.time_in_room = 0;
        }
        d.time_in_room += time;
        if d.time_in_room > 1200 {
            d.go_to = None;
            d.time_in_room -= 1200;
        } else if let Some(g) = d.go_to
            && pos.room != g.room
        {
            destination = Some(g);
        }
        self.store_deer(id, d);
        if let Some(g) = destination {
            self.set_destination(id, g, cx.rng);
        }
    }

    fn store_deer(&mut self, id: CreatureId, d: DeerAbstractAi) {
        if let Some(AiKind::Deer(slot)) = self.kind_mut(id) {
            *slot = d;
        }
    }

    /// `DeerAbstractAI.RoamToRandomRoom`: an allowed node, unless a deer is in its room.
    fn deer_roam_to_random_room(&self, d: &mut DeerAbstractAi, rng: &mut UnityRandom) {
        let len = i32::try_from(d.allowed_nodes.len()).unwrap_or(i32::MAX);
        let pick = d.allowed_nodes[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)];
        let occupied = self
            .room_creatures(pick.room)
            .iter()
            .any(|&c| self.creature(c).kind() == CreatureType::Deer);
        if !occupied {
            d.go_to = Some(pick);
        }
    }

    fn miros(&self, id: CreatureId) -> Option<&MirosBirdAbstractAi> {
        match self.ai_of(id).map(|a| &a.kind) {
            Some(AiKind::MirosBird(m)) => Some(m),
            _ => None,
        }
    }

    fn miros_mut(&mut self, id: CreatureId) -> Option<&mut MirosBirdAbstractAi> {
        match self.kind_mut(id) {
            Some(AiKind::MirosBird(m)) => Some(m),
            _ => None,
        }
    }

    /// `MirosBirdAbstractAI.AbstractBehavior`: a waiting bird counts down; a path; home
    /// at the rain; else back to the off-screen den, where it may lead a raid.
    fn miros_behavior(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        if let Some(m) = self.miros_mut(id)
            && m.wait > 0
        {
            m.wait -= time;
            return;
        }
        if self.has_path(id) {
            self.follow_path(id, time, cx);
            return;
        }
        let pos = self.creature(id).pos;
        let den = self.den_of(id);
        if self.rain_cycle.time_until_rain() < 800 {
            if den.is_none_or(|d| !pos.same_room_and_node(d)) {
                self.go_to_den(id, cx.rng);
            }
            return;
        }
        if self.miros(id).is_none_or(|m| m.allowed_nodes.is_empty()) {
            return;
        }
        if let Some(den) = den {
            self.set_destination(id, den, cx.rng);
        }
        if self.creature(id).pos.room == self.offscreen_den_index() {
            self.miros_raid(id, cx.rng);
        }
    }

    /// A Miros bird free to raid (`wait < 1`; `CanTravelTogether` holds without named
    /// attractions).
    fn miros_ready(&self, other: CreatureId) -> bool {
        self.creature(other).kind() == CreatureType::MirosBird
            && self.miros(other).is_some_and(|m| m.wait < 1)
    }

    /// `MirosBirdAbstractAI.Raid`: the most dominant ready bird of the den, when at least
    /// three are ready or all the world's birds are, leads 2 to 4 of them (itself among
    /// them) to an allowed side exit by way of another wide side exit of that room.
    fn miros_raid(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        let (pos, dominance) = (c.pos, c.personality.dominance);
        let here: Vec<CreatureId> = self.room_creatures(pos.room).to_vec();
        let mut num = 0;
        for &o in &here {
            if self.miros_ready(o) {
                if self.creature(o).personality.dominance > dominance {
                    return;
                }
                num += 1;
            }
        }
        if num < 3 {
            let first = self.first_room_index();
            let total = (0..self.number_of_rooms())
                .flat_map(|i| self.room_creatures(first + i32::try_from(i).unwrap_or(0)))
                .filter(|&&o| self.creature(o).kind() == CreatureType::MirosBird)
                .count();
            if num < total {
                return;
            }
        }
        let Some(allowed) = self.miros(id).map(|m| m.allowed_nodes.clone()) else {
            return;
        };
        if allowed.is_empty() {
            return;
        }
        let len = i32::try_from(allowed.len()).unwrap_or(i32::MAX);
        let target = allowed[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)];
        let template = self.creature(id).template;
        let mut via = WorldCoordinate::new(target.room, -1, -1, -1);
        if let Some(room) = self.abstract_room(target.room) {
            for (k, n) in room.nodes.iter().enumerate() {
                if via.node_defined() {
                    break;
                }
                let k = i32::try_from(k).unwrap_or(-1);
                if k != target.node
                    && n.kind == NodeType::SideExit
                    && n.entrance_width >= 5
                    && self.connection_possible(target.room, target.node, k, &template)
                {
                    via.node = k;
                }
            }
        }
        let den = self.den_of(id).unwrap_or(pos);
        let mut path = vec![den];
        if via.node_defined() {
            path.push(via);
        }
        path.push(target);
        self.set_destination(id, pos, rng);
        if let Some(ai) = self.ai_of_mut(id) {
            ai.path.clone_from(&path);
        }
        let mut num = rng.range_i32(2, 5);
        for &o in &here {
            if num <= 0 {
                break;
            }
            if !self.miros_ready(o) {
                continue;
            }
            self.set_destination(o, pos, rng);
            if let Some(ai) = self.ai_of_mut(o) {
                ai.path.clone_from(&path);
            }
            let inner = rng.range_i32(0, 50);
            let wait = rng.range_i32(0, inner);
            if let Some(m) = self.miros_mut(o) {
                m.wait = wait;
            }
            num -= 1;
        }
    }

    /// `NeedleWormAbstractAI.AbstractBehavior`: the small worm follows its mother (or
    /// looks for one in a random room) and migrates; the big one migrates; then the
    /// migrate-active flag flips every 1200 to 3000 ticks, and a path is followed.
    fn needle_worm_behavior(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        let c = self.creature(id);
        let (kind, pos) = (c.kind(), c.pos);
        let Some(AiKind::NeedleWorm(w)) = self.ai_of(id).map(|a| &a.kind) else {
            return;
        };
        let mut w: NeedleWormAbstractAi = *w;
        let like = !matches!(
            self.attraction(pos.room, kind),
            Some(RoomAttraction::Forbidden | RoomAttraction::Avoid)
        );
        if kind == CreatureType::SmallNeedleWorm {
            if let Some(mother) = w.mother {
                if let Some(ai) = self.ai_of_mut(id) {
                    ai.follow_creature = Some(mother);
                }
                self.move_with_creature(id, mother, false, cx.rng);
                if !self.creature(mother).state.alive
                    && AbstractCreatureAi::time_influenced_roll(0.001_666_666_7, time, cx.rng)
                {
                    w.mother = None;
                    if let Some(ai) = self.ai_of_mut(id) {
                        ai.follow_creature = None;
                    }
                }
            } else {
                let first = self.first_room_index();
                let rooms = i32::try_from(self.number_of_rooms()).unwrap_or(i32::MAX);
                let room = cx.rng.range_i32(first, first + rooms);
                let den = self.den_of(id);
                if let Some(m) = self.find_needle_mother(room, pos, den, cx.rng) {
                    w.mother = Some(m);
                }
                self.store_needle_worm(id, w);
                if w.migrate_active || !like {
                    self.migration_behavior(id, time, cx.rng);
                }
            }
        } else if !self.single_room() && (w.migrate_active || !like) {
            self.migration_behavior(id, time, cx.rng);
        }
        w.migrate_active_counter -= time;
        if w.migrate_active_counter < 0 {
            w.migrate_active_counter = cx.rng.range_i32(1200, 3000);
            w.migrate_active = !w.migrate_active;
        }
        let follows = self.ai_of(id).is_some_and(|a| a.follow_creature.is_some());
        if follows && !w.temp_suppress_follow_creature {
            w.migrate_active = true;
        }
        w.temp_suppress_follow_creature = false;
        self.store_needle_worm(id, w);
        if self.has_path(id) {
            self.follow_path(id, time, cx);
        }
    }

    fn store_needle_worm(&mut self, id: CreatureId, w: NeedleWormAbstractAi) {
        if let Some(AiKind::NeedleWorm(slot)) = self.kind_mut(id) {
            *slot = w;
        }
    }
}

#[cfg(test)]
#[path = "abstract_ai_kinds_tests.rs"]
mod tests;
