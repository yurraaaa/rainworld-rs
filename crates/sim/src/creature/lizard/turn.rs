//! The realized lizards in a room's update list: `Lizard.Update`'s body part with
//! `AirBreatherCreature.Update`, `Creature.Update` and `PhysicalObject.Update` under it,
//! terrain impacts, and their shortcut vessels, lobby turns and `SpitOutOfShortCut`
//! (`Act` and entering shortcuts are `creature/lizard/act_turn.rs`). Decompile: `Lizard.cs` 822-1062 and
//! 1460-1711, `Creature.cs` 369-707, `AirBreatherCreature.cs`. Behaviour:
//! `rainworld-re/notes/creature-model/lizard-body-and-movement.md` section 13.
//!
//! Mirrors: Lizard.cs (Update), AirBreatherCreature.cs, Creature.cs, PhysicalObject.cs

use std::sync::Arc;

use rw_assets::{Room, Terrain};

use crate::creature::lizard::LizardEmotion;
use crate::creature::{CreatureId, LUNG_CAPACITY, WorldCoordinate};
use crate::game::shortcut::CreatureVessel;
use crate::game::{PlayerObserver, RainWorldGame, RoomObject};
use crate::item::Entity;
use crate::physics::room_ext::RoomExt;
use crate::world::LizardRoomCx;
use crate::{
    BodyChunk, Bubble, CosmeticKind, FloatExt, InGameNoise, ScreenMovement, SoundId, Sounds,
    TerrainImpact, TerrainImpactHandler, Vec2, WaterDrip,
};

/// A lizard's terrain impact as `TerrainImpact` meets it: the impact, the chunk's
/// position, radius and mass, the head's position, and the base's screen movement and
/// noise.
struct LizardImpact {
    impact: TerrainImpact,
    pos: Vec2,
    rad: f32,
    mass: f32,
    head: Vec2,
    /// Every chunk's position and radius at the impact, for a floor first contact
    /// (`CreatureSmashedInGround` picks one).
    chunks: Vec<(Vec2, f32)>,
    screen: Option<ScreenMovement>,
    noise: Option<InGameNoise>,
}

/// The impacts of one physics update of lizard `source`.
struct LizardImpacts {
    source: Entity,
    list: Vec<LizardImpact>,
}

impl TerrainImpactHandler for LizardImpacts {
    fn terrain_impact(&mut self, chunks: &mut [BodyChunk], impact: TerrainImpact) {
        let c = &chunks[impact.chunk];
        self.list.push(LizardImpact {
            impact,
            pos: c.pos,
            rad: c.rad,
            mass: c.mass,
            head: chunks[0].pos,
            chunks: if impact.first_contact && impact.direction.y < 0 {
                chunks.iter().map(|c| (c.pos, c.rad)).collect()
            } else {
                Vec::new()
            },
            screen: impact.screen_movement(c),
            noise: impact.noise(c, self.source, 1.0),
        });
    }
}

