//! The world's realized rocks and spears: `ShortCutsReady` realizes a room's objects
//! (61622), each runs in its slot of the room's update list, `Room.Update`'s collision
//! pass pushes those of layers 1 and up apart (63520), and `AbstractRoom.Abstractize`
//! drops them (304092). Behaviour: `rainworld-re/notes/items.md` sections 6-8.

use rw_assets::SoundTable;
use rw_random::UnityRandom;

use super::{RealizedRoom, World};
use crate::creature::WorldCoordinate;
use crate::item::{Entity, ModeChange, ObjectId, Weapon, WeaponKind, WeaponMode, WeaponTick};
use crate::{BodyChunk, SoundId, Sounds, SurfaceChunk, Vec2};

/// What an item's turn in the update list reads of the game.
pub struct ItemTick<'a> {
    pub rng: &'a mut UnityRandom,
    /// The room's sound sink (the camera's, or a silent one).
    pub sounds: &'a mut Sounds,
    /// The camera's sound table (`workingTriggers`).
    pub triggers: &'a SoundTable,
    pub camera_room: i32,
    pub tick: u64,
    pub eu: bool,
}

impl World {
    /// `ShortCutsReady`'s objects: each object among room `index`'s entities that is not
    /// in a den and not connected to a creature (those come with it), in list order, is
    /// realized and placed.
    pub(crate) fn realize_room_objects(&mut self, index: i32, rng: &mut UnityRandom) {
        let ids: Vec<ObjectId> = self.room_objects(index).collect();
        for id in ids {
            let held = self
                .connected_objects(Entity::Object(id))
                .iter()
                .any(|e| e.creature().is_some());
            let o = self.object(id);
            if o.in_den || held {
                continue;
            }
            if o.pos.room != index {
                log::warn!("Object in wrong room! {:?}", o.id);
                continue;
            }
            self.realize_object_in_room(id, rng);
        }
    }

    /// `RealizeInRoom` in the object's room: `Realize` (unless it is), an undefined tile
    /// taken from its node, `PlaceInRoom`.
    pub(crate) fn realize_object_in_room(&mut self, id: ObjectId, rng: &mut UnityRandom) {
        let index = self.object(id).pos.room;
        let o = self.object(id);
        if o.in_den {
            return;
        }
        if o.realized.is_none() {
            let Some(weapon) = Weapon::realize(o, rng) else {
                log::debug!("{} objects are not realized", o.kind.value());
                return;
            };
            self.object_mut(id).realized = Some(Box::new(weapon));
        }
        let Some(room) = self.realized(index).map(|r| r.room.clone()) else {
            log::warn!("TRYING TO REALIZE IN NON REALIZED ROOM!");
            return;
        };
        if !self.object(id).pos.tile_defined() {
            let node = self.object(id).pos.node;
            self.object_mut(id).pos.tile = self.local_coordinate_of_node(index, node, rng);
        }
        let mut seq = self.update_seq;
        let object = &mut self.objects[id.0];
        if let Some(mut weapon) = object.realized.take() {
            let mut cx = ModeChange {
                room: &room,
                rng,
                update_seq: &mut seq,
            };
            weapon.place_in_room(object, index, &mut cx);
            object.realized = Some(weapon);
        }
        self.update_seq = seq;
    }

