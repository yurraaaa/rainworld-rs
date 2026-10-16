//! A lizard taking and dealing damage: `Lizard.Violence` (the head shield, hits in the
//! mouth, a rock's turn), `Stun`, `SpearStick`, `Creature.Die`; its bites (`AttemptBite`,
//! `Bite`, `DamageAttack`, `JawsSnapShut`, `CarryObject`), which nothing calls until the
//! AI decides (lizard step 7); the base `Creature.Violence` of what it bites; a thrown
//! weapon's trace against collision layer 1 (`Weapon.Update` 81340) with
//! `Rock.HitSomething` and `Spear.HitSomething`/`LodgeInCreature`, a spear stuck in a
//! creature (`Spear.Update`'s `StuckInCreature` branch), the room's chunk glue and the
//! layer-1 collisions. Decompile: `Lizard.cs` 1064-1712, `Creature.cs` 831-925,
//! `Weapon.cs` 334-390, `Rock.cs` 50-95, `Spear.cs` 576-822 and 977-1160, `Room.cs`
//! 4688-4760.

use rw_assets::{CreatureType, Terrain};

use super::RainWorldGame;
use crate::cosmetic::{CosmeticKind, ExplosionSpikes, Spark, StationaryEffect};
use crate::creature::{
    CreatureId, DamageType, GrabRequest, RealizedCreature, RelationshipType, Shareability,
};
use crate::item::{Entity, ObjectId, SpearStickKind, StuckIn, WeaponMode};
use crate::room_ext::RoomExt;
use crate::world::UpdatePart;
use crate::{BodyChunk, FloatExt, SoundId, Vec2, WaterDrip};

/// A body chunk of a realized object: the player's, or one the world holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRef {
    pub owner: Entity,
    pub index: usize,
}

/// `Violence`'s `source` chunk: whose it is and where it was.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViolenceSource {
    pub owner: Entity,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
}

/// One hit: `Violence`'s arguments after the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub source: Option<ViolenceSource>,
    pub direction_and_momentum: Option<Vec2>,
    pub chunk: Option<usize>,
    pub kind: DamageType,
    pub damage: f32,
    pub stun_bonus: f32,
}

impl RainWorldGame {
    /// A copy of a chunk.
    pub(super) fn chunk_of(&self, r: ChunkRef) -> Option<BodyChunk> {
        if r.owner == Entity::Creature(self.player_id) {
            return self.player.as_ref()?.body.chunks.get(r.index).cloned();
        }
        self.world.body_chunks(r.owner)?.get(r.index).cloned()
    }

    fn chunk_of_mut(&mut self, r: ChunkRef) -> Option<&mut BodyChunk> {
        if r.owner == Entity::Creature(self.player_id) {
            return self.player.as_mut()?.body.chunks.get_mut(r.index);
        }
        self.world.chunk_mut(r.owner, r.index)
    }

    /// The chunks of a realized creature (the player's two, or the world's).
    fn creature_chunk_refs(&self, id: CreatureId) -> Vec<ChunkRef> {
        let owner = Entity::Creature(id);
        let n = if id == self.player_id {
            self.player.as_ref().map_or(0, |p| p.body.chunks.len())
        } else {
            self.world.body_chunks(owner).map_or(0, <[BodyChunk]>::len)
        };
        (0..n).map(|index| ChunkRef { owner, index }).collect()
    }

    /// `evenUpdate` of a realized object (the player's is the game's).
    fn entity_even_update(&self, e: Entity) -> bool {
        match e {
            Entity::Creature(id) if id == self.player_id => self.even_update(),
            Entity::Creature(id) => match &self.world.creature(id).realized {
                Some(RealizedCreature::Fly(f)) => f.even_update,
                Some(RealizedCreature::Lizard(l)) => l.even_update,
                _ => self.even_update(),
            },
            Entity::Object(id) => self
                .world
                .object(id)
                .realized
                .as_deref()
                .is_some_and(|w| w.even_update),
        }
    }

    /// `Lizard.Stun`: below half health longer (up to 2.5 times); over 5 the graphics'
    /// flicker on a lizard not yet stunned; the longer stun kept, and a stunned lizard
    /// lets go of all.
    pub(super) fn lizard_stun(&mut self, id: CreatureId, st: i32) {
        let health = self.world.creature(id).state.health;
        let mut st = st;
        if health < 0.5 {
            st = (st as f32 * (1.0 + health.inverse_lerp(0.75, 0.0) * 1.5)) as i32;
        }
        let l = self.world.lizard_at(id);
        let stunned = l.stunned();
        if st > 5
            && !stunned
            && let Some(g) = &mut l.graphics
        {
            g.stun(st);
        }
        l.stun = l.stun.max(st);
        if l.stunned() {
            self.lizard_lose_all_grasps(id);
        }
    }

    /// `Creature.Die` of a lizard (it has no override).
    pub(super) fn lizard_die(&mut self, id: CreatureId) {
        self.world.creature_die(id, &mut self.rng);
    }

    /// `Lizard.LoseAllGrasps` (the graphics' sprites are lizard step 5).
    pub(super) fn lizard_lose_all_grasps(&mut self, id: CreatureId) {
        self.world.lose_all_grasps(id);
    }

    /// The room a realized lizard is in, for its sounds.
    pub(super) fn lizard_room(&self, id: CreatureId) -> i32 {
        self.world
            .creature(id)
            .lizard()
            .and_then(|l| l.room)
            .unwrap_or(-1)
    }

    /// `Violence` of any realized creature: a lizard's own, else the base's.
    pub fn creature_violence(&mut self, target: CreatureId, hit: Hit) {
        if self.world.creature(target).lizard().is_some() {
            self.lizard_violence(target, hit);
        } else {
            self.base_violence(target, hit);
        }
    }

    /// `Lizard.Violence`: damage and stun over the template's resistances, a pain
    /// voice (its intensity given: no draw), the head shield turning a hit to the head
    /// (`Lizard_Head_Shield_Deflect`; a tenth of the damage, a third of the push) or a
    /// hit in the mouth (half again the damage, double stun up to 120, the throat
    /// hurt), the push, a rock's turn, the stun lengthened when hurt (one draw), then
    /// the health. It sets no kill tag and never kills outright.
    pub fn lizard_violence(&mut self, id: CreatureId, hit: Hit) {
        let t = self.world.creature(id).template.violence();
        let (mut num, mut num2) = t.violence(hit.kind, hit.damage, hit.stun_bonus, 1.0);
        let mut dir = hit.direction_and_momentum;
        if hit.chunk == Some(0)
            && let Some(d) = dir
        {
            if self.lizard_hit_head_shield(id, d) {
                if let Some(s) = hit.source {
                    self.lizard_head_shield_effects(id, s, (num, num2), d);
                }
                num *= 0.1;
                num2 = (hit.damage * 0.5 * 30.0 + hit.stun_bonus * (2.0 / 3.0))
                    / t.base_stun_resistance;
                if self.world.creature(id).kind() == CreatureType::CyanLizard {
                    num2 *= 1.2;
                }
                dir = Some(d / 3.0);
            } else {
                let holding = self.lizard_holding(id);
                if self.world.lizard_at(id).hit_in_mouth(d, holding) {
                    num *= 1.5;
                    num2 = (num2 * 2.0).min(120.0);
                    if let Some(throat) = self.world.creature_mut(id).state.throat_health_mut() {
                        *throat -= num;
                    }
                }
            }
        }
        if let (Some(d), Some(i)) = (dir, hit.chunk) {
            let c = &mut self.world.lizard_at(id).body.chunks[i];
            c.vel += d / c.mass;
        }
        if let Some(s) = hit.source
            && let Entity::Object(o) = s.owner
            && self.world.object(o).kind == rw_assets::AbstractObjectType::Rock
            && self.world.creature(id).kind() != CreatureType::RedLizard
        {
            let l = self.world.lizard_at(id);
            l.turned_by_rock_direction = if s.pos.x - s.last_pos.x >= 0.0 { 1 } else { -1 };
            l.turned_by_rock_counter = 20;
        }
        let health = self.world.creature(id).state.health;
        num2 *= 1.0 + health.inverse_lerp(0.75, 0.0) * self.rng.value();
        self.lizard_stun(id, num2 as i32);
        self.world.creature_mut(id).state.health -= num;
    }

