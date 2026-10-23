//! `Fly.Update` with the `Creature` and `PhysicalObject` updates under it: the counters,
//! the fell-out-of-room death, entering a shortcut, the body physics, drowning, the
//! flight modes (`Act`, `BatFlight`, `SwarmFlight`, `Hanging`, `WaterBehavior`,
//! burrowing) and the pull of a chain grasp. Decompile lines: `Fly` 184644-184928,
//! `Creature.Update` (Creature.cs 369-707), `PhysicalObject.Update` (PhysicalObject.cs
//! 478).
//!
//! Mirrors: Fly.cs, Creature.cs (Update), PhysicalObject.cs (Update)

use std::sync::Arc;

use rw_assets::{Room, Terrain, tile_centre};
use rw_random::UnityRandom;

use super::{Fly, FlyBehavior, FlyView, HoverEmitter, MovementMode};
use crate::creature::{CreatureId, RealizedCreature, WorldCoordinate};
use crate::game::camera::CameraView;
use crate::game::shortcut::{CreatureVessel, ShortcutHandler, ShortcutVessel, Transport};
use crate::item::Entity;
use crate::physics::room_ext::RoomExt;
use crate::world::{AiMap, EIGHT_DIRECTIONS, World};
use crate::{EntranceColor, FloatExt, PlayerObserver, SoundId, Sounds, Vec2, Water};

/// The slugcat's body as a fly reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerBody {
    pub id: CreatureId,
    /// `realizedCreature.room`: `None` in a shortcut.
    pub room: Option<i32>,
    /// Head and hips.
    pub chunks: [Vec2; 2],
}

/// What a fly's update reads and changes besides the world: the room it runs in, the
/// shared random stream and sounds, camera 0, the player, the shortcut handler.
pub(crate) struct FlyTick<'a> {
    pub room_index: i32,
    pub room: Arc<Room>,
    pub ai_map: Arc<AiMap>,
    pub water: Option<&'a Water>,
    pub rng: &'a mut UnityRandom,
    pub sounds: &'a mut Sounds,
    pub observer: &'a mut dyn PlayerObserver,
    pub camera: CameraView,
    pub camera_shake: &'a mut crate::CameraShake,
    /// The camera's room, for the graphics' culling.
    pub camera_room: Arc<Room>,
    pub player: Option<PlayerBody>,
    pub shortcuts: &'a mut ShortcutHandler,
    pub entrance_colors: &'a mut Vec<EntranceColor>,
    /// The game tick, as the hover emitter's frame count.
    pub tick: u64,
    /// `game.evenUpdate`.
    pub eu: bool,
}

/// A fly's terrain impacts with where its chunk was at each (the drips start there),
/// the screen movement and the noise each makes (where, how strong).
#[derive(Default)]
struct ImpactLog {
    impacts: Vec<LoggedImpact>,
}

struct LoggedImpact {
    impact: crate::TerrainImpact,
    pos: Vec2,
    rad: f32,
    mass: f32,
    screen: Option<crate::ScreenMovement>,
    noise: Option<(Vec2, f32)>,
}

impl crate::TerrainImpactHandler for ImpactLog {
    fn terrain_impact(&mut self, chunks: &mut [crate::BodyChunk], impact: crate::TerrainImpact) {
        let c = &chunks[impact.chunk];
        self.impacts.push(LoggedImpact {
            impact,
            pos: c.pos,
            rad: c.rad,
            mass: c.mass,
            screen: impact.screen_movement(c),
            noise: impact.noise_at(c),
        });
    }
}

/// One realized fly's update in its room.
pub(crate) struct FlyRun<'w, 'c, 'a> {
    pub me: CreatureId,
    pub world: &'w mut World,
    pub cx: &'c mut FlyTick<'a>,
}

