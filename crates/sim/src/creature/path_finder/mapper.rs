//! The world's path finders together: `PathFinder.AccessibilityMapper` (the flood
//! that marks the cells a creature can reach and come back from, shared and merged
//! between creatures with the same pre-baked pathing in one room), the
//! `PathfinderResourceDivider` budgets, and the path finder operations that reach
//! other path finders or the creature (`Update`, `Reset`, `InitiAccessibilityMapping`,
//! `AccessibilityMappingDone`, `OutOfElement`, `LizardPather.FollowPath`).
//!
//! A path finder lives here by a number while its creature's AI holds it; a dropped AI's
//! path finder stays while a mapper still counts it as a client, as the game's
//! reference does, and goes with the mapper.

use std::collections::BTreeMap;

use rw_random::UnityRandom;

use super::{PathEnv, PathFinder, PatherKind};
use crate::creature::{CreatureId, WorldCoordinate};
use crate::world::{Accessibility, Legality, MovementConnection, MovementType, PathCost, World};

/// `PathfinderResourceDivider`: the frame's path steps (50) and accessibility steps
/// (500) shared out by what was asked for last frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathfinderResourceDivider {
    last_total_requested_updates: i32,
    requested_updates: i32,
    budget: i32,
    last_total_requested_acc_updates: i32,
    requested_acc_updates: i32,
    accessibility_budget: i32,
}

impl Default for PathfinderResourceDivider {
    fn default() -> Self {
        Self {
            last_total_requested_updates: 0,
            requested_updates: 0,
            budget: 50,
            last_total_requested_acc_updates: 0,
            requested_acc_updates: 0,
            accessibility_budget: 500,
        }
    }
}

impl PathfinderResourceDivider {
    pub fn update(&mut self) {
        self.last_total_requested_updates = self.requested_updates;
        self.requested_updates = 0;
        self.last_total_requested_acc_updates = self.requested_acc_updates;
        self.requested_acc_updates = 0;
    }

    /// `RequestPathfinderUpdates`.
    pub fn request_pathfinder_updates(&mut self, ideal: i32) -> i32 {
        self.requested_updates += ideal;
        Self::share(ideal, self.last_total_requested_updates, self.budget)
    }

    /// `RequesAccesibilityUpdates`.
    pub fn request_accessibility_updates(&mut self, ideal: i32) -> i32 {
        self.requested_acc_updates += ideal;
        Self::share(
            ideal,
            self.last_total_requested_acc_updates,
            self.accessibility_budget,
        )
    }

    fn share(ideal: i32, last_total: i32, budget: i32) -> i32 {
        if last_total <= budget {
            return ideal;
        }
        let share = ideal as f32 / last_total as f32 * budget as f32;
        (share.floor() as i32).max(1)
    }
}

/// `PathFinder.AccessibilityMapper`.
#[derive(Debug, Clone, PartialEq)]
pub struct AccessibilityMapper {
    /// Path finder numbers; the first is the parent whose cells are read.
    pub clients: Vec<usize>,
    check_next: Vec<WorldCoordinate>,
    /// `mapAccessibilityMode`: 1 seed, 2 reach, 3 reseed, 4 get back, 5 done.
    pub mode: i32,
    already_checked: Option<Vec<(i32, i32)>>,
    pub tainted_by_forbidden_node: bool,
    pub mapping_room: Option<i32>,
    potential_mergers: Vec<usize>,
}

/// Every path finder of the world and their accessibility mappers.
#[derive(Debug, Default)]
pub struct PathFinders {
    finders: BTreeMap<usize, PathFinder>,
    /// Path finders whose AI was dropped while a mapper held them.
    released: Vec<usize>,
    next_finder: usize,
    mappers: BTreeMap<usize, AccessibilityMapper>,
    next_mapper: usize,
    pub divider: PathfinderResourceDivider,
}

impl PathFinders {
    /// A new path finder for `creature` (its cells' draws).
    pub fn create(
        &mut self,
        world: &World,
        creature: CreatureId,
        kind: PatherKind,
        rng: &mut UnityRandom,
    ) -> usize {
        let pf = PathFinder::new(world, creature, kind, rng);
        let id = self.next_finder;
        self.next_finder += 1;
        self.finders.insert(id, pf);
        id
    }