    /// `HitHeadShield`: a hit from the front (not in the mouth) plays
    /// `Lizard_Head_Shield_Deflect`.
    fn lizard_hit_head_shield(&mut self, id: CreatureId, dir: Vec2) -> bool {
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_at(id);
        if !l.hits_head_shield(dir, holding) {
            return false;
        }
        let head = l.head().pos;
        let room = self.lizard_room(id);
        self.play_on_object(room, SoundId::LizardHeadShieldDeflect, head);
        true
    }

    /// `Lizard.SpearStick`: no chunk, or the head shield, and the spear bounces; with
    /// graphics a spear (`spear`) may lodge by the head or a limb (one roll, 1 in 7, 1
    /// in 2 in the mouth, then the part's), which loses its health; in the mouth it
    /// stuns 100. The cyan lizard's gas leak is a later step.
    pub fn lizard_spear_stick(
        &mut self,
        id: CreatureId,
        chunk: Option<usize>,
        dir: Vec2,
        spear: Option<ObjectId>,
    ) -> bool {
        let Some(chunk) = chunk else {
            return false;
        };
        let rot = self.world.lizard_ref(id).chunk_rotation(chunk);
        let side = dir.signed_angle(-rot) * if chunk == 2 { -1.0 } else { 1.0 };
        if chunk == 0 && self.lizard_hit_head_shield(id, dir) {
            return false;
        }
        let holding = self.lizard_holding(id);
        let mouth = chunk == 0 && self.world.lizard_at(id).hit_in_mouth(dir, holding);
        let graphics = self.world.lizard_ref(id).graphics.is_some();
        if let Some(spear) = spear
            && graphics
            && self.rng.value() < 1.0 / if mouth { 2.0 } else { 7.0 }
        {
            self.lizard_spear_body_part(id, spear, chunk, side, mouth);
        }
        if mouth {
            self.lizard_stun(id, 100);
        }
        true
    }

    /// The base `Creature.Violence`: the source creature's kill tag, the push (at most
    /// 10 px a tick), damage and stun over the resistances (a health state's stun
    /// lengthened by one draw), the stun, the health with `quickDeath`'s rolls, and an
    /// outright kill past the template's limit.
    fn base_violence(&mut self, id: CreatureId, hit: Hit) {
        if let Some(Entity::Creature(src)) = hit.source.map(|s| s.owner) {
            self.world.set_kill_tag(id, src);
        }
        if let (Some(d), Some(i)) = (hit.direction_and_momentum, hit.chunk)
            && let Some(c) = self.chunk_of_mut(ChunkRef {
                owner: Entity::Creature(id),
                index: i,
            })
        {
            c.vel += (d / c.mass).clamp_magnitude(10.0);
        }
        let t = self.world.creature(id).template.violence();
        let state = &self.world.creature(id).state;
        let (has_health, health) = (state.has_health(), state.health);
        let mut scale = 1.0;
        if has_health {
            scale = 1.5 + health.inverse_lerp(0.5, 0.0) * self.rng.value();
        }
        let (num, num2) = t.violence(hit.kind, hit.damage, hit.stun_bonus, scale);
        self.creature_stun(id, num2 as i32);
        if has_health {
            let health = {
                let s = &mut self.world.creature_mut(id).state;
                s.health -= num;
                s.health
            };
            if t.quick_death
                && (self.rng.value() < -health
                    || health < -1.0
                    || (health < 0.0 && self.rng.value() < 0.33))
            {
                self.creature_dies_by_violence(id);
            }
        }
        if num >= t.instant_death_damage_limit {
            self.creature_dies_by_violence(id);
        }
    }

    /// `Stun` of a realized creature.
    fn creature_stun(&mut self, id: CreatureId, st: i32) {
        if id == self.player_id {
            let room = self.pos().room;
            let sounds = if room == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            if let Some(p) = self.player.as_mut() {
                p.stun(st, &mut self.rng, sounds);
            }
        } else if self.world.creature(id).lizard().is_some() {
            self.lizard_stun(id, st);
        } else {
            self.world.stun_creature(id, st);
        }
    }

    /// `Die` of a realized creature hurt by `Violence`.
    fn creature_dies_by_violence(&mut self, id: CreatureId) {
        if id == self.player_id {
            self.kill_player();
            return;
        }
        match self.world.creature(id).realized {
            Some(RealizedCreature::Fly(_)) => self.world.fly_die(id, &mut self.rng),
            Some(RealizedCreature::Overseer(_)) => self.overseer_die(id),
            Some(RealizedCreature::Lizard(_)) => self.lizard_die(id),
            _ => {}
        }
    }

    /// `AttemptBite`: a jaw not yet ready opens further; a ready one bites `target`'s
    /// first chunk in reach ahead of the head with `biteChance` (one draw), the whole
    /// body jerking (three draws); a miss homes on the target. No target: a snap at
    /// nothing.
    pub fn lizard_attempt_bite(&mut self, id: CreatureId, target: Option<CreatureId>) {
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_at(id);
        if holding || !l.conscious() {
            return;
        }
        if !l.jaw_ready_for_bite(holding) {
            let jaw = l.read_jaw_open(holding) + 0.05;
            l.set_jaw_open(jaw, holding);
            return;
        }
        let (chance, in_front, bonus, homing) = (
            l.params.bite_chance,
            l.params.bite_in_front,
            l.params.bite_rad_bonus,
            l.params.bite_homing_speed,
        );
        let (head, middle) = (l.body.chunks[0].pos, l.body.chunks[1].pos);
        let mut bit = false;
        if self.rng.value() < chance && target.is_some() {
            let reach = head + middle.direction_to(head) * in_front;
            for r in self.creature_chunk_refs(target.unwrap_or(id)) {
                let Some(c) = self.chunk_of(r) else { continue };
                if reach.dist_less(c.pos, c.rad + bonus) {
                    bit = true;
                    self.lizard_bite(id, Some(r));
                    break;
                }
            }
        } else if target.is_none() {
            self.lizard_bite(id, None);
        }
        // `LegsGripping` is 2 without the graphics' legs.
        if bit {
            for i in 0..3 {
                let v = Vec2::from_degrees(self.rng.value() * 360.0) * 7.0;
                if let Some(l) = self.world.creature_mut(id).lizard_mut() {
                    l.body.chunks[i].vel += v;
                }
            }
        } else if let Some(t) = target
            && let Some(tc) = self.chunk_of(ChunkRef {
                owner: Entity::Creature(t),
                index: 0,
            })
            && let Some(l) = self.world.creature_mut(id).lizard_mut()
        {
            let dir = l.body.chunks[0].pos.direction_to(tc.pos);
            l.body.chunks[0].vel += dir * (3.0 * homing);
            l.body.chunks[1].vel -= dir * homing;
            l.body.chunks[2].vel -= dir * homing;
        }
    }