impl RainWorldGame {
    /// The realized lizards in room `index`'s update list.
    pub(crate) fn room_lizard_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let mut list: Vec<(u64, RoomObject)> = self
            .world
            .room_lizards(index)
            .into_iter()
            .map(|(seq, id)| (seq, RoomObject::Lizard(id)))
            .collect();
        list.extend(self.room_lizard_loop_objects(index));
        list.extend(self.room_lizard_voice_objects(index));
        list
    }

    /// The player spawn tile of room `index`'s shelter door.
    fn shelter_spawn_tile(&self, index: i32) -> Option<(i32, i32)> {
        self.shelter_doors
            .iter()
            .find(|d| d.room == index)
            .map(|d| d.player_spawn_pos)
    }

    /// The sound sink of room `room` (the camera's, or a silent one) with the stream.
    pub(crate) fn room_sounds(&mut self, room: i32) -> (&mut Sounds, &mut rw_random::UnityRandom) {
        let sounds = if room == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        (sounds, &mut self.rng)
    }

    /// Lizard `id`'s slot in room `index`'s update list: `Lizard.Update`. A lizard
    /// slated for deletion is cleaned out.
    pub(crate) fn run_lizard(
        &mut self,
        index: i32,
        id: CreatureId,
        room: &Arc<Room>,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(l) = self.world.creature(id).lizard() else {
            return;
        };
        if l.room != Some(index) {
            return;
        }
        if l.slated {
            self.clean_out_lizard(id);
            return;
        }
        if self.world.creature(id).in_den {
            return;
        }
        let room_gravity = self.room_gravity(index);
        self.world.lizard_at(id).body.params.room_gravity = room_gravity;
        self.lizard_update_start(index, id);
        if self.world.lizard_ref(id).conscious() {
            self.lizard_act(index, id);
        } else {
            self.world.lizard_at(id).apply_gravity = true;
        }
        self.lizard_grabbed_attack(id);
        self.lizard_frictions(id);
        if self
            .world
            .creature(id)
            .grasps
            .first()
            .is_some_and(Option::is_some)
        {
            self.lizard_carry_object(index, id);
        }
        self.lizard_pushes(index, id, room);
        self.lizard_creature_update(index, id, room, observer);
        if self.world.creature(id).lizard().is_some() {
            self.lizard_lungs(index, id);
        }
        if self.world.creature(id).lizard().is_some_and(|l| l.slated) {
            self.clean_out_lizard(id);
        } else if self
            .world
            .creature(id)
            .lizard()
            .is_some_and(|l| l.room == Some(index))
        {
            // `Room.Update`: the graphics module's `Update` right after its owner's.
            self.lizard_graphics_update(index, id, observer);
        }
    }

    /// `CleanOutObjectNotInThisRoom` of a lizard slated for deletion.
    fn clean_out_lizard(&mut self, id: CreatureId) {
        if let Some(l) = self.world.creature_mut(id).lizard_mut() {
            l.leave_room();
        }
        self.world.creature_mut(id).slated_for_deletion = true;
        self.world.remove_entity(id);
    }

    /// `Lizard.Update` before `Act`: `lastJawOpen`, the bite delay counting down with the
    /// jaw open and up with it shut, the forced shut easing, the three health rolls (a
    /// stun, the white flicker of the graphics, a jaw twitch), the stiffness push on
    /// while `BodyStiff`, the turn a rock gave, the bubble and lounge counters; the
    /// voice's `Update` after `lastJawOpen`.
    fn lizard_update_start(&mut self, index: i32, id: CreatureId) {
        let holding = self.lizard_holding(id);
        let health = self.world.creature(id).state.health;
        let l = self.world.lizard_at(id);
        l.last_jaw_open = l.read_jaw_open(holding);
        self.lizard_voice_update(index, id);
        let l = self.world.lizard_at(id);
        if l.read_jaw_open(holding) > 0.9 {
            if l.bite_delay > 0 {
                l.bite_delay -= 1;
            }
        } else if l.bite_delay < l.params.bite_delay {
            l.bite_delay += 1;
        }
        l.jaw_forced_shut = (l.jaw_forced_shut - 1.0 / 60.0).max(0.0);
        let dead = l.dead;
        let graphics = l.graphics.is_some();
        let rng = &mut self.rng;
        if !dead && rng.value() * 0.7 > health && rng.value() < 0.125 {
            let cap = 27 - ((20.0 * health) as i32).clamp(0, 10);
            let inner = rng.range_i32(1, cap);
            let st = rng.range_i32(1, inner);
            self.lizard_stun(id, st);
        }
        let rng = &mut self.rng;
        if !dead && rng.value() * 0.2 > health && rng.value() < 0.025 && graphics {
            let fl = rng.range_i32(1, 5);
            if let Some(g) = &mut self.world.lizard_at(id).graphics {
                g.white_flicker(fl);
            }
        }
        let rng = &mut self.rng;
        if !dead && rng.value() * 0.5 > health && rng.value() < 0.05 {
            let v = rng.value();
            self.world.lizard_at(id).set_jaw_open(v, holding);
        }
        let l = self.world.lizard_at(id);
        l.body.connections[2].active = l.body_stiff();
        if l.turned_by_rock_counter > 0 {
            l.turned_by_rock_counter -= 1;
            let dir = l.turned_by_rock_direction as f32;
            l.weighted_push(0, 2, Vec2::new(dir, 0.0), 6.0);
            if (l.body.chunks[0].pos.y - l.body.chunks[2].pos.y).abs() < 5.0 {
                l.weighted_push(0, 2, Vec2::new(0.0, 1.0), 2.0);
            }
        }
        if l.bubble > 0 {
            l.bubble -= 1;
        }
        l.lounge_delay -= 1;
    }

    /// Whether the lizard's grasp 0 holds something (its jaw stays shut).
    pub(crate) fn lizard_holding(&self, id: CreatureId) -> bool {
        self.world
            .creature(id)
            .grasps
            .first()
            .is_some_and(Option::is_some)
    }

    /// The struggle of an unconscious, living lizard held by something (not a leech):
    /// for 22 ticks its jaw opens and its chunks jerk (a direction and a value each),
    /// and at the 22nd, jaw ready, it may bite its holder free; let go, the count
    /// restarts.
    fn lizard_grabbed_attack(&mut self, id: CreatureId) {
        let c = self.world.creature(id);
        let grabbers: Vec<CreatureId> = c.grabbed_by.iter().map(|g| g.grabber).collect();
        let health = c.state.health;
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_ref(id);
        let grabber_kind = grabbers.first().map(|&g| self.world.creature(g).kind());
        let struggles = !l.conscious()
            && !l.dead
            && l.stun < 35
            && grabber_kind.is_some_and(|k| k != rw_assets::CreatureType::Leech)
            && l.grabbed_attack_counter < 22;
        if !struggles {
            if grabbers.is_empty() {
                self.world.lizard_at(id).grabbed_attack_counter = 0;
            }
            return;
        }
        let l = self.world.lizard_at(id);
        l.grabbed_attack_counter += 1;
        l.jaw_forced_shut = (l.jaw_forced_shut - 1.0 / 24.0).max(0.0);
        let jaw = l.read_jaw_open(holding) + 0.1 + 0.1 * health;
        l.set_jaw_open(jaw, holding);
        for i in 0..3 {
            let dir = Vec2::random_unit(&mut self.rng);
            let v = self.rng.value();
            self.world.lizard_at(id).body.chunks[i].vel += dir * (v * 6.0 * health);
        }
        let l = self.world.lizard_at(id);
        if l.grabbed_attack_counter != 22 || !l.jaw_ready_for_bite(holding) {
            return;
        }
        let (my_mass, free_chance) = (l.total_mass(), l.params.get_free_bite_chance);
        let kind = self.world.creature(id).kind();
        let Some(&grabber) = grabbers.first() else {
            return;
        };
        let g_mass = self.creature_total_mass(grabber);
        let g_kind = self.world.creature(grabber).kind();
        let chance = free_chance * g_mass.lerp_map(my_mass, my_mass * 3.0, 1.0, 0.1) * health;
        use rw_assets::CreatureType as T;
        let bites = (self.rng.value() < chance
            && g_kind != T::RedLizard
            && (g_kind != T::Vulture || self.rng.value() < 0.5)
            && g_kind != T::KingVulture)
            || (kind == T::RedLizard && self.rng.value() < health);
        if bites {
            self.lizard_damage_attack_closest_chunk(id, grabber);
        }
    }

    /// The first two connections pull during a Standard movement animation; the friction
    /// sets: after a lounge, falling with gravity, or held up by the legs
    /// without it.
    fn lizard_frictions(&mut self, id: CreatureId) {
        let l = self.world.lizard_at(id);
        let pull = l
            .movement_animation
            .as_ref()
            .is_some_and(|a| a.connection.kind == crate::world::MovementType::Standard);
        for c in &mut l.body.connections[..2] {
            c.kind = if pull {
                crate::physics::object::ConnectionKind::Pull
            } else {
                crate::physics::object::ConnectionKind::Normal
            };
        }
        let p = &mut l.body.params;
        let through_floors;
        if l.animation == crate::creature::LizardAnimation::Lounge || l.post_lounge_stun > 0 {
            p.air_friction = 0.999;
            p.g = 0.9;
            p.bounce = 0.1;
            p.surface_friction = if l.animation == crate::creature::LizardAnimation::Lounge {
                0.5
            } else {
                0.4
            };
            through_floors = l.lounge_dir.y < 0.0;
            l.post_lounge_stun -= 1;
        } else if l.apply_gravity {
            p.air_friction = 0.999;
            p.g = 0.9;
            p.bounce = 0.1;
            p.surface_friction = 0.3;
            through_floors = l.swim > 0.0;
        } else {
            p.air_friction = 0.8;
            p.g = 0.0;
            p.bounce = 0.1;
            p.surface_friction = 0.5;
            through_floors = true;
        }
        l.body.set_go_through_floors(through_floors);
    }

    /// The forbidden-entrance push (a conscious lizard whose path finder forbids an
    /// entrance is pushed out of the shortcut holes it lies in) and `pullDownFac`.
    fn lizard_pushes(&mut self, index: i32, id: CreatureId, room: &Room) {
        let tile = self.world.creature(id).pos;
        let water = self.world.realized(index).and_then(|r| r.water.clone());
        let narrow = self
            .world
            .realized(index)
            .and_then(|r| r.ai_map.as_ref())
            .is_some_and(|m| m.tile_at(tile).narrow_space);
        let forbidden = self
            .world
            .path_finder_of(id)
            .and_then(|p| self.world.path_finders.get(p))
            .is_some_and(|f| f.forbidden_entrance_counter > 0);
        let l = self.world.lizard_at(id);
        if l.conscious() && forbidden {
            for c in &mut l.body.chunks {
                let t = c.pos.tile();
                if room.tile(t.0, t.1).terrain == Terrain::ShortcutEntrance {
                    let (hx, hy) = room.shortcut_hole_direction(t.0, t.1);
                    c.vel += Vec2::new(hx as f32, hy as f32) * 4.0;
                }
            }
        }
        let fac = l.params.pull_down_fac;
        if fac != 1.0
            && l.body.submersion(water.as_ref()) == 0.0
            && !narrow
            && (room.is_solid_tile(tile.tile.0, tile.tile.1 - 1)
                || room.is_solid_tile(tile.tile.0, tile.tile.1 - 2))
        {
            for c in &mut l.body.chunks[1..] {
                c.vel.y -= fac - 1.0;
            }
        }
    }

    /// `Creature.Update` for a lizard: `NewTile`, the counters, a pacifying holder's
    /// stun, the kill tag, the health death roll, the release and the death under the
    /// room, then the pull into a shortcut being entered or `PhysicalObject.Update`.
    fn lizard_creature_update(
        &mut self,
        index: i32,
        id: CreatureId,
        room: &Arc<Room>,
        observer: &mut dyn PlayerObserver,
    ) {
        let coord = self.world.creature(id).pos;
        let last = self.world.lizard_ref(id).last_coord;
        if (coord.tile != last.tile || coord.room != last.room)
            && let Some(c) = self.world.new_tile(id, last, self.camera_room)
        {
            self.entrance_colors.push(c);
        }
        let pacified = self
            .world
            .creature(id)
            .grabbed_by
            .iter()
            .any(|g| g.pacifying);
        let l = self.world.lizard_at(id);
        l.last_coord = coord;
        if l.new_to_room_invinsibility > 0 {
            l.new_to_room_invinsibility -= 1;
        }
        if l.stun > 0 {
            l.stun -= 1;
        }
        if l.stun < 10 && pacified {
            l.stun = 10;
        }
        if l.shortcut_delay > 0 {
            l.shortcut_delay -= 1;
        }
        self.world.kill_tag_tick(id);
        let health = self.world.creature(id).state.health;
        if !self.world.lizard_ref(id).dead
            && health < 0.0
            && self.rng.value() < -health
            && self.rng.value() < 0.025
        {
            self.lizard_die(id);
        }
        let l = self.world.lizard_ref(id);
        let floor = -l.head().restrict_in_room_range + 1.0;
        let head_y = l.head().pos.y;
        if head_y < floor + 10.0 && !self.world.creature(id).grabbed_by.is_empty() {
            log::info!("FORCE CREATURE RELEASE UNDER ROOM");
            for g in self.world.creature(id).grabbed_by.clone() {
                self.world.release(g);
            }
        }
        let no_water = room.water_level().is_none_or(|w| w < -10);
        if head_y < floor && no_water {
            log::info!("lizard {:?} fell out of room!", self.world.creature(id).id);
            self.lizard_die(id);
            self.world.lizard_at(id).slated = true;
            self.world.destroy_abstract(id);
        }
        if !self.lizard_pull_into_shortcut(index, id, observer) {
            self.lizard_physics(index, id, room);
        }
    }

    /// `PhysicalObject.Update`: the chunks, the abstract tile from the head, the
    /// connections, the holders cleaned, then the stuck objects grabbed again once.
    /// Terrain impacts play their sounds and drip as they came.
    fn lizard_physics(&mut self, index: i32, id: CreatureId, room: &Arc<Room>) {
        let held = !self.world.creature(id).grabbed_by.is_empty();
        let water = self.world.realized_mut(index).and_then(|r| r.water.take());
        let eu = self.even_update();
        let mut log = LizardImpacts {
            source: Entity::Creature(id),
            list: Vec::new(),
        };
        let l = self.world.lizard_at(id);
        l.even_update = eu;
        l.body.held = held;
        l.body.update_chunks(room, water.as_ref(), &mut log);
        let tile = l.head().pos.tile();
        l.body.update_connections();
        if let Some(r) = self.world.realized_mut(index) {
            r.water = water;
        }
        let c = self.world.creature_mut(id);
        c.pos.tile = tile;
        c.pos.room = index;
        self.lizard_terrain_impacts(index, id, &log);
        self.world.clean_grabbed_by(Entity::Creature(id));
        if !self.world.lizard_ref(id).sticks_respawned {
            self.lizard_recreate_sticks(id);
            self.world.lizard_at(id).sticks_respawned = true;
        }
    }

    /// `Lizard.TerrainImpact` of each impact: a first contact over 1.5 plays the light
    /// and the heavy impact by its share of 6 to 14 (pitched by body size); then the
    /// base's screen movement, noise and drips off a wet ceiling, after the graphics'
    /// bubbles.
    fn lizard_terrain_impacts(&mut self, index: i32, id: CreatureId, log: &LizardImpacts) {
        let ceiling_drips = self
            .world
            .realized(index)
            .map_or(0.0, |r| r.settings.ceiling_drips);
        let size = self.world.lizard_ref(id).params.body_size_fac;
        let pitch = (1.0 / size).lerp(1.0, 0.8);
        for i in &log.list {
            let (impact, head) = (&i.impact, &i.head);
            self.lizard_impact_bubbles(index, id, impact);
            if impact.speed > 1.5 && impact.first_contact {
                let num = impact.speed.inverse_lerp(6.0, 14.0);
                let (sounds, rng) = self.room_sounds(index);
                if num < 1.0 {
                    sounds.play_on_object(
                        SoundId::LizardLightTerrainImpact,
                        *head,
                        false,
                        1.0 - num,
                        pitch,
                        rng,
                    );
                }
                if num > 0.0 {
                    sounds.play_on_object(
                        SoundId::LizardHeavyTerrainImpact,
                        *head,
                        false,
                        num,
                        pitch,
                        rng,
                    );
                }
            }
            if let Some(m) = i.screen {
                self.room_screen_movement(index, m);
            }
            if let Some(n) = i.noise {
                self.room_noise(index, n);
            }
            self.world
                .creature_smashed_in_ground(index, id, impact, &i.chunks, &mut self.rng);
            if impact.first_contact {
                let drips = WaterDrip::from_impact(
                    impact,
                    i.pos,
                    i.rad,
                    i.mass,
                    ceiling_drips,
                    &mut self.rng,
                );
                for d in drips {
                    self.world.add_drip(index, d);
                }
            }
        }
    }

    /// `Creature.RecreateSticksFromAbstract`: each grip stick it holds is grabbed again
    /// (a draw for the chunk), or dropped.
    fn lizard_recreate_sticks(&mut self, id: CreatureId) {
        let sticks = self.world.creature(id).sticks.clone();
        for s in sticks.iter().rev() {
            if s.a != id {
                continue;
            }
            let chunks = self.world.chunk_count(s.b);
            let chunk = self.rng.range_i32(0, i32::try_from(chunks).unwrap_or(0));
            let request = crate::creature::GrabRequest {
                grabber: id,
                grabbed: s.b,
                grasp_used: s.grasp,
                chunk_grabbed: usize::try_from(chunk).unwrap_or(0),
                shareability: crate::creature::Shareability::CanNotShare,
                dominance: 0.5,
                override_equally_dominant: true,
                pacifying: true,
            };
            if !self.creature_grab(request) {
                self.world.deactivate_stick(*s);
            }
        }
    }

    /// `AirBreatherCreature.Update` after the base: a living lizard with full lungs
    /// starts losing air under water (a draw every tick); out of water they refill;
    /// under water they empty, and below 0.3 it drowns: bubbles, letting go, struggling,
    /// stuns and at last death.
    pub(crate) fn lizard_lungs(&mut self, index: i32, id: CreatureId) {
        let l = self.world.lizard_ref(id);
        if l.dead {
            return;
        }
        // A lizard sucked into a shortcut this tick has no room: no water.
        let water = l
            .room
            .and_then(|_| self.world.realized(index))
            .and_then(|r| r.water.clone());
        let has_water = water.is_some();
        let submersion = l.body.submersion(water.as_ref());
        let capacity = LUNG_CAPACITY;
        let lungs = l.lungs;
        if lungs == 1.0 {
            if self.rng.value() < 1.0 / 60.0 && has_water && submersion == 1.0 {
                let l = self.world.lizard_at(id);
                l.lungs = (l.lungs - 1.0 / capacity).max(0.0);
            }
            return;
        }
        if !has_water || submersion < 1.0 {
            let l = self.world.lizard_at(id);
            l.lungs = (l.lungs + 1.0 / 30.0).clamp(0.0, 1.0);
            return;
        }
        let l = self.world.lizard_at(id);
        l.lungs = (l.lungs - 1.0 / capacity).max(-1.0);
        let lungs = l.lungs;
        if lungs >= 0.3 {
            return;
        }
        let drown = (lungs.inverse_lerp(0.3, -0.3) * std::f32::consts::PI).sin();
        if self.rng.value() < drown * 0.5 {
            let pos = self.world.lizard_ref(id).body.chunks[0].pos;
            let vel = Vec2::random_unit(&mut self.rng) * (self.rng.value() * 6.0);
            let bubble = Bubble::new(pos, vel, false, false, &mut self.rng);
            self.world.add_cosmetic(index, CosmeticKind::Bubble(bubble));
        }
        if self.rng.value() < 0.025 {
            self.lizard_lose_all_grasps(id);
        }
        for i in 0..3 {
            let rng = &mut self.rng;
            let rad = self.world.lizard_ref(id).body.chunks[i].rad;
            let a = Vec2::random_unit(rng) * (rad * 0.4 * rng.value() * drown);
            let deg = (-30.0f32).lerp(30.0, rng.value());
            let main = if i == 0 { 0.4 } else { 0.2 };
            let b = Vec2::from_degrees(deg) * (rng.value() * main * drown.mathf_pow(2.0));
            self.world.lizard_at(id).body.chunks[i].vel += a + b;
        }
        if lungs <= 0.0 && self.rng.value() < 0.1 {
            let st = self.rng.range_i32(0, 18);
            self.lizard_stun(id, st);
        }
        if lungs < -0.5 && self.rng.value() < 1.0 / lungs.lerp_map(-0.5, -1.0, 90.0, 30.0) {
            self.lizard_die(id);
        }
    }

    /// `ReadyForAI`'s `RealizeInRoom` of abstract lizard `id` in its loaded room (a
    /// shelter's door gives its spawn tile).
    pub(crate) fn realize_lizard_in_room(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        let c = self.world.creature(id);
        if c.is_realized() || c.in_den || crate::creature::LizardBreedParams::of(c.kind()).is_none()
        {
            return;
        }
        let shelter = self
            .world
            .abstract_room(index)
            .is_some_and(rw_assets::world::AbstractRoomData::shelter);
        let spawn = shelter.then(|| self.shelter_spawn_tile(index)).flatten();
        let mut cx = LizardRoomCx {
            rng: &mut self.rng,
            observer,
            camera_room: self.camera_room,
        };
        self.world.realize_lizard_in_room(id, spawn, &mut cx);
    }

    /// `SpitOutCreature` of a lizard's vessel (the large NPC exit sound) and
    /// `Lizard.SpitOutOfShortCut`: the base's stuck objects out at the entrance (a draw
    /// per chunk; a held player as `Player.SpitOutOfShortCut`, a fly as its own), then
    /// the three chunks out of the hole 15 px apart, moving 2 px a tick out of it, and
    /// half the time an `OutOfShortcut` voice.
    pub(crate) fn spit_out_lizard(&mut self, v: CreatureVessel, observer: &mut dyn PlayerObserver) {
        let (room, pos) = (v.room, v.vessel.pos);
        let Some(map_room) = self.world.realized(room).map(|r| Arc::clone(&r.room)) else {
            return;
        };
        let middle = Vec2::middle_of_tile(pos);
        let me = v.creature;
        let exit = self.world.creature(me).template.npc_shortcut_sound(2);
        let (sounds, rng) = self.room_sounds(room);
        sounds.play_at(exit, middle, 1.0, 1.0, rng);
        let l = self.world.lizard_at(me);
        if l.entering_shortcut.is_some() {
            log::warn!("Double shortcut entry issue!");
            l.entering_shortcut = None;
        }
        l.shortcut_delay = 20;
        l.in_shortcut = false;
        let (hx, hy) = map_room.shortcut_hole_direction(pos.0, pos.1);
        let hole = Vec2::new(hx as f32, hy as f32);
        for o in self.world.connected_objects(Entity::Creature(me)) {
            match o {
                Entity::Object(o) => self.world.spit_out_object(o, room, middle, &mut self.rng),
                Entity::Creature(o) if o == self.player_id => {
                    self.carried_player_spat_out(room, pos, &map_room, observer);
                }
                Entity::Creature(o) if self.world.creature(o).fly().is_some() => {
                    self.stuck_fly_spat_out(o, room, middle, hole, observer);
                }
                Entity::Creature(o) if self.world.creature(o).lizard().is_some() => {
                    self.stuck_lizard_spat_out(o, room, middle, pos, &map_room, observer);
                }
                Entity::Creature(_) => {}
            }
        }
        self.lizard_spat_out_body(me, middle, &map_room, pos);
    }

    /// A lizard stuck to a creature spat out of a shortcut (the base `SpitOutOfShortCut`
    /// loop): its chunks at the entrance's `middle` (a draw each), added to `room`,
    /// `Lizard.SpitOutOfShortCut(spitOutAllSticks: false)` and `NewRoom` when it comes
    /// from elsewhere.
    pub(crate) fn stuck_lizard_spat_out(
        &mut self,
        o: CreatureId,
        room: i32,
        middle: Vec2,
        pos: (i32, i32),
        map_room: &Room,
        observer: &mut dyn PlayerObserver,
    ) {
        for c in &mut self.world.lizard_at(o).body.chunks {
            c.hard_set_position(middle + Vec2::random_unit(&mut self.rng));
            c.vel = Vec2::ZERO;
        }
        let came_from = self.world.lizard_ref(o).room;
        let mut cx = LizardRoomCx {
            rng: &mut self.rng,
            observer: &mut *observer,
            camera_room: self.camera_room,
        };
        self.world.lizard_add_object(o, room, &mut cx);
        self.lizard_spat_out_body(o, middle, map_room, pos);
        if came_from != Some(room) {
            let mut cx = LizardRoomCx {
                rng: &mut self.rng,
                observer: &mut *observer,
                camera_room: self.camera_room,
            };
            self.world.lizard_new_room(o, &mut cx);
        }
    }

    /// `Lizard.SpitOutOfShortCut` after the base's stuck objects; with graphics, what
    /// grasp 0 holds goes back into the jaws' container.
    fn lizard_spat_out_body(&mut self, id: CreatureId, middle: Vec2, room: &Room, pos: (i32, i32)) {
        let (hx, hy) = room.shortcut_hole_direction(pos.0, pos.1);
        let hole = Vec2::new(hx as f32, hy as f32);
        let l = self.world.lizard_at(id);
        l.shortcut_delay = 20;
        l.in_shortcut = false;
        for (i, c) in l.body.chunks.iter_mut().enumerate() {
            c.pos = middle - hole * ((-1.5 + i as f32) * 15.0);
            c.last_pos = middle;
            c.vel = hole * 2.0;
        }
        if self.rng.value() < 0.5 {
            self.lizard_voice(id, LizardEmotion::OutOfShortcut);
        }
        if let Some(g) = self.world.creature(id).grasps.first().copied().flatten() {
            self.lizard_contain_in_jaws(id, g.grabbed);
        }
    }

    /// A lizard vessel's lobby turn into a ready room puts it in the room's tubes at
    /// the node; `Abstractize` toward an unrealized one.
    pub(crate) fn abstractize_lizard_to(&mut self, id: CreatureId, coord: WorldCoordinate) {
        self.world.abstractize_lizard(id, coord, &mut self.rng);
    }

    /// The total mass of a creature's body (the player's two chunks, or the world's).
    pub(crate) fn creature_total_mass(&self, id: CreatureId) -> f32 {
        if id == self.player_id {
            return self
                .player
                .as_ref()
                .map_or(0.0, |p| p.body.chunks.iter().map(|c| c.mass).sum());
        }
        self.world.total_mass(Entity::Creature(id))
    }
}

#[cfg(test)]
#[path = "tests/turn.rs"]
mod tests;