    #[must_use]
    pub fn get(&self, id: usize) -> Option<&PathFinder> {
        self.finders.get(&id)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut PathFinder> {
        self.finders.get_mut(&id)
    }

    #[must_use]
    pub fn mapper(&self, id: usize) -> Option<&AccessibilityMapper> {
        self.mappers.get(&id)
    }

    /// The AI holding path finder `id` is gone.
    pub fn release(&mut self, id: usize) {
        if self.finders.get(&id).is_some_and(|f| f.mapper.is_some()) {
            self.released.push(id);
        } else {
            self.finders.remove(&id);
        }
    }

    /// Mappers no held path finder steps any more are dropped, with the released path
    /// finders that waited on them.
    fn collect_garbage(&mut self) {
        let live: Vec<usize> = self
            .finders
            .iter()
            .filter(|(id, _)| !self.released.contains(id))
            .filter_map(|(_, f)| f.mapper)
            .collect();
        self.mappers.retain(|id, _| live.contains(id));
        let mappers = &self.mappers;
        let finders = &mut self.finders;
        self.released.retain(|id| {
            let keep = finders
                .get(id)
                .and_then(|f| f.mapper)
                .is_some_and(|m| mappers.contains_key(&m));
            if !keep {
                finders.remove(id);
            }
            keep
        });
    }

    /// The path finders of the AIs of the creatures in global room `room`, in the room's
    /// creature order.
    fn room_path_finders(world: &World, room: i32) -> Vec<usize> {
        world
            .room_creatures(room)
            .iter()
            .filter_map(|&c| world.path_finder_of(c))
            .collect()
    }

    fn env<'a>(&self, world: &'a World, id: usize) -> PathEnv<'a> {
        let pos = self
            .finders
            .get(&id)
            .map_or(WorldCoordinate::new(-1, -1, -1, -1), |f| {
                world.creature(f.creature).pos
            });
        PathEnv { world, pos }
    }

    /// `PathFinder.Update`: the accessibility mapper's steps (its client's share of the
    /// accessibility budget) and then one path step, or with no mapper the forbidden
    /// entrance count down and the path finder's share of path steps.
    pub fn update(&mut self, id: usize, world: &mut World, rng: &mut UnityRandom) {
        let Some(pf) = self.finders.get_mut(&id) else {
            return;
        };
        pf.update_start();
        let mut mapping = false;
        if pf.mapper.is_some() {
            mapping = true;
            let ideal = pf.accessibility_steps_per_frame;
            let mut n = self.divider.request_accessibility_updates(ideal);
            while n > 0
                && let Some(m) = self.finders.get(&id).and_then(|f| f.mapper)
            {
                self.mapper_step(m, world, rng);
                n -= 1;
            }
        } else if pf.forbidden_entrance_counter > 0 {
            pf.forbidden_entrance_counter -= 1;
        }
        let steps = if mapping {
            1
        } else {
            let ideal = self.finders.get(&id).map_or(0, |f| f.steps_per_frame);
            self.divider.request_pathfinder_updates(ideal)
        };
        for _ in 0..steps {
            let env = self.env(world, id);
            if let Some(pf) = self.finders.get_mut(&id) {
                pf.path_step(&env, rng);
            }
        }
        self.collect_garbage();
    }

    /// `NewRoom`: a path finder in another room than its realized room is `Reset`.
    pub fn new_room(&mut self, id: usize, world: &mut World, room: i32, rng: &mut UnityRandom) {
        if self
            .finders
            .get(&id)
            .is_some_and(|f| f.realized_room != Some(room))
        {
            self.reset(id, world, room, rng);
        }
    }

    /// `Reset(room)`: new room cells, the old mapper culled, then the path from the node
    /// the creature came in by (`InitiatePath`) or the accessibility mapping from where
    /// it stands.
    pub fn reset(&mut self, id: usize, world: &mut World, room: i32, rng: &mut UnityRandom) {
        let Some(map) = world.realized(room).and_then(|r| r.ai_map.clone()) else {
            log::warn!("AIMAP NULL FOR ROOM {room}");
            return;
        };
        let env = self.env(world, id);
        let pos = env.pos;
        let Some(pf) = self.finders.get_mut(&id) else {
            return;
        };
        pf.reset_cells(room, map, pos, rng);
        if let Some(m) = pf.mapper {
            self.cull_clients(m, Some(id), world, rng);
        }
        if let Some(pf) = self.finders.get_mut(&id) {
            pf.mapper = None;
        }
        let mut started = None;
        if !pos.tile_defined() && pos.node_defined() {
            let env = self.env(world, id);
            started = self
                .finders
                .get_mut(&id)
                .and_then(|pf| pf.initiate_path(&env, pos.node, rng));
        }
        match started {
            Some((entrance, path)) => {
                self.initi_accessibility_mapping(id, world, entrance, path, rng);
            }
            None => self.initi_accessibility_mapping(id, world, pos, None, rng),
        }
        if let Some(pf) = self.finders.get_mut(&id) {
            pf.re_assign_destination_once_accessibility_mapping_is_done = true;
        }
    }

    /// `InitiAccessibilityMapping`: a creature of the entrance's room with the same
    /// pathing, in the same realized room and reaching the entrance, hands over its
    /// finished map or takes this one into its running untainted mapper (with the same
    /// forbidden entrance); else a new mapper starts here.
    pub fn initi_accessibility_mapping(
        &mut self,
        id: usize,
        world: &mut World,
        entrance: WorldCoordinate,
        already: Option<Vec<(i32, i32)>>,
        rng: &mut UnityRandom,
    ) {
        let Some(me) = self.finders.get(&id) else {
            return;
        };
        if world.abstract_room(entrance.room).is_some() {
            for other in Self::room_path_finders(world, entrance.room) {
                let Some(o) = self.finders.get(&other) else {
                    continue;
                };
                if !Self::pathing_identical(me, o)
                    || other == id
                    || o.realized_room.is_none()
                    || o.realized_room != me.realized_room
                    || !o.coordinate_reachable_and_getbackable(entrance)
                {
                    continue;
                }
                if o.mapper.is_none() {
                    log::debug!("acc map handover {:?} from {:?}", me.creature, o.creature);
                    self.copy_reachability(other, id);
                    self.accessibility_mapping_done(id, world, rng);
                    return;
                }
                let m = o.mapper.unwrap_or(0);
                if self
                    .mappers
                    .get(&m)
                    .is_some_and(|m| !m.tainted_by_forbidden_node)
                    && o.forbidden_entrance == me.forbidden_entrance
                {
                    self.add_fresh_client(m, id);
                    return;
                }
            }
        }
        let tainted = me.forbidden_node.is_some();
        self.new_mapper(id, world, entrance, already, tainted, rng);
    }

    /// `CreaturesPathingIdentical` with the cell arrays' sizes the callers compare.
    fn pathing_identical(a: &PathFinder, b: &PathFinder) -> bool {
        a.template.pre_baked_pathing_index() == b.template.pre_baked_pathing_index()
            && a.cells().0.len() == b.cells().0.len()
            && a.covered_area.width() == b.covered_area.width()
            && a.covered_area.height() == b.covered_area.height()
    }

    /// The handover: `to`'s reachability flags become `from`'s.
    fn copy_reachability(&mut self, from: usize, to: usize) {
        let Some(src) = self.finders.get(&from) else {
            return;
        };
        let (world_cells, room_cells) = src.cells();
        let flags: Vec<Vec<(bool, bool)>> = world_cells
            .iter()
            .map(|r| {
                r.iter()
                    .map(|c| (c.reachable, c.possible_to_get_back_from))
                    .collect()
            })
            .collect();
        let room_flags: Vec<(bool, bool)> = room_cells
            .iter()
            .map(|c| (c.reachable, c.possible_to_get_back_from))
            .collect();
        let src_room = src.realized_room.is_some();
        let Some(dst) = self.finders.get_mut(&to) else {
            return;
        };
        let dst_room = dst.realized_room.is_some();
        let (world_cells, room_cells) = dst.cells_mut();
        for (row, flags) in world_cells.iter_mut().zip(&flags) {
            for (c, &(r, g)) in row.iter_mut().zip(flags) {
                c.reachable = r;
                c.possible_to_get_back_from = g;
            }
        }
        if src_room && dst_room {
            for (c, &(r, g)) in room_cells.iter_mut().zip(&room_flags) {
                c.reachable = r;
                c.possible_to_get_back_from = g;
            }
        }
    }

    /// The `AccessibilityMapper` constructor: centred on `center`, the first mode run.
    fn new_mapper(
        &mut self,
        id: usize,
        world: &mut World,
        center: WorldCoordinate,
        already: Option<Vec<(i32, i32)>>,
        tainted: bool,
        rng: &mut UnityRandom,
    ) {
        let Some(pf) = self.finders.get_mut(&id) else {
            return;
        };
        pf.accessibility_center = center;
        let m = AccessibilityMapper {
            clients: vec![id],
            check_next: Vec::new(),
            mode: 1,
            already_checked: already,
            tainted_by_forbidden_node: tainted,
            mapping_room: pf.realized_room,
            potential_mergers: Vec::new(),
        };
        let mid = self.next_mapper;
        self.next_mapper += 1;
        self.mappers.insert(mid, m);
        pf.mapper = Some(mid);
        self.mapper_step(mid, world, rng);
    }

    /// `AddFreshClient`: the parent's flags copied, then it joins.
    fn add_fresh_client(&mut self, mid: usize, client: usize) {
        let Some(parent) = self
            .mappers
            .get(&mid)
            .and_then(|m| m.clients.first().copied())
        else {
            return;
        };
        self.copy_reachability(parent, client);
        if let Some(m) = self.mappers.get_mut(&mid) {
            m.clients.push(client);
        }
        if let Some(pf) = self.finders.get_mut(&client) {
            pf.mapper = Some(mid);
        }
    }

    /// `UpdateAccessibilityMapping`: one step of the flood. Mode 1 clears the parent's
    /// flags and seeds from the centre (and a quick path's tiles, reachable and
    /// getbackable at once); mode 2 marks what the creature reaches out of each queued
    /// coordinate; mode 3 seeds again; mode 4 marks what reaches each queued coordinate
    /// (the way back); mode 5 tells every client it is done (a mapper tainted by a
    /// forbidden node then starts again without it). Modes 2 and 4 check for mergers.
    fn mapper_step(&mut self, mid: usize, world: &mut World, rng: &mut UnityRandom) {
        let Some(m) = self.mappers.get(&mid) else {
            return;
        };
        let Some(&parent) = m.clients.first() else {
            return;
        };
        match m.mode {
            1 => self.mapper_seed(mid, parent, world, true),
            2 => self.mapper_flood(mid, parent, world, true),
            3 => self.mapper_seed(mid, parent, world, false),
            4 => self.mapper_flood(mid, parent, world, false),
            5 => self.mapper_done(mid, world, rng),
            _ => {}
        }
        let mode = self.mappers.get(&mid).map_or(0, |m| m.mode);
        if mode == 2 || mode == 4 {
            self.check_merge(mid, world, rng);
        }
    }

    /// Modes 1 and 3.
    fn mapper_seed(&mut self, mid: usize, parent: usize, world: &World, first: bool) {
        let Some(m) = self.mappers.get(&mid) else {
            return;
        };
        let room = m.mapping_room.unwrap_or(-1);
        let has_room = m.mapping_room.is_some();
        let clients = m.clients.clone();
        let already = m.already_checked.clone().unwrap_or_default();
        let Some(pf) = self.finders.get_mut(&parent) else {
            return;
        };
        let center = pf.accessibility_center;
        if first {
            pf.clear_reachability(has_room);
        }
        let mut queue = vec![center];
        let len = already.len();
        for (n, &(x, y)) in already.iter().enumerate() {
            let coord = WorldCoordinate::new(room, x, y, -1);
            queue.push(coord);
            if first {
                let Some(pf) = self.finders.get_mut(&parent) else {
                    continue;
                };
                let generation = pf.path_generation;
                let r = pf.cell_ref(coord);
                let cell = pf.cell_mut(r);
                cell.reachable = true;
                cell.possible_to_get_back_from = true;
                cell.cost_to_goal = PathCost::new((len - n) as f32, Legality::Allowed);
                cell.generation = generation;
                continue;
            }
            for c in &clients {
                if let Some(f) = self.finders.get_mut(c) {
                    let r = f.cell_ref(coord);
                    let cell = f.cell_mut(r);
                    cell.reachable = true;
                    cell.possible_to_get_back_from = true;
                }
            }
        }
        if let Some(m) = self.mappers.get_mut(&mid) {
            m.check_next = queue;
            m.mode = if first { 2 } else { 4 };
        }
        self.update_potential_mergers(mid, world);
    }

    /// Modes 2 and 4.
    fn mapper_flood(&mut self, mid: usize, parent: usize, world: &World, out: bool) {
        let Some(m) = self.mappers.get_mut(&mid) else {
            return;
        };
        if m.check_next.is_empty() {
            m.mode = if out { 3 } else { 5 };
            return;
        }
        let coord = m.check_next.remove(0);
        let clients = m.clients.clone();
        let Some(pf) = self.finders.get(&parent) else {
            return;
        };
        let Some(map) = pf.map().cloned() else {
            return;
        };
        let template = pf.template;
        let between_rooms = template
            .connection_resistance(MovementType::BetweenRooms)
            .allowed();
        let connections = pf.connections(world, out, coord);
        for c in connections {
            let pf = &self.finders[&parent];
            let far = if out { c.dest } else { c.start };
            let done = if out {
                pf.coordinate_reachable(far)
            } else {
                pf.coordinate_possible_to_get_back_from(far)
            };
            if done
                || !map.is_connection_allowed_for_creature(&c, &template)
                || !(c.start.room == c.dest.room || between_rooms)
                || !pf.coordinate_cost(far).allowed()
            {
                continue;
            }
            if let Some(m) = self.mappers.get_mut(&mid) {
                m.check_next.push(far);
            }
            for &cl in &clients {
                if let Some(f) = self.finders.get_mut(&cl) {
                    let r = f.cell_ref(far);
                    let cell = f.cell_mut(r);
                    if out {
                        cell.reachable = true;
                    } else {
                        cell.possible_to_get_back_from = true;
                    }
                }
            }
        }
    }

    /// Mode 5.
    fn mapper_done(&mut self, mid: usize, world: &mut World, rng: &mut UnityRandom) {
        let Some(m) = self.mappers.get(&mid) else {
            return;
        };
        let clients = m.clients.clone();
        let tainted = m.tainted_by_forbidden_node;
        let already = m.already_checked.clone();
        for c in clients {
            self.accessibility_mapping_done(c, world, rng);
            let Some(pf) = self.finders.get_mut(&c) else {
                continue;
            };
            pf.mapper = None;
            if tainted {
                pf.forbidden_node = None;
                let pos = world.creature(pf.creature).pos;
                self.initi_accessibility_mapping(c, world, pos, already.clone(), rng);
            }
        }
    }

    /// `CullClients`: clients in another realized room leave the mapper; those it still
    /// drives map again from where they stand (not `keep`).
    fn cull_clients(
        &mut self,
        mid: usize,
        keep: Option<usize>,
        world: &mut World,
        rng: &mut UnityRandom,
    ) {
        let Some(m) = self.mappers.get(&mid) else {
            return;
        };
        let room = m.mapping_room;
        for i in (0..m.clients.len()).rev() {
            let Some(&c) = self.mappers.get(&mid).and_then(|m| m.clients.get(i)) else {
                continue;
            };
            let Some(pf) = self.finders.get_mut(&c) else {
                continue;
            };
            if pf.realized_room == room {
                continue;
            }
            if pf.mapper == Some(mid) {
                pf.mapper = None;
                if Some(c) != keep {
                    let pos = world.creature(pf.creature).pos;
                    self.initi_accessibility_mapping(c, world, pos, Some(Vec::new()), rng);
                }
            }
            if let Some(m) = self.mappers.get_mut(&mid)
                && i < m.clients.len()
            {
                m.clients.remove(i);
            }
        }
    }

    /// `UpdatePotentialMergers`: the path finders of the mapping room's creatures with
    /// the same pathing, driven by another untainted mapper in the same mode.
    fn update_potential_mergers(&mut self, mid: usize, world: &World) {
        let Some(m) = self.mappers.get(&mid) else {
            return;
        };
        let mut found = Vec::new();
        if let (Some(room), false) = (m.mapping_room, m.tainted_by_forbidden_node)
            && let Some(parent) = m.clients.first().and_then(|p| self.finders.get(p))
        {
            for other in Self::room_path_finders(world, room) {
                let Some(o) = self.finders.get(&other) else {
                    continue;
                };
                let Some(om) = o.mapper.filter(|&x| x != mid) else {
                    continue;
                };
                let Some(other_mapper) = self.mappers.get(&om) else {
                    continue;
                };
                if !Self::pathing_identical(parent, o)
                    || o.realized_room != Some(room)
                    || other_mapper.mode != m.mode
                    || other_mapper.tainted_by_forbidden_node
                    || m.clients.contains(&other)
                    || found.contains(&other)
                {
                    continue;
                }
                found.push(other);
            }
        }
        if let Some(m) = self.mappers.get_mut(&mid) {
            m.potential_mergers = found;
        }
    }

    /// `CheckMerge`: a potential merger whose centre the parent reaches (mode 2) or
    /// reaches and gets back from (mode 4) is absorbed.
    fn check_merge(&mut self, mid: usize, world: &mut World, rng: &mut UnityRandom) {
        let Some(m) = self.mappers.get(&mid) else {
            return;
        };
        for i in (0..m.potential_mergers.len()).rev() {
            let Some(m) = self.mappers.get(&mid) else {
                return;
            };
            let Some(&other) = m.potential_mergers.get(i) else {
                continue;
            };
            let mode = m.mode;
            let room = m.mapping_room;
            let parent = m.clients.first().copied();
            let o = self.finders.get(&other);
            let om = o.and_then(|o| o.mapper);
            let valid = o.is_some_and(|o| o.realized_room == room)
                && om
                    .and_then(|x| self.mappers.get(&x))
                    .is_some_and(|x| x.mode == mode);
            if !valid {
                if let Some(m) = self.mappers.get_mut(&mid) {
                    m.potential_mergers.remove(i);
                }
                continue;
            }
            let center = o.map_or(WorldCoordinate::new(-1, -1, -1, -1), |o| {
                o.accessibility_center
            });
            let p = parent.and_then(|p| self.finders.get(&p));
            let merge = p.is_some_and(|p| {
                (mode == 2 && p.coordinate_reachable(center))
                    || (mode == 4 && p.coordinate_reachable_and_getbackable(center))
            });
            if merge {
                if let Some(m) = self.mappers.get_mut(&mid) {
                    m.potential_mergers.remove(i);
                }
                if let Some(om) = om {
                    self.absorb(mid, om, world, rng);
                }
            }
        }
    }

    /// `Absorb`: after culling both, the other mapper's clients join this one, every
    /// client's flags become the two parents' together, and the other's queue is added.
    fn absorb(&mut self, mid: usize, other: usize, world: &mut World, rng: &mut UnityRandom) {
        if mid == other {
            return;
        }
        self.cull_clients(mid, None, world, rng);
        self.cull_clients(other, None, world, rng);
        let (Some(a), Some(b)) = (self.mappers.get(&mid), self.mappers.get(&other)) else {
            return;
        };
        if a.clients.is_empty() || b.clients.is_empty() {
            log::debug!("cancel mapper merge");
            return;
        }
        let parent = a.clients[0];
        let other_parent = b.clients[0];
        let other_clients = b.clients.clone();
        let other_queue = b.check_next.clone();
        let both_rooms = a.mapping_room.is_some() && b.mapping_room.is_some();
        for c in &other_clients {
            if let Some(f) = self.finders.get_mut(c) {
                f.mapper = Some(mid);
            }
            if let Some(m) = self.mappers.get_mut(&mid)
                && !m.clients.contains(c)
            {
                m.clients.push(*c);
            }
        }
        let union = self.union_flags(parent, other_parent);
        let clients = self
            .mappers
            .get(&mid)
            .map(|m| m.clients.clone())
            .unwrap_or_default();
        for c in clients {
            let Some(f) = self.finders.get_mut(&c) else {
                continue;
            };
            let (world_cells, room_cells) = f.cells_mut();
            for (row, flags) in world_cells.iter_mut().zip(&union.0) {
                for (cell, &(r, g)) in row.iter_mut().zip(flags) {
                    cell.reachable = r;
                    cell.possible_to_get_back_from = g;
                }
            }
            if both_rooms {
                for (cell, &(r, g)) in room_cells.iter_mut().zip(&union.1) {
                    cell.reachable = r;
                    cell.possible_to_get_back_from = g;
                }
            }
        }
        if let Some(m) = self.mappers.get_mut(&mid) {
            for q in other_queue {
                if !m.check_next.contains(&q) {
                    m.check_next.push(q);
                }
            }
        }
        log::debug!("Mapper merging");
    }

    /// The two path finders' flags or-ed, over the cells both have.
    #[allow(clippy::type_complexity)]
    fn union_flags(&self, a: usize, b: usize) -> (Vec<Vec<(bool, bool)>>, Vec<(bool, bool)>) {
        let (Some(a), Some(b)) = (self.finders.get(&a), self.finders.get(&b)) else {
            return (Vec::new(), Vec::new());
        };
        let (aw, ar) = a.cells();
        let (bw, br) = b.cells();
        let world = aw
            .iter()
            .zip(bw)
            .map(|(x, y)| {
                x.iter()
                    .zip(y)
                    .map(|(p, q)| {
                        (
                            p.reachable || q.reachable,
                            p.possible_to_get_back_from || q.possible_to_get_back_from,
                        )
                    })
                    .collect()
            })
            .collect();
        let room = ar
            .iter()
            .zip(br)
            .map(|(p, q)| {
                (
                    p.reachable || q.reachable,
                    p.possible_to_get_back_from || q.possible_to_get_back_from,
                )
            })
            .collect();
        (world, room)
    }

    /// `AccessibilityMappingDone`: the destination assigned again; a creature on a node
    /// it cannot use is put on a random reachable, getbackable one it maps (a draw);
    /// if its node is still not reachable and getbackable it is stranded unless some
    /// node is reachable or an exit is reachable from its tile or the mapping centre,
    /// which becomes its node. The AI's `NewArea` takes the stranding.
    fn accessibility_mapping_done(&mut self, id: usize, world: &mut World, rng: &mut UnityRandom) {
        let env = self.env(world, id);
        let Some(pf) = self.finders.get_mut(&id) else {
            return;
        };
        let dest = pf.destination;
        pf.assign_new_destination(&env, dest, rng);
        let creature = pf.creature;
        let mut pos = world.creature(creature).pos;
        let template = pf.template;
        let Some(data) = world.abstract_room(pf.room) else {
            return;
        };
        let node_usable = usize::try_from(pos.node)
            .ok()
            .and_then(|n| data.nodes.get(n))
            .is_some_and(|n| template.maps_node(n.kind));
        if !node_usable {
            let list: Vec<i32> = (0..data.nodes.len())
                .filter(|&i| {
                    template.maps_node(data.nodes[i].kind)
                        && pf.coordinate_reachable_and_getbackable(WorldCoordinate::new(
                            pos.room,
                            -1,
                            -1,
                            i32::try_from(i).unwrap_or(-1),
                        ))
                })
                .map(|i| i32::try_from(i).unwrap_or(-1))
                .collect();
            if !list.is_empty() {
                let len = i32::try_from(list.len()).unwrap_or(i32::MAX);
                pos.node = list[usize::try_from(rng.range_i32(0, len)).unwrap_or(0)];
                log::debug!("{creature:?} has been moved to an accessible node");
            }
        }
        let mut stranded = false;
        if !pf
            .coordinate_reachable_and_getbackable(WorldCoordinate::new(pos.room, -1, -1, pos.node))
        {
            stranded = true;
            let nodes = world.abstract_room(pos.room).map_or(0, |r| r.nodes.len());
            for j in 0..nodes {
                let n = i32::try_from(j).unwrap_or(-1);
                if pf.coordinate_reachable(WorldCoordinate::new(pos.room, -1, -1, n)) {
                    pos.node = n;
                    stranded = false;
                    break;
                }
            }
            if stranded && let Some(map) = pf.map() {
                let center = pf.accessibility_center.tile;
                for k in 0..nodes {
                    let n = i32::try_from(k).unwrap_or(-1);
                    let specific = template.specific_node_index(data, n);
                    if map.exit_distance_for_creature(pos.tile, specific, &template) > 0
                        || map.exit_distance_for_creature(center, specific, &template) > 0
                    {
                        pos.node = n;
                        stranded = false;
                        break;
                    }
                }
            }
        }
        world.creature_mut(creature).pos.node = pos.node;
        world.ai_new_area(creature, stranded);
    }

    /// `OutOfElement()`: standing on an accessible tile the cells say it cannot reach and
    /// come back from, with no mapper running, it maps again from there.
    pub fn out_of_element(&mut self, id: usize, world: &mut World, rng: &mut UnityRandom) {
        let Some(pf) = self.finders.get(&id) else {
            return;
        };
        let pos = world.creature(pf.creature).pos;
        let accessible = pf
            .map()
            .is_some_and(|m| m.world_coordinate_accessible_to_creature(pos, &pf.template));
        if pf.realized_room.is_some()
            && pf.mapper.is_none()
            && accessible
            && !pf.coordinate_reachable_and_getbackable(pos)
        {
            self.initi_accessibility_mapping(id, world, pos, None, rng);
        }
    }

    /// `SetDestination`.
    pub fn set_destination(
        &mut self,
        id: usize,
        world: &World,
        dest: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        let env = self.env(world, id);
        if let Some(pf) = self.finders.get_mut(&id) {
            pf.set_destination(&env, dest, rng);
        }
    }

    /// `LizardPather.FollowPath`: while an entrance is forbidden, the move away from it
    /// (the count ends once 7 tiles away); else the creature maps again if its cell is
    /// out of reach, and takes the move out of `origin` (inside the covered area) of the
    /// best legality, then newest generation, then lowest cost to the goal. On a floor
    /// with no fall risk below, a Standard move against the body's direction (sideways,
    /// or any in a narrow space) is Unwanted; a move it cannot come back from is
    /// Unallowed unless it walks past the point of no return. Following it for real, a
    /// move out of the room defines its destination's node and the path's generation is
    /// recorded.
    pub fn follow_path(
        &mut self,
        id: usize,
        world: &mut World,
        origin: WorldCoordinate,
        body_direction: Option<usize>,
        actually: bool,
        rng: &mut UnityRandom,
    ) -> Option<MovementConnection> {
        let pf = self.finders.get_mut(&id)?;
        let map = pf.map()?.clone();
        let here = map.tile_at(origin);
        let fall_risk = !here.narrow_space && here.fall_risk_tile.1 < origin.tile.1 - 5;
        if pf.forbidden_entrance_counter > 0 {
            if actually && pf.forbidden_entrance_dist(world, origin) > 6 {
                pf.forbidden_entrance_counter = 0;
            }
            return pf.away_from_forbidden_entrance(world, origin, rng);
        }
        let origin_ref = pf.cell_ref(origin);
        let origin_cell = pf.cell(origin_ref).clone();
        if !origin_cell.reachable || !origin_cell.possible_to_get_back_from {
            self.out_of_element(id, world, rng);
        }
        let pf = self.finders.get(&id)?;
        let mut pick: Option<MovementConnection> = None;
        let mut cost = PathCost::new(0.0, Legality::Unallowed);
        let mut generation = -pf.acceptable_path_age;
        let mut legality = Legality::Unallowed;
        let mut i = 0;
        while let Some(c) = pf.connection_at(world, true, origin, i) {
            i += 1;
            if c.dest.tile_defined() && !pf.covered_area.contains(c.dest_tile()) {
                continue;
            }
            let goal = pf.cell_at(c.dest);
            let mut c_cost = pf.check_connection_cost(origin_cell.coord, goal.coord, &c);
            if !fall_risk
                && let Some(dir) = body_direction
                && c.kind == MovementType::Standard
                && map.tile_at(c.start).acc == Accessibility::Floor
            {
                let step = (
                    (c.dest.tile.0 - c.start.tile.0).clamp(-1, 1),
                    (c.dest.tile.1 - c.start.tile.1).clamp(-1, 1),
                );
                let body = rw_assets::shortcut::FOUR_DIRECTIONS[dir];
                if (step.0 != 0 || map.tile_at(c.start).narrow_space)
                    && step.0 == -body.0
                    && step.1 == -body.1
                {
                    c_cost = c_cost + PathCost::new(0.0, Legality::Unwanted);
                }
            }
            if !goal.possible_to_get_back_from && !pf.walk_past_point_of_no_return {
                c_cost.legality = Legality::Unallowed;
            }
            let total = goal.cost_to_goal + c_cost;
            if c_cost.legality < legality
                || (c_cost.legality == legality
                    && (goal.generation > generation
                        || (goal.generation == generation && total.less_or_equal(cost))))
            {
                pick = Some(c);
                legality = c_cost.legality;
                generation = goal.generation;
                cost = total;
            }
        }
        if actually {
            if pick.is_some_and(|c| !c.dest.tile_defined()) {
                let env = self.env(world, id);
                if let Some(pf) = self.finders.get_mut(&id) {
                    pf.leaving_room(&env, rng);
                }
            }
            if let Some(pf) = self.finders.get_mut(&id) {
                pf.creature_following_generation = generation;
            }
        }
        pick
    }

    /// Every path finder in a room is `Reset` (`Room.UpdatePathfindingCreatures` after a
    /// shelter's map rebuild).
    pub fn reset_room(&mut self, world: &mut World, room: i32, rng: &mut UnityRandom) {
        for id in Self::room_path_finders(world, room) {
            self.reset(id, world, room, rng);
        }
    }
}
