//! The game's side of a room's `ShortcutHelper`: `ShortCutsReady` adding it, its turn in
//! the update list (the player push, the pushers' swelling, the round-robin over
//! `physicalObjects`, the `ObjectPusher`s' turns), `PopsOutOfDeadShortcuts` and
//! `CanBePulledIntoShortcut`. Types: [`crate::game::shortcut_helper`].
//!
//! Omitted until the Five Pebbles oracle is ported (OPEN-ITEMS): `Room.lockedShortcuts`
//! (`SSOracleBehavior.LockShortcuts`); also the Challenge arena's closed exits (DLC).
//!
//! Mirrors: ShortcutHelper.cs (Update, ObjectPusher.Update, GiveUpVerticalSpitOut,
//!   PopsOutOfDeadShortcuts, CanBePulledIntoShortcut), Room.cs (ShortCutsReady)

use std::sync::Arc;

use rw_assets::{Room, Terrain};

use rw_assets::CreatureType;

use crate::creature::CreatureId;
use crate::game::shortcut_helper::{ObjectPusher, ShortcutHelper, ShortcutPusher};
use crate::game::{RainWorldGame, RoomObject};
use crate::item::Entity;
use crate::physics::chunk::BodyChunk;
use crate::world::World;
use crate::{ChunkOwner, FloatExt, SoundId, Vec2};

/// What `ObjectPusher.Update` reads of a creature's shortcut state.
struct CreatureShortcut {
    entering: bool,
    delay: i32,
    in_shortcut: bool,
}

impl World {
    /// `CanPullIntoShortcut(grasp)`: anything but a frog.
    pub(crate) fn can_pull_into_shortcut(&self, grabber: CreatureId) -> bool {
        self.creature(grabber).kind() != CreatureType::Frog
    }

    /// `CanBePulledIntoShortcut(obj)`: some grasp on it can pull.
    pub(crate) fn can_be_pulled_into_shortcut(&self, e: Entity) -> bool {
        self.grabbed_by(e)
            .iter()
            .any(|g| self.can_pull_into_shortcut(g.grabber))
    }
}