    /// `Bite`: nothing while it holds something and is not held, or on a creature new
    /// to the room; the jaw shuts; a snap at nothing misses. The chunk takes the head's
    /// push; prey it eats (or, half the time, something lighter than 1.2 times it) is
    /// grabbed (the dominance drawn), the creature hurt half the time
    /// (`biteDamageChance`; the player for 1.5, others for the breed's damage), with
    /// `Lizard_Jaws_Grab_Player` or `_NPC`; else half the time an attacked creature
    /// takes a `DamageAttack`, or the jaws shut on nothing and lizards holding it let
    /// go. The AI's `BitCreature` is lizard step 7; the player's `Grabbed` is step 8.
    pub fn lizard_bite(&mut self, id: CreatureId, chunk: Option<ChunkRef>) {
        let held = !self.world.creature(id).grabbed_by.is_empty();
        if self.lizard_holding(id) && !held {
            return;
        }
        let room = self.lizard_room(id);
        let l = self.world.lizard_at(id);
        l.bite_control_reset = false;
        l.set_jaw_open(0.0, false);
        l.last_jaw_open = 0.0;
        let (head, head_vel, head_mass) = (l.head().pos, l.head().vel, l.head().mass);
        let my_mass = l.total_mass();
        let params = l.params;
        let Some(chunk) = chunk else {
            self.play_on_object(room, SoundId::LizardJawsShutMissCreature, head);
            return;
        };
        if let Entity::Creature(c) = chunk.owner
            && self
                .world
                .creature(c)
                .lizard()
                .is_some_and(|o| o.new_to_room_invinsibility > 0)
        {
            return;
        }
        if let Some(c) = self.chunk_of_mut(chunk) {
            let mass = c.mass;
            c.vel += head_vel * head_mass.lerp(1.1, 0.5) / mass.max(1.0);
        }
        let template = self.world.creature(id).template;
        let other_mass = match chunk.owner {
            Entity::Creature(c) => self.creature_total_mass(c),
            Entity::Object(_) => self.world.total_mass(chunk.owner),
        };
        let relation = chunk
            .owner
            .creature()
            .map(|c| template.relationship_to(self.world.creature(c).kind()).kind);
        let grabs = relation == Some(RelationshipType::Eats)
            || (self.rng.value() < 0.5 && other_mass < my_mass * 1.2)
            || (relation.is_none() && other_mass < my_mass * 1.2);
        let mut grabbed = false;
        if grabs {
            let dominance = params.bite_dominance * self.rng.value();
            let request = GrabRequest {
                grabber: id,
                grabbed: chunk.owner,
                grasp_used: 0,
                chunk_grabbed: chunk.index,
                shareability: Shareability::CanOnlyShareWithNonExclusive,
                dominance,
                override_equally_dominant: true,
                pacifying: true,
            };
            grabbed = self.world.grab(request, &mut self.rng);
            self.flush_dislodged();
        }
        let player = chunk.owner == Entity::Creature(self.player_id);
        if grabbed {
            if let Entity::Creature(c) = chunk.owner {
                if self.world.creature(id).kind() == CreatureType::RedLizard {
                    if player {
                        for i in 0..2 {
                            self.player_release_grasp(i);
                        }
                    } else {
                        self.world.lose_all_grasps(c);
                    }
                }
                if self.rng.value() < params.bite_damage_chance {
                    let at = self.chunk_of(chunk).map_or(head, |c| c.pos);
                    let damage = if player {
                        1.5
                    } else {
                        params.bite_damage * 0.8f32.lerp(1.2, self.rng.value())
                    };
                    self.creature_violence(
                        c,
                        Hit {
                            source: Some(self.lizard_source(id)),
                            direction_and_momentum: Some(head.direction_to(at) * 0.1),
                            chunk: Some(chunk.index),
                            kind: DamageType::Bite,
                            damage,
                            stun_bonus: 0.0,
                        },
                    );
                }
            }
            let sound = if player {
                SoundId::LizardJawsGrabPlayer
            } else {
                SoundId::LizardJawsGrabNpc
            };
            self.play_on_object(room, sound, head);
            return;
        }
        if self.rng.value() < 0.5 && relation == Some(RelationshipType::Attacks) {
            self.lizard_damage_attack(id, chunk, 0.5);
            return;
        }
        self.play_on_object(room, SoundId::LizardJawsShutMissCreature, head);
        let holders: Vec<_> = self.world.grabbed_by(chunk.owner).to_vec();
        for g in holders.iter().rev() {
            if self.world.creature(g.grabber).lizard().is_some() {
                self.world.release(*g);
            }
        }
    }

    /// The lizard's head as `Violence`'s source.
    fn lizard_source(&self, id: CreatureId) -> ViolenceSource {
        let h = self.world.lizard_ref(id).head();
        ViolenceSource {
            owner: Entity::Creature(id),
            pos: h.pos,
            last_pos: h.last_pos,
            vel: h.vel,
        }
    }

    /// `DamageAttackClosestChunk`: the holder's chunk nearest the head takes a full
    /// `DamageAttack`.
    pub(super) fn lizard_damage_attack_closest_chunk(
        &mut self,
        id: CreatureId,
        target: CreatureId,
    ) {
        let head = self.world.lizard_ref(id).head().pos;
        let mut best = None;
        let mut dst = f32::MAX;
        for r in self.creature_chunk_refs(target) {
            let Some(c) = self.chunk_of(r) else { continue };
            if head.dist_less(c.pos, dst) {
                best = Some(r);
                dst = head.distance(c.pos);
            }
        }
        if let Some(r) = best {
            self.lizard_damage_attack(id, r, 1.0);
        }
    }

