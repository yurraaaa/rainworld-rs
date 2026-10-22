//! A thrown rock or spear meeting the room's creatures: `Weapon.Update`'s `Thrown`
//! branch telling them of it (`INotifyOfFlyingWeapons`, 81490), its walk over collision
//! layer 0 (81606) with `HitSomethingWithoutStopping` (81784) and `Spear`'s
//! `TryImpaleSmallCreature`, the creatures' `Stun` and `Die` (`Overseer.Die`'s guide),
//! `Overseer.HitByWeapon`, and the creatures impaled on a spear held on its shaft (the
//! end of `Spear.Update`, 271475). Behaviour: `rainworld-re/notes/items.md` section 12.
//!
//! Mirrors: Weapon.cs (Update), Spear.cs (TryImpaleSmallCreature, Update), Overseer.cs (Die, HitByWeapon), Creature.cs (Stun, Die)

use rw_assets::CreatureType;

use super::{OverseerCall, PlayerObserver, RainWorldGame};
use crate::creature::{CreatureId, CreatureTemplate, EntityId, RealizedCreature, WorldCoordinate};
use crate::item::{Entity, ObjectId, WeaponMode};
use crate::world::UpdatePart;
use crate::{FloatExt, SoundId, Vec2};

impl RainWorldGame {
    /// Item `id`'s `Update` in room `index`'s update list: up to its `Thrown` branch;
    /// when thrown, the room's creatures are told of it, the branch up to the layer-0
    /// walk, and the walk; then the physics and the rest, and a spear's impaled
    /// creatures held on it.
    pub(super) fn run_weapon(
        &mut self,
        index: i32,
        id: ObjectId,
        observer: &mut dyn PlayerObserver,
    ) {
        let rules = self.world.spear_stick_rules(index, id);
        let thrown = self.with_item_tick(index, |world, cx| {
            world.weapon_part(index, id, cx, &rules, UpdatePart::Start)
        });
        let mut returned = false;
        if thrown {
            self.notify_of_flying_weapon(index, id, observer);
            let trace = self.with_item_tick(index, |world, cx| {
                world.weapon_part(index, id, cx, &rules, UpdatePart::Thrown)
            });
            returned = trace && self.trace_layer_one(index, id);
            if !returned {
                self.thrown_through_layer_zero(index, id, observer);
            }
        }
        if !returned {
            self.recreate_spear_sticks(index, id);
        }
        let part = if returned {
            UpdatePart::Own
        } else {
            let noises = self.with_item_tick(index, |world, cx| {
                world.weapon_part(index, id, cx, &rules, UpdatePart::Physics);
                // `PhysicalObject.Update`'s `grabbedBy` pruning, after the chunks.
                world.clean_grabbed_by(Entity::Object(id));
                std::mem::take(&mut cx.noises)
            });
            for n in noises {
                self.room_noise(index, n);
            }
            UpdatePart::Finish
        };
        self.with_item_tick(index, |world, cx| {
            world.weapon_part(index, id, cx, &rules, part)
        });
        self.spear_stuck_in_creature_update(index, id);
        let eu = self.even_update();
        self.world.hold_impaled(index, id, eu);
        let missed = self
            .world
            .weapon_mut(id)
            .and_then(|w| w.missed_attack.take());
        if let Some((thrower, victim)) = missed {
            self.world.weapon_attack(index, id, thrower, victim, false);
        }
    }

    /// The chunks (position, radius) of creature `id` when it is realized in room
    /// `index`.
    fn creature_chunks_in(&self, id: CreatureId, index: i32) -> Vec<(Vec2, f32)> {
        if id == self.player_id {
            return self
                .player
                .as_ref()
                .filter(|p| self.pos().room == index && !p.in_shortcut())
                .map(|p| p.body.chunks.iter().map(|c| (c.pos, c.rad)).collect())
                .unwrap_or_default();
        }
        let c = self.world.creature(id);
        match &c.realized {
            Some(RealizedCreature::Fly(f)) if f.room == Some(index) => {
                vec![(f.chunk().pos, f.chunk().rad)]
            }
            Some(RealizedCreature::Overseer(b)) if b.in_room && c.pos.room == index => {
                vec![(b.chunk().pos, b.chunk().rad)]
            }
            _ => Vec::new(),
        }
    }

