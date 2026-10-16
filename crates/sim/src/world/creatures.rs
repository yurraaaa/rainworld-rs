//! The world's creatures: the arena, each abstract room's `creatures` and
//! `entitiesInDens` lists, the room attractions, and what the game does with them
//! (`AbstractCreature` construction, `Move`, dens, `AbstractRoom.Update`, realizing an
//! overseer in a loaded room and abstracting it again).
//!
//! A room's `entities` (creatures and objects, [`Entity`]) are kept beside its creature
//! lists; the objects are `world/objects.rs`'.

use std::collections::HashMap;

use rw_assets::{CreatureType, NodeType, RoomAttraction, Timeline};
use rw_random::UnityRandom;

use super::{FliesWorldAi, OverseersWorldAi, RealizedRoom, SwarmRoomMap, World};
use crate::FloatExt;
use crate::creature::ai::AiContext;
use crate::creature::overseer::{AbstractStep, GuideContext, OverseerView, Parent};
use crate::creature::{
    AbstractCreature, AbstractCreatureAi, AiKind, CreatureId, CreatureTemplate, EntityId, IdIssuer,
    OverseerAbstractAi, OverseerAi, OverseerBody, Personality, PlayerGuideState, RealizedCreature,
    WorldCoordinate,
};
use crate::item::Entity;
use crate::room_script::StorySession;

/// One abstract room's lists.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RoomLists {
    /// `entities`: creatures and objects out of dens, in the order they joined.
    pub entities: Vec<Entity>,
    pub creatures: Vec<CreatureId>,
    pub in_dens: Vec<CreatureId>,
    pub even_update: bool,
    /// `quantifiedCreatures`: per node and type, a count.
    pub quantified: Vec<QuantifiedCount>,
}

impl RoomLists {
    /// `entities.Add` unless it is there.
    pub fn add_entity(&mut self, e: Entity) {
        if !self.entities.contains(&e) {
            self.entities.push(e);
        }
    }

    pub fn remove_entity(&mut self, e: Entity) {
        self.entities.retain(|&x| x != e);
    }

    /// Out of `entities` and `creatures` (`RemoveEntity`, `MoveEntityToDen`).
    pub fn remove_creature(&mut self, id: CreatureId) {
        self.remove_entity(Entity::Creature(id));
        self.creatures.retain(|&c| c != id);
    }
}

/// Quantified creatures of one type at one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantifiedCount {
    pub node: i32,
    pub kind: CreatureType,
    pub count: i32,
}

/// The world processes: the overseers' and the flies' are ported; the scavengers' is a
/// marker (its update is not ported, docs/DIVERGENCES.md D14).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldProcesses {
    /// `overseersWorldAI`, made by the first overseer's abstract AI.
    pub overseers: Option<OverseersWorldAi>,
    pub scavengers: bool,
    /// `FliesWorldAI` (made with the world, after the population).
    pub flies: Option<FliesWorldAi>,
    /// `FliesWorldAI.swarmRoomMap` ([`World::map_swarm_rooms`]).
    pub swarm_room_map: Option<SwarmRoomMap>,
}

/// What an abstract update reads of the game.
pub struct AbstractTick<'a> {
    pub rng: &'a mut UnityRandom,
    pub guide: &'a PlayerGuideState,
    pub player: Option<CreatureId>,
    pub player_karma: i32,
    /// `saveState.cycleNumber`.
    pub cycle: i32,
}