    /// `DamageAttack`: a creature's chunk is bitten hard (an aggressive rival for no
    /// damage): `Violence` for `biteDamage * value * fac` and 30 stun, the grab sound,
    /// the jaw shut, a recoil, the bite delay reset, a stun of `16 * fac`, a spray of
    /// drips in a viewed room, and every grasp let go.
    fn lizard_damage_attack(&mut self, id: CreatureId, chunk: ChunkRef, fac: f32) {
        let Entity::Creature(target) = chunk.owner else {
            return;
        };
        let template = self.world.creature(id).template;
        let mut fac = fac;
        if template
            .relationship_to(self.world.creature(target).kind())
            .kind
            == RelationshipType::AgressiveRival
        {
            fac = 0.0;
        }
        let Some(at) = self.chunk_of(chunk).map(|c| c.pos) else {
            return;
        };
        let l = self.world.lizard_ref(id);
        let (head, bite_damage, bite_delay) =
            (l.head().pos, l.params.bite_damage, l.params.bite_delay);
        let damage = bite_damage * self.rng.value() * fac;
        let dir = head.direction_to(at);
        self.creature_violence(
            target,
            Hit {
                source: Some(self.lizard_source(id)),
                direction_and_momentum: Some(dir * 11.0),
                chunk: Some(chunk.index),
                kind: DamageType::Bite,
                damage,
                stun_bonus: 30.0,
            },
        );
        let room = self.lizard_room(id);
        let sound = if target == self.player_id {
            SoundId::LizardJawsGrabPlayer
        } else {
            SoundId::LizardJawsGrabNpc
        };
        self.play_on_object(room, sound, head);
        let Some(l) = self.world.creature_mut(id).lizard_mut() else {
            return;
        };
        l.set_jaw_open(0.0, false);
        l.last_jaw_open = 0.0;
        l.body.chunks[0].vel += dir * 8.0;
        l.body.chunks[1].vel -= dir * 6.0;
        l.body.chunks[2].vel -= dir * 6.0;
        l.bite_delay = bite_delay;
        l.stun = l.stun.max((16.0 * fac) as i32);
        let head = l.head().pos;
        if room == self.camera_room {
            for _ in 0..(3.0 + 6.0 * fac) as i32 {
                let pos = head.lerp(at, self.rng.value());
                let dir = Vec2::random_unit(&mut self.rng);
                let vel = dir * (self.rng.value() * (6.0 + bite_damage * 2.5 * fac));
                let drip = WaterDrip::new(pos, vel, false, &mut self.rng);
                self.world.add_drip(room, drip);
            }
        }
        self.lizard_lose_all_grasps(id);
    }

    /// `JawsSnapShut`: the miss sound, the jaw shut and forced shut, a lunge at `pos`
    /// and the bite delay reset.
    pub fn lizard_jaws_snap_shut(&mut self, id: CreatureId, pos: Vec2) {
        let room = self.lizard_room(id);
        let head = self.world.lizard_ref(id).head().pos;
        self.play_on_object(room, SoundId::LizardJawsShutMissCreature, head);
        let l = self.world.lizard_at(id);
        l.set_jaw_open(0.0, false);
        l.last_jaw_open = 0.0;
        let dir = head.direction_to(pos);
        l.body.chunks[0].vel += dir * 8.0;
        l.body.chunks[1].vel -= dir * 6.0;
        l.body.chunks[2].vel -= dir * 6.0;
        l.bite_delay = l.params.bite_delay;
        l.jaw_forced_shut = 1.0;
    }

    /// `CarryObject`: now and then (one draw) it drops what it holds unless it eats it;
    /// else the held chunk moves with the head, 25 px ahead, and comes loose if it
    /// pulls too hard or falls too far behind; the other chunks are pushed out of the
    /// lizard's head and middle.
    pub(super) fn lizard_carry_object(&mut self, _index: i32, id: CreatureId) {
        let Some(g) = self.world.creature(id).grasps.first().copied().flatten() else {
            return;
        };
        let template = self.world.creature(id).template;
        let eats = g
            .grabbed
            .creature()
            .is_some_and(|c| template.eats(self.world.creature(c).kind()));
        if self.rng.value() < 0.025 && !eats {
            self.lizard_lose_all_grasps(id);
            return;
        }
        let l = self.world.lizard_ref(id);
        let (head, head_vel, head_mass) = (l.head().pos, l.head().vel, l.head().mass);
        let to = head + l.body.chunks[1].pos.direction_to(head) * (25.0 * l.params.head_size);
        let entering = l.entering_shortcut.is_some();
        let r = ChunkRef {
            owner: g.grabbed,
            index: g.chunk_grabbed,
        };
        let Some(c) = self.chunk_of(r) else { return };
        let rel = c.vel - head_vel;
        let _mass = if c.mass <= head_mass / 2.0 {
            c.mass / 2.0
        } else {
            c.mass
        };
        let even = self.entity_even_update(g.grabbed);
        let eu = self.even_update();
        let loose =
            !entering && (rel.length() * c.mass > 30.0 || !to.dist_less(c.pos, 70.0 + c.rad));
        if let Some(c) = self.chunk_of_mut(r) {
            c.vel = head_vel;
            if !loose {
                c.move_from_outside_my_update(even, eu, to);
            }
        }
        let l = self.world.lizard_ref(id);
        if let Entity::Object(o) = g.grabbed
            && let Some(link) = &l.graphics
        {
            let set = (l.body.chunks[0].pos - link.head.pos).perpendicular();
            if let Some(w) = self.world.weapon_mut(o) {
                w.set_rotation = Some(set);
            }
        }
        if loose {
            self.lizard_lose_all_grasps(id);
        }
        if self
            .world
            .creature(id)
            .grasps
            .first()
            .is_some_and(Option::is_some)
        {
            let l = self.world.lizard_ref(id);
            let pushers = [
                (l.body.chunks[0].pos, l.body.chunks[0].rad),
                (l.body.chunks[1].pos, l.body.chunks[1].rad),
            ];
            for (pos, rad) in pushers {
                for other in self.chunk_refs_of(g.grabbed) {
                    if other.index == g.chunk_grabbed {
                        continue;
                    }
                    if let Some(c) = self.chunk_of_mut(other) {
                        c.push_out_of(pos, rad);
                    }
                }
            }
        }
    }

    /// The chunks of any realized object.
    fn chunk_refs_of(&self, e: Entity) -> Vec<ChunkRef> {
        match e {
            Entity::Creature(c) => self.creature_chunk_refs(c),
            Entity::Object(_) => {
                let n = self.world.body_chunks(e).map_or(0, <[BodyChunk]>::len);
                (0..n).map(|index| ChunkRef { owner: e, index }).collect()
            }
        }
    }