    /// The start of the `Thrown` branch: a weapon with a thrower moving sideways keeps
    /// the creature it passes closest to (ahead of its throw, a quantified creature at
    /// double distance), and each realized creature of the room that listens
    /// (`INotifyOfFlyingWeapons`: of the ported, the overseers) hears of it.
    fn notify_of_flying_weapon(
        &mut self,
        index: i32,
        id: ObjectId,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(w) = self.world.realized_weapon(id, index) else {
            return;
        };
        let Some(thrower) = w.thrown_by else { return };
        if w.chunk().vel.x.abs() <= 0.5 {
            return;
        }
        let sign = w.chunk().vel.x.signum();
        let thrown_x = w.thrown_pos.x;
        let creatures = self.world.room_creatures(index).to_vec();
        for cid in creatures {
            if cid == thrower {
                continue;
            }
            let chunks = self.creature_chunks_in(cid, index);
            if chunks.is_empty() {
                continue;
            }
            let f = if self.world.creature(cid).template.quantified {
                2.0
            } else {
                1.0
            };
            for (cpos, rad) in chunks {
                let Some(w) = self.world.weapon_mut(id) else {
                    return;
                };
                let pos = w.chunk().pos;
                let ahead = cpos
                    .x
                    .in_range(thrown_x - sign * (20.0 + rad), thrown_x + sign * 2000.0);
                if ahead && pos.dist_less(cpos, w.closest_crit_dist * f) {
                    w.thrown_closest_to_creature = Some(cid);
                    w.closest_crit_dist = pos.distance(cpos) * f;
                }
            }
            if self.world.creature(cid).overseer_body().is_some() {
                self.run_overseer(cid, OverseerCall::FlyingWeapon(id), observer);
            }
        }
    }