impl World {
    /// Builds the per-room attraction tables from the region's `Room_Attr` lines
    /// (`LoadMapConfig`), and the sky access nodes (`MapBorderConnections`).
    pub(super) fn index_rooms(&mut self) {
        let props = &self.data.properties;
        self.attractions = self
            .data
            .rooms
            .iter()
            .map(|room| {
                let mut table = HashMap::new();
                for line in props.attractions_of(&room.name) {
                    for &(kind, a) in &line.by_type {
                        table.insert(kind, a);
                    }
                }
                table
            })
            .collect();
        self.sky_access_nodes = self
            .data
            .rooms
            .iter()
            .filter(|r| !self.data.disabled_map_rooms.contains(&r.name))
            .flat_map(|r| {
                r.nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.kind == NodeType::SkyExit)
                    .map(|(j, _)| {
                        WorldCoordinate::new(r.index, -1, -1, i32::try_from(j).unwrap_or(-1))
                    })
            })
            .collect();
    }

    /// The creature `id` (panics on an index the world did not hand out).
    #[must_use]
    pub fn creature(&self, id: CreatureId) -> &AbstractCreature {
        &self.creatures[id.0]
    }

    pub fn creature_mut(&mut self, id: CreatureId) -> &mut AbstractCreature {
        &mut self.creatures[id.0]
    }

    /// Every creature ever made, with its id (destroyed ones are `slated_for_deletion`).
    pub fn creatures(&self) -> impl Iterator<Item = (CreatureId, &AbstractCreature)> {
        self.creatures
            .iter()
            .enumerate()
            .map(|(i, c)| (CreatureId(i), c))
    }

    /// `AbstractRoom.creatures` of global room `room`.
    #[must_use]
    pub fn room_creatures(&self, room: i32) -> &[CreatureId] {
        self.local(room)
            .map_or(&[][..], |l| &self.lists[l].creatures[..])
    }

    /// `AbstractRoom.entitiesInDens` (creatures only).
    #[must_use]
    pub fn room_dens(&self, room: i32) -> &[CreatureId] {
        self.local(room)
            .map_or(&[][..], |l| &self.lists[l].in_dens[..])
    }

    /// `quantifiedCreatures` of global room `room`.
    #[must_use]
    pub fn room_quantified(&self, room: i32) -> &[QuantifiedCount] {
        self.local(room)
            .map_or(&[][..], |l| &self.lists[l].quantified[..])
    }

    /// `world.offScreenDen.index`.
    #[must_use]
    pub fn offscreen_den_index(&self) -> i32 {
        self.data
            .rooms
            .iter()
            .find(|r| r.offscreen_den)
            .map_or(-1, |r| r.index)
    }

    /// `world.region.name`; `None` in a single-room world.
    #[must_use]
    pub fn region_name(&self) -> Option<&str> {
        self.data.region.as_ref().map(|r| r.name.as_str())
    }

    #[must_use]
    pub fn properties(&self) -> &rw_assets::region_properties::RegionProperties {
        &self.data.properties
    }

    #[must_use]
    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    #[must_use]
    pub fn sky_access_nodes(&self) -> &[WorldCoordinate] {
        &self.sky_access_nodes
    }

    #[must_use]
    pub fn processes(&self) -> &WorldProcesses {
        &self.processes
    }

    /// `AbstractRoom.AttractionForCreature(type)`; `None` for the game's null (no entry
    /// and no default), which counts as Neutral.
    #[must_use]
    pub fn attraction(&self, room: i32, kind: CreatureType) -> Option<RoomAttraction> {
        self.attraction_at_depth(room, kind, 0)
    }

    fn attraction_at_depth(
        &self,
        room: i32,
        kind: CreatureType,
        depth: u32,
    ) -> Option<RoomAttraction> {
        let data = self.abstract_room(room)?;
        if data.shelter() || data.gate() {
            return Some(RoomAttraction::Forbidden);
        }
        let l = self.local(room)?;
        if let Some(&a) = self.attractions[l].get(&kind) {
            return Some(a);
        }
        let default = self
            .data
            .properties
            .default_attractions
            .iter()
            .rev()
            .find(|(name, _)| CreatureType::from_value(name, self.mods) == Some(kind))?;
        self.resolve_default(room, &default.1, depth + 1)
    }

    /// `ResolveDefaultAttractionString` (named attractions only through the defaults:
    /// no vanilla creature has a `namedAttraction`).
    fn resolve_default(&self, room: i32, value: &str, depth: u32) -> Option<RoomAttraction> {
        if depth > 100 {
            return None;
        }
        if let Some(a) = RoomAttraction::registered(value) {
            return Some(a);
        }
        if let Some(kind) = CreatureType::from_value(value, self.mods) {
            return self.attraction_at_depth(room, kind, depth);
        }
        let data = self.abstract_room(room)?;
        if data.shelter() || data.gate() {
            return Some(RoomAttraction::Forbidden);
        }
        let named = self
            .data
            .properties
            .default_attractions
            .iter()
            .rev()
            .find(|(name, _)| name == value)?;
        self.resolve_default(room, &named.1, depth + 1)
    }

    /// `AttractionValueForCreature` (null is Neutral).
    #[must_use]
    pub fn attraction_value(&self, room: i32, kind: CreatureType) -> f32 {
        self.attraction(room, kind)
            .unwrap_or(RoomAttraction::Neutral)
            .value()
    }

    /// `SizeDependentAttractionValueForCreature`.
    #[must_use]
    pub fn size_dependent_attraction(&self, room: i32, kind: CreatureType) -> f32 {
        let Some(data) = self.abstract_room(room) else {
            return 0.0;
        };
        let (w, h) = (data.size.0 as f32, data.size.1 as f32);
        let value = self.attraction_value(room, kind);
        let fac = if CreatureTemplate::of(kind, self.mods).can_fly {
            w * h / 2000.0
        } else {
            w * w.lerp(1.0, 0.5) * h / 50000.0
        };
        value * fac.lerp(1.0, 0.985)
    }

    /// `RandomRelevantNodeAccountingForEntranceWidth`.
    pub fn random_relevant_node_by_width(
        &self,
        room: i32,
        template: &CreatureTemplate,
        min_width: i32,
        rng: &mut UnityRandom,
    ) -> i32 {
        let Some(data) = self.abstract_room(room) else {
            return -1;
        };
        let relevant: Vec<usize> = data
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| template.maps_node(n.kind))
            .map(|(i, _)| i)
            .collect();
        let mut list: Vec<usize> = (0..relevant.len()).collect();
        while !list.is_empty() {
            let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
            let k = usize::try_from(rng.range_i32(0, len)).unwrap_or(0);
            let node = relevant[list[k]];
            let n = &data.nodes[node];
            if matches!(n.kind, NodeType::SkyExit | NodeType::SideExit)
                && n.entrance_width < min_width
            {
                list.remove(k);
                continue;
            }
            return i32::try_from(node).unwrap_or(-1);
        }
        -1
    }

    /// `NeedleWormAbstractAI.FindMotherInRoom` for a small worm at `pos` (its
    /// `denPosition` is `den`), over the room's creatures then its dens.
    pub(crate) fn find_needle_mother(
        &self,
        pos: WorldCoordinate,
        den: Option<WorldCoordinate>,
        rng: &mut UnityRandom,
    ) -> Option<CreatureId> {
        let mut best = f32::MIN;
        let mut mother = None;
        for &id in self
            .room_creatures(pos.room)
            .iter()
            .chain(self.room_dens(pos.room))
        {
            let c = self.creature(id);
            if c.kind() != CreatureType::BigNeedleWorm || !c.state.alive {
                continue;
            }
            let mut score = rng.value();
            if c.pos == pos {
                score += 1.0;
            }
            let their_den =
                c.ai.as_ref()
                    .and_then(|a| a.den_position(c, self.offscreen_den_index()));
            if their_den.is_some() && den.is_some() && their_den == den {
                score += 1.0;
            }
            if score > best {
                best = score;
                mother = Some(id);
            }
        }
        mother
    }

    /// `new AbstractCreature(world, template, null, pos, id)`: personality, state, the
    /// abstract AI with its draws, then the den counter (`Range(100, 1000)` in a den of
    /// a real room). Not yet in any room list.
    pub(crate) fn new_creature(
        &self,
        template: CreatureTemplate,
        pos: WorldCoordinate,
        id: EntityId,
        session: &StorySession,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) -> AbstractCreature {
        let mut c = AbstractCreature {
            template,
            id,
            pos,
            personality: Personality::new(id, rng),
            state: AbstractCreature::state_for(&template),
            ai: None,
            remain_in_den_counter: -1,
            spawn_den: None,
            spawn_data: None,
            night_creature: false,
            ignore_cycle: false,
            in_den: false,
            time_spent_here: 0,
            even_update: false,
            distance_to_my_node: -1,
            slated_for_deletion: false,
            realized: None,
            carried: Vec::new(),
            grasps: Vec::new(),
            grabbed_by: Vec::new(),
            sticks: Vec::new(),
            saved_state: None,
        };
        let room = self.abstract_room(pos.room);
        if template.top_ancestor() == CreatureType::GarbageWorm {
            c.remain_in_den_counter = 0;
            if let Some(r) = room
                && let Some(i) = r
                    .nodes
                    .iter()
                    .position(|n| n.kind == NodeType::GarbageHoles)
            {
                c.pos.node = i32::try_from(i).unwrap_or(-1);
            }
        }
        if template.ai {
            let mut cx = AiContext {
                world: self,
                session,
                ids,
                rng: &mut *rng,
            };
            c.ai = Some(AbstractCreatureAi::new(&mut c, &mut cx));
        }
        // The den check reads the constructor's `pos` argument, not the moved field.
        if let Some(room) = room {
            let in_den = usize::try_from(pos.node)
                .ok()
                .and_then(|n| room.nodes.get(n))
                .is_some_and(|n| n.kind == NodeType::Den);
            if in_den && !pos.tile_defined() {
                c.remain_in_den_counter = if room.offscreen_den {
                    1
                } else {
                    rng.range_i32(100, 1000)
                };
                if let Some(ai) = &mut c.ai {
                    ai.den_position = Some(pos);
                }
                c.spawn_den = Some(pos);
            }
        } else {
            log::warn!("{} had invalid room", template.kind);
        }
        if template.kind == CreatureType::TentaclePlant || template.kind == CreatureType::PoleMimic
        {
            c.remain_in_den_counter = 0;
        }
        c
    }

    /// Puts a new creature into the arena (in no list yet), making the world processes
    /// its AI constructor would make.
    pub(crate) fn insert_creature(&mut self, c: AbstractCreature) -> CreatureId {
        match c.ai.as_ref().map(|a| &a.kind) {
            Some(AiKind::Overseer(_)) => {
                if self.processes.overseers.is_none() {
                    self.processes.overseers = Some(OverseersWorldAi::new(&self.data));
                }
            }
            Some(AiKind::Scavenger { .. }) => self.processes.scavengers = true,
            _ => {}
        }
        self.creatures.push(c);
        CreatureId(self.creatures.len() - 1)
    }

    /// `AbstractRoom.AddEntity`.
    pub(crate) fn add_entity(&mut self, id: CreatureId) {
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room) {
            let lists = &mut self.lists[l];
            lists.add_entity(Entity::Creature(id));
            if !lists.creatures.contains(&id) {
                lists.creatures.push(id);
            }
        }
    }

    /// `entitiesInDens.Add` without `IsEnteringDen` (the overseers `GeneratePopulation`
    /// puts in the off-screen den).
    pub(crate) fn add_to_dens(&mut self, id: CreatureId) {
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room) {
            self.lists[l].in_dens.push(id);
        }
    }

    /// `AbstractRoom.MoveEntityToDen` (the den is the creature's position: no move).
    pub(crate) fn move_entity_to_den(&mut self, id: CreatureId) {
        self.creature_mut(id).in_den = true;
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room) {
            let lists = &mut self.lists[l];
            lists.remove_creature(id);
            if !lists.in_dens.contains(&id) {
                lists.in_dens.push(id);
            }
        }
    }

    /// `AbstractRoom.RemoveEntity`: out of its room's lists.
    pub(crate) fn remove_entity(&mut self, id: CreatureId) {
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room) {
            self.lists[l].remove_creature(id);
        }
    }

    /// `OpportunityToEnterDen` of a fly's vessel at a den node: it goes in when it wants
    /// to stay there for the rest of the cycle or still has to stay in. `MoveEntityToDen`
    /// then runs the Fly's `IsEnteringDen` (`RespawnOneFly`, `RemoveEntity`, no `InDen`)
    /// and keeps it in the room's den list. Returns whether it went in.
    pub(crate) fn fly_opportunity_to_enter_den(&mut self, id: CreatureId) -> bool {
        let time_until_rain = self.rain_cycle.time_until_rain();
        let c = self.creature_mut(id);
        if c.wants_to_stay_in_den(time_until_rain) {
            c.remain_in_den_counter = -1;
        } else if c.remain_in_den_counter <= 0 {
            return false;
        }
        self.respawn_one_fly();
        self.remove_entity(id);
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room)
            && !self.lists[l].in_dens.contains(&id)
        {
            self.lists[l].in_dens.push(id);
        }
        true
    }

    /// `AbstractRoom.MoveEntityOutOfDen` (`IsExitingDen`: the room is not realized, or
    /// the creature stays abstract, see [`Self::move_creature`]).
    fn move_entity_out_of_den(&mut self, id: CreatureId) {
        let c = self.creature_mut(id);
        c.in_den = false;
        c.time_spent_here = 0;
        let room = c.pos.room;
        if self.realized(room).is_some() {
            log::debug!(
                "{} leaves its den into a realized room; it stays abstract",
                self.creature(id).kind()
            );
        }
        if let Some(l) = self.local(room) {
            self.lists[l].in_dens.retain(|&c| c != id);
        }
        self.add_entity(id);
    }

    /// `AbstractRoom.AddQuantifiedCreature(node, type, amount)` (node -1 draws one of the
    /// room's connections).
    pub(crate) fn add_quantified(
        &mut self,
        room: i32,
        node: i32,
        kind: CreatureType,
        amount: i32,
        rng: &mut UnityRandom,
    ) {
        let Some(data) = self.abstract_room(room) else {
            return;
        };
        if data.offscreen_den {
            return;
        }
        let mut node = node;
        if node == -1 {
            if data.connections.is_empty() {
                return;
            }
            let n = i32::try_from(data.connections.len()).unwrap_or(i32::MAX);
            node = rng.range_i32(0, n);
        }
        self.add_quantified_amount(room, node, kind, amount);
    }

    /// `AddQuantifiedCreature(node, type)` with a defined node (no draw).
    pub(crate) fn add_quantified_at(&mut self, room: i32, node: i32, kind: CreatureType) {
        self.add_quantified_amount(room, node, kind, 1);
    }

    fn add_quantified_amount(&mut self, room: i32, node: i32, kind: CreatureType, amount: i32) {
        let Some(l) = self.local(room) else { return };
        let list = &mut self.lists[l].quantified;
        match list.iter_mut().find(|q| q.node == node && q.kind == kind) {
            Some(q) => q.count += amount,
            None => list.push(QuantifiedCount {
                node,
                kind,
                count: amount,
            }),
        }
    }

    /// `AbstractRoom.RemoveQuantifiedCreature`: one fewer at the node when
    /// `NumberOfQuantifiedCreatureInNode` sees any (it takes from the node's count even
    /// where that number counts creatures).
    pub(crate) fn remove_quantified(&mut self, room: i32, node: i32, kind: CreatureType) {
        if self.number_of_quantified_creature_in_node(room, kind, node) <= 0 {
            log::warn!("Couldn't remove quantified creature! Nowhere to draw from!");
            return;
        }
        let Some(l) = self.local(room) else { return };
        let list = &mut self.lists[l].quantified;
        match list.iter_mut().find(|q| q.node == node && q.kind == kind) {
            Some(q) => q.count -= 1,
            None => list.push(QuantifiedCount {
                node,
                kind,
                count: -1,
            }),
        }
    }

    /// Sets a creature's position as the player's body reports it (no `Move` hooks; a
    /// new room moves it between the room lists, as `ChangeRooms`).
    pub(crate) fn place_creature(&mut self, id: CreatureId, to: WorldCoordinate) {
        let from = self.creature(id).pos;
        self.creature_mut(id).pos = to;
        if from.room != to.room {
            if let Some(l) = self.local(from.room) {
                self.lists[l].remove_creature(id);
            }
            self.add_entity(id);
        }
    }

    /// `AbstractCreature.Move(newCoord)`: `ChangeRooms` moves it between the room lists
    /// (realizing it first is [`Self::change_rooms_realize`]).
    pub(crate) fn move_creature(&mut self, id: CreatureId, mut to: WorldCoordinate) {
        let from = self.creature(id).pos;
        if let Some(ai) = &mut self.creature_mut(id).ai
            && to.room != from.room
        {
            ai.last_room = from.room;
        }
        if !to.same_room_and_node(from) {
            self.creature_mut(id).time_spent_here = 0;
            if to.room != from.room {
                if let Some(l) = self.local(from.room) {
                    let lists = &mut self.lists[l];
                    lists.remove_creature(id);
                    lists.in_dens.retain(|&c| c != id);
                }
                self.creature_mut(id).pos.room = to.room;
                self.add_entity(id);
            }
            if !to.tile_defined() && from.room == to.room {
                to.tile = from.tile;
            }
            self.creature_mut(id).pos = to;
        }
        self.moved(id);
    }

    /// `AbstractCreatureAI.Moved` (and the overseer's, whose guide case needs an angry
    /// guide: not in a new game).
    fn moved(&mut self, id: CreatureId) {
        let pos = self.creature(id).pos;
        if let Some(ai) = &mut self.creature_mut(id).ai
            && ai.migration_destination.is_some_and(|m| m.room == pos.room)
        {
            ai.migration_destination = None;
        }
    }

    /// `AbstractRoom.Update(timePassed)` of global room `room`: its entities not yet
    /// updated this round (an object only counts `timeSpentHere`), then
    /// `UpdateCreaturesInDens`.
    pub fn update_abstract_room(&mut self, room: i32, time: i32, cx: &mut AbstractTick<'_>) {
        let Some(l) = self.local(room) else { return };
        let even = !self.lists[l].even_update;
        self.lists[l].even_update = even;
        'restart: loop {
            let mut i = 0;
            while let Some(&e) = self.lists[l].entities.get(i) {
                if self.entity_even_update(e) != even {
                    let before = self.entity_pos(e).room;
                    match e {
                        Entity::Creature(id) => {
                            self.creature_mut(id).even_update = even;
                            self.update_creature(id, time, cx);
                        }
                        Entity::Object(id) => {
                            let o = self.object_mut(id);
                            o.even_update = even;
                            o.time_spent_here += time;
                        }
                    }
                    let list = &self.lists[l].entities;
                    let changed = list.get(i).is_none_or(|&x| {
                        self.entity_pos(x).room != before || self.entity_in_den(x)
                    });
                    if changed {
                        continue 'restart;
                    }
                }
                i += 1;
            }
            break;
        }
        let mut n = self.lists[l].in_dens.len();
        while n > 0 {
            n -= 1;
            if let Some(&id) = self.lists[l].in_dens.get(n) {
                self.in_den_update(id, time);
            }
        }
    }

    /// `AbstractCreature.Update(time)`: the abstract AI of a living, unrealized creature
    /// out of its den.
    fn update_creature(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        let c = self.creature_mut(id);
        c.time_spent_here += time;
        if c.in_den || c.realized.is_some() || !c.state.alive {
            return;
        }
        let (pos, ignore_cycle) = (c.pos, c.ignore_cycle);
        let Some(mut ai) = c.ai.take() else { return };
        ai.time_buffer += time;
        let mut step = AbstractStep::Stay;
        if self.realized(pos.room).is_none()
            && let AiKind::Overseer(mut overseer) = std::mem::replace(&mut ai.kind, AiKind::Plain)
        {
            let parent = Parent {
                id,
                pos,
                realized: false,
                ignore_cycle,
                tutorial: overseer.has_tutorial(),
            };
            let mut guide = GuideContext {
                world: self,
                rain: &self.rain_cycle,
                rng: &mut *cx.rng,
                guide: cx.guide,
                player: cx.player,
                player_karma: cx.player_karma,
            };
            step = overseer.abstract_behavior(&mut ai, parent, time, &mut guide);
            ai.kind = AiKind::Overseer(overseer);
        }
        self.creature_mut(id).ai = Some(ai);
        if let AbstractStep::Move(to) = step {
            self.change_rooms_realize(id, to, cx.cycle, cx.rng);
            self.move_creature(id, to);
        }
    }

    /// `AbstractCreature.ChangeRooms`' first part: an unrealized creature moving into a
    /// realized room with its shortcuts ready is realized there and waits in the
    /// shortcut handler's lobby (`CreatureEnterFromAbstractRoom`, a teleportation vessel
    /// for an overseer). Overseers and flies are realized; other types stay abstract.
    pub(crate) fn change_rooms_realize(
        &mut self,
        id: CreatureId,
        to: WorldCoordinate,
        cycle: i32,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        if c.is_realized()
            || to.room == c.pos.room
            || !self
                .realized(to.room)
                .is_some_and(RealizedRoom::shortcuts_ready)
        {
            return;
        }
        let kind = c.kind();
        if kind == CreatureType::Fly {
            self.realize_fly(id, rng);
        } else if !self.realize_overseer_parts(id, cycle, rng) {
            log::warn!("{kind} entering realized room {} stays abstract", to.room);
            return;
        }
        match &mut self.creature_mut(id).realized {
            Some(RealizedCreature::Overseer(body)) => body.in_shortcut = true,
            Some(RealizedCreature::Fly(fly)) => fly.in_shortcut = true,
            _ => {}
        }
        self.entering.push((id, to));
    }

    /// `Realize` of an overseer: the `Overseer` (no draws) and its `OverseerAI` (one
    /// draw). Other types are not realized.
    fn realize_overseer_parts(
        &mut self,
        id: CreatureId,
        cycle: i32,
        rng: &mut UnityRandom,
    ) -> bool {
        let c = self.creature(id);
        if c.kind() != CreatureType::Overseer || c.is_realized() {
            return false;
        }
        let guide = c.overseer_ai().is_some_and(|a| a.is_player_guide);
        let body = OverseerBody::new(c, guide, rng);
        let ai = OverseerAi::new(guide, cycle, rng);
        let c = self.creature_mut(id);
        c.realized = Some(RealizedCreature::Overseer(Box::new(body)));
        if let Some(base) = &mut c.ai {
            base.real_ai = true;
            if let AiKind::Overseer(o) = &mut base.kind {
                o.real_ai = Some(Box::new(ai));
            }
        }
        true
    }

    /// Takes a realized overseer's parts out of the world to run it.
    pub(crate) fn take_overseer(&mut self, id: CreatureId) -> Option<OverseerParts> {
        let c = self.creature_mut(id);
        let Some(RealizedCreature::Overseer(_)) = &c.realized else {
            return None;
        };
        let mut base = c.ai.take()?;
        let AiKind::Overseer(mut abs) = std::mem::replace(&mut base.kind, AiKind::Plain) else {
            c.ai = Some(base);
            return None;
        };
        let Some(ai) = abs.real_ai.take() else {
            base.kind = AiKind::Overseer(abs);
            c.ai = Some(base);
            return None;
        };
        let Some(RealizedCreature::Overseer(body)) = c.realized.take() else {
            return None;
        };
        Some(OverseerParts {
            body,
            ai,
            abs,
            base,
        })
    }

    /// Puts an overseer's parts back, with its abstract position.
    pub(crate) fn put_overseer(
        &mut self,
        id: CreatureId,
        parts: OverseerParts,
        pos: WorldCoordinate,
    ) {
        let OverseerParts {
            body,
            ai,
            mut abs,
            mut base,
        } = parts;
        abs.real_ai = Some(ai);
        base.kind = AiKind::Overseer(abs);
        let c = self.creature_mut(id);
        c.realized = Some(RealizedCreature::Overseer(body));
        c.ai = Some(base);
        if pos.room == c.pos.room {
            c.pos = pos;
        } else {
            log::warn!("realized overseer changed rooms during its update");
        }
    }

    /// What the graphics module reads of realized overseer `id`.
    #[must_use]
    pub fn overseer_view(&self, id: CreatureId) -> Option<OverseerView<'_>> {
        let c = self.creature(id);
        let body = c.overseer_body()?;
        let abs = c.overseer_ai()?;
        let ai = abs.real_ai.as_deref()?;
        let partner = body.conversation_partner.map(|p| self.creature(p).id);
        Some(OverseerView::new(c.id, body, ai, abs.color(), partner))
    }

    /// The realized overseers of room `room`, in `updateList` order (newest first).
    #[must_use]
    pub(crate) fn room_overseers(&self, room: i32) -> Vec<(u64, CreatureId)> {
        let mut list: Vec<(u64, CreatureId)> = self
            .room_creatures(room)
            .iter()
            .filter_map(|&id| {
                let b = self.creature(id).overseer_body()?;
                b.in_room.then_some((b.update_order, id))
            })
            .collect();
        list.sort_by_key(|&(seq, _)| std::cmp::Reverse(seq));
        list
    }

    /// The overseer holograms in room `room`'s update list: (update order, owner, id).
    /// A hologram stays in its room when its overseer leaves.
    pub(crate) fn room_holograms(&self, room: i32) -> Vec<(u64, CreatureId, u64)> {
        self.creatures()
            .filter_map(|(id, c)| Some((id, c.overseer_body()?)))
            .flat_map(|(id, b)| {
                b.holograms
                    .iter()
                    .filter(|h| h.room == room && !h.slated)
                    .map(move |h| (h.update_order, id, h.id))
            })
            .collect()
    }

    /// `AbstractCreature.InDenUpdate`. Only creatures whose abstract AI is ported (the
    /// overseers) leave their dens; the rest stay in (docs/DIVERGENCES.md D14).
    fn in_den_update(&mut self, id: CreatureId, time: i32) {
        let time_until_rain = self.rain_cycle.time_until_rain();
        let day_night = self.rain_cycle.day_night_counter;
        let c = self.creature_mut(id);
        if c.kind() != CreatureType::Overseer {
            return;
        }
        let night_ok = !c.night_creature || day_night as f32 > 600.0 || c.ignore_cycle;
        if c.remain_in_den_counter <= -1 || !night_ok {
            return;
        }
        if c.wants_to_stay_in_den(time_until_rain) {
            c.remain_in_den_counter = -1;
            return;
        }
        c.remain_in_den_counter -= time;
        if c.remain_in_den_counter < 0 {
            c.remain_in_den_counter = -1;
            self.move_entity_out_of_den(id);
        }
    }

    /// `Room.ReadyForAI` for one creature: `AbstractCreature.RealizeInRoom` of an
    /// overseer in loaded room `room` up to `PlaceInRoom`: `PlaceInRealizedRoom`'s draws,
    /// then `Realize`. Returns whether it was realized (the caller places it). Other
    /// types are not realized.
    pub(crate) fn realize_overseer(
        &mut self,
        id: CreatureId,
        cycle: i32,
        rng: &mut UnityRandom,
    ) -> bool {
        let c = self.creature(id);
        if c.kind() != CreatureType::Overseer || c.is_realized() || c.in_den {
            return false;
        }
        let room_index = c.pos.room;
        let Some(realized) = self.realized(room_index) else {
            return false;
        };
        let (width, height) = (realized.room.width(), realized.room.height());
        let mut pos = c.pos;
        if !pos.node_defined() {
            // An overseer maps no node types: `Range(0, 0)` makes no draw.
            pos.node = -1;
        }
        // `PlaceInRealizedRoom`.
        let mut place = pos;
        if !place.tile_defined() && place.node_defined() {
            place.tile = self.local_coordinate_of_node(room_index, place.node, rng);
        }
        place.tile.0 = place.tile.0.clamp(0, width - 1);
        place.tile.1 = place.tile.1.clamp(0, height - 1);
        let dest = c.ai.as_ref().map_or(place, |a| a.destination);
        let manhattan = (place.tile.0 - dest.tile.0).abs() + (place.tile.1 - dest.tile.1).abs();
        if place.room == dest.room
            && place.node == dest.node
            && (manhattan < 20 || !dest.tile_defined())
        {
            // `RandomizeSpawnPositionInRoom` with `timeBuffer * offScreenSpeed` (0) steps.
            rng.range_i32(0, width);
            rng.range_i32(0, height);
            pos.tile = place.tile;
        }
        if !self.realize_overseer_parts(id, cycle, rng) {
            return false;
        }
        if !pos.tile_defined() {
            pos.tile = self.local_coordinate_of_node(room_index, pos.node, rng);
        }
        self.creature_mut(id).pos = pos;
        true
    }

    /// `Room.LocalCoordinateOfNode`: exits, dens and region transports by their
    /// shortcut's entrance, a random tile of a bat hive or of the garbage holes. Border
    /// exits draw one of their border tiles, which are not mapped (`MapRoomBorders` is
    /// not ported): the draw is made and the tile is `(-1, -1)`.
    pub(crate) fn local_coordinate_of_node(
        &self,
        room: i32,
        node: i32,
        rng: &mut UnityRandom,
    ) -> (i32, i32) {
        let (Some(realized), Some(data)) = (self.realized(room), self.abstract_room(room)) else {
            return (-1, -1);
        };
        let Some(n) = usize::try_from(node).ok().and_then(|n| data.nodes.get(n)) else {
            log::warn!("Unidentified entrance node! {node} / {}", data.nodes.len());
            return (-1, -1);
        };
        match n.kind {
            NodeType::Exit | NodeType::Den | NodeType::RegionTransportation => realized
                .room
                .shortcut_map()
                .leading_to_node(usize::try_from(node).unwrap_or(0))
                .map_or((-1, -1), |s| s.start),
            NodeType::SideExit | NodeType::SkyExit | NodeType::SeaExit => {
                rng.next_u32();
                log::warn!("a border exit's tiles are not mapped");
                (-1, -1)
            }
            NodeType::BatHive => {
                let first = data
                    .nodes
                    .iter()
                    .position(|n| n.kind == NodeType::BatHive)
                    .unwrap_or(0);
                let index = usize::try_from(node).unwrap_or(0).saturating_sub(first);
                let Some(hive) = realized.room.hives().get(index) else {
                    return (-1, -1);
                };
                let len = i32::try_from(hive.len()).unwrap_or(i32::MAX);
                hive[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)]
            }
            NodeType::GarbageHoles => {
                let holes = realized.room.garbage_holes();
                let len = i32::try_from(holes.len()).unwrap_or(i32::MAX);
                holes
                    .get(usize::try_from(rng.range_i32(0, len)).unwrap_or(0))
                    .copied()
                    .unwrap_or((-1, -1))
            }
        }
    }

    /// `AbstractRoom.Abstractize` for the creatures of global room `room`, newest first:
    /// a realized overseer drops its body and keeps its node; its realized AI stays. A
    /// realized fly (in the room, a hive or a shortcut) drops its body and its grasps'
    /// flies with it. Rooms out of the camera's view have no graphics modules to drop.
    pub(crate) fn abstractize_creatures(&mut self, room: i32) {
        let ids: Vec<CreatureId> = self.room_creatures(room).iter().rev().copied().collect();
        for id in ids {
            let c = self.creature_mut(id);
            match c.realized {
                Some(RealizedCreature::Overseer(_)) => {
                    c.realized = None;
                    c.time_spent_here = 0;
                    c.distance_to_my_node = -1;
                    let pos = c.pos;
                    self.move_creature(id, pos);
                }
                Some(RealizedCreature::Fly(_)) => {
                    let pos = c.pos;
                    self.abstractize_fly(id, pos);
                }
                _ => {}
            }
        }
    }

    /// `AbstractCreature.Abstractize(coord)` of a realized fly: at its own position the
    /// node stays (it is always defined for a fly) and `distanceToMyNode` is its exit
    /// distance; then `Move(coord)`, the flies it holds abstracted to `coord` too.
    pub(crate) fn abstractize_fly(&mut self, id: CreatureId, coord: WorldCoordinate) {
        let c = self.creature(id);
        if c.fly().is_none() {
            return;
        }
        let room = c.pos.room;
        if !self.realized(room).is_some_and(RealizedRoom::ready_for_ai) {
            self.creature_mut(id).realized = None;
            return;
        }
        let pos = self.creature(id).pos;
        let mut distance = 0;
        if coord == pos {
            if !pos.node_defined() {
                log::warn!("QuickConnectivity.DefineNodeOfLocalCoordinate is not ported");
            }
            let specific = self
                .fly_nodes(room)
                .iter()
                .position(|&(n, _)| n == pos.node)
                .map_or(-1, |i| i32::try_from(i).unwrap_or(-1));
            distance = self
                .realized(room)
                .and_then(|r| r.ai_map.as_ref())
                .map_or(-1, |m| {
                    m.fly_exit_distance_and_check_neighbours(pos.tile, specific)
                });
            if distance == -1 {
                log::debug!("abstracted fly without an exit distance to its node");
            }
        }
        let c = self.creature_mut(id);
        c.time_spent_here = 0;
        c.distance_to_my_node = distance;
        self.move_creature(id, coord);
        let held: Vec<CreatureId> = self
            .creature(id)
            .grasps
            .iter()
            .flatten()
            .filter_map(|g| g.grabbed_creature())
            .collect();
        // Cleared first: a circular chain stops here (the game releases such a grasp).
        self.creature_mut(id).realized = None;
        for other in held {
            self.abstractize_fly(other, coord);
        }
    }

    /// `AbstractCreature.Abstractize(coord)` of a realized overseer in the lobby whose
    /// destination is not realized: it moves there abstractly and drops its body.
    pub(crate) fn abstractize_to(&mut self, id: CreatureId, coord: WorldCoordinate) {
        let room = self.creature(id).pos.room;
        if !self.realized(room).is_some_and(RealizedRoom::ready_for_ai) {
            self.creature_mut(id).realized = None;
            return;
        }
        let c = self.creature_mut(id);
        c.time_spent_here = 0;
        c.distance_to_my_node = 0;
        self.move_creature(id, coord);
        self.creature_mut(id).realized = None;
    }
}

