//! The world's realized rocks and spears: `ShortCutsReady` realizes a room's objects
//! (61622), each runs in its slot of the room's update list (a thrown one walks
//! collision layer 0 in the middle, 81610), `Room.Update`'s collision pass pushes those
//! of layers 1 and up apart (63520), and `AbstractRoom.Abstractize` drops them
//! (304092). Behaviour: `rainworld-re/notes/items.md` sections 6-11.
//!
//! Mirrors: Room.cs, AbstractRoom.cs, Weapon.cs

use rw_assets::{PlacedObjectType, ResizableData, SoundTable};
use rw_random::UnityRandom;

use super::{RealizedRoom, World};
use crate::creature::{CreatureId, WorldCoordinate};
use crate::item::{
    Entity, ModeChange, ObjectId, Throw, Weapon, WeaponKind, WeaponMode, WeaponTick,
};
use crate::{
    BodyChunk, Cosmetic, CosmeticKind, FloatExt, ImpactCall, InGameNoise, LoopEmitter, RoomCall,
    SoundId, Sounds, Spark, Vec2,
};

/// A loop emitter left in a room's update list by an item that is no longer there; its
/// next update finds the owner's `room` null and destroys it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrphanLoop {
    pub emitter: LoopEmitter,
    pub pos: Vec2,
}

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
    /// The player's creature and its `ThrowDirection`.
    pub player: Option<(CreatureId, i32)>,
    /// The room is a shelter whose door `IsClosing` or `IsOpening`.
    pub shelter_door_moving: bool,
    /// The noises and screen movements the update made, for the game to make in order.
    pub calls: Vec<RoomCall>,
}

/// What a thrown spear sticks by: the abstract tiles of the room's stuck spears and
/// its `NoSpearStickZone`s (position, radius).
#[derive(Debug, Default)]
pub(crate) struct SpearStickRules {
    stuck: Vec<(i32, i32)>,
    zones: Vec<(Vec2, f32)>,
}