impl FlyRun<'_, '_, '_> {
    pub(crate) fn f(&mut self) -> &mut Fly {
        self.world.fly_at(self.me)
    }

    pub(crate) fn fr(&self) -> &Fly {
        self.world.fly_ref(self.me)
    }

    /// `FlyPos`.
    pub(crate) fn pos(&self) -> Vec2 {
        self.fr().pos()
    }

    /// `mainBodyChunk.submersion`.
    fn submersion(&self) -> f32 {
        self.fr().chunk().submersion(self.cx.water)
    }

    /// `Room.PlaySound(id, mainBodyChunk)`, a one-shot.
    pub(crate) fn play_on_chunk(&mut self, id: SoundId) {
        let pos = self.pos();
        self.cx.sounds.play_at(id, pos, 1.0, 1.0, self.cx.rng);
    }

    /// `Fly.Update`.
    pub(crate) fn update(&mut self) {
        self.creature_update();
        if self.fr().room.is_none() {
            return;
        }
        let f = self.f();
        f.last_flap_depth = f.flap_depth;
        let grabbed_by_fly_or_free = self.grabbed_by_none_or_fly();
        let f = self.f();
        let collide =
            grabbed_by_fly_or_free && (!f.conscious() || f.mov_mode != Some(MovementMode::Burrow));
        f.set_collide_with_terrain(collide);
        if !self.fr().dead {
            let wet = if self.submersion() == 1.0 { 1.0 } else { -1.0 };
            let f = self.f();
            f.drown = (f.drown + 0.0125 * wet).clamp(0.0, 1.0);
            if f.drown == 1.0 {
                self.die();
            }
            // `Random.value` comes first in the condition: drawn every tick.
            if self.cx.rng.value() < 1.0 / 160.0 && self.grabbed_by_player() {
                self.die();
            }
        }
        if self.fr().conscious() && !self.fr().in_shortcut {
            if self.submersion() == 0.0 {
                self.act();
            } else {
                self.water_behavior();
            }
            if self.fr().room.is_none() {
                return;
            }
            let tile = self.pos().tile();
            let room = Arc::clone(&self.cx.room);
            if self.fr().shortcut_delay == 0
                && room.tile(tile.0, tile.1).terrain == Terrain::ShortcutEntrance
                && room.shortcut_map().type_at(tile) != rw_assets::ShortcutType::DeadEnd
            {
                self.f().entering_shortcut = Some(tile);
                self.world.lose_all_grasps(self.me);
            }
        } else {
            self.f().flap_depth = 1.0;
        }
        self.pull_chain();
        let f = self.f();
        if f.eaten > 0 {
            f.eaten -= 1;
            if f.eaten == 0 {
                f.slated = true;
            }
        }
    }

    /// `grabbedBy.Count == 0 || grabbedBy[0].grabber is Fly`.
    fn grabbed_by_none_or_fly(&self) -> bool {
        self.world
            .creature(self.me)
            .grabbed_by
            .first()
            .is_none_or(|g| self.world.creature(g.grabber).fly().is_some())
    }

    /// `grabbedBy.Count > 0 && grabbedBy[0].grabber is Player`.
    fn grabbed_by_player(&self) -> bool {
        self.world
            .creature(self.me)
            .grabbed_by
            .first()
            .is_some_and(|g| {
                matches!(
                    self.world.creature(g.grabber).realized,
                    Some(RealizedCreature::Player)
                )
            })
    }

    /// The `grasps[0]` block: a held chunk more than 8 px away pulls the fly and is
    /// pulled half as much (its position too unless the chain's first fly hangs from a
    /// spot).
    fn pull_chain(&mut self) {
        let Some(g) = self
            .world
            .creature(self.me)
            .grasps
            .first()
            .copied()
            .flatten()
        else {
            return;
        };
        let Some(held) = self.world.chunk_pos(g.grabbed, g.chunk_grabbed) else {
            return;
        };
        let pos = self.pos();
        let dist = pos.distance(held);
        if dist <= 8.0 {
            return;
        }
        let dir = pos.direction_to(held);
        let pull = dir * ((8.0 - dist) * 0.5);
        let c = self.f().chunk_mut();
        c.pos -= pull;
        c.vel -= pull;
        let first = self.world.first_in_chain(self.me);
        let first_hangs = self.world.fly_ref(first).mov_mode == Some(MovementMode::Hang)
            && self.world.fly_ref(first).burrow_or_hang_spot.is_some();
        let push = dir * ((8.0 - dist) * 0.25);
        if let Some(c) = self.world.chunk_mut(g.grabbed, g.chunk_grabbed) {
            c.vel += push;
            if !first_hangs {
                c.pos += push;
            }
        }
    }

    /// `Fly.Die`.
    pub(crate) fn die(&mut self) {
        self.world.fly_die(self.me, self.cx.rng);
    }

    /// `Creature.Update` (the parts a fly reaches) with `PhysicalObject.Update`.
    fn creature_update(&mut self) {
        let coord = self.world.creature(self.me).pos;
        let last = self.fr().last_coord;
        if coord.tile != last.tile || coord.room != last.room {
            let camera_room = self.cx.camera.room;
            if let Some(c) = self.world.new_tile(self.me, last, camera_room) {
                self.cx.entrance_colors.push(c);
            }
        }
        let pacified = self
            .world
            .creature(self.me)
            .grabbed_by
            .iter()
            .any(|g| g.pacifying);
        let f = self.f();
        f.last_coord = coord;
        if f.new_to_room_invinsibility > 0 {
            f.new_to_room_invinsibility -= 1;
        }
        if f.stun > 0 {
            f.stun -= 1;
        }
        if f.stun < 10 && pacified {
            f.stun = 10;
        }
        if f.shortcut_delay > 0 {
            f.shortcut_delay -= 1;
        }
        let floor = -f.chunk().restrict_in_room_range + 1.0;
        self.world.kill_tag_tick(self.me);
        let no_water = self.cx.room.water_level().is_none_or(|l| l < -10);
        let f = self.fr();
        // `canFly`: only a stunned or dead fly falls out.
        if f.pos().y < floor && no_water && (f.stunned() || f.dead) {
            log::info!(
                "fly {:?} fell out of the room",
                self.world.creature(self.me).id
            );
            self.die();
            self.f().slated = true;
            self.world.destroy_abstract(self.me);
        }
        if let Some(entrance) = self.fr().entering_shortcut {
            self.pull_into_shortcut(entrance);
        } else {
            self.f().short_cut_rad_add = 0.9;
            self.physical_update();
        }
    }

    /// `PhysicalObject.Update`.
    fn physical_update(&mut self) {
        let held = !self.world.creature(self.me).grabbed_by.is_empty();
        let room = Arc::clone(&self.cx.room);
        let water = self.cx.water;
        let mut log = ImpactLog::default();
        let eu = self.cx.eu;
        let f = self.f();
        // `UpdatableAndDeletable.Update`, reached through `PhysicalObject.Update` only.
        f.even_update = eu;
        f.body.held = held;
        f.body.update_with(&room, water, &mut log);
        let tile = f.pos().tile();
        self.impact_drips(&log);
        let index = self.cx.room_index;
        let c = self.world.creature_mut(self.me);
        c.pos.tile = tile;
        c.pos.room = index;
        self.world.clean_grabbed_by(Entity::Creature(self.me));
        if !self.fr().sticks_respawned {
            self.recreate_sticks();
            self.f().sticks_respawned = true;
        }
    }

    /// The base `Creature.TerrainImpact` of each impact: `PhysicalObject.TerrainImpact`'s
    /// screen movement and noise, then on a first contact drips off a wet ceiling.
    fn impact_drips(&mut self, log: &ImpactLog) {
        let index = self.cx.room_index;
        let ceiling_drips = self
            .world
            .realized(index)
            .map_or(0.0, |r| r.settings.ceiling_drips);
        for LoggedImpact {
            impact,
            pos,
            rad,
            mass,
            screen,
            noise,
        } in &log.impacts
        {
            if let Some(m) = *screen {
                self.world.screen_movement(
                    index,
                    m,
                    &self.cx.camera,
                    self.cx.camera_shake,
                    self.cx.rng,
                );
            }
            if let Some((at, strength)) = *noise {
                let noise = crate::InGameNoise {
                    pos: at,
                    strength,
                    source: Some(Entity::Creature(self.me).into()),
                    interesting: 1.0,
                };
                self.world.in_game_noise(
                    index,
                    noise,
                    self.cx.water,
                    self.cx.player.as_ref(),
                    self.cx.rng,
                );
            }
            if !impact.first_contact {
                continue;
            }
            let drips = crate::WaterDrip::from_impact(
                impact,
                *pos,
                *rad,
                *mass,
                ceiling_drips,
                self.cx.rng,
            );
            for d in drips {
                self.world.add_drip(index, d);
            }
        }
    }

    /// `Creature.RecreateSticksFromAbstract`: each grip stick it holds is grabbed again
    /// (a draw for the chunk), or dropped.
    fn recreate_sticks(&mut self) {
        let sticks = self.world.creature(self.me).sticks.clone();
        for s in sticks.iter().rev() {
            if s.a != self.me {
                continue;
            }
            let chunks = self.world.chunk_count(s.b);
            let chunk = self.cx.rng.range_i32(0, chunks as i32);
            let request = crate::creature::GrabRequest {
                grabber: self.me,
                grabbed: s.b,
                grasp_used: s.grasp,
                chunk_grabbed: usize::try_from(chunk).unwrap_or(0),
                shareability: crate::creature::Shareability::CanNotShare,
                dominance: 0.5,
                override_equally_dominant: true,
                pacifying: true,
            };
            let grabbed = self.world.grab(request, self.cx.rng);
            if !grabbed {
                self.world.deactivate_stick(*s);
            }
        }
    }

    /// The entering branch of `Creature.Update`: the graphics update, the pull of every
    /// connected object toward the tube, `SuckedIntoShortCut` once the fly's own chunk
    /// arrived; a stun cancels.
    fn pull_into_shortcut(&mut self, entrance: (i32, i32)) {
        self.f().short_cut_rad_add += 0.2;
        if self.fr().graphics {
            self.graphics_update();
        }
        let hole = self.cx.room.shortcut_hole_direction(entrance.0, entrance.1);
        let target = Vec2::from(tile_centre(entrance.0, entrance.1))
            + Vec2::new(hole.0 as f32, hole.1 as f32) * -5.0;
        let radius = 10.0f32.max(self.fr().short_cut_rad_add);
        let mut moving = 0;
        for other in self.world.connected_objects(Entity::Creature(self.me)) {
            let Some(chunks) = self.world.body_chunks_mut(other) else {
                continue;
            };
            for c in chunks {
                c.vel *= 0.06;
                if c.pos.dist_less(target, radius) {
                    c.pos = target;
                    c.last_pos = target;
                } else {
                    c.last_pos = c.pos;
                    c.pos += c.pos.direction_to(target) * 4.5;
                    if other == Entity::Creature(self.me) {
                        moving += 1;
                    }
                }
            }
        }
        if moving == 0 {
            self.sucked_into_shortcut(entrance);
        }
        if self.fr().stunned() {
            self.f().entering_shortcut = None;
        }
    }

    /// `Creature.SuckedIntoShortCut` and `ShortcutHandler.SuckInCreature` (its
    /// `Small_NPC_Enter_Shortcut` has no sounds.txt line: no draws): a vessel at the
    /// entrance, and every connected object leaves the room.
    fn sucked_into_shortcut(&mut self, entrance: (i32, i32)) {
        let room = Arc::clone(&self.cx.room);
        if room.tile(entrance.0, entrance.1).terrain != Terrain::ShortcutEntrance {
            return;
        }
        let data = room.shortcut_map().data_at(entrance).cloned();
        if let Some(d) = &data
            && d.kind == rw_assets::ShortcutType::Normal
        {
            self.world.creature_mut(self.me).pos.tile = d.dest;
        }
        // `shortcutData` gives an empty placeholder at the tile itself when none starts.
        let start = data.as_ref().map_or(entrance, |d| d.start);
        self.cx
            .shortcuts
            .transport
            .push(Transport::Creature(CreatureVessel {
                creature: self.me,
                room: self.cx.room_index,
                vessel: ShortcutVessel::new(start),
                entrance_node: -1,
            }));
        self.f().entering_shortcut = None;
        for other in self.world.connected_objects(Entity::Creature(self.me)) {
            match other {
                Entity::Creature(o) => {
                    if let Some(f) = self.world.creature_mut(o).fly_mut() {
                        f.in_shortcut = true;
                        f.room = None;
                    }
                }
                Entity::Object(o) => self.world.object_leaves_room(o),
            }
        }
    }

    /// `Act`.
    fn act(&mut self) {
        self.ai_update();

        if self.fr().ai.stuck() {
            let v = Vec2::from_degrees(self.cx.rng.value() * 360.0) * 2.0;
            self.f().chunk_mut().vel += v;
        }
        match self.fr().mov_mode {
            Some(MovementMode::BatFlight) => self.bat_flight(false),
            Some(MovementMode::SwarmFlight) => self.swarm_flight(),
            Some(MovementMode::Burrow) => self.burrow(),
            Some(MovementMode::Panic) => self.bat_flight(true),
            Some(MovementMode::Hang) => self.hanging(),
            Some(MovementMode::Passive) | None => {}
        }
    }

    /// The Burrow branch of `Act`: sinking into the grass until the spot is 5 px above
    /// ground, then `Burrowed`.
    fn burrow(&mut self) {
        let Some(spot) = self.fr().burrow_or_hang_spot else {
            self.f().burrow_or_hang_spot = None;
            return;
        };
        let jitter = (-2.0f32).lerp(2.0, self.cx.rng.value());
        self.f().chunk_mut().pos = spot + Vec2::new(jitter, 0.0);
        let spot = spot + Vec2::new(0.0, -0.4);
        self.f().burrow_or_hang_spot = Some(spot);
        if self.cx.room.is_solid_at(spot + Vec2::new(0.0, 5.0)) {
            self.burrowed();
        }
    }

    /// `Burrowed`.
    fn burrowed(&mut self) {
        self.ai_burrowed();
        self.play_on_chunk(SoundId::BatDiveIntoGrass);
        let index = self.cx.room_index;
        let me = self.me;
        if let Some(ai) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.flies_room_ai.as_mut())
        {
            ai.flies.retain(|&f| f != me);
            ai.in_hive.push(me);
            self.f().room = None;
        }
    }

    /// `WaterBehavior`.
    fn water_behavior(&mut self) {
        if self.submersion() == 1.0 {
            let d = (-1.0f32).lerp(1.0, self.cx.rng.value()) * 0.1;
            let f = self.f();
            f.flap = (f.flap + d).clamp(0.0, 1.0);
            let v = Vec2::from_degrees(-45.0 + 90.0 * self.cx.rng.value()) * 0.75;
            self.f().chunk_mut().vel += v;
        } else if self.world.creature(self.me).grabbed_by.is_empty()
            || self.world.creature(self.me).grasps[0].is_none()
        {
            self.f().chunk_mut().vel.y += 7.0;
        }
    }

    /// `BatFlight`.
    fn bat_flight(&mut self, panic: bool) {
        if !panic {
            self.steer();
        }
        let gravity = self.fr().body.params.gravity();
        let f = self.f();
        let dir = f.dir;
        let c = f.chunk_mut();
        c.vel *= 0.92;
        c.vel.y += gravity * 0.45;
        c.vel.x += dir.x * 0.6;
        if self.cx.rng.value() < 0.025 {
            let afraid = self.fr().ai.afraid;
            let id = if self.cx.rng.value() < afraid {
                SoundId::BatAfraidFlyingSounds
            } else {
                SoundId::BatIdleFlyingSounds
            };
            self.play_on_chunk(id);
        }
        let f = self.f();
        if panic {
            let d = f.dir;
            f.chunk_mut().vel += d * 0.2;
        }
        f.flap_depth = f.dir.y.inverse_lerp(-1.0, 1.0);
        f.flap += f.flap_speed * if panic { 1.2 } else { 1.0 };
        if f.flap_speed > 0.0 {
            let lift = (1.0 + f.flap_speed).mathf_pow(1.5) * 1.1.lerp(1.7, f.flap_depth);
            f.chunk_mut().vel.y += lift;
            if f.flap > 1.0 {
                f.flap = 1.0;
                f.flap_speed = -0.04 - f.dir.y.inverse_lerp(-1.0, 1.0) * 0.15;
            }
            let contact = f.chunk().contact().x;
            if contact != 0 {
                f.chunk_mut().vel.x = f32::from(contact) * -6.0;
            }
        } else if f.flap_speed < 0.0 && f.flap < 0.0 {
            f.flap = 0.0;
            f.flap_speed = 0.2 + f.dir.y.inverse_lerp(-1.0, 1.0) * 0.8;
            self.play_on_chunk(SoundId::FlyWingFlap);
        }
    }

    /// The steering of a calm `BatFlight`: toward the local goal, off walls ahead, and
    /// away from nearby terrain while `wallAvoidance` lasts.
    fn steer(&mut self) {
        let goal = self.fr().ai.local_goal;
        let pos = self.pos();
        let vel = self.fr().chunk().vel;
        let mut dir = pos.direction_to(goal);
        if self.cx.room.is_solid_at(pos + vel * 3.0) {
            dir = dir.lerp(-vel.normalized(), 0.3);
        }
        let map = Arc::clone(&self.cx.ai_map);
        let mut avoided = false;
        let goal_tile = goal.tile();
        if !self.fr().ai.stuck() && map.terrain_proximity(goal_tile.0, goal_tile.1) > 1 {
            let here = pos.tile();
            let prox = map.terrain_proximity(here.0, here.1);
            let mut away = Vec2::ZERO;
            if prox < 5 {
                for (dx, dy) in EIGHT_DIRECTIONS {
                    if map.terrain_proximity(here.0 + dx, here.1 + dy) < prox {
                        let to = Vec2::middle_of_tile((here.0 - dx, here.1 - dy));
                        away += pos.direction_to(to);
                        avoided = true;
                    }
                }
            }
            if avoided {
                let t = self.fr().wall_avoidance * 0.3;
                dir = dir.slerp(away.normalized(), t);
            }
        }
        let f = self.f();
        f.dir = dir;
        if avoided {
            f.wall_avoidance -= 0.05;
        } else {
            f.wall_avoidance += 0.025;
        }
        f.wall_avoidance = f.wall_avoidance.clamp(0.0, 1.0);
    }

    /// `SwarmFlight`, with the hover loop it keeps alive.
    fn swarm_flight(&mut self) {
        let flock = self.world.flocks[self.fr().ai.flock];
        let goal = self.fr().ai.local_goal;
        let phase = flock.sin_cycle + goal.y / flock.swarm_flight_vertical_displace;
        let sway = (f64::from(phase * 2.0 * std::f32::consts::PI)).sin() as f32
            * flock.swarm_flight_sin_ampl;
        let gravity = self.fr().body.params.gravity();
        let pos = self.pos();
        let f = self.f();
        f.dir = pos.direction_to(goal + Vec2::new(sway, 0.0));
        let dir = f.dir;
        let c = f.chunk_mut();
        c.vel *= 0.89;
        c.vel.y += gravity * 0.9;
        c.vel += dir * 0.6;
        let live = self.fr().hover.filter(|h| !h.slated);
        match live {
            None => self.start_hover(),
            Some(_) => {
                let vy = self.fr().chunk().vel.y;
                let pitch = 0.9 + 0.2 * vy.inverse_lerp(-4.0, 4.0);
                if let Some(h) = &mut self.f().hover {
                    h.alive = true;
                    let id = h.sound;
                    self.cx.sounds.update_emitter(id, 1.0, pitch);
                }
            }
        }
        let f = self.f();
        f.flap = 1.0;
        f.flap_depth = 0.5;
        f.flap_speed = 1.0;
        let contact = f.chunk().contact().x;
        if contact != 0 {
            f.chunk_mut().vel.x = f32::from(contact) * -6.0;
        }
    }

    /// `Room.PlaySound(Bat_Hover_Fly_LOOP, mainBodyChunk, loop)`: a new emitter in the
    /// room's update list.
    fn start_hover(&mut self) {
        let pos = self.pos();
        let before = self.cx.sounds.events().len();
        let sound = self.cx.sounds.play_on_object(
            SoundId::BatHoverFlyLoop,
            pos,
            true,
            1.0,
            1.0,
            self.cx.rng,
        );
        let playing =
            self.cx.room_index == self.cx.camera.room && self.cx.sounds.events().len() > before;
        let update_order = self.world.next_update_seq();
        let room = self.cx.room_index;
        self.f().hover = Some(HoverEmitter {
            sound,
            room,
            update_order,
            alive: true,
            slated: false,
            still_playing: true,
            last_playing_tick: self.cx.tick,
            playing,
        });
    }

    /// `Hanging`.
    fn hanging(&mut self) {
        if self.world.creature(self.me).grasps[0].is_none()
            && let Some(spot) = self.fr().burrow_or_hang_spot
        {
            let pos = self.pos();
            let dist = pos.distance(spot);
            if dist > 8.0 {
                let v = pos.direction_to(spot) * (8.0 - dist);
                let c = self.f().chunk_mut();
                c.pos -= v;
                c.vel -= v;
            }
        }
        self.f().flap_depth = 1.0;
        if self.fr().chain_behavior_variation == 0 && self.cx.rng.value() < 0.000_666_666_66 {
            self.f().chain_behavior_variation = self.cx.rng.range_i32(1, 5);
        } else if self.cx.rng.value() < 1.0 / 160.0 {
            self.f().chain_behavior_variation = 0;
        }
        match self.fr().chain_behavior_variation {
            1 => {
                let sin = self.world.flocks[self.fr().ai.flock].sin_cycle;
                let push = f64::from(sin * 2.0 * std::f32::consts::PI).sin() as f32;
                let flap = self.cx.rng.value();
                let f = self.f();
                f.chunk_mut().vel.x += push;
                f.flap_depth = 0.1;
                f.flap = flap;
            }
            2 => {
                let a = self.cx.rng.value();
                let b = self.cx.rng.value();
                let f = self.f();
                f.flap_depth = 0.3;
                f.flap = (f.flap + a * b * 0.1).clamp(0.0, 1.0);
            }
            3 => {
                let v = Vec2::from_degrees(self.cx.rng.value() * 360.0) * 0.1;
                let f = self.f();
                f.chunk_mut().vel += v;
                f.flap_depth = 0.3;
                f.flap = 1.0;
            }
            4 => {
                let d = Vec2::from_degrees(self.cx.rng.value() * 360.0);
                let a = self.cx.rng.value();
                let b = self.cx.rng.value();
                self.f().chunk_mut().vel += d * (a * b * 0.2);
            }
            _ => {}
        }
    }

    /// The graphics module's `Update` (the observer runs `FlyGraphics.Update`).
    pub(crate) fn graphics_update(&mut self) {
        let view = self.world.fly_view(self.me, self.cx.water);
        let cameras = self.cx.room.cameras().len();
        self.cx.observer.fly_updated(
            &view,
            &self.cx.room,
            cameras,
            &self.cx.camera_room,
            &self.cx.camera,
            self.cx.rng,
        );
    }

    /// `FlyAI.Burrowed`.
    fn ai_burrowed(&mut self) {
        self.change_behavior(FlyBehavior::Idle);
        let ai = &mut self.f().ai;
        ai.drop_status = Some(super::DropStatus::HaventDropped);
        ai.afraid = 0.0;
        ai.threats.clear();
    }
}

