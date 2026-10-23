//! `FlyAI`'s behaviour (185299-186205): its update, the behaviours, the flocking, the
//! path along the baked Fly layer and the threats. Base game: no lures (`luredCounter`
//! stays 0), no safari control, no arena challenge limits, every creature on ripple
//! layer 0, `RoomNotACycleHazard` only matters with Remix.
//!
//! Mirrors: FlyAI.cs

use std::sync::Arc;

use rw_assets::{ActiveMods, CreatureType, NodeType, Terrain};

use super::update::FlyRun;
use super::{DropStatus, FlockBehavior, FlyAi, FlyBehavior, MovementMode, TrackedThreat};
use crate::creature::{CreatureId, CreatureTemplate, GrabRequest, RealizedCreature, Shareability};
use crate::item::Entity;
use crate::physics::room_ext::RoomExt;
use crate::world::{EIGHT_DIRECTIONS, MovementConnection, MovementType, World};
use crate::{FloatExt, SoundId, Vec2};

impl FlyRun<'_, '_, '_> {
    fn ai(&mut self) -> &mut FlyAi {
        &mut self.f().ai
    }

    fn air(&self) -> &FlyAi {
        &self.fr().ai
    }

    fn flock(&self) -> FlockBehavior {
        self.world.flocks[self.air().flock]
    }

    /// `ChangeBehavior`: any change clears the hang or burrow spot (its condition is
    /// always true); leaving a held chain detaches with a sound, fleeing startles.
    pub(super) fn change_behavior(&mut self, to: FlyBehavior) {
        if self.air().behavior == Some(to) {
            return;
        }
        self.f().burrow_or_hang_spot = None;
        let holding = self.world.creature(self.me).grasps[0].is_some();
        if self.air().behavior == Some(FlyBehavior::Chain) && holding {
            self.play_on_chunk(SoundId::BatDetatchFromChain);
        } else if to == FlyBehavior::Flee {
            self.play_on_chunk(SoundId::BatStartled);
        }
        self.ai().behavior = Some(to);
    }

    /// `InActiveSwarmRoom`.
    fn in_active_swarm_room(&self) -> bool {
        if self.world.story && !self.world.single_room() {
            self.world.active_swarm_room(self.cx.room_index)
        } else {
            true
        }
    }

    /// `ExitDistanceForCreature(tile, node, Fly)`.
    fn exit_distance(&self, tile: (i32, i32), node: i32) -> i32 {
        let fly = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
        self.cx.ai_map.exit_distance_for_creature(tile, node, &fly)
    }

    /// The room's nodes the Fly maps, in creature-specific order (common index, type).
    fn relevant_nodes(&self) -> Vec<(i32, NodeType)> {
        self.world.fly_nodes(self.cx.room_index)
    }

    /// `room.exitAndDenIndex.Length`.
    fn exits_and_dens(&self) -> i32 {
        i32::try_from(self.cx.room.shortcut_map().nodes().len()).unwrap_or(0)
    }

    /// `CurrentFollowDijkstraIsToHive`.
    fn following_hive(&self) -> bool {
        let following = self.air().following_dijkstra_map;
        let Ok(i) = usize::try_from(following) else {
            return false;
        };
        self.relevant_nodes()
            .get(i)
            .is_some_and(|&(_, kind)| kind == NodeType::BatHive)
    }

    /// `FlyAI.Update`.
    pub(super) fn ai_update(&mut self) {
        let pos = self.pos();
        let map = Arc::clone(&self.cx.ai_map);
        let tile = pos.tile();
        if self.air().get_back_to_ground > 0 {
            self.ai().get_back_to_ground -= 1;
        } else if map.terrain_proximity(tile.0, tile.1) > 8
            || map.smoothed_floor_altitude(tile.0, tile.1) > 8
        {
            let r = self.cx.rng.range_i32(80, 200);
            self.ai().get_back_to_ground = r;
        }
        if self.air().doing_specific_animation() {
            self.world.lose_all_grasps(self.me);
        } else {
            self.pick_other_fly();
            if self.air().flee_from_rain {
                self.change_behavior(FlyBehavior::FleeFromRain);
            } else if self.air().afraid >= 1.0 && self.air().lured_counter < 1 {
                self.change_behavior(FlyBehavior::Flee);
            }
            if self.world.creature(self.me).grasps[0].is_some() {
                self.ai().behavior = Some(FlyBehavior::Chain);
            }
            if self.air().drop_status == Some(DropStatus::Dropping) {
                self.change_behavior(FlyBehavior::Drop);
            }
        }
        self.f().mov_mode = Some(MovementMode::BatFlight);
        match self.air().behavior {
            Some(FlyBehavior::Idle) => self.idle_update(),
            Some(FlyBehavior::Swarm) => {
                let r = self.cx.rng.range_i32(20, 80);
                self.ai().no_swarm_counter = r;
                self.f().mov_mode = Some(MovementMode::SwarmFlight);
                self.swarm_update();
            }
            Some(FlyBehavior::Flee) => {
                self.flee_update();
                if self.air().afraid <= 0.0 {
                    self.change_behavior(FlyBehavior::Idle);
                }
            }
            Some(FlyBehavior::Drop) => {
                self.f().mov_mode = Some(MovementMode::Passive);
                self.drop_update();
            }
            Some(FlyBehavior::Burrow) => {
                self.f().mov_mode = Some(MovementMode::Burrow);
                let pos = self.pos();
                let near = self
                    .fr()
                    .burrow_or_hang_spot
                    .is_some_and(|s| pos.dist_less(s, 100.0));
                if !near {
                    self.f().burrow_or_hang_spot = None;
                    self.change_behavior(FlyBehavior::Idle);
                }
            }
            Some(FlyBehavior::FleeFromRain) => self.flee_from_rain_update(),
            Some(FlyBehavior::Chain) => {
                let pos = self.pos();
                let near = self
                    .fr()
                    .burrow_or_hang_spot
                    .is_some_and(|s| pos.dist_less(s, 40.0));
                if near || self.world.creature(self.me).grasps[0].is_some() {
                    self.f().mov_mode = Some(MovementMode::Hang);
                    self.hang_in_chain_update();
                } else {
                    self.change_behavior(FlyBehavior::Idle);
                }
            }
            Some(FlyBehavior::Lure | FlyBehavior::LeaveRoom) | None => {}
        }
        let flock = self.air().flock;
        if self.world.flocks[flock].leader == self.me {
            self.world.flocks[flock].update();
        }
        if self.world.flocks[flock].life <= 0.0 {
            let new = FlockBehavior::new(self.me, self.cx.rng);
            self.world.flocks.push(new);
            self.ai().flock = self.world.flocks.len() - 1;
        }
        if self.air().behavior != Some(FlyBehavior::Swarm) && self.air().lured_counter < 1 {
            self.update_threats();
        }
        let pos = self.pos();
        let hive = self
            .cx
            .room
            .tile_at_pos(pos)
            .flags
            .contains(rw_assets::TileFlags::HIVE);
        if self.air().afraid >= 1.0 && hive {
            self.f().chunk_mut().vel.y -= 1.0;
        }
        let on_floor = self.fr().chunk().contact().y == -1;
        if on_floor && self.air().afraid >= 0.75 && hive {
            self.change_behavior(FlyBehavior::Burrow);
            self.f().burrow_or_hang_spot = Some(pos);
        }
        if self.cx.rng.value() < 0.0125
            && self.air().behavior != Some(FlyBehavior::Flee)
            && self.world.rain_approaching() < 0.5
        {
            self.ai().flee_from_rain = true;
        }
        if self.cx.rng.value() < 1.0 / 60.0
            && self.air().behavior != Some(FlyBehavior::Chain)
            && self.world.creature(self.me).grasps[0].is_some()
        {
            self.world.lose_all_grasps(self.me);
        }
        self.update_stuck();
        let ai = self.ai();
        ai.afraid = (ai.afraid - 0.003_125).clamp(0.0, 1.0);
    }

    /// `otherFly`: a random fly of the room's `FliesRoomAI`, the last of its chain, if it
    /// is not this one and is in sight; then `ConsiderOtherFly`.
    fn pick_other_fly(&mut self) {
        let index = self.cx.room_index;
        let flies = self
            .world
            .realized(index)
            .and_then(|r| r.flies_room_ai.as_ref())
            .map(|a| a.flies.clone())
            .unwrap_or_default();
        let random = if flies.is_empty() {
            None
        } else {
            let i = self.cx.rng.range_i32(0, flies.len() as i32);
            flies.get(usize::try_from(i).unwrap_or(0)).copied()
        };
        let mut other = random.map(|f| self.world.last_in_chain(f));
        if let Some(o) = other {
            let seen = self
                .cx
                .room
                .visual_contact(self.pos(), self.world.fly_ref(o).pos());
            if o == self.me || !seen {
                other = None;
            }
        }
        self.ai().other_fly = other;
        if other.is_some() {
            self.consider_other_fly();
        }
    }

    /// The stuck counter: 40 ticks within 40 px makes the fly stuck for 120 ticks if a
    /// Solid tile is next to it.
    fn update_stuck(&mut self) {
        let behavior = self.air().behavior;
        if behavior == Some(FlyBehavior::Chain) || behavior == Some(FlyBehavior::Burrow) {
            return;
        }
        let pos = self.pos();
        if self.air().stuck_counter >= 40 {
            self.ai().stuck_counter -= 1;
            return;
        }
        if !pos.dist_less(self.air().stuck_pos, 40.0) {
            let ai = self.ai();
            ai.stuck_counter = 0;
            ai.stuck_pos = pos;
            return;
        }
        self.ai().stuck_counter += 1;
        if self.air().stuck_counter == 40 {
            self.ai().stuck_counter = 0;
            let tile = pos.tile();
            let walled = EIGHT_DIRECTIONS
                .iter()
                .any(|&(dx, dy)| self.cx.room.tile(tile.0 + dx, tile.1 + dy).is_solid());
            if walled {
                self.ai().stuck_counter = 120;
            }
        }
    }

    /// `IdleUpdate`.
    fn idle_update(&mut self) {
        self.generic_flight_update();
        if self.air().other_fly.is_some() {
            self.gravitate_to_flock();
        }
        if self.air().no_swarm_counter > 0 {
            self.ai().no_swarm_counter -= 1;
        } else if self.in_active_swarm_room()
            && self.following_hive()
            && self.exit_distance(self.pos().tile(), self.air().following_dijkstra_map) < 30
        {
            let test = self.pos().lerp(self.air().local_goal, 0.5);
            if self.valid_swarm_position(test) {
                self.ai().local_goal = test;
                self.change_behavior(FlyBehavior::Swarm);
            }
        }
        if self.air().afraid >= 0.5 || !self.flock().form_chains || self.air().flee_from_rain {
            return;
        }
        let other = self.air().other_fly;
        let above_water = other.is_some_and(|o| {
            let p = self.world.fly_ref(o).pos();
            p.y > self.float_water_level(p) + 40.0
        });
        let mut hang = false;
        if let Some(o) = other
            && self.cx.rng.value() < 1.0 / 3.0
            && self.can_i_hang_from(o)
        {
            let below = self.world.fly_ref(o).pos() + Vec2::new(0.0, -20.0);
            hang = self.cx.room.tile_at_pos(below).terrain != Terrain::Solid;
        }
        if hang && above_water {
            let o = other.unwrap_or(self.me);
            let opos = self.world.fly_ref(o).pos();
            if opos.dist_less(self.pos(), 20.0) {
                self.attach_to(o);
            } else {
                self.change_behavior(FlyBehavior::Idle);
                let ai = self.ai();
                ai.local_goal = opos;
                ai.following_dijkstra_map = -1;
            }
        } else if self.cx.rng.value() < 0.125 && self.chain_tile(self.pos().tile()) {
            self.change_behavior(FlyBehavior::Chain);
            let pos = self.pos();
            let room = Arc::clone(&self.cx.room);
            let here = room.tile_at_pos(pos);
            let middle = pos.tile_middle();
            let spot = if here.horizontal_pole() {
                Vec2::new(pos.x, middle.y - 4.0)
            } else if !here.vertical_pole()
                && room.tile_at_pos(pos + Vec2::new(0.0, 20.0)).vertical_pole()
            {
                middle + Vec2::new(0.0, 10.0)
            } else {
                Vec2::new(pos.x, middle.y + 10.0)
            };
            self.f().burrow_or_hang_spot = Some(spot);
        }
    }

    /// Hanging from fly `o`: the grasp, the loop check, the sound, and every fly of `o`'s
    /// chain is drawn in front.
    fn attach_to(&mut self, o: CreatureId) {
        self.change_behavior(FlyBehavior::Chain);
        let request = GrabRequest {
            grabber: self.me,
            grabbed: Entity::Creature(o),
            grasp_used: 0,
            chunk_grabbed: 0,
            shareability: Shareability::NonExclusive,
            dominance: 1.0,
            override_equally_dominant: false,
            pacifying: false,
        };
        self.world.grab(request, self.cx.rng);
        self.world.check_chain_for_loops(self.me);
        self.play_on_chunk(SoundId::BatAttatchToChain);
        let mut fly = o;
        loop {
            if self.world.fly_ref(fly).graphics {
                let id = self.world.creature(fly).id;
                self.cx.observer.fly_to_front(id);
            }
            match self.world.creature(fly).grasps[0].and_then(|g| g.grabbed_creature()) {
                Some(o) if self.world.creature(o).fly().is_some() => fly = o,
                _ => break,
            }
        }
    }

    /// `Room.FloatWaterLevel(pos)`: the surface, or `float.MinValue` without water.
    fn float_water_level(&self, pos: Vec2) -> f32 {
        self.cx.water.map_or(f32::MIN, |w| w.level_at(pos.x))
    }

    /// `SwarmUpdate`.
    fn swarm_update(&mut self) {
        self.ai().local_goal.y += 0.1;
        if self.air().other_fly.is_some() {
            self.gravitate_to_flock();
        }
        let pos = self.pos();
        if pos.y < self.air().lowest_other_pos.y && self.valid_swarm_position(pos) {
            self.ai().lowest_other_pos = pos;
        }
        let x = self.cx.rng.value() * self.cx.room.pixel_width();
        let y = self.cx.rng.value() * self.air().lowest_other_pos.y;
        let test = Vec2::new(x, y);
        if self.valid_swarm_position(test) && self.cx.room.visual_contact(pos, test) {
            self.ai().lowest_other_pos = test;
        }
        let goal = self.air().local_goal;
        let height = self.cx.room.pixel_height();
        if !self.valid_swarm_position(goal)
            || self.cx.room.is_solid_at(goal + Vec2::new(0.0, 15.0))
            || goal.y > height
        {
            let lowest = self.air().lowest_other_pos;
            if self.valid_swarm_position(lowest) {
                let dx = (-10.0f32).lerp(10.0, self.cx.rng.value());
                let ai = self.ai();
                ai.local_goal = lowest + Vec2::new(dx, -15.0);
                ai.lowest_other_pos = Vec2::new(ai.local_goal.x, height);
            } else {
                self.change_behavior(FlyBehavior::Idle);
            }
        }
        let goal = self.air().local_goal;
        if !pos.dist_less(goal, 400.0) || !self.cx.room.visual_contact(pos, goal) {
            self.change_behavior(FlyBehavior::Idle);
        }
    }

    /// The most pressing threat: the highest `Severity / distance`.
    fn worst_threat(&self) -> Option<TrackedThreat> {
        let pos = self.pos();
        let mut best = 0.0;
        let mut worst = None;
        for t in &self.air().threats {
            let score = t.severity() / pos.distance(t.last_seen_pos);
            if score > best {
                best = score;
                worst = Some(*t);
            }
        }
        worst
    }

    /// `FleeUpdate`.
    fn flee_update(&mut self) {
        let threat = self.worst_threat();
        let pos = self.pos();
        if self.air().panic_flee > 0
            && !self.air().doing_specific_animation()
            && !self.air().stuck()
            && let Some(t) = threat
        {
            self.f().dir = t.last_seen_pos.direction_to(pos);
            self.f().mov_mode = Some(MovementMode::Panic);
            let contact = self.fr().chunk().contact();
            let inside = self.world.creature(self.me).pos.inside(&self.cx.room);
            if contact.x != 0 || contact.y != 0 || !inside {
                self.ai().panic_flee = 0;
            }
        } else {
            self.pick_flee_node();
        }
        self.generic_flight_update();
        if let Some(t) = threat
            && self.in_active_swarm_room()
            && !self.air().stuck()
            && !CreatureTemplate::of(t.crit_kind, self.world.mods()).can_fly
            && self
                .current_threat(t.crit)
                .is_some_and(|c| c.currently_scared_of)
        {
            self.climb_away(t.last_seen_pos);
        }
        if self.air().afraid < 1.0 {
            return;
        }
        let room = Arc::clone(&self.cx.room);
        if room
            .tile_at_pos(self.pos())
            .flags
            .contains(rw_assets::TileFlags::HIVE)
        {
            self.f().chunk_mut().vel.y -= 1.0;
            return;
        }
        let abs = self.world.creature(self.me).pos.tile;
        let mut y = abs.1;
        while y > abs.1 - 10 && room.tile(abs.0, y).terrain != Terrain::Solid {
            if room
                .tile(abs.0, y)
                .flags
                .contains(rw_assets::TileFlags::HIVE)
            {
                let clear = self.air().threats.iter().all(|t| {
                    let (tx, ty) = t.last_seen_pos.tile();
                    !(tx >= abs.0 - 2 && tx <= abs.0 + 2 && ty >= y - 1 && ty <= abs.1 + 1)
                });
                if clear {
                    self.ai().drop_status = Some(DropStatus::Dropping);
                }
            }
            y -= 1;
        }
    }

    /// The tracked threat of creature `crit` as it is now (the threats may have
    /// changed since `worst_threat` copied it; they do not within `FleeUpdate`).
    fn current_threat(&self, crit: CreatureId) -> Option<&TrackedThreat> {
        self.air().threats.iter().find(|t| t.crit == crit)
    }

    /// `FleeUpdate`'s node choice: the fly-relevant node that puts the most path
    /// between the fly and its threats; never a den, exits a little less.
    fn pick_flee_node(&mut self) {
        let pos = self.pos().tile();
        let mut best = f32::MIN;
        let threats = self.air().threats.clone();
        for (i, (_, kind)) in self.relevant_nodes().into_iter().enumerate() {
            let i = i32::try_from(i).unwrap_or(-1);
            let mine = self.exit_distance(pos, i);
            let mut score = 0.0f32;
            for t in &threats {
                let theirs = self.exit_distance(t.last_seen_pos.tile(), i);
                score += (theirs - mine) as f32 * 0.5.lerp(1.0, t.severity());
                if theirs < mine {
                    score -= 1000.0;
                }
            }
            if kind == NodeType::Den {
                score = f32::MIN;
            } else if kind == NodeType::Exit {
                score -= 20.0;
            }
            if score > best {
                best = score;
                self.ai().following_dijkstra_map = i;
            }
        }
    }

    /// A threat that cannot fly and is not above: with three open tiles overhead the
    /// local goal turns upward (scaled by the goal's own magnitude, as the game does).
    fn climb_away(&mut self, threat_pos: Vec2) {
        let pos = self.pos();
        if self.fr().dir.y <= -0.5 || pos.direction_to(threat_pos).y >= 0.5 {
            return;
        }
        let room = Arc::clone(&self.cx.room);
        let tile = pos.tile();
        let mut open = 0;
        let mut y = tile.1;
        while y < room.height() && room.tile(tile.0, y).terrain != Terrain::Solid {
            open += 1;
            y += 1;
        }
        if open >= 3 {
            let goal = self.air().local_goal;
            let dir = pos.direction_to(goal).slerp(Vec2::new(0.0, 1.0), 0.2);
            self.ai().local_goal = pos + dir * goal.length();
        }
    }

    /// `DropUpdate`.
    fn drop_update(&mut self) {
        let pos = self.pos();
        let room = Arc::clone(&self.cx.room);
        let below = (pos + Vec2::new(0.0, -20.0)).tile();
        let wet = room.water_int(below.0, below.1) > 0;
        if self.fr().chunk().contact().y < 0 || wet || pos.y < -50.0 {
            self.ai().drop_status = Some(DropStatus::HasDropped);
            if room
                .tile_at_pos(pos)
                .flags
                .contains(rw_assets::TileFlags::HIVE)
            {
                self.change_behavior(FlyBehavior::Burrow);
                self.f().burrow_or_hang_spot = Some(pos);
            } else {
                self.change_behavior(FlyBehavior::Idle);
            }
        }
        self.f().chunk_mut().vel.x *= 0.6;
    }

    /// `FleeFromRainUpdate`: toward the nearest hive, else out by the migration's exit.
    fn flee_from_rain_update(&mut self) {
        self.ai().afraid = 2.0;
        let hives = i32::try_from(self.cx.room.hives().len()).unwrap_or(0);
        let index = self.cx.room_index;
        if hives != 0 {
            let nodes = self.world.abstract_room(index).map_or(0, |r| r.nodes.len());
            if (self.air().following_dijkstra_map as i64) < nodes as i64 {
                let tile = self.world.creature(self.me).pos.tile;
                let first = self.exits_and_dens();
                let mut best = i32::MAX;
                let mut pick = -1;
                for i in first..first + hives {
                    let d = self.exit_distance(tile, i);
                    if d < best {
                        best = d;
                        pick = i;
                    }
                }
                if pick != -1 {
                    let ai = self.ai();
                    ai.leave_room_dijkstra = -1;
                    ai.following_dijkstra_map = pick;
                }
            }
        } else {
            let pos = self.world.creature(self.me).pos;
            let to = self.world.migration_direction(pos, self.cx.rng);
            let exit = self
                .world
                .abstract_room(index)
                .map_or(-1, |r| r.exit_index(to));
            let ai = self.ai();
            ai.leave_room_dijkstra = exit;
            ai.following_dijkstra_map = exit;
        }
        let following = self.air().following_dijkstra_map;
        if following >= 0 {
            let goal = self.air().local_goal;
            self.ai().local_goal = self.progress_local_goal(goal, following);
        }
    }

    /// `HangInChainUpdate`.
    fn hang_in_chain_update(&mut self) {
        let grasp = self.world.creature(self.me).grasps[0];
        match grasp {
            None => {
                let Some(spot) = self.fr().burrow_or_hang_spot else {
                    self.change_behavior(FlyBehavior::Idle);
                    return;
                };
                let pos = self.pos();
                let dist = pos.distance(spot);
                if dist > 8.0 {
                    let v = pos.direction_to(spot) * (8.0 - dist);
                    let c = self.f().chunk_mut();
                    c.pos -= v;
                    c.vel -= v;
                }
                let scared = self.cx.rng.value() < 0.05 && self.air().afraid > 1.9;
                let bored = !scared
                    && self.cx.rng.value() < 1.0 / 120.0
                    && self.world.last_in_chain(self.me) == self.me;
                if scared || bored {
                    self.change_behavior(FlyBehavior::Idle);
                }
                if let Some(o) = self.air().other_fly
                    && self.world.fly_ref(o).ai.behavior == Some(FlyBehavior::Chain)
                    && self.world.creature(o).grasps[0].is_none()
                    && self.world.fly_ref(o).pos().dist_less(self.pos(), 30.0)
                {
                    self.change_behavior(FlyBehavior::Idle);
                }
            }
            Some(g) => {
                let Some(held) = g.grabbed_creature() else {
                    return;
                };
                if self.leave_chain(held)
                    || (self.cx.rng.value() < 0.004_166_667
                        && self.world.last_in_chain(self.me) == self.me
                        && self.world.first_in_chain(self.me) == held)
                {
                    self.change_behavior(FlyBehavior::Idle);
                }
            }
        }
        let pos = self.pos();
        let low = pos.y < self.float_water_level(pos) + 10.0;
        if self.fr().chunk().contact().y < 0 || low || self.air().flee_from_rain {
            self.change_behavior(FlyBehavior::Idle);
        }
    }

    /// The `||` chain that lets a hanging fly let go, its draws short-circuiting as the
    /// game's.
    fn leave_chain(&mut self, held: CreatureId) -> bool {
        if self.world.creature(held).fly().is_none() {
            return true;
        }
        if self.cx.rng.value() < 1.0 / 140.0 {
            let first = self.world.first_in_chain(self.me);
            if self.world.fly_ref(first).mov_mode != Some(MovementMode::Hang) {
                return true;
            }
        }
        if self.cx.rng.value() < 0.05 && self.air().panic_flee as f32 > 0.0 {
            return true;
        }
        if self.cx.rng.value() < 0.003_571_428_6 && self.air().afraid == 2.0 {
            return true;
        }
        if self.air().leave_room_dijkstra > -1 {
            return true;
        }
        self.cx.rng.value() < 0.0002
    }

    /// `GenericFlightUpdate`.
    fn generic_flight_update(&mut self) {
        self.update_follow_dijkstra();
        let following = self.air().following_dijkstra_map;
        if following >= 0 {
            let goal = self.air().local_goal;
            self.ai().local_goal = self.progress_local_goal(goal, following);
        }
    }

    /// `GravitateToFlock`.
    fn gravitate_to_flock(&mut self) {
        let Some(o) = self.air().other_fly else {
            return;
        };
        let flock = self.flock();
        if flock.collective_movement == 0.0 || self.air().get_back_to_ground > 0 {
            return;
        }
        let pos = self.pos();
        let other = self.world.fly_ref(o);
        let (opos, ogoal) = (other.pos(), other.ai.local_goal);
        let mut target = if other.ai.behavior == Some(FlyBehavior::Flee) {
            pos + opos.direction_to(pos) * 60.0 + opos.direction_to(ogoal) * 30.0
        } else {
            let mut t = opos.lerp(ogoal, flock.exaggerate_direction_of_others.clamp(0.0, 1.0));
            if t.dist_less(opos, flock.distance_to_others) {
                t = opos + opos.direction_to(t) * flock.distance_to_others;
            }
            if t.dist_less(ogoal, flock.distance_to_others) {
                t = ogoal + ogoal.direction_to(t) * flock.distance_to_others;
            }
            t
        };
        let near = 50.0.lerp(300.0, flock.collective_movement);
        let t = (pos.distance(opos) * flock.collective_movement).inverse_lerp(near, 40.0);
        target = self.air().local_goal.lerp(target, t);
        let map = Arc::clone(&self.cx.ai_map);
        let (tt, ft) = (target.tile(), pos.tile());
        if map.terrain_proximity(tt.0, tt.1) >= map.terrain_proximity(ft.0, ft.1)
            && self.ai_visual_contact(target)
        {
            self.ai().local_goal = target;
            let theirs = self.world.fly_ref(o).ai.following_dijkstra_map;
            if theirs >= 0
                && self.cx.rng.value().mathf_pow(0.5) < flock.collective_movement
                && self.exit_distance(pos.tile(), theirs) > 10
            {
                self.ai().following_dijkstra_map = theirs;
            }
        }
    }

    /// `ValidSwarmPosition`: the test position's terrain proximity, but the floor
    /// altitudes of the current local goal (as the game reads them).
    fn valid_swarm_position(&self, test: Vec2) -> bool {
        let map = &self.cx.ai_map;
        let t = test.tile();
        let g = self.air().local_goal.tile();
        let prox = map.terrain_proximity(t.0, t.1);
        prox > 2
            && prox < 15
            && map.smoothed_floor_altitude(g.0, g.1) <= 8
            && map.floor_altitude(g.0, g.1) <= 12
    }

    /// `ChainTile`.
    fn chain_tile(&self, (x, y): (i32, i32)) -> bool {
        let room = &self.cx.room;
        let here = room.tile(x, y);
        let above = room.tile(x, y + 1);
        if above.terrain != Terrain::Solid
            && !here.horizontal_pole()
            && (!above.vertical_pole() || here.vertical_pole())
        {
            return false;
        }
        for row in (y - 4..=y).rev() {
            let t = room.tile(x, row);
            if t.terrain == Terrain::Solid
                || room.water_int(
                    x.clamp(0, room.width() - 1),
                    row.clamp(0, room.height() - 1),
                ) > 0
            {
                return false;
            }
        }
        true
    }

    /// `CanIHangFromThisFly`.
    fn can_i_hang_from(&self, o: CreatureId) -> bool {
        let w = &self.world;
        if w.fly_ref(o).ai.behavior != Some(FlyBehavior::Chain) || w.next_in_chain(o).is_some() {
            return false;
        }
        if w.creature(o).grasps[0].is_some_and(|g| g.grabbed == Entity::Creature(self.me)) {
            return false;
        }
        if w.last_in_chain(o) != o {
            return false;
        }
        let first = w.first_in_chain(o);
        w.fly_ref(first).burrow_or_hang_spot.is_some()
            && w.fly_ref(first).ai.behavior == Some(FlyBehavior::Chain)
    }

    /// `ConsiderOtherFly`.
    fn consider_other_fly(&mut self) {
        let Some(o) = self.air().other_fly else {
            return;
        };
        let other = self.world.fly_ref(o);
        let (opos, ogoal, oflock) = (other.pos(), other.ai.local_goal, other.ai.flock);
        let othreats = other.ai.threats.len();
        if opos.y < self.air().lowest_other_pos.y && self.valid_swarm_position(opos) {
            self.ai().lowest_other_pos = opos;
        }
        let flock = self.flock();
        let goal = self.air().local_goal;
        if ogoal.dist_less(goal, flock.distance_to_others) {
            let v = ogoal + ogoal.direction_to(goal) * flock.distance_to_others;
            let t = v.tile();
            if self.cx.ai_map.terrain_proximity(t.0, t.1) > 1 && self.ai_visual_contact(v) {
                self.ai().local_goal = v;
            }
        }
        if self.world.flocks[oflock].dominance() > self.flock().dominance() {
            self.ai().flock = oflock;
        }
        if othreats > 0 {
            let i = self.cx.rng.range_i32(0, othreats as i32);
            let crit = self.world.fly_ref(o).ai.threats[usize::try_from(i).unwrap_or(0)].crit;
            self.consider_other_creature(crit);
        }
    }

    /// `UpdateFollowDijsktra`.
    fn update_follow_dijkstra(&mut self) {
        if self.air().leave_room_dijkstra >= 0 {
            let leave = self.air().leave_room_dijkstra;
            self.ai().following_dijkstra_map = leave;
            return;
        }
        let following = self.air().following_dijkstra_map;
        if following >= 0 {
            let idle = self.air().behavior == Some(FlyBehavior::Idle);
            let near = if self.following_hive() { 4 } else { 15 };
            if !idle || self.exit_distance(self.pos().tile(), following) >= near {
                return;
            }
        }
        if self.in_active_swarm_room() {
            let relevant = i32::try_from(self.relevant_nodes().len()).unwrap_or(0);
            let pick = self.cx.rng.range_i32(self.exits_and_dens(), relevant);
            let next = if following == pick {
                self.cx.rng.range_i32(0, relevant)
            } else {
                pick
            };
            self.ai().following_dijkstra_map = next;
        } else {
            let index = self.cx.room_index;
            let n = self
                .world
                .abstract_room(index)
                .map_or(0, |r| r.connections.len());
            let next = self.cx.rng.range_i32(0, i32::try_from(n).unwrap_or(0));
            self.ai().following_dijkstra_map = next;
        }
    }

    /// `ProgressLocalGoalAlongDijkstraMap`.
    fn progress_local_goal(&self, goal: Vec2, follow: i32) -> Vec2 {
        let room = &self.cx.room;
        let goal = Vec2::new(
            goal.x.clamp(10.0, room.pixel_width() - 10.0),
            goal.y.clamp(10.0, room.pixel_height() - 10.0),
        );
        let pos = self.pos();
        if !pos.dist_less(goal, 400.0) {
            if room.visual_contact(pos, goal) {
                return goal;
            }
            return pos + pos.direction_to(goal);
        }
        if self.exit_distance(self.air().local_goal.tile(), follow) < 1 {
            return self.air().local_goal;
        }
        let map = &self.cx.ai_map;
        let t = goal.tile();
        let mut best = i32::MAX;
        let mut pick: Option<MovementConnection> = None;
        let fly = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
        for c in &map.tile(t.0, t.1).outgoing_paths {
            let d = self.exit_distance(c.dest_tile(), follow);
            if map.is_connection_allowed_for_creature(c, &fly) && d < best && d > -1 {
                best = d;
                pick = Some(*c);
            }
        }
        let Some(c) = pick else {
            return pos + pos.direction_to(goal);
        };
        if c.kind == MovementType::ShortCut {
            return Vec2::middle_of_tile(c.start_tile());
        }
        let next = Vec2::middle_of_tile(c.dest_tile());
        if !room.visual_contact(pos, next) {
            if room.visual_contact(pos, goal) {
                return goal;
            }
            return pos + pos.direction_to(goal);
        }
        next
    }

    /// `FlyAI.VisualContact`: a swarming fly sees no farther than 60 px.
    fn ai_visual_contact(&self, point: Vec2) -> bool {
        let pos = self.pos();
        if self.air().behavior == Some(FlyBehavior::Swarm) && !pos.dist_less(point, 60.0) {
            return false;
        }
        self.cx.room.visual_contact(pos, point)
    }

    /// `UpdateThreats`.
    fn update_threats(&mut self) {
        if self.air().panic_flee > 0 {
            self.ai().panic_flee -= 1;
        }
        let pos = self.pos();
        if self.air().threats.is_empty() {
            if !self.air().flee_from_rain {
                self.ai().afraid = 0.0;
            }
        } else {
            let mut fear = 0.0f32;
            let chain = self.air().behavior == Some(FlyBehavior::Chain);
            let threats = self.air().threats.clone();
            for t in &threats {
                fear += 600.0 * t.severity() / t.last_seen_pos.distance(pos);
                if chain && t.last_seen_pos.y < pos.y - 80.0 {
                    fear /= 5.0;
                } else if self.air().panic_flee < 1
                    && t.visual_contact
                    && pos.dist_less(t.last_seen_pos, FlyAi::afraid_distance(t.crit_kind) / 3.0)
                {
                    self.ai().panic_flee = 1;
                }
            }
            let ai = self.ai();
            if fear > 1.0 {
                ai.afraid += fear / 100.0;
            } else {
                ai.afraid -= 0.0125;
            }
            ai.afraid = ai.afraid.clamp(if fear > 1.0 { 1.0 } else { 0.0 }, 2.0);
            if ai.panic_flee > 0 {
                ai.afraid = 2.0;
            }
        }
        let contact = self.fr().chunk().contact();
        if contact.x != 0 || contact.y != 0 {
            self.ai().panic_flee = 0;
        }
        let creatures = self.world.room_creatures(self.cx.room_index).to_vec();
        if !creatures.is_empty() {
            let i = self.cx.rng.range_i32(0, creatures.len() as i32);
            self.consider_other_creature(creatures[usize::try_from(i).unwrap_or(0)]);
        }
        let mut i = self.air().threats.len();
        while i > 0 {
            i -= 1;
            if self.air().threats[i].delete_me_next_frame {
                self.ai().threats.remove(i);
            } else {
                self.update_threat(i);
            }
        }
    }

    /// The chunk positions of realized creature `id`, if the port knows its body.
    fn creature_chunks(&self, id: CreatureId) -> Option<Vec<Vec2>> {
        match &self.world.creature(id).realized {
            Some(RealizedCreature::Player) => {
                let p = self.cx.player.filter(|p| p.id == id)?;
                Some(p.chunks.to_vec())
            }
            Some(RealizedCreature::Fly(f)) => Some(f.body.chunks.iter().map(|c| c.pos).collect()),
            _ => None,
        }
    }

    /// The realized creature's room (`realizedCreature.room`).
    fn creature_room(&self, id: CreatureId) -> Option<i32> {
        match &self.world.creature(id).realized {
            Some(RealizedCreature::Player) => self.cx.player.filter(|p| p.id == id)?.room,
            Some(RealizedCreature::Fly(f)) => f.room,
            _ => None,
        }
    }

    /// `ConsiderOtherCreature`: a realized creature the Fly fears, not yet tracked,
    /// within its afraid distance and in sight, becomes a threat; the fly flees unless it
    /// hangs in a chain.
    fn consider_other_creature(&mut self, crit: CreatureId) {
        let c = self.world.creature(crit);
        if c.realized.is_none() || !FlyAi::afraid_of(c.kind()) {
            return;
        }
        let kind = c.kind();
        if self.air().threats.iter().any(|t| t.crit == crit) {
            return;
        }
        let Some(chunks) = self.creature_chunks(crit) else {
            log::warn!("a realized {kind} without a ported body is not considered");
            return;
        };
        let i = self.cx.rng.range_i32(0, chunks.len() as i32);
        let chunk = chunks[usize::try_from(i).unwrap_or(0)];
        let pos = self.pos();
        let distance = FlyAi::afraid_distance(kind);
        if pos.dist_less(chunk, distance) && self.ai_visual_contact(chunk) {
            self.ai().threats.push(TrackedThreat::new(crit, kind));
            if self.air().behavior != Some(FlyBehavior::Chain) {
                self.change_behavior(FlyBehavior::Flee);
                let add = 10 + (distance - pos.distance(chunk)) as i32;
                self.ai().panic_flee += add;
            }
        }
    }

    /// `TrackedThreat.Update` of threat `i`.
    fn update_threat(&mut self, i: usize) {
        let mut t = self.air().threats[i];
        t.ticks_since_seen += 1;
        let here = self.cx.room_index;
        if t.ticks_since_seen > 10 {
            t.visual_contact = false;
            let realized = self.world.creature(t.crit).realized.is_some();
            if self.world.creature(t.crit).pos.room == here && realized {
                match self.creature_chunks(t.crit) {
                    Some(chunks) => {
                        let k = self.cx.rng.range_i32(0, chunks.len() as i32);
                        let chunk = chunks[usize::try_from(k).unwrap_or(0)];
                        if self.ai_visual_contact(chunk) {
                            t.visual_contact = true;
                            t.ticks_since_seen = 0;
                        }
                    }
                    None => log::warn!("a tracked threat without a ported body"),
                }
            }
            if t.ticks_since_seen > t.ticks_to_forget {
                t.delete_me_next_frame = true;
            }
        } else if self.creature_room(t.crit) == Some(here) {
            if let Some(chunks) = self.creature_chunks(t.crit) {
                t.last_seen_pos = chunks[0];
            }
        } else {
            t.visual_contact = false;
        }
        let pos = self.pos();
        t.currently_scared_of =
            t.visual_contact && pos.dist_less(t.last_seen_pos, FlyAi::afraid_distance(t.crit_kind));
        self.ai().threats[i] = t;
    }
}

