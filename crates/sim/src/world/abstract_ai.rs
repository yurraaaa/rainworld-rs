//! The plain `AbstractCreatureAI` (302062): what a creature in an unrealized room does
//! without a body. `Update`, `AbstractBehavior`, `MigrationBehavior`, `FollowPath`,
//! `SetDestination`/`FindPath`, `RandomMoveWithinRoom`, `RandomMoveToOtherRoom`,
//! `MoveToMoreAttractiveNeighborRoom`, `GoToDen`, `AbstractCreature.OpportunityToEnterDen`
//! and `QuickConnectivity.DefineNodeOfLocalCoordinate`'s abstract branch, with every
//! random draw in the game's order. Runs for the templates whose
//! [`CreatureTemplate::abstract_ai_ported`]; the subclasses' own `AbstractBehavior`s are
//! `abstract_ai_kinds.rs`.

use rw_assets::{CreatureType, NodeType, RoomAttraction};
use rw_random::UnityRandom;

use super::abstract_path::{
    AbstractSpaceNodeFinder, AbstractSpacePathFinder, FloodMethod, SearchingFor,
};
use super::{AbstractTick, World};
use crate::FloatExt;
use crate::creature::grasp::GripStick;
use crate::creature::{
    AbstractCreatureAi, AbstractMovement, CreatureId, CreatureTemplate, EntityId, WorldCoordinate,
};
use crate::item::Entity;

impl World {
    /// `IsRoomInRegion`.
    #[must_use]
    pub fn room_in_region(&self, room: i32) -> bool {
        self.local(room).is_some()
    }

    /// `World.GetNode(coord).type`; `None` for an undefined or missing node.
    #[must_use]
    pub fn node_kind(&self, coord: WorldCoordinate) -> Option<NodeType> {
        let room = self.abstract_room(coord.room)?;
        let node = usize::try_from(coord.node).ok()?;
        room.nodes.get(node).map(|n| n.kind)
    }

    /// `AbstractRoom.ConnectivityCost(start, dest, template)`: -1 out of range.
    #[must_use]
    pub fn connectivity_cost(&self, room: i32, start: i32, dest: i32, t: &CreatureTemplate) -> i32 {
        let Some(data) = self.abstract_room(room) else {
            return -1;
        };
        let Some(node) = usize::try_from(start).ok().and_then(|n| data.nodes.get(n)) else {
            return -1;
        };
        if dest < 0 || usize::try_from(dest).unwrap_or(usize::MAX) >= data.nodes.len() {
            return -1;
        }
        node.connection_cost(dest, t.pre_baked_pathing_index())
    }

    /// `AbstractRoom.ConnectionLength(start, dest, template)`: -1 out of range.
    #[must_use]
    pub fn connection_length(&self, room: i32, start: i32, dest: i32, t: &CreatureTemplate) -> i32 {
        let Some(data) = self.abstract_room(room) else {
            return -1;
        };
        let Some(node) = usize::try_from(start).ok().and_then(|n| data.nodes.get(n)) else {
            return -1;
        };
        if dest < 0 || usize::try_from(dest).unwrap_or(usize::MAX) >= data.nodes.len() {
            return -1;
        }
        node.connection_length(dest, t.pre_baked_pathing_index())
    }

    /// `AbstractRoom.ConnectionPossible`.
    #[must_use]
    pub fn connection_possible(
        &self,
        room: i32,
        start: i32,
        dest: i32,
        t: &CreatureTemplate,
    ) -> bool {
        self.connectivity_cost(room, start, dest, t) > -1
    }

    /// `AbstractRoom.ConnectionAndBackPossible`.
    #[must_use]
    pub fn connection_and_back_possible(
        &self,
        room: i32,
        start: i32,
        dest: i32,
        t: &CreatureTemplate,
    ) -> bool {
        start == dest
            || (self.connection_possible(room, start, dest, t)
                && self.connection_possible(room, dest, start, t))
    }