/// Which part of an item's `Update` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdatePart {
    /// Up to the `Thrown` branch; true when thrown.
    Start,
    /// The `Thrown` branch after the creatures are told of it, up to the layer-0 walk;
    /// true when the trace against collision layer 1 is due.
    Thrown,
    /// The trace found nothing.
    TracedNothing,
    /// A creature hit behind a wall: `HitWall`, and the update returns.
    HitWall,
    /// `PhysicalObject.Update`: the physics, whose noises go to the tick.
    Physics,
    /// The rest after the physics.
    Finish,
    /// Only `Rock.Update`'s or `Spear.Update`'s own part (after `Weapon.Update`
    /// returned early).
    Own,
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
        if self.object(id).in_den {
            return;
        }
        self.realize_object(id, rng);
        if self.object(id).realized.is_none() {
            return;
        }
        if self.realized(index).is_none() {
            log::warn!("TRYING TO REALIZE IN NON REALIZED ROOM!");
            return;
        }
        if !self.object(id).pos.tile_defined() {
            let node = self.object(id).pos.node;
            self.object_mut(id).pos.tile = self.local_coordinate_of_node(index, node, rng);
        }
        self.place_object(id, rng);
    }

    /// `AbstractPhysicalObject.Realize` of a rock or spear (others are not realized).
    pub(crate) fn realize_object(&mut self, id: ObjectId, rng: &mut UnityRandom) {
        let o = self.object(id);
        if o.realized.is_some() {
            return;
        }
        let Some(weapon) = Weapon::realize(o, rng) else {
            log::debug!("{} objects are not realized", o.kind.value());
            return;
        };
        self.object_mut(id).realized = Some(Box::new(weapon));
    }

    /// `PlaceInRoom` of realized object `id` in the realized room of its position.
    pub(crate) fn place_object(&mut self, id: ObjectId, rng: &mut UnityRandom) {
        let index = self.object(id).pos.room;
        let Some(room) = self.realized(index).map(|r| r.room.clone()) else {
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
        let seq = self.update_seq();
        if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
            w.change_overlap(overlap, seq);
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
        self.change_weapon_mode_spun(id, mode, None, rng);
    }

    /// [`Self::change_weapon_mode`]; a spear going `Free` with `spin` takes that spin
    /// instead of drawing it (`SpearOnBack.DropSpear` drew it already).
    pub(crate) fn change_weapon_mode_spun(
        &mut self,
        id: ObjectId,
        mode: WeaponMode,
        spin: Option<crate::item::SpearSpin>,
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
            weapon.change_mode_spun(mode, object, &mut cx, spin);
            object.realized = Some(weapon);
        }
        self.update_seq = seq;
        self.pull_out_of_stuck_object(id);
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

    /// The loop emitters, held or let go, of the items in room `room`, by their place in
    /// its update list.
    #[must_use]
    pub fn room_weapon_loops(&self, room: i32) -> Vec<(u64, ObjectId)> {
        self.objects()
            .filter_map(|(id, o)| Some((id, o.realized.as_deref()?)))
            .flat_map(|(id, w)| {
                w.sound_loop
                    .emitters()
                    .filter(move |e| e.room == room)
                    .map(move |e| (e.update_order, id))
            })
            .collect()
    }

    #[must_use]
    pub fn orphan_loop_count(&self) -> usize {
        self.orphan_loops.len()
    }

    /// The orphaned loop emitters in room `room`'s update list, by their place in it.
    #[must_use]
    pub fn room_orphan_loops(&self, room: i32) -> Vec<u64> {
        self.orphan_loops
            .iter()
            .filter(|o| o.emitter.room == room)
            .map(|o| o.emitter.update_order)
            .collect()
    }

    /// `ChunkSoundEmitter.Update` of the orphaned emitter that joined room `index` at
    /// `order`: its chunk's owner has no room, so it is destroyed and its sound cut.
    pub(crate) fn update_orphan_loop(&mut self, index: i32, order: u64, cx: &mut ItemTick<'_>) {
        let Some(o) = self
            .orphan_loops
            .iter_mut()
            .find(|o| o.emitter.update_order == order && o.emitter.room == index)
        else {
            return;
        };
        cx.sounds.move_emitter(o.emitter.sound, o.pos);
        o.emitter.update(None, cx.camera_room, cx.tick);
        if o.emitter.slated {
            cx.sounds.stop_emitter(o.emitter.sound);
        }
        self.orphan_loops.retain(|o| !o.emitter.slated);
    }

    /// What a thrown spear of room `index` sticks by (nothing for other weapons).
    pub(crate) fn spear_stick_rules(&self, index: i32, id: ObjectId) -> SpearStickRules {
        let thrown_spear = self
            .realized_weapon(id, index)
            .is_some_and(|w| w.mode == WeaponMode::Thrown && w.spear().is_some());
        if thrown_spear {
            SpearStickRules {
                stuck: self.stuck_spear_tiles(index),
                zones: self.no_spear_stick_zones(index),
            }
        } else {
            SpearStickRules::default()
        }
    }

    /// One part of object `id`'s `Update` in room `index`; the sparks it makes join the
    /// room. Returns the part's answer (see [`UpdatePart`]).
    pub(crate) fn weapon_part(
        &mut self,
        index: i32,
        id: ObjectId,
        cx: &mut ItemTick<'_>,
        rules: &SpearStickRules,
        part: UpdatePart,
    ) -> bool {
        let Some(local) = self.local(index) else {
            return false;
        };
        let mut added = Vec::new();
        let World {
            objects,
            realized,
            update_seq,
            ..
        } = self;
        let object = &mut objects[id.0];
        let Some(room) = realized[local].as_mut() else {
            return false;
        };
        let Some(mut weapon) = object.realized.take() else {
            return false;
        };
        let mut walk = false;
        if weapon.room == index {
            let ready_for_ai = room.ready_for_ai();
            weapon.body.params.room_gravity = room.gravity;
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
                player: cx.player,
                stuck_spear_tiles: &rules.stuck,
                no_stick_zones: &rules.zones,
                shelter_door_moving: cx.shelter_door_moving,
                cosmetics: &mut added,
                calls: &mut cx.calls,
            };
            match part {
                UpdatePart::Start => walk = weapon.update_start(object, &mut tick),
                UpdatePart::Thrown => walk = weapon.thrown_update(object, &mut tick),
                UpdatePart::TracedNothing => weapon.traced_nothing(),
                UpdatePart::HitWall => {
                    weapon.hit_wall(object, &mut tick);
                    weapon.floor_bounce_frames = 0;
                }
                UpdatePart::Physics => {
                    let calls = weapon.carryable_physics(&mut tick);
                    tick.calls.extend(calls.into_iter().map(|c| match c {
                        ImpactCall::ScreenMovement(m) => RoomCall::ScreenMovement(m),
                        ImpactCall::Noise(pos, strength) => RoomCall::Noise(InGameNoise {
                            pos,
                            strength,
                            source: Some(Entity::Object(id)),
                            interesting: 4.0,
                        }),
                    }));
                }
                UpdatePart::Finish => weapon.update_after_physics(object, &mut tick),
                UpdatePart::Own => weapon.update_own(object, &mut tick),
            }
            room.water = water;
        }
        object.realized = Some(weapon);
        for (seq, kind) in added {
            self.add_cosmetic_at(index, seq, kind);
        }
        walk
    }

    /// The abstract tiles of room `index`'s realized spears stuck in walls.
    fn stuck_spear_tiles(&self, index: i32) -> Vec<(i32, i32)> {
        self.room_objects(index)
            .filter_map(|id| {
                let o = self.object(id);
                let w = o.realized.as_deref()?;
                (w.spear().is_some() && w.mode == WeaponMode::StuckInWall).then_some(o.pos.tile)
            })
            .collect()
    }

    /// Room `index`'s `NoSpearStickZone`s (every one in its settings: the game does not
    /// look at `active` here), as position and radius.
    fn no_spear_stick_zones(&self, index: i32) -> Vec<(Vec2, f32)> {
        let Some(r) = self.realized(index) else {
            return Vec::new();
        };
        r.settings
            .placed_objects
            .iter()
            .filter(|o| o.object_type(self.mods) == Some(PlacedObjectType::NoSpearStickZone))
            .filter_map(|o| {
                let rad = ResizableData::parse(&o.data)?.rad();
                Some((Vec2::new(o.pos[0], o.pos[1]), rad))
            })
            .collect()
    }

    /// `room.AddObject` of a cosmetic sprite at update-list place `seq`.
    pub(crate) fn add_cosmetic_at(&mut self, room: i32, seq: u64, kind: CosmeticKind) {
        self.drip_ids += 1;
        let cosmetic = Cosmetic {
            id: self.drip_ids,
            room,
            update_order: seq,
            slated: false,
            kind,
        };
        if let Some(r) = self.realized_mut(room) {
            r.cosmetics.push(cosmetic);
        }
    }

    /// `room.AddObject` of a cosmetic sprite at the end of the update list.
    pub fn add_cosmetic(&mut self, room: i32, kind: CosmeticKind) {
        let seq = self.next_update_seq();
        self.add_cosmetic_at(room, seq, kind);
    }

    /// `room.physicalObjects[0]` of room `index` without weapon `me`: its flies,
    /// overseers and thrown or wall-stuck weapons, in the order they joined the layer.
    pub(crate) fn layer_zero(&self, index: i32, me: ObjectId) -> Vec<Entity> {
        let mut layer: Vec<(u64, Entity)> = self
            .creatures()
            .filter_map(|(cid, cr)| {
                let f = cr.fly().filter(|f| f.room == Some(index))?;
                Some((f.update_order, Entity::Creature(cid)))
            })
            .collect();
        layer.extend(
            self.room_overseers(index)
                .into_iter()
                .map(|(seq, cid)| (seq, Entity::Creature(cid))),
        );
        for (oid, o) in self.objects() {
            if let Some(w) = o.realized.as_deref().filter(|w| w.room == index)
                && w.collision_layer == 0
                && oid != me
            {
                layer.push((w.layer_order, Entity::Object(oid)));
            }
        }
        layer.sort_unstable_by_key(|&(seq, _)| seq);
        layer.into_iter().map(|(_, e)| e).collect()
    }

    /// The realized body chunk of a layer-0 object (a fly's, an overseer's or a
    /// weapon's only chunk).
    pub(crate) fn layer_zero_chunk(&mut self, e: Entity) -> Option<&mut BodyChunk> {
        match e {
            Entity::Object(id) => self.weapon_mut(id).map(Weapon::chunk_mut),
            Entity::Creature(id) => match &mut self.creature_mut(id).realized {
                Some(crate::creature::RealizedCreature::Fly(f)) => Some(f.chunk_mut()),
                Some(crate::creature::RealizedCreature::Overseer(b)) => Some(b.chunk_mut()),
                _ => None,
            },
        }
    }

    /// `canBeHitByWeapons` of a layer-0 object.
    #[must_use]
    pub fn can_be_hit_by_weapons(&self, e: Entity) -> bool {
        match e {
            Entity::Object(_) => true,
            Entity::Creature(id) => self
                .creature(id)
                .overseer_body()
                .is_none_or(|b| b.can_be_hit_by_weapons),
        }
    }

    /// `HitAnotherThrownWeapon` (81710): two thrown weapons crossing in opposite
    /// directions parry, with sparks (3, plus 2 for each spear) and a shared random
    /// direction they glance off along.
    pub(crate) fn hit_another_thrown_weapon(
        &mut self,
        index: i32,
        a: ObjectId,
        b: ObjectId,
        cx: &mut ItemTick<'_>,
    ) {
        let (Some(wa), Some(wb)) = (
            self.realized_weapon(a, index),
            self.realized_weapon(b, index),
        ) else {
            return;
        };
        let (ca, cb) = (wa.chunk(), wb.chunk());
        if (cb.pos.x - cb.last_pos.x < 0.0) == (ca.pos.x - ca.last_pos.x < 0.0) {
            return;
        }
        let between = cb.last_pos.lerp(ca.last_pos, 0.5);
        let n =
            3 + if wa.spear().is_some() { 2 } else { 0 } + if wb.spear().is_some() { 2 } else { 0 };
        for _ in 0..n {
            let at = between + Vec2::from_degrees(cx.rng.value() * 360.0) * (5.0 * cx.rng.value());
            let dir = Vec2::from_degrees(cx.rng.value() * 360.0);
            let vel = dir * (2.0f32.lerp(7.0, cx.rng.value()) * n as f32);
            let spark = Spark::new(at, vel, [1.0; 4], 10, 170, cx.rng);
            self.add_cosmetic(index, CosmeticKind::Spark(spark));
        }
        let dir = Vec2::from_degrees(cx.rng.value() * 360.0);
        let speed = self
            .realized_weapon(a, index)
            .map_or(0.0, |w| w.chunk().vel.length());
        self.weapon_deflect(a, between, dir, speed, cx.rng);
        let speed = self
            .realized_weapon(a, index)
            .map_or(0.0, |w| w.chunk().vel.length());
        self.weapon_deflect(b, between, -dir, speed, cx.rng);
        cx.sounds.play_at(
            SoundId::SpearBounceOffCreautureShell,
            between,
            1.0,
            1.0,
            cx.rng,
        );
    }

    /// `WeaponDeflect` of realized object `id`.
    fn weapon_deflect(
        &mut self,
        id: ObjectId,
        between: Vec2,
        dir: Vec2,
        speed: f32,
        rng: &mut UnityRandom,
    ) {
        let room = self.object(id).pos.room;
        let Some(room) = self.realized(room).map(|r| r.room.clone()) else {
            return;
        };
        let mut seq = self.update_seq;
        let object = &mut self.objects[id.0];
        if let Some(mut w) = object.realized.take() {
            let mut cx = ModeChange {
                room: &room,
                rng,
                update_seq: &mut seq,
            };
            w.weapon_deflect(between, dir, speed, object, &mut cx);
            object.realized = Some(w);
        }
        self.update_seq = seq;
    }

    /// `Weapon.Thrown` of realized object `id` (it must be in its room); returns the
    /// throw sound `Rock`'s or `Spear`'s override plays.
    pub(crate) fn throw_weapon(
        &mut self,
        id: ObjectId,
        throw: Throw,
        rng: &mut UnityRandom,
    ) -> Option<SoundId> {
        let room = self.object(id).pos.room;
        let room = self.realized(room).map(|r| r.room.clone())?;
        let mut seq = self.update_seq;
        let object = &mut self.objects[id.0];
        let mut w = object.realized.take()?;
        let mut cx = ModeChange {
            room: &room,
            rng,
            update_seq: &mut seq,
        };
        let sound = w.thrown(throw, object, &mut cx);
        object.realized = Some(w);
        self.update_seq = seq;
        Some(sound)
    }

    /// `obj is Spear`: a realized spear.
    #[must_use]
    pub fn is_spear(&self, e: Entity) -> bool {
        e.object()
            .and_then(|id| self.object(id).realized.as_deref())
            .is_some_and(|w| w.spear().is_some())
    }

    /// The realized weapon `id`, mutable.
    pub(crate) fn weapon_mut(&mut self, id: ObjectId) -> Option<&mut Weapon> {
        self.object_mut(id).realized.as_deref_mut()
    }

    /// The cosmetic sprites in room `index`'s update list.
    #[must_use]
    pub fn room_cosmetics(&self, index: i32) -> &[Cosmetic] {
        self.realized(index).map_or(&[], |r| &r.cosmetics)
    }

    /// Cosmetic `id`'s `Update` in room `index`; a destroyed one leaves the room.
    pub(crate) fn update_cosmetic(
        &mut self,
        index: i32,
        id: u64,
        tie: crate::cosmetic::CosmeticTie,
        rng: &mut UnityRandom,
        sounds: &mut crate::Sounds,
    ) {
        let Some(RealizedRoom {
            room,
            water,
            cosmetics,
            ..
        }) = self.realized_mut(index)
        else {
            return;
        };
        let Some(c) = cosmetics.iter_mut().find(|c| c.id == id) else {
            return;
        };
        let drip = c.update(room, water.as_mut(), tie, rng, sounds);
        if c.slated {
            cosmetics.retain(|c| c.id != id);
        }
        if let Some(drip) = drip {
            self.add_drip(index, drip);
        }
    }

    /// The `ChunkSoundEmitter.Update` of object `id`'s loop emitter that joined room
    /// `index` at `order`: it follows the chunk and dies when no longer kept alive or
    /// heard.
    pub(crate) fn update_weapon_loop(
        &mut self,
        index: i32,
        id: ObjectId,
        order: u64,
        cx: &mut ItemTick<'_>,
    ) {
        let Some(w) = self.objects[id.0].realized.as_deref_mut() else {
            return;
        };
        let (owner_room, pos) = (w.room, w.chunk().pos);
        let Some(e) = w
            .sound_loop
            .emitter_mut(order)
            .filter(|e| e.room == index && !e.slated)
        else {
            return;
        };
        cx.sounds.move_emitter(e.sound, pos);
        e.update(Some(owner_room), cx.camera_room, cx.tick);
        if e.slated {
            cx.sounds.stop_emitter(e.sound);
            w.sound_loop.released.retain(|e| !e.slated);
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
        self.orphan_loops.retain(|o| o.emitter.room != room);
        for id in ids.into_iter().rev() {
            let o = self.object_mut(id);
            o.time_spent_here = 0;
            o.realized = None;
        }
    }

    /// The realized items in room `room` as physical objects of its update list.
    pub(crate) fn room_weapon_entities(&self, room: i32) -> Vec<(u64, Entity)> {
        self.room_weapons(room)
            .into_iter()
            .map(|(seq, id)| (seq, Entity::Object(id)))
            .collect()
    }
}