impl RainWorldGame {
    /// `ShortCutsReady`'s `AddObject(new ShortcutHelper(this))`.
    pub(crate) fn add_shortcut_helper(&mut self, index: i32) {
        let Some(abstract_room) = self.world.abstract_room(index) else {
            return;
        };
        let connections = abstract_room.connections.clone();
        let gate = abstract_room.gate_index >= 0;
        let single = self.world.single_room();
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.shortcut_helper = Some(ShortcutHelper::new(
                &r.room,
                &connections,
                single,
                gate,
                order,
            ));
        }
    }

    /// Room `index`'s `ShortcutHelper` in its update list.
    pub(crate) fn room_shortcut_helper_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.shortcut_helper.as_ref())
            .map(|h| (h.update_order, RoomObject::ShortcutHelper))
            .into_iter()
            .collect()
    }

    /// `ShortcutHelper.Update` of room `index`'s helper.
    pub(crate) fn run_shortcut_helper(&mut self, index: i32) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(mut helper) = r.shortcut_helper.take() else {
            return;
        };
        let (room, gravity) = (Arc::clone(&r.room), r.gravity);
        self.update_shortcut_helper(index, &room, gravity, &mut helper);
        if let Some(r) = self.world.realized_mut(index) {
            r.shortcut_helper = Some(helper);
        }
    }

    fn update_shortcut_helper(
        &mut self,
        index: i32,
        room: &Room,
        gravity: f32,
        helper: &mut ShortcutHelper,
    ) {
        let player_entity = Entity::Creature(self.player_id);
        if self.player_here(index)
            && !self.world.can_be_pulled_into_shortcut(player_entity)
            && let Some(p) = self.player.as_mut().filter(|p| p.is_conscious())
        {
            p.pushed_by_shortcuts(&mut helper.pushers, gravity);
        }
        for pusher in &mut helper.pushers {
            pusher.settle_swell();
        }
        helper.check_obj = helper.check_obj.wrapping_add(1);
        self.round_robin_push_outs(index, room, helper);
        let mut i = helper.push_out_objects.len();
        while i > 0 {
            i -= 1;
            if helper.push_out_objects[i].slated_for_deletion {
                helper.push_out_objects.remove(i);
            } else {
                let mut pusher = helper.push_out_objects[i].clone();
                self.update_object_pusher(index, room, &helper.pushers, &mut pusher);
                helper.push_out_objects[i] = pusher;
            }
        }
    }

    /// The `checkObj` loop: in each layer of `physicalObjects`, the object at
    /// `checkObj % count` that pops out of dead shortcuts and has a chunk in an entrance
    /// tile gets an `ObjectPusher`.
    fn round_robin_push_outs(&mut self, index: i32, room: &Room, helper: &mut ShortcutHelper) {
        let player_seq = self.player_here(index).then_some(self.player_seq);
        let objects = self.world.physical_objects(index, player_seq);
        for owner in ShortcutHelper::checked_objects(&objects, helper.check_obj) {
            if !self.pops_out_of_dead_shortcuts(owner) {
                continue;
            }
            let in_entrance = (0..)
                .map_while(|i| self.electric_chunk(owner, i))
                .any(|c| room.tile_at(c.pos.to_array()).terrain == Terrain::ShortcutEntrance);
            if in_entrance {
                helper.add_push_out_object(owner);
            }
        }
    }

    /// `PopsOutOfDeadShortcuts`: an unheld unconscious creature, an unheld Hazer or
    /// VultureGrub, an unheld item or weapon.
    pub(crate) fn pops_out_of_dead_shortcuts(&self, owner: ChunkOwner) -> bool {
        match owner {
            ChunkOwner::Segment(_) => false,
            ChunkOwner::Player => {
                self.world
                    .grabbed_by(Entity::Creature(self.player_id))
                    .is_empty()
                    && self.player.as_ref().is_some_and(|p| !p.is_conscious())
            }
            ChunkOwner::Entity(e) => {
                if !self.world.grabbed_by(e).is_empty() {
                    return false;
                }
                match e {
                    Entity::Object(_) => true,
                    Entity::Creature(id) => {
                        let c = self.world.creature(id);
                        matches!(c.kind(), CreatureType::Hazer | CreatureType::VultureGrub)
                            || c.fly().is_some_and(|f| !f.conscious())
                            || c.lizard().is_some_and(|l| !l.conscious())
                            || c.overseer_body().is_some_and(|b| b.dead || b.stun >= 10)
                    }
                }
            }
        }
    }

    /// `obj.room` of a pushed object.
    fn pusher_object_room(&self, owner: ChunkOwner) -> Option<i32> {
        match owner {
            ChunkOwner::Player => {
                let p = self.player.as_ref()?;
                (!p.in_shortcut() && !p.is_destroyed()).then(|| self.pos().room)
            }
            ChunkOwner::Segment(s) => Some(s.room),
            ChunkOwner::Entity(Entity::Object(id)) => {
                self.world.object(id).realized.as_deref().map(|w| w.room)
            }
            ChunkOwner::Entity(Entity::Creature(id)) => {
                let c = self.world.creature(id);
                if let Some(f) = c.fly() {
                    f.room
                } else if let Some(l) = c.lizard() {
                    l.room
                } else {
                    c.overseer_body().filter(|b| b.in_room).map(|_| c.pos.room)
                }
            }
        }
    }

    fn pusher_creature_shortcut(&self, owner: ChunkOwner) -> Option<CreatureShortcut> {
        match owner {
            ChunkOwner::Player => self.player.as_ref().map(|p| CreatureShortcut {
                entering: p.entering_shortcut().is_some(),
                delay: p.shortcut_delay(),
                in_shortcut: p.in_shortcut(),
            }),
            ChunkOwner::Entity(Entity::Creature(id)) => {
                let c = self.world.creature(id);
                if let Some(f) = c.fly() {
                    Some(CreatureShortcut {
                        entering: f.entering_shortcut.is_some(),
                        delay: f.shortcut_delay,
                        in_shortcut: f.in_shortcut,
                    })
                } else {
                    c.lizard().map(|l| CreatureShortcut {
                        entering: l.entering_shortcut.is_some(),
                        delay: l.shortcut_delay,
                        in_shortcut: l.in_shortcut,
                    })
                }
            }
            ChunkOwner::Entity(Entity::Object(_)) | ChunkOwner::Segment(_) => None,
        }
    }

    fn pusher_start_entering(&mut self, owner: ChunkOwner, tile: (i32, i32)) {
        match owner {
            ChunkOwner::Player => {
                if let Some(p) = self.player.as_mut() {
                    p.set_entering_shortcut(tile);
                }
            }
            ChunkOwner::Entity(Entity::Creature(id)) => {
                if self.world.creature(id).fly().is_some() {
                    self.world.fly_at(id).entering_shortcut = Some(tile);
                } else if self.world.creature(id).lizard().is_some() {
                    self.world.lizard_at(id).entering_shortcut = Some(tile);
                }
            }
            ChunkOwner::Entity(Entity::Object(_)) | ChunkOwner::Segment(_) => {}
        }
    }

    /// Every chunk of a pushed object's body (`None` once the body is gone).
    fn pusher_chunks(&self, owner: ChunkOwner) -> Option<Vec<BodyChunk>> {
        let chunks: Vec<BodyChunk> = (0..).map_while(|i| self.electric_chunk(owner, i)).collect();
        (!chunks.is_empty()).then_some(chunks)
    }

    /// `PhysicalObject.GoThroughFloors` of a pushed object: every chunk's
    /// `goThroughFloors`, which reads true while the object can be pulled into a
    /// shortcut (`grabsDisableFloors` is true outside slugpups).
    fn pusher_goes_through_floors(&self, owner: ChunkOwner, chunks: &[BodyChunk]) -> bool {
        let entity = match owner {
            ChunkOwner::Player => Some(Entity::Creature(self.player_id)),
            ChunkOwner::Entity(e) => Some(e),
            ChunkOwner::Segment(_) => None,
        };
        entity.is_some_and(|e| self.world.can_be_pulled_into_shortcut(e))
            || chunks.iter().all(|c| c.go_through_floors())
    }

    /// `ObjectPusher.Update`.
    pub(crate) fn update_object_pusher(
        &mut self,
        index: i32,
        room: &Room,
        pushers: &[ShortcutPusher],
        op: &mut ObjectPusher,
    ) {
        if op.given_up {
            return;
        }
        let object_room = self.pusher_object_room(op.owner);
        if object_room.is_some() {
            op.counter += 1;
        }
        let is_creature = match op.owner {
            ChunkOwner::Player => true,
            ChunkOwner::Entity(e) => matches!(e, Entity::Creature(_)),
            ChunkOwner::Segment(_) => false,
        };
        if is_creature && !self.pops_out_of_dead_shortcuts(op.owner) {
            op.slated_for_deletion = true;
        }
        // D43: a body that is gone is dropped at once.
        let Some(mut chunks) = self.pusher_chunks(op.owner) else {
            op.slated_for_deletion = true;
            return;
        };
        if op.wait_counter > 0 {
            op.wait_counter -= 1;
        } else {
            op.has_been_out_counter += 1;
            self.push_out_of_entrances(index, room, pushers, op, &mut chunks);
        }
        self.sidestep_out_of_entrance(room, op, &mut chunks);
        for (i, c) in chunks.iter().enumerate() {
            if let Some(real) = self.electric_chunk_mut(op.owner, i) {
                real.pos = c.pos;
                real.vel = c.vel;
                real.set_go_through_floors(c.raw_go_through_floors());
            }
        }
        let limit = if object_room.is_none() { 400 } else { 120 };
        if object_room.is_some_and(|r| r != index) || op.has_been_out_counter > limit {
            op.slated_for_deletion = true;
        }
    }

    /// The pusher loop of `ObjectPusher.Update`: a chunk near an entrance is shoved out
    /// of it (two draws), and the object waits 40 ticks.
    fn push_out_of_entrances(
        &mut self,
        index: i32,
        room: &Room,
        pushers: &[ShortcutPusher],
        op: &mut ObjectPusher,
        chunks: &mut [BodyChunk],
    ) {
        let is_creature = !matches!(op.owner, ChunkOwner::Entity(Entity::Object(_)));
        let object_room = self.pusher_object_room(op.owner);
        for pusher in pushers {
            let dir = pusher.shortcut_dir;
            let dir_vec = Vec2::new(dir.0 as f32, dir.1 as f32);
            let middle = Vec2::middle_of_tile(pusher.shortcut_pos);
            let mut found = false;
            for j in 0..chunks.len() {
                let (pos, rad) = (chunks[j].pos, chunks[j].rad);
                if (dir.0 == 0 || !pos.dist_less(pusher.push_pos, rad + 10.0))
                    && !pos.dist_less(middle, rad + 10.0)
                {
                    continue;
                }
                found = true;
                op.search_towards_side = if self.rng.value() < 0.5 { -1 } else { 1 };
                let side = op.search_towards_side as f32;
                if !room
                    .tile_at((pos + Vec2::new(-20.0 * side, 20.0)).to_array())
                    .is_solid()
                    || !room
                        .tile_at((pos + Vec2::new(-20.0 * side, 40.0)).to_array())
                        .is_solid()
                {
                    op.search_towards_side = -op.search_towards_side;
                }
                let counter = op.counter as f32;
                let mut aim = (pusher.push_pos - dir_vec * 40.0).to_aim(pos);
                aim += 30.0
                    * self.rng.value()
                    * op.search_towards_side as f32
                    * counter.lerp_map(20.0, 120.0, 0.3, 1.0);
                let up = dir.1 == 1;
                let mut speed = counter.lerp_map_pow(
                    0.0,
                    if up { 180.0 } else { 500.0 },
                    2.5,
                    15.0,
                    if up { 0.8 } else { 3.0 },
                );
                if up && self.pusher_goes_through_floors(op.owner, chunks) {
                    for c in chunks.iter_mut() {
                        c.set_go_through_floors(false);
                    }
                }
                let beside_walls = dir.1 >= 1
                    && room
                        .tile_at((pos + Vec2::new(-20.0, 0.0)).to_array())
                        .is_solid()
                    && room
                        .tile_at((pos + Vec2::new(20.0, 0.0)).to_array())
                        .is_solid();
                if !beside_walls {
                    op.search_towards_side_x_tile = -1000;
                    op.search_towards_side = 0;
                } else {
                    op.search_towards_side_x_tile = pos.tile().0;
                    if op.counter > 60
                        && up
                        && Self::give_up_vertical_spit_out(room, pusher.shortcut_pos)
                    {
                        if is_creature {
                            let free = self
                                .pusher_creature_shortcut(op.owner)
                                .is_some_and(|s| !s.entering && s.delay < 1 && !s.in_shortcut);
                            if free && object_room == Some(index) {
                                self.pusher_start_entering(op.owner, pusher.shortcut_pos);
                            }
                            if op.counter > 170 {
                                op.given_up = true;
                            }
                        } else {
                            op.given_up = true;
                            speed += 10.0;
                        }
                    }
                }
                for c in chunks.iter_mut() {
                    if dir.0 != 0 {
                        c.vel.x *= 0.0;
                    } else {
                        c.vel.y *= 0.0;
                    }
                    c.vel += Vec2::from_degrees(aim) * speed;
                    if !c.pos.dist_less(pusher.push_pos, c.rad + 10.0)
                        && c.pos.dist_less(middle, c.rad + 10.0)
                    {
                        c.pos = c.pos.lerp(pusher.push_pos, 0.5);
                    }
                    c.pos += dir_vec * 10.0;
                }
                let sounds = if index == self.camera_room {
                    &mut self.sounds
                } else {
                    &mut self.silent
                };
                sounds.play_at(
                    SoundId::MediumNpcTickAlongInShortcut,
                    chunks[j].pos,
                    1.0,
                    1.0,
                    &mut self.rng,
                );
            }
            if found {
                op.has_been_out_counter = 0;
                op.wait_counter = 40;
                break;
            }
        }
    }

    /// The `searchTowardsSide` block of `ObjectPusher.Update`: an object stuck in a shaft
    /// above an entrance slides toward the open side while it falls.
    fn sidestep_out_of_entrance(
        &self,
        room: &Room,
        op: &mut ObjectPusher,
        chunks: &mut [BodyChunk],
    ) {
        if op.search_towards_side == 0 || op.counter <= 20 {
            return;
        }
        let counter = op.counter as f32;
        for m in 0..chunks.len() {
            if chunks[m].pos.tile().0 != op.search_towards_side_x_tile {
                op.search_towards_side = 0;
                break;
            }
            let side = Vec2::new(op.search_towards_side as f32, 0.0);
            if chunks[m].last_contact().x == 0 {
                chunks[m].vel += side * counter.lerp_map(20.0, 180.0, 0.0, 0.1);
            }
            let ahead = chunks[m].pos + Vec2::new(20.0 * op.search_towards_side as f32, 0.0);
            if chunks[m].pos.y >= chunks[m].last_pos.y || room.tile_at(ahead.to_array()).is_solid()
            {
                continue;
            }
            for n in 0..chunks.len() {
                let side = Vec2::new(op.search_towards_side as f32, 0.0);
                chunks[n].pos += side * counter.lerp_map(20.0, 180.0, 0.0, 3.0);
                chunks[n].vel += side * counter.lerp_map(20.0, 180.0, 0.0, 0.8);
                let behind =
                    chunks[m].pos + Vec2::new(20.0 * (-op.search_towards_side) as f32, 0.0);
                if !room.tile_at(behind.to_array()).is_solid() {
                    op.search_towards_side = 0;
                }
            }
            break;
        }
    }

    /// `GiveUpVerticalSpitOut`: a four-tile shaft above the entrance.
    pub(crate) fn give_up_vertical_spit_out(room: &Room, tile: (i32, i32)) -> bool {
        (1..5).all(|i| {
            room.tile(tile.0 - 1, tile.1 + i).is_solid()
                && room.tile(tile.0 + 1, tile.1 + i).is_solid()
        })
    }
}