impl World {
    /// `Fly.PlayerAutoGrabable`: conscious, out of a shortcut's delay, and held by nothing
    /// but flies.
    #[must_use]
    pub fn fly_auto_grabable(&self, id: CreatureId) -> bool {
        let c = self.creature(id);
        let Some(f) = c.fly() else { return false };
        let free = c
            .grabbed_by
            .first()
            .is_none_or(|g| self.creature(g.grabber).fly().is_some());
        f.conscious() && free && f.shortcut_delay < 1
    }

    /// `Fly.Die`: `surfaceFriction` 0.3, then `Creature.Die`.
    pub(crate) fn fly_die(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        self.fly_at(id).body.params.surface_friction = 0.3;
        self.creature_die(id, rng);
    }

    /// The realized fly `id` (an invariant of the callers).
    pub(crate) fn fly_at(&mut self, id: CreatureId) -> &mut Fly {
        self.creature_mut(id).fly_mut().expect("a realized fly")
    }

    pub(crate) fn fly_ref(&self, id: CreatureId) -> &Fly {
        self.creature(id).fly().expect("a realized fly")
    }

    /// Chunk `i` of a realized object with a body in the world (a fly, a rock or a
    /// spear).
    pub(crate) fn chunk_pos(&self, e: Entity, i: usize) -> Option<Vec2> {
        self.body_chunks(e).and_then(|c| c.get(i)).map(|c| c.pos)
    }