impl World {
    /// The guide's tutorial kills the living green lizards of room `room`
    /// (`LizardState.Die`).
    pub(crate) fn kill_green_lizards(&mut self, room: i32) {
        for id in self.room_creatures(room).to_vec() {
            let c = self.creature_mut(id);
            if c.kind() == CreatureType::GreenLizard && c.state.alive {
                log::info!("the tutorial kills a green lizard in room {room}");
                c.state.alive = false;
            }
        }
    }
}

/// A realized overseer's parts, taken out of the world while it runs.
pub(crate) struct OverseerParts {
    pub body: Box<OverseerBody>,
    pub ai: Box<OverseerAi>,
    pub abs: OverseerAbstractAi,
    pub base: AbstractCreatureAi,
}

impl AbstractCreature {
    /// `WantToStayInDenUntilEndOfCycle` for a creature whose realized AI never ran.
    fn wants_to_stay_in_den(&self, time_until_rain: i32) -> bool {
        if !self.state.alive {
            return true;
        }
        if self.state.has_health() && self.state.health < 0.6 {
            return true;
        }
        if time_until_rain < 60 * 40 && !self.night_creature && !self.ignore_cycle {
            return true;
        }
        if let Some(ai) = &self.ai {
            // A realized AI that is kept (an overseer's) answers false; else
            // `AbstractCreatureAI.WantToStayInDenUntilEndOfCycle` without one.
            if ai.real_ai {
                return false;
            }
            return (!self.night_creature && !self.ignore_cycle && time_until_rain < 60 * 40)
                || (self.state.kind == crate::creature::StateKind::Lizard
                    && self.state.health < 0.5)
                || (self.state.has_health() && self.state.health < 0.75);
        }
        false
    }
}