    /// `World.GetAttractionForRoom(coord, creature)`: Forbidden outside the region,
    /// `None` for the game's null.
    #[must_use]
    pub fn attraction_for_room(&self, room: i32, kind: CreatureType) -> Option<RoomAttraction> {
        if !self.room_in_region(room) {
            return Some(RoomAttraction::Forbidden);
        }
        self.attraction(room, kind)
    }

    /// `World.GetAttractionValueForRoom(coord, creature)`: 0 outside the region.
    #[must_use]
    pub fn attraction_value_for_room(&self, room: i32, kind: CreatureType) -> f32 {
        if !self.room_in_region(room) {
            return 0.0;
        }
        self.attraction_value(room, kind)
    }

    /// `AttractionForCreature(...) == Forbidden` of room `room`.
    #[must_use]
    pub fn forbidden_for(&self, room: i32, t: &CreatureTemplate) -> bool {
        self.attraction(room, t.kind) == Some(RoomAttraction::Forbidden)
    }

    /// The connection type of a border node's highway.
    #[must_use]
    pub fn highway_of(kind: NodeType) -> AbstractMovement {
        match kind {
            NodeType::SideExit => AbstractMovement::SideHighway,
            NodeType::SkyExit => AbstractMovement::SkyHighway,
            NodeType::SeaExit => AbstractMovement::SeaHighway,
            _ => AbstractMovement::RegionTransportation,
        }
    }

    /// `side/sky/sea/regionAccessNodes` (`MapBorderConnections`): every node of that
    /// type in rooms not disabled on the map.
    #[must_use]
    pub fn border_access_nodes(&self, kind: NodeType) -> Vec<WorldCoordinate> {
        self.data
            .rooms
            .iter()
            .filter(|r| !self.data.disabled_map_rooms.contains(&r.name))
            .flat_map(|r| {
                r.nodes
                    .iter()
                    .enumerate()
                    .filter(move |(_, n)| n.kind == kind)
                    .map(|(j, _)| {
                        WorldCoordinate::new(r.index, -1, -1, i32::try_from(j).unwrap_or(-1))
                    })
            })
            .collect()
    }