    pub(crate) fn chunk_mut(&mut self, e: Entity, i: usize) -> Option<&mut crate::BodyChunk> {
        self.body_chunks_mut(e).and_then(|c| c.get_mut(i))
    }

    pub(crate) fn body_chunks(&self, e: Entity) -> Option<&[crate::BodyChunk]> {
        match e {
            Entity::Creature(id) => match &self.creature(id).realized {
                Some(RealizedCreature::Fly(f)) => Some(&f.body.chunks[..]),
                Some(RealizedCreature::Lizard(l)) => Some(&l.body.chunks[..]),
                _ => None,
            },
            Entity::Object(id) => self
                .object(id)
                .realized
                .as_deref()
                .map(|w| &w.body.chunks[..]),
        }
    }

    pub(crate) fn body_chunks_mut(&mut self, e: Entity) -> Option<&mut [crate::BodyChunk]> {
        match e {
            Entity::Creature(id) => match &mut self.creature_mut(id).realized {
                Some(RealizedCreature::Fly(f)) => Some(&mut f.body.chunks[..]),
                Some(RealizedCreature::Lizard(l)) => Some(&mut l.body.chunks[..]),
                _ => None,
            },
            Entity::Object(id) => self
                .object_mut(id)
                .realized
                .as_deref_mut()
                .map(|w| &mut w.body.chunks[..]),
        }
    }