    /// `Room.RemoveObject` of a realized object carried into a shortcut: it leaves the
    /// room's lists (no room until it is spat out).
    pub(crate) fn object_leaves_room(&mut self, id: ObjectId) {
        if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
            w.room = -1;
        }
    }

    /// `Room.AddObject` of realized object `id` into room `room`: the end of its update
    /// list and of its collision layer.
    pub(crate) fn object_joins_room(&mut self, id: ObjectId, room: i32) {
        let seq = self.next_update_seq();
        if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
            w.room = room;
            w.update_order = seq;
            w.layer_order = seq;
        }
    }

    /// The base `Creature.SpitOutOfShortCut`'s loop for a carried object: its chunk at
    /// the entrance's middle (`RNV`, one draw), still, `AddObject` into `room`, and
    /// `NewRoom` when it comes from another room.
    pub(crate) fn spit_out_object(
        &mut self,
        id: ObjectId,
        room: i32,
        middle: Vec2,
        rng: &mut UnityRandom,
    ) {
        let Some(w) = self.object_mut(id).realized.as_deref_mut() else {
            return;
        };
        let came_from = w.room;
        for c in &mut w.body.chunks {
            c.hard_set_position(middle + Vec2::random_unit(rng));
            c.vel = Vec2::ZERO;
        }
        self.object_joins_room(id, room);
        if came_from != room
            && let Some(w) = self.object_mut(id).realized.as_deref_mut()
        {
            w.new_room();
        }
    }

    /// `AbstractPhysicalObject.Abstractize(coord)` of one object: `timeSpentHere` 0,
    /// `Move(coord)`, the realized object out of its room and dropped.
    pub(crate) fn abstractize_object(&mut self, id: ObjectId, coord: WorldCoordinate) {
        self.object_mut(id).time_spent_here = 0;
        self.move_object(id, coord);
        self.object_mut(id).realized = None;
    }

    /// `PlayerCarryableItem.Forbid` of realized object `id`.
    pub(crate) fn forbid(&mut self, id: ObjectId) {
        if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
            w.forbid();
        }
    }

    /// `Weapon.ChangeOverlap` of realized object `id`.
    pub(crate) fn change_overlap(&mut self, id: ObjectId, overlap: bool) {
        if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
            w.change_overlap(overlap);
        }
    }

    /// A held weapon turned the way its hand holds it: `setRotation`, no spin.
    pub(crate) fn set_held_rotation(&mut self, id: ObjectId, dir: Vec2) {
        if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
            w.set_rotation = Some(dir);
            w.rotation_speed = 0.0;
        }
    }

    /// `Rock.PickedUp` or `Spear.PickedUp` up to its sound, which it returns: a spear
    /// with a wall's pole state is pulled out first (the realized room's tiles get their
    /// poles back), then `ChangeMode(Carried)`.
    pub(crate) fn weapon_picked_up(&mut self, id: ObjectId, rng: &mut UnityRandom) -> SoundId {
        let Some(w) = self.object(id).realized.as_deref() else {
            return SoundId::SlugcatPickUpMiscInanimate;
        };
        if w.spear().is_none() {
            return SoundId::SlugcatPickUpRock;
        }
        let room = w.room;
        if w.has_horizontal_beam_state()
            && let Some(local) = self.local(room)
        {
            let World {
                objects, realized, ..
            } = self;
            let object = &mut objects[id.0];
            if let (Some(r), Some(mut w)) = (realized[local].as_mut(), object.realized.take()) {
                w.pull_out_of_wall(object, std::sync::Arc::make_mut(&mut r.room));
                object.realized = Some(w);
            }
        }
        self.change_weapon_mode(id, WeaponMode::Carried, rng);
        SoundId::SlugcatPickUpSpear
    }

    /// `ChangeMode` of realized object `id`, with the realized room of its `pos`.
    pub(crate) fn change_weapon_mode(
        &mut self,
        id: ObjectId,
        mode: WeaponMode,
        rng: &mut UnityRandom,
    ) {
        let room = self.object(id).pos.room;
        let Some(room) = self.realized(room).map(|r| r.room.clone()) else {
            return;
        };
        let mut seq = self.update_seq;
        let object = &mut self.objects[id.0];
        if let Some(mut weapon) = object.realized.take() {
            let mut cx = ModeChange {
                room: &room,
                rng,
                update_seq: &mut seq,
            };
            weapon.change_mode(mode, object, &mut cx);
            object.realized = Some(weapon);
        }
        self.update_seq = seq;
    }

    /// `Player.CanBeSwallowed` of a realized object (the base game's, without the
    /// Spearmaster's refusal): of the ported objects, a rock.
    #[must_use]
    pub fn can_be_swallowed(&self, e: Entity) -> bool {
        e.object()
            .and_then(|id| self.object(id).realized.as_deref())
            .is_some_and(|w| w.kind == WeaponKind::Rock)
    }

    /// An `IPlayerEdible` that is `Edible`: of the ported objects, a realized fly.
    #[must_use]
    pub fn player_edible(&self, e: Entity) -> bool {
        e.creature()
            .is_some_and(|id| self.creature(id).fly().is_some())
    }

    /// The realized object `id` if it is in room `room`.
    #[must_use]
    pub fn realized_weapon(&self, id: ObjectId, room: i32) -> Option<&Weapon> {
        self.object(id)
            .realized
            .as_deref()
            .filter(|w| w.room == room)
    }

    /// The realized objects in room `room` by their place in its update list, and their
    /// sound loops' emitters.
    #[must_use]
    pub fn room_weapons(&self, room: i32) -> Vec<(u64, ObjectId)> {
        self.objects()
            .filter_map(|(id, o)| {
                let w = o.realized.as_deref().filter(|w| w.room == room)?;
                Some((w.update_order, id))
            })
            .collect()
    }

    /// The loop emitters of the items in room `room`, by their place in its update list.
    #[must_use]
    pub fn room_weapon_loops(&self, room: i32) -> Vec<(u64, ObjectId)> {
        self.objects()
            .filter_map(|(id, o)| {
                let e = o.realized.as_deref()?.sound_loop.emitter?;
                (e.room == room && !e.slated).then_some((e.update_order, id))
            })
            .collect()
    }

    /// Object `id`'s `Update` in room `index`'s update list.
    pub(crate) fn update_weapon(&mut self, index: i32, id: ObjectId, cx: &mut ItemTick<'_>) {
        let Some(local) = self.local(index) else {
            return;
        };
        let World {
            objects,
            realized,
            update_seq,
            ..
        } = self;
        let object = &mut objects[id.0];
        let Some(room) = realized[local].as_mut() else {
            return;
        };
        let Some(mut weapon) = object.realized.take() else {
            return;
        };
        if weapon.room == index {
            let ready_for_ai = room.ready_for_ai();
            let water = room.water.take();
            let mut tick = WeaponTick {
                room_index: index,
                room: &mut room.room,
                water: water.as_ref(),
                ready_for_ai,
                rng: cx.rng,
                sounds: cx.sounds,
                triggers: cx.triggers,
                camera_room: cx.camera_room,
                tick: cx.tick,
                eu: cx.eu,
                update_seq,
            };
            weapon.update(object, &mut tick);
            room.water = water;
        }
        object.realized = Some(weapon);
    }

    /// The `ChunkSoundEmitter.Update` of object `id`'s loop in room `index`: it follows
    /// the chunk and dies when no longer kept alive or heard.
    pub(crate) fn update_weapon_loop(&mut self, index: i32, id: ObjectId, cx: &mut ItemTick<'_>) {
        let Some(w) = self.objects[id.0].realized.as_deref_mut() else {
            return;
        };
        let (owner_room, pos) = (w.room, w.chunk().pos);
        let Some(e) = w
            .sound_loop
            .emitter
            .as_mut()
            .filter(|e| e.room == index && !e.slated)
        else {
            return;
        };
        cx.sounds.move_emitter(e.sound, pos);
        e.update(Some(owner_room), cx.camera_room, cx.tick);
        if e.slated {
            cx.sounds.stop_emitter(e.sound);
        }
    }

    /// `Room.Update`'s collision pass for the items (layer 2; the player alone is layer
    /// 1): each pair closer than both `collisionRange`s (50) on each axis, chunks that
    /// collide with objects and overlap are pushed apart by mass; a pair at the same x
    /// gets a tiny random nudge each (two draws).
    pub(crate) fn collide_weapons(&mut self, index: i32, rng: &mut UnityRandom) {
        let mut layer: Vec<(u64, ObjectId)> = self
            .objects()
            .filter_map(|(id, o)| {
                let w = o.realized.as_deref().filter(|w| w.room == index)?;
                (w.collision_layer == 2).then_some((w.layer_order, id))
            })
            .collect();
        layer.sort_unstable();
        for i in 0..layer.len() {
            for j in i + 1..layer.len() {
                self.collide_pair(layer[i].1, layer[j].1, rng);
            }
        }
    }

    /// One pair of the collision pass (no `Collide` override for rocks and spears).
    fn collide_pair(&mut self, a: ObjectId, b: ObjectId, rng: &mut UnityRandom) {
        let (Some(wa), Some(wb)) = (
            self.objects[a.0].realized.as_deref(),
            self.objects[b.0].realized.as_deref(),
        ) else {
            return;
        };
        let (ca, cb) = (wa.chunk(), wb.chunk());
        let range = 50.0 + 50.0;
        if (ca.pos.x - cb.pos.x).abs() >= range || (ca.pos.y - cb.pos.y).abs() >= range {
            return;
        }
        if !(ca.collide_with_objects && cb.collide_with_objects)
            || !ca.pos.dist_less(cb.pos, ca.rad + cb.rad)
        {
            return;
        }
        let sum = ca.rad + cb.rad;
        let dist = ca.pos.distance(cb.pos);
        let dir = ca.pos.direction_to(cb.pos);
        let share = cb.mass / (ca.mass + cb.mass);
        let va = dir * ((sum - dist) * share);
        let vb = dir * ((sum - dist) * (1.0 - share));
        let mut nudge = [Vec2::ZERO; 2];
        let pa = ca.pos - va;
        let pb = cb.pos + vb;
        if pa.x == pb.x {
            nudge[0] = Vec2::random_unit(rng) * 0.0001;
            nudge[1] = Vec2::random_unit(rng) * 0.0001;
        }
        if let Some(w) = self.objects[a.0].realized.as_deref_mut() {
            let c = w.chunk_mut();
            c.pos -= va;
            c.vel -= va;
            c.vel += nudge[0];
        }
        if let Some(w) = self.objects[b.0].realized.as_deref_mut() {
            let c = w.chunk_mut();
            c.pos += vb;
            c.vel += vb;
            c.vel += nudge[1];
        }
    }

    /// `AbstractRoom.Abstractize`'s objects, newest first: each `Abstractize`s at its own
    /// position (`timeSpentHere` 0, the realized object dropped).
    pub(crate) fn abstractize_objects(&mut self, room: i32) {
        let ids: Vec<ObjectId> = self.room_objects(room).collect();
        for id in ids.into_iter().rev() {
            let o = self.object_mut(id);
            o.time_spent_here = 0;
            o.realized = None;
        }
    }

    /// `Water.Update` of room `index`: the realized items' chunks of layer 0 (stuck or
    /// thrown), then the player's (`player`, layer 1), then the other items' (layer 2).
    pub(crate) fn update_water(
        &mut self,
        index: i32,
        player: &mut [BodyChunk],
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        let Some(local) = self.local(index) else {
            return;
        };
        let World {
            objects, realized, ..
        } = self;
        let Some(RealizedRoom {
            room,
            water: Some(water),
            ..
        }) = realized[local].as_mut()
        else {
            return;
        };
        let mut items: Vec<(i32, u64, &mut BodyChunk)> = objects
            .iter_mut()
            .filter_map(|o| o.realized.as_deref_mut().filter(|w| w.room == index))
            .map(|w| (w.collision_layer, w.layer_order, &mut w.body.chunks[0]))
            .collect();
        items.sort_by_key(|&(layer, order, _)| (layer, order));
        let split = items.partition_point(|&(layer, _, _)| layer < 1);
        let after = items.split_off(split);
        let chunks = items
            .into_iter()
            .map(|(_, _, c)| SurfaceChunk::Object(c))
            .chain(SurfaceChunk::player(player))
            .chain(after.into_iter().map(|(_, _, c)| SurfaceChunk::Object(c)));
        water.update(room, chunks, rng, sounds);
    }

    /// The realized items in room `room` as physical objects of its update list.
    pub(crate) fn room_weapon_entities(&self, room: i32) -> Vec<(u64, Entity)> {
        self.room_weapons(room)
            .into_iter()
            .map(|(seq, id)| (seq, Entity::Object(id)))
            .collect()
    }
}
