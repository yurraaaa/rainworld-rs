//! The slugcat and the objects it holds, base game with Remix off: `Grabability`
//! (92773), `HeavyCarry` (93075), `FreeHand` (93092), `CanIPickThisUp` (92675),
//! `PickupCandidate` (93393) with `PlayerCarryableItem.Blink` (79330), the pick-up and
//! lay-down end of `GrabUpdate` (92399-92580) with `ReleaseObject` (93614) and
//! `PickedUp`, swallowing and regurgitating (`GrabUpdate` 92210-92280, `SwallowObject`
//! 93760, `Regurgitate` 93817), `GetHeldItemDirection` (87891), and the stomach's and
//! hands' save strings (`BringUpToDate` 47383, the `Player` constructor 86588).
//! Behaviour: `rainworld-re/notes/items.md` section 10.
//!
//! Mirrors: Player.cs (Grabability, HeavyCarry, FreeHand, CanIPickThisUp, PickupCandidate, GrabUpdate, ReleaseObject, SwallowObject, Regurgitate),
//!   PlayerCarryableItem.cs (Blink)

use super::{PlayerObserver, RainWorldGame};
use crate::item::{Entity, ObjectId, SavedObject, Throw, WeaponMode};
use crate::player::StomachObject;
use crate::room_ext::RoomExt;
use crate::world::OrphanLoop;
use crate::{
    Animation, BodyMode, CosmeticKind, ExplosionSpikes, FloatExt, Input, SoundId, Vec2, WaterDrip,
};

/// `Player.ObjectGrabability`, in the game's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grabability {
    CantGrab,
    OneHand,
    BigOneHand,
    TwoHands,
    Drag,
}

impl RainWorldGame {
    /// `Player.Grabability` of a realized object: a spear is held big in one hand, a
    /// rock in one hand, a fly in one hand when the player holds it or it is out of a
    /// shortcut's delay; anything else cannot be grabbed.
    #[must_use]
    pub fn grabability(&self, e: Entity) -> Grabability {
        match e {
            Entity::Object(id) => match self.world.object(id).realized.as_deref() {
                Some(w) if w.spear().is_some() => Grabability::BigOneHand,
                Some(_) => Grabability::OneHand,
                None => Grabability::CantGrab,
            },
            Entity::Creature(id) => {
                let c = self.world.creature(id);
                let Some(f) = c.fly() else {
                    return Grabability::CantGrab;
                };
                let held_by_me = c
                    .grabbed_by
                    .first()
                    .is_some_and(|g| g.grabber == self.player_id);
                if held_by_me || f.shortcut_delay == 0 {
                    Grabability::OneHand
                } else {
                    Grabability::CantGrab
                }
            }
        }
    }

    /// `HeavyCarry`: dragged, two-handed, or heavier than 0.6 of the slugcat.
    fn heavy_carry(&self, e: Entity) -> bool {
        let g = self.grabability(e);
        g == Grabability::Drag
            || g == Grabability::TwoHands
            || self.world.total_mass(e) > self.player_ref().total_mass() * 0.6
    }

    /// `FreeHand`: none while hand 0 carries something heavy, else the first empty one.
    fn free_hand(&self) -> Option<usize> {
        let grasps = self.player_grasps();
        if grasps[0].is_some_and(|e| self.heavy_carry(e)) {
            return None;
        }
        (0..2).find(|&i| grasps[i].is_none())
    }