    /// The trace of thrown weapon `id` against collision layer 1 (the player, which
    /// threw it and is exempt, and the lizards in the order they joined the room): the
    /// first chunk the move ahead crosses. A creature behind a wall is a `HitWall` that
    /// ends the update (true). Else a hit creature gets the thrower's kill tag and
    /// `HitSomething`; the weapon is out of reach for 40 ticks and its thrower forgotten.
    pub(super) fn trace_layer_one(&mut self, index: i32, id: ObjectId) -> bool {
        let Some(w) = self.world.realized_weapon(id, index) else {
            return false;
        };
        let c = w.chunk().clone();
        let from = w.first_frame_trace_from_pos.unwrap_or(c.pos);
        let to = c.pos + c.vel;
        let thrower = w.thrown_by;
        let rad = c.rad
            + if thrower == Some(self.player_id) {
                5.0
            } else {
                0.0
            };
        let first_frame = w.first_frame_trace_from_pos.is_some();
        let throw_dir = w.throw_dir_vec().normalized();
        let mut lizards = self.world.room_lizards(index);
        lizards.sort_unstable();
        let mut best = f32::MAX;
        let mut hit = None;
        for (_, lid) in lizards {
            if Some(lid) == thrower
                || thrower.is_some_and(|t| {
                    self.world
                        .creature(lid)
                        .grabbed_by
                        .iter()
                        .any(|g| g.grabber == t)
                })
            {
                continue;
            }
            for (i, ch) in self.world.lizard_ref(lid).chunks().iter().enumerate() {
                if first_frame && (ch.pos - c.last_pos).normalized().dot(throw_dir) <= 0.3 {
                    continue;
                }
                let t = from.circles_collision_time(ch.pos, to - from, rad, ch.rad);
                if t > 0.0 && t < 1.0 && t < best {
                    best = t;
                    hit = Some((lid, i, from.lerp(to, t)));
                }
            }
        }
        if let Some(w) = self.world.weapon_mut(id) {
            w.first_frame_trace_from_pos = None;
        }
        let Some((lid, chunk, point)) = hit else {
            self.with_item_tick(index, |world, cx| {
                world.weapon_part(
                    index,
                    id,
                    cx,
                    &Default::default(),
                    UpdatePart::TracedNothing,
                )
            });
            return false;
        };
        let Some(room) = self
            .world
            .realized(index)
            .map(|r| std::sync::Arc::clone(&r.room))
        else {
            return false;
        };
        let ahead = c.pos.distance(c.pos + c.vel.normalized() * 19.0);
        if !room.ray_trace_tiles_for_terrain(c.pos.tile(), to.tile())
            && c.pos.distance(point) > ahead
        {
            self.with_item_tick(index, |world, cx| {
                world.weapon_part(index, id, cx, &Default::default(), UpdatePart::HitWall)
            });
            return true;
        }
        if let Some(t) = thrower {
            if let Some(w) = self.world.weapon_mut(id) {
                w.thrown_closest_to_creature = None;
                w.closest_crit_dist = f32::MAX;
            }
            self.world.set_kill_tag(lid, t);
        }
        let spear = self
            .world
            .weapon_mut(id)
            .is_some_and(|w| w.spear().is_some());
        let hit_something = if spear {
            self.spear_hit_creature(index, id, lid, chunk, point)
        } else {
            self.rock_hit_creature(index, id, lid, chunk, point)
        };
        if let Some(w) = self.world.weapon_mut(id) {
            w.forbidden_to_player = 40;
        }
        // `SocialEventRecognizer.WeaponAttack`: the creatures' reactions are the AI's
        // (lizard steps 6 and 7); a slugcat's hit breaks the peaceful record.
        if thrower.is_some_and(|t| self.world.creature(t).kind() == CreatureType::Slugcat)
            && hit_something
        {
            self.world.break_peaceful(lid);
        }
        if let Some(w) = self.world.weapon_mut(id) {
            w.thrown_by = None;
        }
        false
    }

    /// The weapon's chunk as a `Violence` source.
    fn weapon_source(&self, id: ObjectId) -> Option<ViolenceSource> {
        let w = self.world.object(id).realized.as_deref()?;
        Some(ViolenceSource {
            owner: Entity::Object(id),
            pos: w.chunk().pos,
            last_pos: w.chunk().last_pos,
            vel: w.chunk().vel,
        })
    }

    /// `Rock.HitSomething` on a creature: `Violence` (blunt, 0.01, 45 stun), `Free`,
    /// bounced back off it (two draws), `Rock_Hit_Creature`, a ring of spikes where it
    /// struck, a new spin.
    fn rock_hit_creature(
        &mut self,
        index: i32,
        id: ObjectId,
        target: CreatureId,
        chunk: usize,
        point: Vec2,
    ) -> bool {
        let Some(w) = self.world.weapon_mut(id) else {
            return false;
        };
        w.vibrate = 20;
        let (vel, mass) = (w.chunk().vel, w.chunk().mass);
        self.creature_violence(
            target,
            Hit {
                source: self.weapon_source(id),
                direction_and_momentum: Some(vel * mass),
                chunk: Some(chunk),
                kind: DamageType::Blunt,
                damage: 0.01,
                stun_bonus: 45.0,
            },
        );
        self.world
            .change_weapon_mode(id, WeaponMode::Free, &mut self.rng);
        self.flush_dislodged();
        self.weapon_bounce_off(id);
        let pos = self
            .world
            .chunk_pos(Entity::Object(id), 0)
            .unwrap_or_default();
        self.play_on_object(index, SoundId::RockHitCreature, pos);
        let target_chunk = self.chunk_of(ChunkRef {
            owner: Entity::Creature(target),
            index: chunk,
        });
        if let (Some(tc), Some(room)) = (
            target_chunk,
            self.world.realized(index).map(|r| r.room.clone()),
        ) {
            let at = tc.pos + tc.pos.direction_to(point) * tc.rad;
            let spikes = ExplosionSpikes::new(
                &room,
                at,
                5,
                2.0,
                4.0,
                4.5,
                30.0,
                [1.0, 1.0, 1.0, 0.5],
                &mut self.rng,
            );
            self.world.add_cosmetic(index, CosmeticKind::Spikes(spikes));
        }
        if let Some(w) = self.world.weapon_mut(id) {
            w.set_random_spin(&mut self.rng);
        }
        true
    }

    /// `vel = vel * -0.5 + DegToVec(value * 360) * (Lerp(0.1, 0.4, value) * |vel|)`.
    fn weapon_bounce_off(&mut self, id: ObjectId) {
        let a = self.rng.value();
        let b = self.rng.value();
        if let Some(w) = self.world.weapon_mut(id) {
            let v = w.chunk().vel;
            w.chunk_mut().vel =
                v * -0.5 + Vec2::from_degrees(a * 360.0) * (0.1f32.lerp(0.4, b) * v.length());
        }
    }