    /// `bodyChunks.Length` of a realized object.
    pub(crate) fn chunk_count(&self, e: Entity) -> usize {
        match e {
            Entity::Creature(id) => match &self.creature(id).realized {
                Some(RealizedCreature::Fly(f)) => f.body.chunks.len(),
                Some(RealizedCreature::Player) => 2,
                Some(RealizedCreature::Overseer(_)) => 1,
                Some(RealizedCreature::Lizard(l)) => l.chunks().len(),
                None => 0,
            },
            Entity::Object(id) => self
                .object(id)
                .realized
                .as_ref()
                .map_or(0, |w| w.body.chunks.len()),
        }
    }

    /// `abstractCreature.Destroy()`: its sticks go, and it is slated for deletion.
    pub(crate) fn destroy_abstract(&mut self, id: CreatureId) {
        let sticks = self.creature(id).sticks.clone();
        for s in sticks.iter().rev() {
            self.deactivate_stick(*s);
        }
        for s in self.spear_sticks_of(Entity::Creature(id)) {
            self.deactivate_spear_stick(s.serial);
        }
        self.creature_mut(id).slated_for_deletion = true;
    }

    /// `NextInChain`: the fly whose grasp holds this one, if the first fly among its
    /// grabbers holds it with grasp 0.
    pub(crate) fn next_in_chain(&self, id: CreatureId) -> Option<CreatureId> {
        let g = self
            .creature(id)
            .grabbed_by
            .iter()
            .find(|g| self.creature(g.grabber).fly().is_some())?;
        let holds = self.creature(g.grabber).grasps.first().copied().flatten();
        holds
            .filter(|h| h.grabbed == Entity::Creature(id))
            .map(|_| g.grabber)
    }