impl World {
    /// `FlyAI.NewRoom`.
    pub(crate) fn fly_ai_new_room(&mut self, id: CreatureId, room_height: f32) {
        let fly = self.fly_at(id);
        let pos = fly.pos();
        let ai = &mut fly.ai;
        ai.local_goal = pos;
        ai.following_dijkstra_map = -1;
        ai.leave_room_dijkstra = -1;
        ai.lowest_other_pos = Vec2::new(pos.x, room_height);
    }

    /// `FlyAI.LeaveRoom(coord)`: the exit (creature-specific) toward `coord`'s room.
    pub(crate) fn fly_leave_room(&mut self, id: CreatureId, to_room: i32, to_node: i32) {
        let Some(room) = self.fly_ref(id).room else {
            return;
        };
        let common = if to_room == room {
            to_node
        } else {
            Self::node_in_a_leading_to_b(&self.data, room, to_room).node
        };
        let specific = self
            .fly_nodes(room)
            .iter()
            .position(|&(c, _)| c == common)
            .map_or(-1, |i| i32::try_from(i).unwrap_or(-1));
        self.fly_at(id).ai.leave_room_dijkstra = specific;
    }

    /// The nodes of room `room` the Fly maps, in creature-specific order: (common index,
    /// type).
    #[must_use]
    pub(crate) fn fly_nodes(&self, room: i32) -> Vec<(i32, NodeType)> {
        let fly = CreatureTemplate::of(rw_assets::CreatureType::Fly, self.mods);
        self.abstract_room(room)
            .map(|r| {
                r.nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| fly.maps_node(n.kind))
                    .map(|(i, n)| (i32::try_from(i).unwrap_or(-1), n.kind))
                    .collect()
            })
            .unwrap_or_default()
    }
}