    /// `Spear.HitSomething` on a creature (More Slugcats off): `Violence` (stab, the
    /// damage bonus, 20 stun), then `SpearStick` with a drawn damage: it lodges
    /// (`Spear_Stick_In_Creature`) or bounces off the shell (`Free`, two draws, a new
    /// spin).
    fn spear_hit_creature(
        &mut self,
        index: i32,
        id: ObjectId,
        target: CreatureId,
        chunk: usize,
        point: Vec2,
    ) -> bool {
        let Some(w) = self.world.weapon_mut(id) else {
            return false;
        };
        let (vel, mass) = (w.chunk().vel, w.chunk().mass);
        let bonus = w.spear().map_or(1.0, |s| s.spear_damage_bonus);
        self.creature_violence(
            target,
            Hit {
                source: self.weapon_source(id),
                direction_and_momentum: Some(vel * (mass * 2.0)),
                chunk: Some(chunk),
                kind: DamageType::Stab,
                damage: bonus,
                stun_bonus: 20.0,
            },
        );
        let _dmg = 0.55f32.lerp(0.62, self.rng.value());
        let vel = self.world.weapon_mut(id).map_or(vel, |w| w.chunk().vel);
        let sticks = if self.world.creature(target).lizard().is_some() {
            self.lizard_spear_stick(target, Some(chunk), vel, Some(id))
        } else {
            true
        };
        let pos = self
            .world
            .chunk_pos(Entity::Object(id), 0)
            .unwrap_or_default();
        if sticks {
            self.play_on_object(index, SoundId::SpearStickInCreature, pos);
            self.lodge_in_creature(index, id, target, chunk, point);
            return true;
        }
        self.play_on_object(index, SoundId::SpearBounceOffCreautureShell, pos);
        if let Some(w) = self.world.weapon_mut(id) {
            w.vibrate = 20;
        }
        self.world
            .change_weapon_mode(id, WeaponMode::Free, &mut self.rng);
        self.flush_dislodged();
        self.weapon_bounce_off(id);
        if let Some(w) = self.world.weapon_mut(id) {
            w.set_random_spin(&mut self.rng);
        }
        false
    }

    /// `LodgeInCreature` at a chunk: `StuckInCreature`; pinned to the wall behind a
    /// chunk in open air with Solid ahead (`stuckInWall`, 300 ticks), else turned by
    /// the throw against the chunk's rotation; moved with the chunk; the
    /// `AbstractSpearStick`; eight drips in a viewed room.
    fn lodge_in_creature(
        &mut self,
        index: i32,
        id: ObjectId,
        target: CreatureId,
        chunk: usize,
        point: Vec2,
    ) {
        self.world
            .change_weapon_mode(id, WeaponMode::StuckInCreature, &mut self.rng);
        self.flush_dislodged();
        let r = ChunkRef {
            owner: Entity::Creature(target),
            index: chunk,
        };
        let Some(tc) = self.chunk_of(r) else { return };
        let Some(room) = self
            .world
            .realized(index)
            .map(|x| std::sync::Arc::clone(&x.room))
        else {
            return;
        };
        let Some(w) = self.world.weapon_mut(id) else {
            return;
        };
        let throw_dir = w.throw_dir;
        let dir = w.throw_dir_vec();
        let bonus = w.spear().map_or(1.0, |s| s.spear_damage_bonus);
        let (body_part, kept_rotation) = w
            .spear()
            .map_or((-1, 0.0), |s| (s.stuck_body_part, s.stuck_rotation));
        let rotation = w.rotation;
        let tile = tc.pos.tile();
        let ahead = (tile.0 + throw_dir.0, tile.1 + throw_dir.1);
        let mut stuck = StuckIn {
            creature: target,
            chunk,
            rotation: kept_rotation,
            body_part,
        };
        let mut pinned = None;
        if bonus > 0.9
            && room.tile(ahead.0, ahead.1).terrain == Terrain::Solid
            && room.tile_at_pos(tc.pos).terrain == Terrain::Air
        {
            pinned = Some(tc.pos.tile_middle() + dir * (10.0 - tc.rad));
            stuck.rotation = rotation.to_degrees();
            stuck.body_part = -1;
        } else if body_part == -1 {
            let rot = self.chunk_rotation(r);
            stuck.rotation = dir.signed_angle(rot);
        }
        if let Some(at) = pinned {
            let even = self.entity_even_update(r.owner);
            let eu = self.even_update();
            if let Some(c) = self.chunk_of_mut(r) {
                c.move_from_outside_my_update(even, eu, at);
            }
        }
        if let Some(s) = self.world.weapon_mut(id).and_then(|w| w.spear_mut()) {
            s.stuck_in = Some(stuck);
            s.stuck_body_part = stuck.body_part;
            s.stuck_rotation = stuck.rotation;
            if pinned.is_some() {
                s.stuck_in_wall = pinned;
                s.pin_to_wall_counter = 300;
            }
        }
        self.move_with_other_object(id, r, Vec2::ZERO);
        self.world.add_spear_stick(
            id,
            target,
            SpearStickKind::Lodged {
                chunk,
                body_part: stuck.body_part,
                angle: stuck.rotation,
            },
        );
        if index == self.camera_room {
            let vel = self
                .world
                .weapon_mut(id)
                .map_or(Vec2::ZERO, |w| w.chunk().vel);
            for _ in 0..8 {
                let a = self.rng.value();
                let deg = self.rng.value();
                let b = self.rng.value();
                let v =
                    -vel * (a * 0.5) + Vec2::from_degrees(360.0 * deg) * (vel.length() * b * 0.5);
                let drip = WaterDrip::new(point, v, false, &mut self.rng);
                self.world.add_drip(index, drip);
            }
        }
    }

    /// `BodyChunk.Rotation`: toward it from its rotation chunk (a lizard's chunks turn
    /// by the constructor's connections; others have none: straight up).
    fn chunk_rotation(&self, r: ChunkRef) -> Vec2 {
        match r.owner {
            Entity::Creature(c) => self
                .world
                .creature(c)
                .lizard()
                .map_or(Vec2::new(0.0, 1.0), |l| l.chunk_rotation(r.index)),
            Entity::Object(_) => Vec2::new(0.0, 1.0),
        }
    }

    /// `BodyChunk.MoveWithOtherObject` of spear `id`'s chunk: at once when both owners
    /// are in step with the game, else by the room's chunk glue at the end of its
    /// update.
    fn move_with_other_object(&mut self, id: ObjectId, other: ChunkRef, rel: Vec2) {
        let eu = self.even_update();
        let Some(oc) = self.chunk_of(other) else {
            return;
        };
        if self.entity_even_update(other.owner) == eu {
            let mine = self.entity_even_update(Entity::Object(id));
            if let Some(c) = self.world.chunk_mut(Entity::Object(id), 0) {
                if mine == eu {
                    c.pos = oc.pos + rel;
                } else {
                    c.set_pos = Some(oc.pos + rel);
                }
            }
            return;
        }
        self.chunk_glue.push((id, other, rel));
    }

    /// `Room.chunkGlue`, applied after the update list.
    pub(super) fn apply_chunk_glue(&mut self) {
        for (id, other, rel) in std::mem::take(&mut self.chunk_glue) {
            let Some(oc) = self.chunk_of(other) else {
                continue;
            };
            if let Some(c) = self.world.chunk_mut(Entity::Object(id), 0) {
                c.pos = oc.pos + rel;
            }
        }
    }

