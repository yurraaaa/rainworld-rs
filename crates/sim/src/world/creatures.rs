//! The world's creatures: the arena, each abstract room's `creatures` and
//! `entitiesInDens` lists, the room attractions, and what the game does with them
//! (`AbstractCreature` construction, `Move`, dens, `AbstractRoom.Update`, realizing an
//! overseer in a loaded room and abstracting it again).
//!
//! Objects other than creatures are not kept as entities: a room's `entities` would hold
//! the same creatures in the same order plus carried items, whose updates draw nothing.

use std::collections::HashMap;

use rw_assets::{CreatureType, NodeType, RoomAttraction, Timeline};
use rw_random::UnityRandom;

use super::World;
use crate::creature::ai::AiContext;
use crate::creature::overseer::GuideContext;
use crate::creature::{
    AbstractCreature, AbstractCreatureAi, AiKind, CreatureId, CreatureTemplate, EntityId, IdIssuer,
    OverseerBody, Personality, PlayerGuideState, RealizedCreature, WorldCoordinate,
};
use crate::room_ext::RoomExt;
use crate::room_script::StorySession;
use crate::{FloatExt, PlayerObserver};

/// One abstract room's lists.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RoomLists {
    pub creatures: Vec<CreatureId>,
    pub in_dens: Vec<CreatureId>,
    pub even_update: bool,
    /// `quantifiedCreatures`: per node and type, a count.
    pub quantified: Vec<QuantifiedCount>,
}

/// Quantified creatures of one type at one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantifiedCount {
    pub node: i32,
    pub kind: CreatureType,
    pub count: i32,
}

/// The world processes that exist as markers (their updates draw nothing in the base
/// game, or are not ported: `FliesWorldAI.Update`, docs/DIVERGENCES.md D1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldProcesses {
    pub overseers: bool,
    pub scavengers: bool,
    /// `FliesWorldAI.migrationTargets`.
    pub flies_migration_targets: Option<Vec<i32>>,
}

/// What an abstract update reads of the game.
pub struct AbstractTick<'a> {
    pub rng: &'a mut UnityRandom,
    pub guide: &'a PlayerGuideState,
    pub player: Option<CreatureId>,
    pub player_karma: i32,
}