    /// `FirstInChain`.
    pub(crate) fn first_in_chain(&self, id: CreatureId) -> CreatureId {
        let mut fly = id;
        while let Some(o) = self
            .creature(fly)
            .grasps
            .first()
            .copied()
            .flatten()
            .and_then(|g| g.grabbed_creature())
            && self.creature(o).fly().is_some()
        {
            fly = o;
        }
        fly
    }

    /// `LastInChain`.
    pub(crate) fn last_in_chain(&self, id: CreatureId) -> CreatureId {
        let mut fly = id;
        while let Some(next) = self.next_in_chain(fly) {
            fly = next;
        }
        fly
    }

    /// `CheckChainForLoops`: a chain that comes back to `id` lets go of it.
    pub(crate) fn check_chain_for_loops(&mut self, id: CreatureId) -> bool {
        let mut fly = self.next_in_chain(id);
        while let Some(f) = fly {
            fly = self.next_in_chain(f);
            if fly == Some(id) {
                if let Some(next) = self.next_in_chain(id) {
                    self.release_grasp(next, 0);
                }
                return true;
            }
        }
        false
    }

    /// What `FlyGraphics` reads of realized fly `id`, if it is one.
    #[must_use]
    pub fn realized_fly_view(&self, id: CreatureId) -> Option<FlyView<'_>> {
        self.creature(id).fly()?;
        let room = self.creature(id).pos.room;
        let water = self.realized(room).and_then(|r| r.water.as_ref());
        Some(self.fly_view(id, water))
    }

    /// What `FlyGraphics` reads of fly `id`.
    pub(crate) fn fly_view(&self, id: CreatureId, water: Option<&Water>) -> FlyView<'_> {
        let c = self.creature(id);
        let fly = c.fly().expect("a realized fly");
        let grasp_pos = c
            .grasps
            .first()
            .copied()
            .flatten()
            .and_then(|g| self.chunk_pos(g.grabbed, 0));
        let grabber = c
            .grabbed_by
            .first()
            .and_then(|g| match &self.creature(g.grabber).realized {
                Some(RealizedCreature::Fly(f)) => Some((f.pos(), true)),
                Some(RealizedCreature::Player) => self.player_body.map(|p| (p.chunks[0], false)),
                _ => None,
            });
        FlyView {
            id: c.id,
            fly,
            grasp_pos,
            grabber,
            submersion: fly.chunk().submersion(water),
        }
    }
}

impl WorldCoordinate {
    /// `Room.IsPositionInsideBoundries(Tile)`.
    pub(crate) fn inside(self, room: &Room) -> bool {
        room.contains(self.tile.0, self.tile.1)
    }
}