    /// `Side/Sky/SeaHighwayDistanceBetweenNodes` and
    /// `RegionTransportationDistanceBetweenNodes` (whose second point adds A's half size,
    /// as the game's does).
    #[must_use]
    pub fn highway_distance(&self, kind: NodeType, a: WorldCoordinate, b: WorldCoordinate) -> i32 {
        let (Some(ra), Some(rb)) = (self.abstract_room(a.room), self.abstract_room(b.room)) else {
            return -1;
        };
        let (pa, pb) = if kind == NodeType::RegionTransportation {
            if a.room == b.room || ra.offscreen_den || rb.offscreen_den {
                return 1;
            }
            let half = [ra.size.0 as f32 * 0.5, ra.size.1 as f32 * 0.5];
            (
                [ra.map_pos[0] + half[0], ra.map_pos[1] + half[1]],
                [rb.map_pos[0] + half[0], rb.map_pos[1] + half[1]],
            )
        } else {
            (ra.map_pos, rb.map_pos)
        };
        let d = ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2)).sqrt();
        ((d - 100.0).max(0.0) * 0.15) as i32 + 1
    }

    pub(super) fn ai_of(&self, id: CreatureId) -> Option<&AbstractCreatureAi> {
        self.creature(id).ai.as_ref()
    }

    pub(super) fn ai_of_mut(&mut self, id: CreatureId) -> Option<&mut AbstractCreatureAi> {
        self.creature_mut(id).ai.as_mut()
    }

    /// `offscreenSpeedFac`: `Lerp(0.1, 0.9, (1 - attraction)^2)` of the creature's room
    /// while it follows no creature.
    fn offscreen_speed_fac(&self, id: CreatureId) -> f32 {
        let c = self.creature(id);
        if c.ai.as_ref().is_some_and(|a| a.follow_creature.is_some()) {
            return 1.0;
        }
        let value = self.attraction_value(c.pos.room, c.kind());
        0.1.lerp(0.9, (1.0 - value).mathf_pow(2.0))
    }

    /// `QuickConnectivity.DefineNodeOfLocalCoordinate`: a defined node as is, else a
    /// random relevant node (one draw, a second over all nodes when there is none).
    /// In a room ready for AI the game then picks the node by the creature's exit
    /// distances, which needs its AI map (not ported for these creatures): the drawn
    /// node is kept there.
    pub(crate) fn define_node(
        &self,
        coord: WorldCoordinate,
        t: &CreatureTemplate,
        rng: &mut UnityRandom,
    ) -> WorldCoordinate {
        if coord.node_defined() {
            return coord;
        }
        let Some(room) = self.abstract_room(coord.room) else {
            return coord;
        };
        let specific = rng.range_i32(0, t.nodes_relevant(room));
        let mut node = t.common_node_index(room, specific);
        if node < 0 {
            let n = i32::try_from(room.nodes.len()).unwrap_or(i32::MAX);
            node = rng.range_i32(0, n);
        }
        if self
            .realized(coord.room)
            .is_some_and(super::RealizedRoom::ready_for_ai)
        {
            log::warn!(
                "DefineNodeOfLocalCoordinate in a room ready for AI: {} has no AI map",
                t.kind
            );
        }
        WorldCoordinate { node, ..coord }
    }

    /// `AbstractCreatureAI.Update(time)` after `timeBuffer += time` (the caller's): in
    /// an unrealized room `AbstractBehavior`, then a destination without a path is reset
    /// to where it stands.
    pub fn abstract_ai_update(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        let room = self.creature(id).pos.room;
        if self.realized(room).is_some() {
            return;
        }
        self.kind_abstract_behavior(id, time, cx);
        let rng = &mut *cx.rng;
        let pos = self.creature(id).pos;
        let Some(ai) = self.ai_of(id) else { return };
        if ai.destination.same_room_and_node(pos) && !ai.has_path_to(ai.destination, pos) {
            log::debug!(
                "abstract {} has a destination but no path to it",
                self.creature(id).kind()
            );
            self.set_destination(id, pos, rng);
            if let Some(ai) = self.ai_of_mut(id) {
                ai.migration_destination = None;
            }
        }
    }

    /// `AbstractBehavior` of an unrealized creature.
    pub(super) fn abstract_behavior(
        &mut self,
        id: CreatureId,
        time: i32,
        cx: &mut AbstractTick<'_>,
    ) {
        let rng = &mut *cx.rng;
        let c = self.creature(id);
        let t = c.template;
        let mut going_home = false;
        let stow = t.stow_food_in_den()
            && c.ai.as_ref().is_some_and(|ai| {
                ai.den_position(c, self.offscreen_den_index())
                    .is_some_and(|d| d != ai.destination)
            });
        if stow {
            let sticks: Vec<GripStick> = c.sticks.clone();
            for stick in sticks {
                // `stuckObjects[i].B` is a creature this template eats.
                if let Entity::Creature(prey) = stick.b
                    && t.eats(self.creature(prey).kind())
                {
                    self.go_to_den(id, rng);
                    going_home = true;
                }
            }
        }
        let follow = self.ai_of(id).and_then(|a| a.follow_creature);
        if let Some(other) = follow
            && !going_home
        {
            self.move_with_creature(id, other, false, rng);
        }
        if self.ai_of(id).is_some_and(|a| !a.path.is_empty()) {
            self.follow_path(id, time, cx);
        } else if !self.migration_behavior(id, time, cx.rng) {
            let chance = self.creature(id).template.roam_in_room_chance;
            if AbstractCreatureAi::time_influenced_roll(chance, time, cx.rng) {
                self.random_move_within_room(id, cx.rng);
            }
        }
    }

    /// `MigrationBehavior`: a creature following another moves with it while it lives
    /// (and stops once its like of it is below 0.9 either way); else, with a social
    /// memory, one relationship is drawn and a like or hate above 0.95 starts following
    /// that creature wherever it is out of a den. Then it rolls to leave the room once
    /// it has been there a while (`abstractedLaziness`), half the time on a random walk
    /// of rooms, else to a more attractive neighbour.
    pub(super) fn migration_behavior(
        &mut self,
        id: CreatureId,
        time: i32,
        rng: &mut UnityRandom,
    ) -> bool {
        let c = self.creature(id);
        let Some(ai) = &c.ai else { return false };
        if let Some(other) = ai.follow_creature {
            let o = self.creature(other);
            let subject = o.id;
            if o.state.alive && !o.slated_for_deletion {
                self.move_with_creature(id, other, false, rng);
                let stop = self
                    .creature(id)
                    .state
                    .social_memory
                    .as_ref()
                    .is_some_and(|m| m.get(subject).is_some_and(|r| r.like.abs() < 0.9));
                if stop {
                    log::debug!("{id:?} stopped following {other:?}");
                    if let Some(ai) = self.ai_of_mut(id) {
                        ai.follow_creature = None;
                    }
                }
                return true;
            }
            match &mut self.creature_mut(id).state.social_memory {
                Some(m) => m.discard(subject),
                // The game reads the social memory without a null check here; no base
                // creature without one follows a creature into this branch.
                None => log::warn!("{id:?} followed a dead creature without a social memory"),
            }
            if let Some(ai) = self.ai_of_mut(id) {
                ai.follow_creature = None;
            }
        } else if let Some(m) = &c.state.social_memory
            && !m.relationships.is_empty()
        {
            let n = i32::try_from(m.relationships.len()).unwrap_or(i32::MAX);
            let k = usize::try_from(rng.range_i32(0, n)).unwrap_or(0);
            let r = &m.relationships[k];
            if r.like.abs() > 0.95
                && let Some(found) = self.find_creature_out_of_dens(r.subject)
            {
                log::debug!("{id:?} following creature {found:?}");
                if let Some(ai) = self.ai_of_mut(id) {
                    ai.follow_creature = Some(found);
                }
                return true;
            }
        }
        let c = self.creature(id);
        let Some(ai) = &c.ai else { return false };
        let t = c.template;
        let pos = c.pos;
        if ai.migration_target(pos).room != pos.room {
            return false;
        }
        let leave = if self.attraction_for_room(pos.room, t.kind) == Some(RoomAttraction::Forbidden)
        {
            true
        } else {
            let lazy = c.time_spent_here > rng.range_i32(0, t.abstracted_laziness);
            lazy && {
                let value = self.attraction_value(pos.room, t.kind);
                let chance = t.roam_between_rooms_chance * 2.0.lerp(0.1, value);
                AbstractCreatureAi::time_influenced_roll(chance, time, rng)
            }
        };
        if !leave {
            return false;
        }
        if rng.value() < 0.5 {
            let max = (250.0 * rng.value().mathf_pow(0.25)) as i32;
            self.random_move_to_other_room(id, max, rng);
        } else {
            self.move_to_more_attractive_neighbor_room(id, rng);
        }
        true
    }

    /// The first creature with ID `subject` in the rooms' `creatures` lists, rooms in
    /// order (`EntityID ==`: the number).
    fn find_creature_out_of_dens(&self, subject: EntityId) -> Option<CreatureId> {
        let first = self.first_room_index();
        let rooms = i32::try_from(self.number_of_rooms()).unwrap_or(i32::MAX);
        (first..first + rooms)
            .flat_map(|r| self.room_creatures(r).iter().copied())
            .find(|&c| self.creature(c).id.same_entity(subject))
    }

    /// `MoveWithCreature`: towards the room of `other` (or of its destination) unless
    /// already going there.
    pub(super) fn move_with_creature(
        &mut self,
        id: CreatureId,
        other: CreatureId,
        to_destination: bool,
        rng: &mut UnityRandom,
    ) {
        let o = self.creature(other);
        let mut target = o.pos;
        if to_destination && let Some(a) = &o.ai {
            target = a.destination;
        }
        let c = self.creature(id);
        let Some(ai) = &c.ai else { return };
        if target.room != ai.destination.room || target.room != ai.migration_target(c.pos).room {
            self.try_to_go_to_room(id, target, rng);
        }
    }

    /// `TryToGoToRoom`: `coord` itself when the creature maps its node, else one of the
    /// room's mapped nodes at random.
    fn try_to_go_to_room(&mut self, id: CreatureId, coord: WorldCoordinate, rng: &mut UnityRandom) {
        let t = self.creature(id).template;
        let Some(room) = self.abstract_room(coord.room) else {
            return;
        };
        if coord.node_defined() && t.maps_room_node(room, coord.node) {
            self.internal_set_destination(id, coord, rng);
            return;
        }
        let list: Vec<WorldCoordinate> = (0..i32::try_from(room.nodes.len()).unwrap_or(0))
            .filter(|&i| t.maps_room_node(room, i))
            .map(|i| WorldCoordinate::new(coord.room, -1, -1, i))
            .collect();
        if !list.is_empty() {
            let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
            let k = usize::try_from(rng.range_i32(0, len)).unwrap_or(0);
            self.internal_set_destination(id, list[k], rng);
        }
    }

    /// `FollowPath`: once the time buffer covers the way to the creature's node and on
    /// to the path's next step, it moves there (`Move`) and pays the time; at a den node
    /// it may go in. A creature with no distance to its node and a full buffer is stuck:
    /// half the time it jumps to a random node it can reach.
    pub(super) fn follow_path(&mut self, id: CreatureId, time: i32, cx: &mut AbstractTick<'_>) {
        let t = self.creature(id).template;
        let speed = t.off_screen_speed * self.offscreen_speed_fac(id);
        let c = self.creature(id);
        let Some(ai) = &c.ai else { return };
        let pos = c.pos;
        if pos.node_defined()
            && c.distance_to_my_node > -1
            && ai.time_buffer > (c.distance_to_my_node as f32 / speed).to_int_cs()
        {
            if let Some(&next) = ai.path.last() {
                let mut next = next;
                if next == pos {
                    let Some(ai) = self.ai_of_mut(id) else { return };
                    ai.path.pop();
                    match ai.path.last() {
                        Some(&n) => next = n,
                        None => return,
                    }
                }
                let num = if next.room == pos.room {
                    self.connection_length(pos.room, pos.node, next.node, &t)
                } else {
                    match self.node_kind(pos) {
                        Some(NodeType::Exit | NodeType::Den) => {
                            self.data.total_shortcut_length_between(pos.room, next.room)
                        }
                        Some(k @ (NodeType::SkyExit | NodeType::SeaExit)) => {
                            self.highway_distance(k, pos, next)
                        }
                        _ => 0,
                    }
                };
                let buffer = self.ai_of(id).map_or(0, |a| a.time_buffer);
                if buffer as f32 > num as f32 / speed {
                    self.abstract_move(id, next, cx);
                    if let Some(ai) = self.ai_of_mut(id) {
                        ai.time_buffer -= (num as f32 / speed).to_int_cs();
                    }
                }
            }
            let pos = self.creature(id).pos;
            if pos.node_defined() && self.node_kind(pos) == Some(NodeType::Den) {
                self.opportunity_to_enter_den(id, cx.rng);
            }
        }
        let c = self.creature(id);
        let pos = c.pos;
        let buffer = c.ai.as_ref().map_or(0, |a| a.time_buffer);
        if pos.node_defined()
            && (c.distance_to_my_node >= 0
                || buffer <= 200
                || !AbstractCreatureAi::time_influenced_roll(0.5, time, cx.rng))
        {
            return;
        }
        let Some(room) = self.abstract_room(pos.room) else {
            return;
        };
        let list: Vec<i32> = (0..i32::try_from(room.nodes.len()).unwrap_or(0))
            .filter(|&i| {
                t.maps_room_node(room, i)
                    && (!pos.node_defined() || self.connection_possible(pos.room, pos.node, i, &t))
            })
            .collect();
        if list.is_empty() {
            return;
        }
        let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
        let k = usize::try_from(cx.rng.range_i32(0, len)).unwrap_or(0);
        let to = WorldCoordinate::new(pos.room, pos.tile.0, pos.tile.1, list[k]);
        self.abstract_move(id, to, cx);
        let pos = self.creature(id).pos;
        self.set_destination(id, pos, cx.rng);
        log::debug!(
            "abstract stuck {} moved to random accessible node {}",
            t.kind,
            pos.node
        );
        self.creature_mut(id).distance_to_my_node = 0;
    }

    /// `AbstractCreature.Move` of an abstract creature: `ChangeRooms` into a realized
    /// room would realize it (held abstract until its body is ported), then the room
    /// lists, position and `Moved`.
    pub(crate) fn abstract_move(
        &mut self,
        id: CreatureId,
        to: WorldCoordinate,
        cx: &mut AbstractTick<'_>,
    ) {
        self.change_rooms_realize(id, to, cx.cycle, cx.rng);
        self.move_creature(id, to);
    }

    /// `RandomMoveWithinRoom`: one node drawn; when it differs and the creature can go
    /// there and back, it is the destination and the whole path.
    fn random_move_within_room(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        let (pos, t) = (c.pos, c.template);
        let Some(room) = self.abstract_room(pos.room) else {
            return;
        };
        if room.nodes.is_empty() {
            return;
        }
        let n = i32::try_from(room.nodes.len()).unwrap_or(i32::MAX);
        let num = rng.range_i32(0, n);
        if num != pos.node && self.connection_and_back_possible(pos.room, pos.node, num, &t) {
            self.internal_set_destination(id, WorldCoordinate::new(room.index, -1, -1, num), rng);
            if let Some(ai) = self.ai_of_mut(id) {
                ai.path = vec![ai.destination];
            }
        }
    }

    /// `CanRoamThroughRoom`.
    fn can_roam_through(&self, room: i32, t: &CreatureTemplate) -> bool {
        !self.forbidden_for(room, t)
    }

    /// `RandomMoveToOtherRoom(maxRoamDistance)`: a random walk of up to 100 rooms, each
    /// step to a neighbour not on the walk yet that the creature can reach and come back
    /// from (one draw for a pick, then one weighted by size-dependent attraction that
    /// usually overrides it), until the in-room distance passes `max`. A walk ending in a
    /// less attractive room than this one is cut back to its most attractive end.
    fn random_move_to_other_room(&mut self, id: CreatureId, max: i32, rng: &mut UnityRandom) {
        let c = self.creature(id);
        let (pos, t) = (c.pos, c.template);
        if self
            .abstract_room(pos.room)
            .is_none_or(|r| r.connections.is_empty())
        {
            return;
        }
        let mut here = self.define_node(pos, &t, rng);
        let mut list: Vec<WorldCoordinate> = Vec::new();
        let mut distance = 0;
        for _ in 0..100 {
            let Some(room) = self.abstract_room(here.room) else {
                break;
            };
            let mut options: Vec<WorldCoordinate> = Vec::new();
            for (j, &conn) in room.connections.iter().enumerate() {
                if conn <= -1 {
                    continue;
                }
                let free = self.can_roam_through(conn, &t) && !list.iter().any(|l| l.room == conn);
                if !free {
                    continue;
                }
                let Some(other) = self.abstract_room(conn) else {
                    continue;
                };
                let entrance = WorldCoordinate::new(conn, -1, -1, other.exit_index(room.index));
                let j = i32::try_from(j).unwrap_or(-1);
                if !self.connection_and_back_possible(room.index, here.node, j, &t) {
                    continue;
                }
                let Some(node) = usize::try_from(entrance.node)
                    .ok()
                    .and_then(|n| other.nodes.get(n))
                else {
                    log::warn!("{} has no exit back to {}", other.name, room.name);
                    continue;
                };
                if t.water_relationship
                    .abstract_submersion_legal(node.submerged)
                {
                    options.push(entrance);
                }
            }
            if options.is_empty() {
                break;
            }
            let len = i32::try_from(options.len()).unwrap_or(i32::MAX);
            let mut pick = options[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)];
            let total: f32 = options
                .iter()
                .map(|o| self.size_dependent_attraction(o.room, t.kind))
                .sum();
            let mut roll = rng.value() * total;
            for o in &options {
                let w = self.size_dependent_attraction(o.room, t.kind);
                if roll < w {
                    pick = *o;
                    break;
                }
                roll -= w;
            }
            let exit = room.exit_index(pick.room);
            list.insert(0, WorldCoordinate::new(room.index, -1, -1, exit));
            list.insert(0, pick);
            if here.node != exit {
                distance += usize::try_from(here.node)
                    .ok()
                    .and_then(|n| room.nodes.get(n))
                    .map_or(-1, |n| {
                        n.connection_length(exit, t.pre_baked_pathing_index())
                    });
            }
            if distance > max {
                break;
            }
            here = pick;
        }
        let here_attraction = self.size_dependent_attraction(pos.room, t.kind);
        if list
            .first()
            .is_some_and(|l| self.size_dependent_attraction(l.room, t.kind) < here_attraction)
        {
            while list.len() > 1
                && (list[0].room == list[1].room
                    || self.size_dependent_attraction(list[0].room, t.kind)
                        < self.size_dependent_attraction(list[1].room, t.kind))
            {
                list.remove(0);
            }
        }
        if let Some(&dest) = list.first() {
            if let Some(ai) = self.ai_of_mut(id) {
                ai.path = list;
            }
            self.internal_set_destination(id, dest, rng);
        }
    }

    /// `MoveToMoreAttractiveNeighborRoom`: unless the room is a Stay room, a neighbour
    /// weighted by size-dependent attraction (a tenth for one less attractive than
    /// here) among those more attractive than here, each other one with a 1 in 40 roll.
    pub(crate) fn move_to_more_attractive_neighbor_room(
        &mut self,
        id: CreatureId,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        let (pos, t) = (c.pos, c.template);
        if self.attraction(pos.room, t.kind) == Some(RoomAttraction::Stay) {
            return;
        }
        let Some(room) = self.abstract_room(pos.room) else {
            return;
        };
        let here_size = self.size_dependent_attraction(pos.room, t.kind);
        let here_value = self.attraction_value(pos.room, t.kind);
        let mut weights: Vec<f32> = Vec::new();
        let mut rooms: Vec<i32> = Vec::new();
        let mut total = 0.0;
        for (i, &conn) in room.connections.iter().enumerate() {
            let i = i32::try_from(i).unwrap_or(-1);
            let candidate = conn > -1
                && (pos.node == i || self.connection_possible(pos.room, pos.node, i, &t))
                && self.can_roam_through(conn, &t)
                && (rng.value() < 0.025 || self.attraction_value(conn, t.kind) > here_value);
            if !candidate {
                continue;
            }
            let mut w = self.size_dependent_attraction(conn, t.kind);
            if w < here_size {
                w /= 10.0;
            }
            total += w;
            weights.push(w);
            rooms.push(conn);
        }
        let mut roll = total * rng.value();
        for (k, &r) in rooms.iter().enumerate() {
            if roll < weights[k] {
                let entrance = self.abstract_room(r).map_or(-1, |o| o.exit_index(pos.room));
                self.internal_set_destination(id, WorldCoordinate::new(r, -1, -1, entrance), rng);
                break;
            }
            roll -= weights[k];
        }
    }

    /// `InternalSetDestination` of an unrealized creature (`SetDestination`).
    fn internal_set_destination(
        &mut self,
        id: CreatureId,
        dest: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        self.set_destination(id, dest, rng);
    }

    /// `MigrateTo`.
    pub fn migrate_to(&mut self, id: CreatureId, dest: WorldCoordinate, rng: &mut UnityRandom) {
        self.internal_set_destination(id, dest, rng);
    }

    /// `SetDestination` of an abstract creature: a dropped realized AI's flag cleared
    /// when the destination's room is not realized; without a path there, the node is
    /// defined and a path searched (`FindPath`); with a path the destination is set and
    /// a new room becomes the migration destination.
    pub fn set_destination(
        &mut self,
        id: CreatureId,
        dest: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        let t = c.template;
        let pos = c.pos;
        let Some(ai) = &c.ai else { return };
        if ai.freeze_destination {
            return;
        }
        let realized_dest = self.realized(dest.room).is_some();
        let has_path = ai.has_path_to(dest, pos);
        if let Some(ai) = self.ai_of_mut(id)
            && ai.real_ai
            && !realized_dest
        {
            ai.real_ai = false;
        }
        let mut dest = dest;
        if !has_path {
            dest = self.define_node(dest, &t, rng);
            self.find_path(id, dest, rng);
        }
        let pos = self.creature(id).pos;
        if let Some(ai) = self.ai_of_mut(id)
            && !ai.path.is_empty()
        {
            ai.destination = dest;
            if dest.room != pos.room {
                ai.migration_destination = Some(dest);
            }
        }
    }

    /// `FindPath`: in the same room with a connection the path is the destination;
    /// else `AbstractSpacePathFinder.Path`, empty when it finds none. Defines the
    /// creature's own node first.
    fn find_path(&mut self, id: CreatureId, dest: WorldCoordinate, rng: &mut UnityRandom) {
        let t = self.creature(id).template;
        let dest = self.define_node(dest, &t, rng);
        let pos = self.creature(id).pos;
        let pos = self.define_node(pos, &t, rng);
        self.creature_mut(id).pos = pos;
        if dest.room == pos.room && self.connection_possible(pos.room, pos.node, dest.node, &t) {
            if let Some(ai) = self.ai_of_mut(id) {
                ai.path = vec![dest];
            }
            return;
        }
        if self.ai_of(id).is_some_and(|a| a.has_path_to(dest, pos)) {
            return;
        }
        let finder = AbstractSpacePathFinder {
            world: self,
            template: &t,
        };
        let path = finder.path(pos, dest, rng).unwrap_or_else(|| {
            log::warn!("NO PATH TO DESTINATION! {}", t.kind);
            Vec::new()
        });
        if let Some(ai) = self.ai_of_mut(id) {
            ai.path = path;
        }
    }

    /// `GoToDen`: to the den position, else (once per room it is stranded in) the
    /// closest den by `AbstractSpaceNodeFinder`.
    pub fn go_to_den(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let c = self.creature(id);
        let t = c.template;
        let pos = c.pos;
        let Some(ai) = &c.ai else { return };
        if let Some(den) = ai.den_position(c, self.offscreen_den_index()) {
            self.set_destination(id, den, rng);
            if let Some(ai) = self.ai_of_mut(id) {
                ai.stranded_in_room = -1;
            }
            return;
        }
        if pos.room == ai.stranded_in_room {
            return;
        }
        let start = self.define_node(pos, &t, rng);
        let finder = AbstractSpaceNodeFinder {
            world: self,
            template: &t,
            searching_for: SearchingFor::Den,
            flood_method: FloodMethod::Cost,
        };
        match finder.run(start, rng) {
            Some(path) => {
                let den = path[0];
                if let Some(ai) = self.ai_of_mut(id) {
                    ai.path = path;
                    ai.den_position = Some(den);
                }
                self.set_destination(id, den, rng);
            }
            None => {
                if let Some(ai) = self.ai_of_mut(id) {
                    ai.stranded_in_room = pos.room;
                }
            }
        }
    }

    /// `AbstractCreature.OpportunityToEnterDen` at its den node: in when it wants to
    /// stay there for the rest of the cycle (counter -1) or still has time to wait, or
    /// with prey to stow (counter 200). Precycles and `doesNotUseDens` (only DLC
    /// templates set it) are DLC.
    pub(crate) fn opportunity_to_enter_den(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        if self.wants_to_stay_in_den(id) {
            log::debug!("{} stay in den until cycle end", self.creature(id).kind());
            self.creature_mut(id).remain_in_den_counter = -1;
            self.move_entity_to_den(id, rng);
            return;
        }
        let c = self.creature(id);
        if c.remain_in_den_counter > 0 {
            self.move_entity_to_den(id, rng);
        } else if c.template.stow_food_in_den() {
            let carrying = c
                .sticks
                .iter()
                .any(|s| s.a == id && matches!(s.b, Entity::Creature(_)));
            if carrying {
                self.creature_mut(id).remain_in_den_counter = 200;
                self.move_entity_to_den(id, rng);
            }
        }
    }
}

#[cfg(test)]
#[path = "abstract_ai_tests.rs"]
mod tests;