    /// `Spear.Update`'s `StuckInCreature` branch: moved with its chunk and turned with
    /// it; pinned to a wall, it holds the chunk there until it is pulled hard enough
    /// (one draw a tick under 160), the pin counting down; a slated creature drops it.
    pub(super) fn spear_stuck_in_creature_update(&mut self, index: i32, id: ObjectId) {
        let Some(w) = self.world.realized_weapon(id, index) else {
            return;
        };
        if w.mode != WeaponMode::StuckInCreature {
            return;
        }
        let Some(s) = w.spear() else { return };
        let Some(stuck) = s.stuck_in else { return };
        let pinned = s.stuck_in_wall;
        let r = ChunkRef {
            owner: Entity::Creature(stuck.creature),
            index: stuck.chunk,
        };
        let Some(tc) = self.chunk_of(r) else { return };
        match pinned {
            None => {
                let part = self.stuck_body_part_pos(index, stuck);
                let rot = self.chunk_rotation(r);
                if let Some(w) = self.world.weapon_mut(id) {
                    w.chunk_mut().vel = tc.vel;
                    w.set_rotation = Some(Vec2::from_degrees(
                        stuck.rotation
                            + part.map_or(rot.to_degrees(), |p| (p - tc.pos).to_degrees()),
                    ));
                }
                let offset = part.map_or(Vec2::ZERO, |p| tc.pos.lerp(p, 0.5) - tc.pos);
                self.move_with_other_object(id, r, offset);
            }
            Some(wall) => self.pinned_spear_update(id, r, stuck, wall),
        }
        let slated = match &self.world.creature(stuck.creature).realized {
            Some(RealizedCreature::Lizard(l)) => l.slated,
            Some(RealizedCreature::Fly(f)) => f.slated,
            _ => false,
        };
        if slated {
            self.world
                .change_weapon_mode(id, WeaponMode::Free, &mut self.rng);
            self.flush_dislodged();
        }
    }

    /// `BodyPartByIndex(stuckBodyPart)`'s position, which a lodged spear follows while
    /// its room is viewed and the creature's graphics have the part.
    fn stuck_body_part_pos(&self, index: i32, stuck: StuckIn) -> Option<Vec2> {
        if stuck.body_part < 0 || index != self.camera_room {
            return None;
        }
        let link = self
            .world
            .creature(stuck.creature)
            .lizard()?
            .graphics
            .as_ref()?;
        link.parts
            .get(usize::try_from(stuck.body_part).ok()?)
            .copied()
    }

    /// `SpearStick`'s body part for a spear lodging in chunk `chunk` of lizard `id`
    /// with graphics (`side` is the hit's angle against the chunk): the head (chunk 0, a
    /// third of the time or in the mouth), else a limb on the hit's side (for the
    /// middle chunk only half the time), whose health goes to 0; then
    /// `Spear.ProvideRotationBodyPart`.
    fn lizard_spear_body_part(
        &mut self,
        id: CreatureId,
        spear: ObjectId,
        chunk: usize,
        side: f32,
        mouth: bool,
    ) {
        let head = self
            .world
            .lizard_ref(id)
            .graphics
            .as_ref()
            .map_or(0, |g| g.parts.len().saturating_sub(1));
        let part = if chunk == 0 && ((self.rng.value() < 1.0 / 3.0) | mouth) {
            Some(head)
        } else if chunk != 1 || self.rng.value() < 0.5 {
            let limb = match (chunk != 2, side < 0.0) {
                (true, true) => 1,
                (true, false) => 0,
                (false, true) => 2,
                (false, false) => 3,
            };
            if let Some((_, l)) = self.world.creature_mut(id).state.lizard_health_parts_mut() {
                l[limb] = 0.0;
            }
            Some(limb)
        } else {
            None
        };
        let Some(part) = part else { return };
        let Some(w) = self.world.weapon_mut(spear) else {
            return;
        };
        let vel = w.chunk().vel;
        let chunk_pos = self.world.lizard_ref(id).body.chunks[chunk].pos;
        let Some(pos) = self
            .world
            .lizard_at(id)
            .graphics
            .as_mut()
            .and_then(|g| g.push_part(part, vel))
        else {
            return;
        };
        if let Some(s) = self.world.weapon_mut(spear).and_then(|w| w.spear_mut()) {
            s.stuck_body_part = i32::try_from(part).unwrap_or(-1);
            s.stuck_rotation = vel.signed_angle((pos - chunk_pos).normalized());
        }
    }

    /// `Lizard.Violence`'s head-shield effects with graphics and a source: up to 25
    /// sparks off the source (four draws each and the spark's), the white flicker, the
    /// flash, and the head knocked by twice the hit.
    fn lizard_head_shield_effects(
        &mut self,
        id: CreatureId,
        source: ViolenceSource,
        (num, num2): (f32, f32),
        dir: Vec2,
    ) {
        if self.world.lizard_ref(id).graphics.is_none() {
            return;
        }
        let Some(room) = self.world.lizard_ref(id).room else {
            return;
        };
        let n = (num2 / 2.0).max(((num * 30.0) as i32) as f32).min(25.0) as i32;
        for _ in 0..n {
            let d = Vec2::from_degrees(self.rng.value() * 360.0);
            let pos = source.pos + d * (5.0 * self.rng.value());
            let d = Vec2::from_degrees(self.rng.value() * 360.0);
            let speed = 0.2f32.lerp(0.4, self.rng.value()) * source.vel.length();
            let vel = source.vel * -0.1 + d * speed;
            let mut spark = Spark::new(pos, vel, [1.0; 4], 10, 170, &mut self.rng);
            let flicker = self
                .world
                .lizard_ref(id)
                .graphics
                .as_ref()
                .map_or(0, |g| g.white_flicker);
            spark.lizard = Some((id, flicker));
            self.world.add_cosmetic(room, CosmeticKind::Spark(spark));
        }
        let Some(g) = self.world.lizard_at(id).graphics.as_mut() else {
            return;
        };
        g.white_flicker(n);
        let head = g.parts.len().saturating_sub(1);
        g.pushes.push((head, dir * 2.0));
        let flash = StationaryEffect::flashing_orb(source.pos, Some(id));
        self.world.add_cosmetic(room, CosmeticKind::Flash(flash));
    }

    /// The pinned part of the `StuckInCreature` branch.
    fn pinned_spear_update(&mut self, id: ObjectId, r: ChunkRef, stuck: StuckIn, wall: Vec2) {
        let Some(tc) = self.chunk_of(r) else { return };
        let total = match r.owner {
            Entity::Creature(c) => self.creature_total_mass(c),
            Entity::Object(_) => tc.mass,
        };
        let entering = r
            .owner
            .creature()
            .and_then(|c| self.world.creature(c).lizard())
            .is_some_and(|l| l.entering_shortcut.is_some());
        let Some(s) = self.world.weapon_mut(id).and_then(|w| w.spear_mut()) else {
            return;
        };
        if s.pin_to_wall_counter > 0 {
            s.pin_to_wall_counter -= 1;
        }
        let counter = s.pin_to_wall_counter as f32;
        let set = if tc.vel.length() * tc.mass > counter.lerp_map(160.0, 0.0, 7.0, 2.0) {
            (Vec2::from_degrees(stuck.rotation) + (tc.vel * (tc.mass * 0.005)).clamp_magnitude(0.1))
                .normalized()
        } else {
            Vec2::from_degrees(stuck.rotation)
        };
        let mut dislodged = entering;
        if counter < 160.0 && self.rng.value() < 0.025 {
            let limit = counter.lerp_map(160.0, 0.0, 140.0, 30.0 / (1.0 + total * 0.2));
            if tc.vel.length() > limit {
                log::info!("Chunk dislodged by velocity");
                dislodged = true;
            }
        }
        if let Some(w) = self.world.weapon_mut(id) {
            w.set_rotation = Some(set);
            let c = w.chunk_mut();
            c.vel = Vec2::ZERO;
            c.pos = wall;
        }
        if dislodged {
            let rot = self.chunk_rotation(r);
            if let Some(s) = self.world.weapon_mut(id).and_then(|w| w.spear_mut()) {
                if let Some(st) = &mut s.stuck_in {
                    st.rotation = set.signed_angle(rot);
                    s.stuck_rotation = st.rotation;
                }
                s.stuck_in_wall = None;
            }
        } else {
            let even = self.entity_even_update(r.owner);
            let eu = self.even_update();
            if let Some(c) = self.chunk_of_mut(r) {
                c.move_from_outside_my_update(even, eu, wall);
                c.vel = Vec2::ZERO;
            }
        }
    }

