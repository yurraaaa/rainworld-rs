//! The world's realized rocks and spears: `ShortCutsReady` realizes a room's objects
//! (61622), each runs in its slot of the room's update list, `Room.Update`'s collision
//! pass pushes those of layers 1 and up apart (63520), and `AbstractRoom.Abstractize`
//! drops them (304092). Behaviour: `rainworld-re/notes/items.md` sections 6-8.

use rw_assets::SoundTable;
use rw_random::UnityRandom;

use super::{RealizedRoom, World};
use crate::item::{Entity, ModeChange, ObjectId, Weapon, WeaponTick};
use crate::{BodyChunk, Sounds, SurfaceChunk, Vec2};

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
    /// in a den, in list order, is realized and placed (`RealizeInRoom`: `Realize`, an
    /// undefined tile taken from its node, `PlaceInRoom`). Objects held by a creature
    /// are realized with it (step 3).
    pub(crate) fn realize_room_objects(&mut self, index: i32, rng: &mut UnityRandom) {
        let Some(room) = self.realized(index).map(|r| r.room.clone()) else {
            return;
        };
        let ids: Vec<ObjectId> = self.room_objects(index).collect();
        for id in ids {
            let o = self.object(id);
            if o.in_den {
                continue;
            }
            if o.pos.room != index {
                log::warn!("Object in wrong room! {:?}", o.id);
                continue;
            }
            if o.realized.is_none() {
                let Some(weapon) = Weapon::realize(o, rng) else {
                    log::debug!("{} objects are not realized", o.kind.value());
                    continue;
                };
                self.object_mut(id).realized = Some(Box::new(weapon));
            }
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