    /// The end of the `Thrown` branch: each object of collision layer 0 that can be hit
    /// and is not held by the thrower, chunk by chunk, if the weapon's last move crossed
    /// it. Another thrown heavy weapon is a parry (`HitAnotherThrownWeapon`) that ends
    /// the walk; anything else is pushed along and hit without stopping until the hit
    /// chunks weigh more than 0.6, when the weapon drops there (`Free`, half speed,
    /// `Spear_Hit_Small_Creature`). Short of that, an appendage the move crossed is
    /// pushed and hit too.
    fn thrown_through_layer_zero(
        &mut self,
        index: i32,
        id: ObjectId,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(me) = self.world.realized_weapon(id, index) else {
            return;
        };
        let thrower = me.thrown_by;
        let rad_add = if thrower == Some(self.player_id) {
            5.0
        } else {
            0.0
        };
        let mut mass = 0.0f32;
        for e in self.world.layer_zero(index, id) {
            let held =
                thrower.is_some_and(|t| self.world.grabbed_by(e).iter().any(|g| g.grabber == t));
            if !self.world.can_be_hit_by_weapons(e) || held {
                continue;
            }
            'chunks: {
                let Some(me) = self.world.realized_weapon(id, index) else {
                    return;
                };
                let (mode, heavy) = (me.mode, me.heavy_weapon());
                let c = me.chunk();
                let (last, pos, vel, rad) = (c.last_pos, c.pos, c.vel, c.rad + rad_add);
                let Some(other) = self.world.layer_zero_chunk(e) else {
                    break 'chunks;
                };
                let (opos, orad) = (other.pos, other.rad);
                let t = last.circles_collision_time(opos, pos - last, rad, orad);
                if !(t > 0.0 && t < 1.0) {
                    break 'chunks;
                }
                if let Entity::Object(oid) = e
                    && let Some(o) = self.world.realized_weapon(oid, index)
                    && mode == WeaponMode::Thrown
                    && o.mode == WeaponMode::Thrown
                    && o.heavy_weapon()
                {
                    if heavy {
                        self.with_item_tick(index, |world, cx| {
                            world.hit_another_thrown_weapon(index, id, oid, cx);
                        });
                        mass = f32::MAX;
                    }
                    break 'chunks;
                }
                let Some(other) = self.world.layer_zero_chunk(e) else {
                    break 'chunks;
                };
                other.vel += vel;
                let other_mass = other.mass;
                self.hit_something_without_stopping(index, id, e, observer);
                mass += other_mass;
                if mass > 0.6 {
                    let at = self.world.layer_zero_chunk(e).map_or(opos, |c| c.pos);
                    if let Some(w) = self.world.weapon_mut(id) {
                        w.chunk_mut().pos = at;
                    }
                    self.world
                        .change_weapon_mode(id, WeaponMode::Free, &mut self.rng);
                    if let Some(w) = self.world.weapon_mut(id) {
                        w.chunk_mut().vel *= 0.5;
                    }
                    let sounds = if index == self.camera_room {
                        &mut self.sounds
                    } else {
                        &mut self.silent
                    };
                    sounds.play_on_object(
                        SoundId::SpearHitSmallCreature,
                        at,
                        false,
                        1.0,
                        1.0,
                        &mut self.rng,
                    );
                }
            }
            if mass <= 0.6
                && let Entity::Creature(cid) = e
            {
                self.hit_appendage(index, id, cid, observer);
            }
            if mass > 0.6 {
                break;
            }
        }
    }

    /// The walk's appendage part for an overseer (the only ported creature with one):
    /// crossed while it can be hit, `ApplyForceOnAppendage` pushes its body by the
    /// weapon's momentum, then it is hit without stopping.
    fn hit_appendage(
        &mut self,
        index: i32,
        id: ObjectId,
        cid: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(me) = self.world.realized_weapon(id, index) else {
            return;
        };
        let c = me.chunk();
        let (last, pos, momentum) = (c.last_pos, c.pos, c.vel * c.mass);
        let Some(RealizedCreature::Overseer(b)) = &mut self.world.creature_mut(cid).realized else {
            return;
        };
        if !b.can_be_hit_by_weapons || !b.appendage_line_cross(last, pos) {
            return;
        }
        let mass = b.chunk().mass;
        b.chunk_mut().vel += momentum / mass;
        self.hit_something_without_stopping(index, id, Entity::Creature(cid), observer);
    }

    /// `Weapon.HitSomethingWithoutStopping` with `Spear`'s override: a creature gets the
    /// thrower's kill tag; a spear kills it, a rock kills a small creature with chance
    /// 0.8 for a fly and 0.2 for others (one draw), else stuns it 80 ticks. Then
    /// `HitByWeapon` (only the overseer's does anything), and a spear impales a fly. A
    /// rock or spear hit this way does nothing.
    fn hit_something_without_stopping(
        &mut self,
        index: i32,
        id: ObjectId,
        e: Entity,
        observer: &mut dyn PlayerObserver,
    ) {
        let Entity::Creature(cid) = e else { return };
        let Some(w) = self.world.realized_weapon(id, index) else {
            return;
        };
        let (thrower, spear) = (w.thrown_by, w.spear().is_some());
        if let Some(t) = thrower {
            self.world.set_kill_tag(cid, t);
        }
        let c = self.world.creature(cid);
        let fly = c.kind() == CreatureType::Fly;
        let small = c.template.small_creature;
        if spear || (small && self.rng.value() < if fly { 0.8 } else { 0.2 }) {
            self.creature_dies(cid);
        } else {
            self.world.stun_creature(cid, 80);
        }
        if self.world.creature(cid).overseer_body().is_some() {
            let by_slugcat =
                thrower.is_some_and(|t| self.world.creature(t).kind() == CreatureType::Slugcat);
            self.run_overseer(cid, OverseerCall::HitByWeapon { by_slugcat }, observer);
        }
        if spear && fly {
            self.world.try_impale_small_creature(id, cid);
        }
    }

    /// `Die` of a realized fly or overseer (the player is never hit in layer 0).
    fn creature_dies(&mut self, id: CreatureId) {
        match self.world.creature(id).realized {
            Some(RealizedCreature::Fly(_)) => self.world.fly_die(id, &mut self.rng),
            Some(RealizedCreature::Overseer(_)) => self.overseer_die(id),
            _ => {}
        }
    }

    /// `Overseer.Die`: a living player guide comes back as a new guide in the
    /// off-screen den (ID -1.5), unless the slugcat killed it in view in a story game:
    /// then it is gone till the next cycle. Its hologram goes, then `Creature.Die`.
    pub(crate) fn overseer_die(&mut self, id: CreatureId) {
        let c = self.world.creature(id);
        let guide = c.overseer_ai().is_some_and(|a| a.is_player_guide);
        if !self.world.realized_dead(id) && guide {
            let by_slugcat = c
                .kill_tag
                .is_some_and(|k| self.world.creature(k).kind() == CreatureType::Slugcat);
            let room = c.pos.room;
            let pos = c.overseer_body().map_or(Vec2::ZERO, |b| b.chunk().pos);
            let viewed = self
                .world
                .realized(room)
                .is_some_and(|r| self.camera_view.viewed(room, &r.room, pos, 0.0));
            if by_slugcat && self.world.story && viewed {
                log::info!("player guide killed by player - gone till next cycle");
            } else {
                self.respawn_player_guide();
            }
        }
        if let Some(RealizedCreature::Overseer(b)) = &mut self.world.creature_mut(id).realized {
            b.destroy_hologram();
        }
        self.world.creature_die(id, &mut self.rng);
    }

    /// The new guide `Overseer.Die` puts in the off-screen den.
    fn respawn_player_guide(&mut self) {
        let den = self.world.offscreen_den_index();
        let pos = WorldCoordinate::new(den, -1, -1, 0);
        let template = CreatureTemplate::of(CreatureType::Overseer, self.world.mods());
        let session = self.story_session();
        let c = self.world.new_creature(
            template,
            pos,
            EntityId::new(-1, 5),
            &session,
            &mut self.ids,
            &mut self.rng,
        );
        let id = self.world.insert_creature(c);
        self.world.add_to_dens(id);
        if let Some(a) = self.world.creature_mut(id).overseer_ai_mut() {
            a.set_as_player_guide(1);
        }
        self.world.set_player_guide(id);
        log::info!("------Player guide respawned in offscreen den.");
    }
}

#[cfg(test)]
#[path = "game_hits_tests.rs"]
mod tests;