    /// `Spear.RecreateSticksFromAbstract` in its first physics update (a spear realized
    /// with a lodged stick in a realized lizard): lodged again, `StuckInCreature`.
    pub(super) fn recreate_spear_sticks(&mut self, index: i32, id: ObjectId) {
        let Some(w) = self.world.realized_weapon(id, index) else {
            return;
        };
        if w.spear().is_none_or(|s| s.stuck_in.is_some()) {
            return;
        }
        for s in self.world.spear_sticks_of(Entity::Object(id)) {
            let SpearStickKind::Lodged {
                chunk,
                body_part,
                angle,
            } = s.kind
            else {
                continue;
            };
            if self.world.creature(s.creature).lizard().is_none() {
                continue;
            }
            self.world
                .change_weapon_mode(id, WeaponMode::StuckInCreature, &mut self.rng);
            if let Some(p) = self.world.weapon_mut(id).and_then(|w| w.spear_mut()) {
                p.stuck_in = Some(StuckIn {
                    creature: s.creature,
                    chunk,
                    rotation: angle,
                    body_part,
                });
                p.stuck_body_part = body_part;
                p.stuck_rotation = angle;
            }
        }
    }

    /// The dislodge sounds `ChangeMode` asked for (`Spear_Dislodged_From_Creature`).
    pub(super) fn flush_dislodged(&mut self) {
        for (room, pos) in std::mem::take(&mut self.world.dislodged) {
            self.play_on_object(room, SoundId::SpearDislodgedFromCreature, pos);
        }
    }

    /// The layer-1 collisions of room `index` (the player and the lizards, in the order
    /// they joined it): pairs within both collision ranges (50) that do not hold each
    /// other push their overlapping chunks apart by mass (a nudge of two draws at the
    /// same x) and `Collide` once each way. `Player.Collide`'s belly slide stuns the
    /// other; `Lizard.Collide` bites what it would kill (by the static relationship:
    /// the tracker and the rest are the AI's, lizard steps 6 and 7).
    pub(super) fn collide_creatures(&mut self, index: i32) {
        let mut layer: Vec<(u64, CreatureId)> = self.world.room_lizards(index);
        if self
            .player
            .as_ref()
            .is_some_and(|p| !p.in_shortcut() && !p.is_destroyed())
            && self.pos().room == index
        {
            layer.push((self.player_seq, self.player_id));
        }
        layer.sort_unstable();
        for i in 0..layer.len() {
            for j in i + 1..layer.len() {
                self.collide_pair(layer[i].1, layer[j].1);
            }
        }
    }

    fn collide_pair(&mut self, a: CreatureId, b: CreatureId) {
        let (ra, rb) = (self.creature_chunk_refs(a), self.creature_chunk_refs(b));
        let (Some(a0), Some(b0)) = (
            ra.first().and_then(|&r| self.chunk_of(r)),
            rb.first().and_then(|&r| self.chunk_of(r)),
        ) else {
            return;
        };
        if (a0.pos.x - b0.pos.x).abs() >= 100.0 || (a0.pos.y - b0.pos.y).abs() >= 100.0 {
            return;
        }
        if self.creature_holds(a, b) || self.creature_holds(b, a) {
            return;
        }
        let mut collided = false;
        for &ca in &ra {
            for &cb in &rb {
                let (Some(x), Some(y)) = (self.chunk_of(ca), self.chunk_of(cb)) else {
                    continue;
                };
                if !(x.collide_with_objects && y.collide_with_objects)
                    || !x.pos.dist_less(y.pos, x.rad + y.rad)
                {
                    continue;
                }
                let sum = x.rad + y.rad;
                let dist = x.pos.distance(y.pos);
                let dir = x.pos.direction_to(y.pos);
                let share = y.mass / (x.mass + y.mass);
                let va = dir * ((sum - dist) * share);
                let vb = dir * ((sum - dist) * (1.0 - share));
                let mut nudge = [Vec2::ZERO; 2];
                if x.pos.x - va.x == y.pos.x + vb.x {
                    nudge[0] = Vec2::from_degrees(self.rng.value() * 360.0) * 0.0001;
                    nudge[1] = Vec2::from_degrees(self.rng.value() * 360.0) * 0.0001;
                }
                if let Some(c) = self.chunk_of_mut(ca) {
                    c.pos -= va;
                    c.vel -= va;
                    c.vel += nudge[0];
                }
                if let Some(c) = self.chunk_of_mut(cb) {
                    c.pos += vb;
                    c.vel += vb;
                    c.vel += nudge[1];
                }
                if !collided {
                    self.creature_collide(a, b, ca.index, cb.index);
                    self.creature_collide(b, a, cb.index, ca.index);
                }
                collided = true;
            }
        }
    }

    /// Whether creature `a` holds `b` in one of its grasps.
    fn creature_holds(&self, a: CreatureId, b: CreatureId) -> bool {
        self.world
            .creature(a)
            .grasps
            .iter()
            .flatten()
            .any(|g| g.grabbed == Entity::Creature(b))
    }

    /// `Collide(other, myChunk, otherChunk)` of `me`.
    fn creature_collide(
        &mut self,
        me: CreatureId,
        other: CreatureId,
        my_chunk: usize,
        other_chunk: usize,
    ) {
        if me == self.player_id {
            let stun = self.player.as_mut().and_then(|p| p.collide_with_creature());
            if let Some(st) = stun {
                self.creature_stun(other, st);
            }
            return;
        }
        if self.world.creature(me).lizard().is_none() {
            return;
        }
        let template = self.world.creature(me).template;
        let rel = template.relationship_to(self.world.creature(other).kind());
        let go_for_kill = matches!(rel.kind, RelationshipType::Eats | RelationshipType::Attacks)
            && rel.intensity > 0.0;
        let l = self.world.lizard_ref(me);
        if l.conscious() && !self.lizard_holding(me) && go_for_kill {
            if l.animation == crate::creature::LizardAnimation::Lounge && my_chunk == 0 {
                let r = ChunkRef {
                    owner: Entity::Creature(other),
                    index: other_chunk,
                };
                self.lizard_bite(me, Some(r));
            } else {
                self.lizard_attempt_bite(me, Some(other));
            }
        }
    }
}