    /// `CanIPickThisUp` (Remix off: a spear stuck in a wall stays; the Hunter's spear on
    /// the back is not ported).
    fn can_i_pick_this_up(&self, e: Entity) -> bool {
        let g = self.grabability(e);
        if g == Grabability::CantGrab {
            return false;
        }
        let grasps = self.player_grasps();
        let weapon = e
            .object()
            .and_then(|id| self.world.object(id).realized.as_deref());
        if weapon.is_some_and(|w| w.spear().is_some() && w.mode == WeaponMode::OnBack) {
            return false;
        }
        if g == Grabability::BigOneHand
            && grasps
                .iter()
                .flatten()
                .any(|&h| self.grabability(h) > Grabability::OneHand)
        {
            return false;
        }
        if let Some(w) = weapon
            && (matches!(w.mode, WeaponMode::StuckInWall | WeaponMode::Thrown)
                || w.forbidden_to_player > 0)
        {
            return false;
        }
        let mut big = 0;
        for &h in grasps.iter().flatten() {
            if h == e {
                return false;
            }
            if self.grabability(h) > Grabability::OneHand {
                big += 1;
            }
        }
        big < 2 && !(big > 0 && g > Grabability::BigOneHand)
    }

    /// Room `index`'s `physicalObjects` as `PickupCandidate` walks them: layer 0 (flies,
    /// thrown and stuck weapons), then layer 2 (the other weapons), each in the order
    /// they joined it. The player (layer 1) and the overseers (which cannot be grabbed)
    /// are left out.
    fn pickup_order(&self, index: i32) -> Vec<Entity> {
        let mut objects: Vec<(i32, u64, Entity)> = self
            .world
            .creatures()
            .filter_map(|(id, c)| {
                let f = c.fly().filter(|f| f.room == Some(index))?;
                Some((0, f.update_order, Entity::Creature(id)))
            })
            .collect();
        for (_, id) in self.world.room_weapons(index) {
            if let Some(w) = self.world.object(id).realized.as_deref() {
                objects.push((w.collision_layer, w.layer_order, Entity::Object(id)));
            }
        }
        objects.sort_unstable_by_key(|&(layer, order, _)| (layer, order));
        objects.into_iter().map(|(_, _, e)| e).collect()
    }

    /// `PickupCandidate(favorSpears)`: the nearest object within reach (40 px beyond its
    /// radius, and in sight or within 20) the player can pick up; a spear counts
    /// `favor_spears` nearer and an object on the facing side 10 px nearer.
    pub(super) fn pickup_candidate(
        &self,
        index: i32,
        room: &rw_assets::Room,
        favor_spears: f32,
    ) -> Option<Entity> {
        let p = self.player_ref();
        let head = p.head().pos;
        let flip = p.flip_direction();
        let mut best = None;
        let mut best_dist = f32::MAX;
        for e in self.pickup_order(index) {
            let Some(c) = self.world.body_chunks(e).and_then(|c| c.first()) else {
                continue;
            };
            let (pos, rad) = (c.pos, c.rad);
            let weapon = e
                .object()
                .and_then(|id| self.world.object(id).realized.as_deref());
            if weapon.is_some_and(|w| w.forbidden_to_player >= 1) {
                continue;
            }
            let in_reach = head.dist_less(pos, rad + 40.0)
                && (head.dist_less(pos, rad + 20.0)
                    || crate::room_ext::RoomExt::visual_contact(room, head, pos));
            if !in_reach || !self.can_i_pick_this_up(e) {
                continue;
            }
            let mut dist = head.distance(pos);
            if weapon.is_some_and(|w| w.spear().is_some()) {
                dist -= favor_spears;
            }
            if (pos.x < head.x) == (flip < 0) {
                dist -= 10.0;
            }
            if dist < best_dist {
                best = Some(e);
                best_dist = dist;
            }
        }
        best
    }

    /// `GrabUpdate`'s pick-up candidate: none while hand 0 holds something two-handed,
    /// else `PickupCandidate(20)` unless grabbing is held off; a new candidate that is a
    /// carryable item `Blink`s.
    pub(super) fn update_pickup_candidate(&mut self, index: i32, room: &rw_assets::Room) {
        if self.player_grasps()[0].is_some_and(|e| self.grabability(e) == Grabability::TwoHands) {
            self.player_mut().grab_mut().pick_up_candidate = None;
            return;
        }
        let candidate = if self.player_ref().grab_state().dont_grab_stuff < 1 {
            self.pickup_candidate(index, room, 20.0)
        } else {
            None
        };
        if candidate != self.player_ref().grab_state().pick_up_candidate
            && let Some(Entity::Object(id)) = candidate
        {
            self.blink_item(index, id);
        }
        self.player_mut().grab_mut().pick_up_candidate = candidate;
    }

