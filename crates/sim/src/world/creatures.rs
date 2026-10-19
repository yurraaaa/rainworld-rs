//! The world's creatures: the arena, each abstract room's `creatures` and
//! `entitiesInDens` lists, the room attractions, and what the game does with them
//! (`AbstractCreature` construction, `Move`, dens, `AbstractRoom.Update`, realizing an
//! overseer in a loaded room and abstracting it again).
//!
//! A room's `entities` (creatures and objects, [`Entity`]) are kept beside its creature
//! lists; the objects are `world/objects.rs`'.
//!
//! Mirrors: AbstractCreature.cs, AbstractRoom.cs, AbstractCreatureAI.cs

use rustc_hash::FxHashMap;
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
/// marker (its update is not ported, docs/divergences/ D14).
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
    /// `nextIssuedId` (a cicada's caught fly takes a `GetNewID`).
    pub ids: &'a mut IdIssuer,
    /// What a new creature's constructor reads of the session.
    pub session: &'a StorySession,
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
                let mut table = FxHashMap::default();
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

    /// `NeedleWormAbstractAI.FindMotherInRoom(room)` for a small worm at `pos` (its
    /// `denPosition` is `den`), over the room's creatures then its dens: one draw per
    /// living big worm.
    pub(crate) fn find_needle_mother(
        &self,
        room: i32,
        pos: WorldCoordinate,
        den: Option<WorldCoordinate>,
        rng: &mut UnityRandom,
    ) -> Option<CreatureId> {
        let mut best = f32::MIN;
        let mut mother = None;
        for &id in self.room_creatures(room).iter().chain(self.room_dens(room)) {
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
            pre_cycle: false,
            in_den: false,
            time_spent_here: 0,
            even_update: false,
            distance_to_my_node: 0,
            slated_for_deletion: false,
            realized: None,
            carried: Vec::new(),
            grasps: Vec::new(),
            grabbed_by: Vec::new(),
            sticks: Vec::new(),
            kill_tag: None,
            kill_tag_counter: 0,
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

    /// `AbstractRoom.RemoveEntity`: out of its room's lists.
    pub(crate) fn remove_entity(&mut self, id: CreatureId) {
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room) {
            self.lists[l].remove_creature(id);
        }
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

    /// `AbstractCreature.Move(newCoord)`: a carried quantified creature (`Quantify`)
    /// moving into an unrealized room becomes a count at the new node; a garbage worm
    /// first takes its new room's garbage-hole node; `ChangeRooms` moves it between the
    /// room lists (realizing it first is [`Self::change_rooms_realize`]); the creatures
    /// stuck to it follow (`AbstractPhysicalObject.Move`), then `Moved`.
    pub(crate) fn move_creature(&mut self, id: CreatureId, mut to: WorldCoordinate) {
        let c = self.creature(id);
        if c.quantify() && self.realized(to.room).is_none() {
            log::debug!("quantified creature delete itself!");
            let kind = c.kind();
            self.remove_entity(id);
            self.destroy_abstract(id);
            self.add_quantified_at(to.room, to.node, kind);
            return;
        }
        if c.kind() == CreatureType::GarbageWorm
            && let Some(node) = self.abstract_room(to.room).and_then(|r| {
                r.nodes
                    .iter()
                    .position(|n| n.kind == NodeType::GarbageHoles)
            })
        {
            self.creature_mut(id).pos.node = i32::try_from(node).unwrap_or(-1);
        }
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
            // Only between abstract creatures: what a realized player carries is moved
            // by its own code.
            let sticks = self.creature(id).sticks.clone();
            for s in sticks {
                for other in [Some(s.a), s.b.creature()].into_iter().flatten() {
                    let c = self.creature(id);
                    if other != id
                        && c.realized.is_none()
                        && self.creature(other).realized.is_none()
                        && c.sticks.iter().any(|t| t.serial == s.serial)
                    {
                        self.move_creature(other, to);
                    }
                }
            }
        }
        self.moved(id);
    }

    /// `AbstractCreatureAI.Moved` (and the overseer's, whose guide case needs an angry
    /// guide: not in a new game): the path is cut at the node reached, and a migration
    /// destination in this room is done.
    pub(crate) fn moved(&mut self, id: CreatureId) {
        let pos = self.creature(id).pos;
        if let Some(ai) = &mut self.creature_mut(id).ai {
            if let Some(i) = ai
                .path
                .iter()
                .rposition(|p| p.room == pos.room && p.node == pos.node)
            {
                ai.path.truncate(i);
            }
            if ai.migration_destination.is_some_and(|m| m.room == pos.room) {
                ai.migration_destination = None;
            }
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
                self.in_den_update(id, time, cx.rng);
            }
        }
    }

    /// `AbstractCreature.Update(time)`: the abstract AI of a living, unrealized creature
    /// out of its den and not carried: the overseers', and the plain or own one of every
    /// template that [`CreatureTemplate::abstract_ai_ported`] (all but the scavengers).
    fn update_creature(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        let c = self.creature_mut(id);
        c.time_spent_here += time;
        if c.in_den || c.realized.is_some() || !c.state.alive || c.pacified_because_carried(id) {
            return;
        }
        let (pos, ignore_cycle, plain) = (c.pos, c.ignore_cycle, c.template.abstract_ai_ported());
        let Some(mut ai) = c.ai.take() else { return };
        ai.time_buffer += time;
        if plain {
            self.creature_mut(id).ai = Some(ai);
            self.abstract_ai_update(id, time, cx);
            return;
        }
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
        } else if self.realize_lizard(id, rng) {
        } else if !self.realize_overseer_parts(id, cycle, rng) {
            log::warn!("{kind} entering realized room {} stays abstract", to.room);
            return;
        }
        match &mut self.creature_mut(id).realized {
            Some(RealizedCreature::Overseer(body)) => body.in_shortcut = true,
            Some(RealizedCreature::Fly(fly)) => fly.in_shortcut = true,
            Some(RealizedCreature::Lizard(l)) => l.in_shortcut = true,
            _ => {}
        }
        self.entering.push((id, to));
    }

    /// Whether creature `id` is carried by a creature the port keeps abstract (a
    /// cicada's caught fly): it stays abstract with its carrier (D14).
    #[must_use]
    pub fn held_by_abstract_creature(&self, id: CreatureId) -> bool {
        self.creature(id).sticks.iter().any(|s| {
            s.a != id && s.b == Entity::Creature(id) && self.creature(s.a).realized.is_none()
        })
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
        let mut body = OverseerBody::new(c, guide, rng);
        // `Creature`'s constructor: `dead = State.dead` (a dead overseer realized again
        // dies away again).
        body.dead = !c.state.alive;
        let ai = OverseerAi::new(guide, cycle, rng);
        let c = self.creature_mut(id);
        c.realized = Some(RealizedCreature::Overseer(Box::new(body)));
        c.kill_tag = None;
        c.kill_tag_counter = 0;
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

    /// `AbstractCreature.InDenUpdate`: the den counter runs down (a night creature's
    /// only after dark), and at below 0 the creature comes out; one that wants to stay
    /// for the rest of the cycle stops counting (-1). The scavengers, whose abstract AI is
    /// not ported, stay in (docs/divergences/ D14). A pre-cycle creature passes the
    /// game's precycle test (no precycles without More Slugcats) and then wants to stay.
    fn in_den_update(&mut self, id: CreatureId, time: i32, rng: &mut UnityRandom) {
        let day_night = self.rain_cycle.day_night_counter;
        let wants_to_stay = self.wants_to_stay_in_den(id);
        let c = self.creature_mut(id);
        if c.template.held_in_den() {
            return;
        }
        let night_ok = !c.night_creature || day_night as f32 > 600.0 || c.ignore_cycle;
        if c.remain_in_den_counter <= -1 || !night_ok {
            return;
        }
        if wants_to_stay {
            c.remain_in_den_counter = -1;
            return;
        }
        c.remain_in_den_counter -= time;
        if c.remain_in_den_counter < 0 {
            c.remain_in_den_counter = -1;
            self.move_entity_out_of_den(Entity::Creature(id), rng);
        }
    }

    /// `AbstractCreature.WantToStayInDenUntilEndOfCycle` (the rain's margin is 60 s in a
    /// story session, 15 s otherwise).
    pub(crate) fn wants_to_stay_in_den(&self, id: CreatureId) -> bool {
        let margin = if self.story { 60 * 40 } else { 15 * 40 };
        self.creature(id)
            .wants_to_stay_in_den(self.rain_cycle.time_until_rain(), margin)
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
    /// shortcut's entrance, a random tile of a border exit, a bat hive or the garbage
    /// holes.
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
                let exits = realized.room.shortcut_map().nodes().len();
                let index = usize::try_from(node).unwrap_or(0).saturating_sub(exits);
                let tiles = realized
                    .ai_map
                    .as_ref()
                    .and_then(|m| m.border_exits().get(index))
                    .map(|e| e.tiles.as_slice())
                    .unwrap_or_default();
                let len = i32::try_from(tiles.len()).unwrap_or(i32::MAX);
                let i = usize::try_from(rng.range_i32(0, len)).unwrap_or(0);
                tiles.get(i).copied().unwrap_or((-1, -1))
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
    pub(crate) fn abstractize_creatures(&mut self, room: i32, rng: &mut UnityRandom) {
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
                    self.abstractize_fly(id, pos, rng);
                }
                Some(RealizedCreature::Lizard(_)) => {
                    let pos = c.pos;
                    self.abstractize_lizard(id, pos, rng);
                }
                _ => {}
            }
        }
    }

    /// `AbstractCreature.Abstractize(coord)` of a realized fly: at its own position the
    /// node stays (it is always defined for a fly) and `distanceToMyNode` is its exit
    /// distance; then `Move(coord)`, the flies it holds abstracted to `coord` too.
    pub(crate) fn abstractize_fly(
        &mut self,
        id: CreatureId,
        coord: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        if c.fly().is_none() {
            return;
        }
        let room = c.pos.room;
        if !self.realized(room).is_some_and(RealizedRoom::ready_for_ai) {
            self.creature_mut(id).realized = None;
            return;
        }
        let mut pos = self.creature(id).pos;
        let mut distance = 0;
        if coord == pos {
            let fly = CreatureTemplate::of(CreatureType::Fly, rw_assets::ActiveMods::NONE);
            pos = self.define_node(pos, &fly, rng);
            self.creature_mut(id).pos = pos;
            let specific = self
                .fly_nodes(room)
                .iter()
                .position(|&(n, _)| n == pos.node)
                .map_or(-1, |i| i32::try_from(i).unwrap_or(-1));
            distance = self
                .realized(room)
                .and_then(|r| r.ai_map.as_ref())
                .map_or(-1, |m| {
                    m.exit_distance_for_creature_and_check_neighbours(pos.tile, specific, &fly)
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
            self.abstractize_fly(other, coord, rng);
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
    /// `WantToStayInDenUntilEndOfCycle` with `margin` ticks of rain warning: dead, hurt,
    /// the rain close (not for night creatures or those ignoring the cycle), a
    /// pre-cycle creature, then the abstract AI's own test (a kept realized AI, an
    /// overseer's, answers false).
    fn wants_to_stay_in_den(&self, time_until_rain: i32, margin: i32) -> bool {
        if !self.state.alive {
            return true;
        }
        if self.state.has_health() && self.state.health < 0.6 {
            return true;
        }
        let rain_close = time_until_rain < margin && !self.night_creature && !self.ignore_cycle;
        if rain_close || self.pre_cycle {
            return true;
        }
        if let Some(ai) = &self.ai {
            if ai.real_ai {
                return false;
            }
            return rain_close
                || (self.state.kind == crate::creature::StateKind::Lizard
                    && self.state.health < 0.5)
                || (self.state.has_health() && self.state.health < 0.75);
        }
        false
    }
}