/// `Custom.eightDirectionsAndZero`, the first eight as `Overseer.PlaceInRoom` reads them.
const EIGHT_DIRECTIONS_AND_ZERO: [(i32, i32); 9] = [
    (0, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

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
            if c.kind() != CreatureType::BIG_NEEDLE_WORM || !c.state.alive {
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
        };
        let room = self.abstract_room(pos.room);
        if template.top_ancestor() == CreatureType::GARBAGE_WORM {
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
        if template.kind == CreatureType::TENTACLE_PLANT
            || template.kind == CreatureType::POLE_MIMIC
        {
            c.remain_in_den_counter = 0;
        }
        c
    }

    /// Puts a new creature into the arena (in no list yet), making the world processes
    /// its AI constructor would make.
    pub(crate) fn insert_creature(&mut self, c: AbstractCreature) -> CreatureId {
        match c.ai.as_ref().map(|a| &a.kind) {
            Some(AiKind::Overseer(_)) => self.processes.overseers = true,
            Some(AiKind::Scavenger { .. }) => self.processes.scavengers = true,
            _ => {}
        }
        self.creatures.push(c);
        CreatureId(self.creatures.len() - 1)
    }

    /// `AbstractRoom.AddEntity`.
    pub(crate) fn add_entity(&mut self, id: CreatureId) {
        let room = self.creature(id).pos.room;
        if let Some(l) = self.local(room)
            && !self.lists[l].creatures.contains(&id)
        {
            self.lists[l].creatures.push(id);
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
            lists.creatures.retain(|&c| c != id);
            if !lists.in_dens.contains(&id) {
                lists.in_dens.push(id);
            }
        }
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

    /// Sets a creature's position as the player's body reports it (no `Move` hooks; a
    /// new room moves it between the room lists, as `ChangeRooms`).
    pub(crate) fn place_creature(&mut self, id: CreatureId, to: WorldCoordinate) {
        let from = self.creature(id).pos;
        self.creature_mut(id).pos = to;
        if from.room != to.room {
            if let Some(l) = self.local(from.room) {
                self.lists[l].creatures.retain(|&c| c != id);
            }
            self.add_entity(id);
        }
    }

    /// `AbstractCreature.Move(newCoord)`: `ChangeRooms` moves it between the room lists.
    /// Entering a realized room does not realize it (`CreatureEnterFromAbstractRoom` is
    /// not ported, docs/DIVERGENCES.md D14): it stays abstract there.
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
                if self.realized(to.room).is_some() {
                    log::debug!(
                        "{} enters realized room {} abstractly",
                        self.creature(id).kind(),
                        to.room
                    );
                }
                if let Some(l) = self.local(from.room) {
                    let lists = &mut self.lists[l];
                    lists.creatures.retain(|&c| c != id);
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

    /// `AbstractRoom.Update(timePassed)` of global room `room`: its creatures not yet
    /// updated this round, then `UpdateCreaturesInDens`.
    pub fn update_abstract_room(&mut self, room: i32, time: i32, cx: &mut AbstractTick<'_>) {
        let Some(l) = self.local(room) else { return };
        let even = !self.lists[l].even_update;
        self.lists[l].even_update = even;
        'restart: loop {
            let mut i = 0;
            while let Some(&id) = self.lists[l].creatures.get(i) {
                if self.creature(id).even_update != even {
                    let before = self.creature(id).pos.room;
                    self.creature_mut(id).even_update = even;
                    self.update_creature(id, time, cx);
                    let list = &self.lists[l].creatures;
                    let changed = list.get(i).is_none_or(|&c| {
                        self.creature(c).pos.room != before || self.creature(c).in_den
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
        let Some(mut ai) = c.ai.take() else { return };
        ai.time_buffer += time;
        let mut moved = None;
        if self.realized(self.creature(id).pos.room).is_none()
            && let AiKind::Overseer(mut overseer) = std::mem::replace(&mut ai.kind, AiKind::Plain)
        {
            let mut guide = GuideContext {
                world: self,
                rng: &mut *cx.rng,
                guide: cx.guide,
                player: cx.player,
                player_karma: cx.player_karma,
            };
            moved = overseer.abstract_behavior(&mut ai, id, time, &mut guide);
            ai.kind = AiKind::Overseer(overseer);
        }
        self.creature_mut(id).ai = Some(ai);
        if let Some(to) = moved {
            self.move_creature(id, to);
        }
    }

    /// `AbstractCreature.InDenUpdate`. Only creatures whose abstract AI is ported (the
    /// overseers) leave their dens; the rest stay in (docs/DIVERGENCES.md D14).
    fn in_den_update(&mut self, id: CreatureId, time: i32) {
        let time_until_rain = self.rain_cycle.time_until_rain();
        let day_night = self.rain_cycle.day_night_counter;
        let c = self.creature_mut(id);
        if c.kind() != CreatureType::OVERSEER {
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
    /// overseer in loaded room `room` (`viewed`: the camera's room, where `AddObject`
    /// builds its graphics module). Other types are not realized.
    pub(crate) fn realize_overseer(
        &mut self,
        id: CreatureId,
        viewed: bool,
        observer: &mut dyn PlayerObserver,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        if c.kind() != CreatureType::OVERSEER || c.is_realized() || c.in_den {
            return;
        }
        let room_index = c.pos.room;
        let Some(realized) = self.realized(room_index) else {
            return;
        };
        let room = realized.room.clone();
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
        place.tile.0 = place.tile.0.clamp(0, room.width() - 1);
        place.tile.1 = place.tile.1.clamp(0, room.height() - 1);
        let dest = c.ai.as_ref().map_or(place, |a| a.destination);
        let manhattan = (place.tile.0 - dest.tile.0).abs() + (place.tile.1 - dest.tile.1).abs();
        if place.room == dest.room
            && place.node == dest.node
            && (manhattan < 20 || !dest.tile_defined())
        {
            // `RandomizeSpawnPositionInRoom` with `timeBuffer * offScreenSpeed` (0) steps.
            rng.range_i32(0, room.width());
            rng.range_i32(0, room.height());
            pos.tile = place.tile;
        }
        // `RealizeInRoom`: `Realize` (the `Overseer` and its `OverseerAI`), then
        // `PlaceInRoom`.
        let mut body = OverseerBody::new(self.creature(id), rng);
        if !pos.tile_defined() {
            pos.tile = self.local_coordinate_of_node(room_index, pos.node, rng);
        }
        if viewed {
            observer.overseer_viewed(self.creature(id).id, body.size, rng);
        }
        // `Creature.PlaceInRoom`'s chunk offset.
        rng.value();
        let resets = body.place_in_room(&room, pos.tile, rng);
        if viewed {
            for _ in 0..resets {
                observer.overseer_reset(self.creature(id).id, rng);
            }
        }
        let c = self.creature_mut(id);
        c.pos = pos;
        c.realized = Some(RealizedCreature::Overseer(body));
        if let Some(ai) = &mut c.ai {
            ai.real_ai = true;
        }
    }

    /// `Room.LocalCoordinateOfNode` as far as the port's room data reaches: exits, dens
    /// and region transports through their shortcut; border exits, hives and garbage
    /// holes (which draw a tile) are not mapped, so these answer `(-1, -1)`.
    fn local_coordinate_of_node(&self, room: i32, node: i32, _rng: &mut UnityRandom) -> (i32, i32) {
        let (Some(realized), Some(data)) = (self.realized(room), self.abstract_room(room)) else {
            return (-1, -1);
        };
        let Some(n) = usize::try_from(node).ok().and_then(|n| data.nodes.get(n)) else {
            return (-1, -1);
        };
        match n.kind {
            NodeType::Exit | NodeType::Den | NodeType::RegionTransportation => realized
                .room
                .shortcut_map()
                .leading_to_node(usize::try_from(node).unwrap_or(0))
                .map_or((-1, -1), |s| s.start),
            _ => {
                log::warn!("local coordinate of a {:?} node is not ported", n.kind);
                (-1, -1)
            }
        }
    }

    /// `AbstractRoom.Abstractize` for the creatures of global room `room`, newest first:
    /// a realized overseer drops its body and keeps its node; its realized AI stays. Rooms
    /// out of the camera's view have no graphics modules to drop.
    pub(crate) fn abstractize_creatures(&mut self, room: i32) {
        let ids: Vec<CreatureId> = self.room_creatures(room).iter().rev().copied().collect();
        for id in ids {
            let c = self.creature_mut(id);
            if !matches!(c.realized, Some(RealizedCreature::Overseer(_))) {
                continue;
            }
            c.realized = None;
            c.time_spent_here = 0;
            c.distance_to_my_node = -1;
            let pos = c.pos;
            self.move_creature(id, pos);
        }
    }

    /// The camera moved from room `from` to room `to` (`NoLongerViewed`, `NowViewed`):
    /// realized overseers lose or get their graphics modules.
    pub(crate) fn camera_changed_room(
        &mut self,
        from: i32,
        to: i32,
        observer: &mut dyn PlayerObserver,
        rng: &mut UnityRandom,
    ) {
        for &id in self.room_creatures(from) {
            if let Some(RealizedCreature::Overseer(_)) = self.creature(id).realized {
                observer.overseer_unviewed(self.creature(id).id);
            }
        }
        for &id in self.room_creatures(to) {
            if let Some(RealizedCreature::Overseer(body)) = &self.creature(id).realized {
                observer.overseer_viewed(self.creature(id).id, body.size, rng);
            }
        }
    }
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

impl OverseerBody {
    /// `Overseer.PlaceInRoom` after the base: a random open tile next to a solid one,
    /// else the abstract tile, else any random tile (`HardSetTile`). Returns how often
    /// the graphics module is reset (twice on the last fallback).
    pub(crate) fn place_in_room(
        &mut self,
        room: &rw_assets::Room,
        tile: (i32, i32),
        rng: &mut UnityRandom,
    ) -> u32 {
        for _ in 0..100 {
            let t = (
                rng.range_i32(0, room.width()),
                rng.range_i32(0, room.height()),
            );
            if room.is_solid_tile(t.0, t.1) {
                continue;
            }
            if let Some(d) = EIGHT_DIRECTIONS_AND_ZERO[..8]
                .iter()
                .find(|d| room.is_solid_tile(t.0 + d.0, t.1 + d.1))
            {
                self.hard_set_tile(t, (t.0 + d.0, t.1 + d.1));
                return 1;
            }
        }
        // `abstractCreature.pos.TileDefined`.
        if (tile.0 != -1 || tile.1 != -1)
            && let Some(d) = EIGHT_DIRECTIONS_AND_ZERO[..8]
                .iter()
                .find(|d| room.is_solid_tile(tile.0 + d.0, tile.1 + d.1))
        {
            self.hard_set_tile(tile, (tile.0 + d.0, tile.1 + d.1));
            return 1;
        }
        let t = (
            rng.range_i32(0, room.width()),
            rng.range_i32(0, room.height()),
        );
        self.hard_set_tile(t, t);
        2
    }

    fn hard_set_tile(&mut self, hover: (i32, i32), root: (i32, i32)) {
        self.hover_tile = hover;
        self.root_tile = root;
    }
}