    /// `PlayerCarryableItem.Blink`: an item nobody holds plays
    /// `UI_Weapon_In_Range_To_Pick_Up` and blinks for 5 ticks.
    fn blink_item(&mut self, index: i32, id: ObjectId) {
        if !self.world.object(id).grabbed_by.is_empty() {
            return;
        }
        let Some(pos) = self.world.chunk_pos(Entity::Object(id), 0) else {
            return;
        };
        self.play_on_object(index, SoundId::UiWeaponInRangeToPickUp, pos);
        if let Some(w) = self.world.object_mut(id).realized.as_deref_mut() {
            w.blink = 5;
        }
    }

    /// The end of `GrabUpdate` while `wantToPickUp` lasts: down held for 5 ticks (10 and
    /// no sideways input in deep water), or anything heavy in hand 0, lays the first held
    /// object down; otherwise the pick-up candidate is picked up in the first free hand
    /// (a two-handed one empties both hands first; with both hands full a held fly is let
    /// go).
    pub(super) fn drop_or_pick_up(
        &mut self,
        index: i32,
        input: &[Input; crate::INPUT_HISTORY],
        observer: &mut dyn PlayerObserver,
    ) {
        let grasps = self.player_grasps();
        let p = self.player_ref();
        let mut drop = if p.animation() == Animation::DeepSwim {
            grasps.iter().any(Option::is_some) && input[..10].iter().all(|i| i.y <= -1 && i.x == 0)
        } else {
            input[..5].iter().all(|i| i.y <= -1)
        };
        if grasps[0].is_some_and(|e| self.heavy_carry(e)) {
            drop = true;
        }
        if drop {
            if let Some(i) = (0..2).find(|&i| grasps[i].is_some()) {
                self.player_mut().grab_mut().want_to_pick_up = 0;
                self.release_object(index, i);
            }
            return;
        }
        let Some(candidate) = self.player_ref().grab_state().pick_up_candidate else {
            return;
        };
        let free = grasps.iter().filter(|g| g.is_none()).count();
        if self.grabability(candidate) == Grabability::TwoHands {
            for i in 0..2 {
                if self.player_grasps()[i].is_some() {
                    self.player_release_grasp(i);
                }
            }
        } else if free == 0 {
            let fly = (0..2).find(|&i| {
                grasps[i]
                    .and_then(Entity::creature)
                    .is_some_and(|f| self.world.creature(f).fly().is_some())
            });
            if let Some(i) = fly {
                self.player_release_grasp(i);
            }
        }
        if let Some(k) = (0..2).find(|&k| self.player_grasps()[k].is_none()) {
            match candidate {
                Entity::Creature(fly) => {
                    let pos = self.world.fly_ref(fly).pos();
                    self.play_in_room(index, SoundId::SlugcatPickUpCreature, pos);
                }
                Entity::Object(id) => self.picked_up(index, id),
            }
            self.slugcat_grab(candidate, k, observer);
            if let Entity::Creature(fly) = candidate
                && self.world.fly_ref(fly).graphics
            {
                observer.fly_to_front(self.world.creature(fly).id);
            }
            log::info!("picked up {}", self.entity_name(candidate));
        }
        self.player_mut().grab_mut().want_to_pick_up = 0;
    }

    /// The pick-up of a carryable item: whoever else holds it lets go
    /// (`GrabbedObjectSnatched`), then `PickedUp` and its sound.
    fn picked_up(&mut self, index: i32, id: ObjectId) {
        let holders = self.world.object(id).grabbed_by.clone();
        for g in holders {
            self.world.release_grasp(g.grabber, g.grasp_used);
        }
        let sound = self.world.weapon_picked_up(id, &mut self.rng);
        self.flush_dislodged();
        if let Some(pos) = self.world.chunk_pos(Entity::Object(id), 0) {
            self.play_on_object(index, sound, pos);
        }
    }

    /// The object's kind, for the log.
    fn entity_name(&self, e: Entity) -> &'static str {
        match e {
            Entity::Creature(_) => "a bat fly",
            Entity::Object(id) => self.world.object(id).kind.value(),
        }
    }

    /// `ReleaseObject`: `Slugcat_Lay_Down_Creature` or `_Object` on the held chunk, a
    /// carryable item forbidden to the player for 40 ticks, let go.
    fn release_object(&mut self, index: i32, grasp: usize) {
        let Some(obj) = self.player_grasps()[grasp] else {
            return;
        };
        let pos = self.world.chunk_pos(obj, 0).unwrap_or_default();
        match obj {
            Entity::Creature(_) => self.play_in_room(index, SoundId::SlugcatLayDownCreature, pos),
            Entity::Object(id) => {
                self.play_on_object(index, SoundId::SlugcatLayDownObject, pos);
                self.world.forbid(id);
            }
        }
        self.player_release_grasp(grasp);
    }

    /// `ThrowObject`'s weapon branch (93127-93222, Remix and More Slugcats off, no
    /// adrenaline): thrown along `ThrowDirection` (straight down from a flip holding
    /// only down) from 10 px ahead of the head and 4 up (the head itself when that is
    /// Solid), with `frc` 1; `ThrownSpear`; the belly-slide throws; the recoil, the
    /// throwing hand's animation and `Blink(15)`.
    pub(super) fn throw_weapon(
        &mut self,
        index: i32,
        grasp: usize,
        id: ObjectId,
        observer: &mut dyn PlayerObserver,
    ) {
        let eu = self.even_update();
        let p = self.player_ref();
        let input = p.input_history()[0];
        let mut dir = (p.throw_direction(), 0);
        if p.animation() == Animation::Flip && input.y < 0 && input.x == 0 {
            dir = (0, -1);
        }
        let d = Vec2::new(dir.0 as f32, dir.1 as f32);
        let (main_pos, main_vel) = (p.head().pos, p.head().vel);
        let mut pos = main_pos + d * 10.0 + Vec2::new(0.0, 4.0);
        let solid = self
            .world
            .realized(index)
            .is_some_and(|r| r.room.tile_at_pos(pos).is_solid());
        if solid {
            pos = main_pos;
        }
        let throw = Throw {
            by: self.player_id,
            by_vel: main_vel,
            pos,
            first_frame_trace_from_pos: Some(main_pos - d * 10.0),
            dir,
            frc: 1.0,
            eu,
        };
        let Some(sound) = self.world.throw_weapon(id, throw, &mut self.rng) else {
            return;
        };
        if let Some(at) = self.world.chunk_pos(Entity::Object(id), 0) {
            self.play_on_object(index, sound, at);
        }
        let spear = self
            .world
            .weapon_mut(id)
            .is_some_and(|w| w.spear().is_some());
        if spear {
            self.thrown_spear(id);
        }
        self.belly_slide_throw(index, id, dir.0);
        let p = self.player_mut();
        p.body.chunks[0].vel += d * 8.0;
        p.body.chunks[1].vel -= d * 4.0;
        observer.player_throw_object(grasp, Entity::Object(id));
        self.player_mut().blink(15);
    }

    /// `ThrownSpear` (93423): with no throwing skill (the Monk, a malnourished slugcat)
    /// 18 throw frames, a damage bonus of `0.6 + 0.3 value^4` (one draw) and 0.77 of the
    /// speed; the Hunter's skill 2 a bonus of 1.25 and 1.2 times the speed.
    pub(super) fn thrown_spear(&mut self, id: ObjectId) {
        let skill = self.character_stats.throwing_skill;
        let bonus = match skill {
            0 => Some(0.6 + 0.3 * self.rng.value().mathf_pow(4.0)),
            2 => Some(1.25),
            _ => None,
        };
        let Some(w) = self.world.weapon_mut(id) else {
            return;
        };
        let Some(bonus) = bonus else { return };
        if skill == 0 {
            w.throw_mode_frames = 18;
            w.chunk_mut().vel.x *= 0.77;
        } else {
            w.chunk_mut().vel.x *= 1.2;
        }
        w.set_spear_damage_bonus(bonus);
    }

    /// The belly-slide throws (ticks 9 to 14 of a slide): forward, with some throwing
    /// skill, the weapon gets 15 px a tick more and skims the floor (30 bounce frames, a
    /// spear always sticks, through no floors, 5 down); backward, unless the slide is
    /// already long, it goes up 3 (a rock 5) and the slugcat kicks off into a long belly
    /// slide with spikes behind it and `Slugcat_Rocket_Jump`.
    fn belly_slide_throw(&mut self, index: i32, id: ObjectId, dir_x: i32) {
        let p = self.player_ref();
        let n = p.roll_counter();
        if p.animation() != Animation::BellySlide || n <= 8 || n >= 15 {
            return;
        }
        let roll = p.roll_direction();
        let (long, hips) = (p.long_belly_slide(), p.hips().pos);
        let skill = self.character_stats.throwing_skill;
        if dir_x == roll && skill > 0 {
            let Some(w) = self.world.weapon_mut(id) else {
                return;
            };
            w.chunk_mut().vel.x += dir_x as f32 * 15.0;
            if w.heavy_weapon() {
                w.floor_bounce_frames = 30;
                w.set_always_stick_in_walls();
                let c = w.chunk_mut();
                c.go_through_floors = false;
                c.vel.y -= 5.0;
            }
            w.change_dir_counter = 0;
        } else if dir_x == -roll && !long {
            let Some(w) = self.world.weapon_mut(id) else {
                return;
            };
            w.chunk_mut().vel.y += if w.spear().is_some() { 3.0 } else { 5.0 };
            w.change_dir_counter = 0;
            let at = hips + Vec2::new(roll as f32 * -40.0, 0.0);
            if let Some(room) = self.world.realized(index).map(|r| r.room.clone()) {
                let spikes = ExplosionSpikes::new(
                    &room,
                    at,
                    6,
                    5.5,
                    4.0,
                    4.5,
                    21.0,
                    [1.0, 1.0, 1.0, 0.25],
                    &mut self.rng,
                );
                self.world.add_cosmetic(index, CosmeticKind::Spikes(spikes));
            }
            self.player_mut().start_long_belly_slide();
            self.play_on_player(index, SoundId::SlugcatRocketJump);
        }
    }

    /// The swallow-and-regurgitate counter while pick-up is held (`flag2`): over 110 with
    /// something in the stomach it comes back up, over 90 with an empty stomach the
    /// first swallowable object in hand goes down (the head pulled 2 px toward it,
    /// `swallowing` 20 on the graphics).
    pub(super) fn swallow_and_regurgitate_update(
        &mut self,
        index: i32,
        observer: &mut dyn PlayerObserver,
    ) {
        let g = self.player_mut().grab_mut();
        g.swallow_and_regurgitate_counter += 1;
        let counter = g.swallow_and_regurgitate_counter;
        let stomach = self.player_ref().object_in_stomach().is_some();
        if stomach && counter > 110 {
            self.regurgitate(index, observer);
            self.player_mut().grab_mut().swallow_and_regurgitate_counter = 0;
        } else if !stomach && counter > 90 {
            let grasps = self.player_grasps();
            let Some(i) =
                (0..2).find(|&i| grasps[i].is_some_and(|e| self.world.can_be_swallowed(e)))
            else {
                return;
            };
            let Some(at) = grasps[i].and_then(|e| self.world.chunk_pos(e, 0)) else {
                return;
            };
            let head = &mut self.player_mut().body.chunks[0];
            head.pos += at.direction_to(head.pos) * 2.0;
            self.swallow_object(index, i);
            self.player_mut().grab_mut().swallow_and_regurgitate_counter = 0;
            observer.player_swallowing();
        }
    }

    /// `SwallowObject`: the held object goes into the stomach (a spear's wall cycles
    /// cleared), the hand lets go, the realized object leaves the room and is abstracted
    /// at the player's position, out of the room's entities; the body bobs up 2 px a tick
    /// and `Slugcat_Swallow_Item` plays.
    fn swallow_object(&mut self, index: i32, grasp: usize) {
        let Some(Entity::Object(id)) = self.player_grasps()[grasp] else {
            return;
        };
        if let Some(s) = self.world.object_mut(id).spear_mut() {
            s.stuck_in_wall_cycles = 0;
        }
        self.player_release_grasp(grasp);
        let coord = self.pos();
        let orphans = self
            .world
            .weapon_mut(id)
            .map(|w| {
                let pos = w.chunk().pos;
                w.sound_loop
                    .take_emitters()
                    .into_iter()
                    .map(|emitter| OrphanLoop { emitter, pos })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.world.orphan_loops.extend(orphans);
        self.world.abstractize_object(id, coord);
        self.world.remove_object_entity(id);
        self.world.abstractize_object(id, coord);
        let p = self.player_mut();
        p.set_object_in_stomach(Some(StomachObject::Object(id)));
        p.body.chunks[0].vel.y += 2.0;
        self.play_on_player(index, SoundId::SlugcatSwallowItem);
        log::info!("swallowed a {}", self.world.object(id).kind.value());
    }

    /// `Regurgitate`: the stomach's object joins the player's room at its position and is
    /// realized there (`RealizeInRoom`'s draws), set at the head (5 px further out when
    /// the body points up, then thrown back down) moving at most 6 px a tick, the head
    /// pushed the other way; the graphics' head shakes (two draws, only with graphics),
    /// three drips, `Slugcat_Regurgitate_Item`, and coughed up with the body upright it
    /// lands in a free hand. An object the port only keeps as text stays down.
    fn regurgitate(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        let id = match self.player_ref().object_in_stomach() {
            Some(StomachObject::Object(id)) => *id,
            Some(StomachObject::Saved(s)) => {
                log::warn!("cannot regurgitate an object the port does not type: {s}");
                return;
            }
            None => return,
        };
        self.world.object_mut(id).pos = self.pos();
        self.world.add_object_entity(id);
        self.world.realize_object_in_room(id, &mut self.rng);
        let p = self.player_ref();
        let (head, hips) = (p.head().pos, p.hips().pos);
        let flip = p.flip_direction();
        let mut pos = head;
        let mut vector = hips.direction_to(head);
        let upright = (head.y - hips.y).abs() > (head.x - hips.x).abs() && head.y > hips.y;
        if upright {
            pos += hips.direction_to(head) * 5.0;
            vector *= -1.0;
            vector.x += 0.4 * flip as f32;
            vector = vector.normalized();
        }
        let dir = Vec2::random_unit(&mut self.rng);
        let spread = self.rng.value();
        if let Some(w) = self.world.object_mut(id).realized.as_deref_mut() {
            let mass = w.chunk().mass;
            let c = w.chunk_mut();
            c.hard_set_position(pos);
            c.vel = ((vector * 2.0 + dir * spread) / mass).clamp_length(6.0);
        }
        let head = &mut self.player_mut().body.chunks[0];
        head.pos -= vector * 2.0;
        head.vel -= vector * 2.0;
        if observer.has_player_graphics() {
            let dir = Vec2::random_unit(&mut self.rng);
            let shake = self.rng.value() * 3.0;
            observer.nudge_player_head(dir * shake);
        }
        for _ in 0..3 {
            let at = Vec2::random_unit(&mut self.rng);
            let at = pos + at * (self.rng.value() * 1.5);
            let out = Vec2::random_unit(&mut self.rng);
            let out = out * (3.0 * self.rng.value());
            let vel = out + vector * 2.0f32.lerp(6.0, self.rng.value());
            let drip = WaterDrip::new(at, vel, false, &mut self.rng);
            self.world.add_drip(index, drip);
        }
        self.play_on_player(index, SoundId::SlugcatRegurgitateItem);
        if upright && let Some(hand) = self.free_hand() {
            self.slugcat_grab(Entity::Object(id), hand, observer);
        }
        self.player_mut().set_object_in_stomach(None);
        log::info!("regurgitated a {}", self.world.object(id).kind.value());
    }

    /// `GetHeldItemDirection(hand)` for a held weapon: across the line from the main
    /// chunk to it (along it while hanging from a beam), along the body toward it while
    /// crawling, upward along a climbed pole, and a spear swung forward with `spear_dir`.
    pub(super) fn held_item_direction(&self, hand: usize, spear_dir: f32) -> Vec2 {
        let Some(obj) = self.player_grasps()[hand] else {
            return Vec2::ZERO;
        };
        let at = self.world.chunk_pos(obj, 0).unwrap_or_default();
        let p = self.player_ref();
        let (head, hips) = (p.head().pos, p.hips().pos);
        let side = if hand == 0 { -1.0 } else { 1.0 };
        let mut v = head.direction_to(at) * side;
        if p.animation() != Animation::HangFromBeam {
            v = v.perpendicular();
        }
        let spear = obj
            .object()
            .and_then(|id| self.world.object(id).realized.as_deref())
            .is_some_and(|w| w.spear().is_some());
        if p.body_mode() == BodyMode::Crawl {
            v = hips.direction_to(at.lerp(head, 0.8));
        } else if p.animation() == Animation::ClimbOnBeam {
            v.y = v.y.abs();
            v = v.slerp(hips.direction_to(head), 0.75);
        } else if spear {
            let foot = if p.left_foot() { 9 } else { 3 };
            let phase = (p.animation_frame() + foot) as f32 / 12.0 * 2.0 * std::f32::consts::PI;
            let swing = Vec2::from_degrees((80.0 + phase.cos() * 4.0 * spear_dir) * spear_dir);
            v = v.slerp(swing, spear_dir.abs());
        }
        v
    }

    /// The player's `objectInStomach` read from the save state (`swallowedItems[0]`): an
    /// object the port types is put into the arena, in no room, at the player's
    /// position; anything else stays text.
    pub(super) fn stomach_from_save(&mut self) -> Option<StomachObject> {
        let text = self
            .story
            .swallowed_item
            .clone()
            .filter(|s| !s.is_empty() && s != "0")?;
        if !text.contains("<oA>") {
            return text.contains("<cA>").then_some(StomachObject::Saved(text));
        }
        let pos = self.pos();
        match SavedObject::parse(&text, self.world.mods) {
            Some(saved) => {
                let mut object = saved.into_object(pos.room);
                object.pos = pos;
                Some(StomachObject::Object(
                    self.world.add_detached_object(object),
                ))
            }
            None => Some(StomachObject::Saved(text)),
        }
    }

    /// `BringUpToDate`'s `playerGrasps`: the save string of each held object, hand 0
    /// first (`AbstractCreatureToStringStoryWorld` for a creature).
    #[must_use]
    pub fn player_grasp_save_strings(&self) -> Vec<String> {
        self.player_grasps()
            .iter()
            .flatten()
            .map(|&e| self.entity_save_string(e))
            .collect()
    }

    /// `BringUpToDate`'s `swallowedItems[0]`: the stomach's object as saved.
    #[must_use]
    pub fn swallowed_item_save_string(&self) -> Option<String> {
        match self.player.as_ref()?.object_in_stomach()? {
            StomachObject::Object(id) => Some(self.entity_save_string(Entity::Object(*id))),
            StomachObject::Saved(s) => Some(s.clone()),
        }
    }

    fn entity_save_string(&self, e: Entity) -> String {
        match e {
            Entity::Creature(id) => {
                let c = self.world.creature(id);
                self.world.creature_save_string(c, c.pos)
            }
            Entity::Object(id) => {
                let o = self.world.object(id);
                o.save_string(&self.world.coordinate_save_string(o.pos))
            }
        }
    }
}

#[cfg(test)]
#[path = "game_items_tests.rs"]
mod tests;
