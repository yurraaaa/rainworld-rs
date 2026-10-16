//! Realized rocks and spears: `Weapon` (81224) over `PlayerCarryableItem` (79261) and
//! `PhysicalObject` (78561), with `Rock` (246997) and `Spear` (270804) on top. Ported:
//! the constructors, `PlaceInRoom`, `ChangeMode`, and `Update` in the `Free`, `Carried`
//! and `StuckInWall` modes, a wall-stuck spear's poles and the items' sound loops.
//! Thrown weapons (step 4), creature hits (step 5) and the grasps (step 3) are not
//! ported yet. Behaviour: `rainworld-re/notes/items.md` sections 6-8.

use std::sync::Arc;

use rw_assets::{AbstractObjectType, Room, SoundTable, TileFlags};
use rw_random::UnityRandom;

use super::{AbstractObject, AbstractSpear, ObjectData};
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{BodyChunk, LoopEmitter, PhysicalObject, PhysicsParams, SoundId, Sounds, Vec2, Water};

/// `EffectiveRoomGravity`: no gravity effects are ported.
const ROOM_GRAVITY: f32 = 1.0;

/// The physics scalars `Rock` and `Spear` set in their constructors (the same for both).
pub const WEAPON_PARAMS: PhysicsParams = PhysicsParams {
    gravity: 0.9,
    air_friction: 0.999,
    bounce: 0.4,
    surface_friction: 0.4,
    impact_threshold: 1.0,
    buoyancy: 0.4,
    water_friction: 0.98,
    water_retardation_immunity: 0.0,
};

/// `Weapon.Mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponMode {
    Free,
    Frozen,
    Thrown,
    Carried,
    StuckInCreature,
    StuckInWall,
    OnBack,
}

impl WeaponMode {
    /// The game's spelling.
    #[must_use]
    pub const fn value(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Frozen => "Frozen",
            Self::Thrown => "Thrown",
            Self::Carried => "Carried",
            Self::StuckInCreature => "StuckInCreature",
            Self::StuckInWall => "StuckInWall",
            Self::OnBack => "OnBack",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::Free,
            Self::Frozen,
            Self::Thrown,
            Self::Carried,
            Self::StuckInCreature,
            Self::StuckInWall,
            Self::OnBack,
        ]
        .into_iter()
        .find(|m| m.value() == value)
    }
}

/// `Spear`'s own fields (the More Slugcats needle and bug-spear ones are left out but the
/// needle type, whose constructor draw is made).
#[derive(Debug, Clone, PartialEq)]
pub struct SpearParts {
    pub spinning: bool,
    pub pivot_at_tip: bool,
    pub last_pivot_at_tip: bool,
    pub stuck_in_wall: Option<Vec2>,
    pub add_poles: bool,
    /// `wasHorizontalBeam`: the pole flags of the stuck tile and its two neighbours
    /// before the spear wrote them.
    pub was_horizontal_beam: [bool; 3],
    pub has_horizontal_beam_state: bool,
    pub still_counter: i32,
    pub spear_damage_bonus: f32,
    pub always_stick_in_walls: bool,
    /// `spearmasterNeedleType`, drawn in the constructor (More Slugcats reads it).
    pub needle_type: i32,
    pub needle_fade_counter: i32,
}

/// What kind of weapon.
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponKind {
    Rock,
    /// A `Spear`. An `ExplosiveSpear` is realized as one too (docs/DIVERGENCES.md D14).
    Spear(Box<SpearParts>),
}

/// `ChunkDynamicSoundLoop` (138330): a loop on the item's chunk that starts when `sound`
/// changes or the volume rises from 0, and stops when the volume falls to 0.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkDynamicSoundLoop {
    /// `sound`; `None` is `SoundID.None`.
    pub sound: Option<SoundId>,
    last_sound: Option<SoundId>,
    v: f32,
    last_vol: f32,
    p: f32,
    pub emitter: Option<LoopEmitter>,
}

impl Default for ChunkDynamicSoundLoop {
    fn default() -> Self {
        Self {
            sound: None,
            last_sound: None,
            v: 1.0,
            last_vol: 1.0,
            p: 1.0,
            emitter: None,
        }
    }
}

/// What an item's update reads and writes of its room and the game.
pub struct WeaponTick<'a> {
    pub room_index: i32,
    /// The realized room; a wall-stuck spear writes its poles into it.
    pub room: &'a mut Arc<Room>,
    pub water: Option<&'a Water>,
    /// `room.readyForAI`.
    pub ready_for_ai: bool,
    pub rng: &'a mut UnityRandom,
    /// The room's sound sink (the camera's, or a silent one).
    pub sounds: &'a mut Sounds,
    /// `soundLoader.workingTriggers`: the camera's sound table.
    pub triggers: &'a SoundTable,
    pub camera_room: i32,
    pub tick: u64,
    /// `game.evenUpdate`.
    pub eu: bool,
    /// The world's `updateList` position counter (a loop's emitter joins the room).
    pub update_seq: &'a mut u64,
}

/// What `ChangeMode` and `PlaceInRoom` read.
pub struct ModeChange<'a> {
    pub room: &'a Room,
    pub rng: &'a mut UnityRandom,
    pub update_seq: &'a mut u64,
}

/// A realized `Rock` or `Spear`.
#[derive(Debug, Clone, PartialEq)]
pub struct Weapon {
    pub kind: WeaponKind,
    /// One chunk, rad 5, mass 0.07.
    pub body: PhysicalObject,
    /// `room`: the realized room it is in.
    pub room: i32,
    /// Its place in the room's `updateList`.
    pub update_order: u64,
    pub collision_layer: i32,
    /// Its place in `physicalObjects[collisionLayer]` (joining a layer appends).
    pub layer_order: u64,
    /// `UpdatableAndDeletable.evenUpdate`.
    pub even_update: bool,
    pub mode: WeaponMode,
    pub last_mode: WeaponMode,
    pub rotation: Vec2,
    pub last_rotation: Vec2,
    /// Degrees per tick.
    pub rotation_speed: f32,
    pub set_rotation: Option<Vec2>,
    pub tail_pos: Vec2,
    pub throw_dir: (i32, i32),
    pub vibrate: i32,
    /// `exitThrownModeSpeed` (30).
    pub exit_thrown_mode_speed: f32,
    pub override_exit_thrown_speed: f32,
    pub throw_mode_frames: i32,
    pub floor_bounce_frames: i32,
    pub closest_crit_dist: f32,
    /// `inFrontOfObjects`: -1 unset, 1 in `Items`, 0 in `Background`.
    pub in_front_of_objects: i32,
    pub blink: i32,
    pub forbidden_to_player: i32,
    pub last_outside_terrain_pos: Option<Vec2>,
    /// `firstChunk.loudness`: Rock 9, Spear 7 (for the noise of terrain impacts).
    pub loudness: f32,
    pub sound_loop: ChunkDynamicSoundLoop,
}

impl Weapon {
    /// `AbstractPhysicalObject.Realize` for a rock or a spear: the `Weapon` constructor's
    /// rotation draw, then the `Spear` constructor's needle-type draw. `None` for other
    /// kinds.
    #[must_use]
    pub fn realize(object: &AbstractObject, rng: &mut UnityRandom) -> Option<Self> {
        if !matches!(
            object.kind,
            AbstractObjectType::Rock | AbstractObjectType::Spear
        ) {
            return None;
        }
        let rotation = Vec2::from_degrees(rng.value() * 360.0);
        let (kind, loudness) = match object.data {
            ObjectData::Spear(_) => {
                let parts = SpearParts {
                    spinning: false,
                    pivot_at_tip: false,
                    last_pivot_at_tip: false,
                    stuck_in_wall: None,
                    add_poles: false,
                    was_horizontal_beam: [false; 3],
                    has_horizontal_beam_state: false,
                    still_counter: 0,
                    spear_damage_bonus: 1.0,
                    always_stick_in_walls: false,
                    needle_type: rng.range_i32(0, 3),
                    needle_fade_counter: 400,
                };
                (WeaponKind::Spear(Box::new(parts)), 7.0)
            }
            ObjectData::Plain => (WeaponKind::Rock, 9.0),
        };
        Some(Self {
            kind,
            body: PhysicalObject::new(vec![BodyChunk::new(Vec2::ZERO, 5.0, 0.07)], WEAPON_PARAMS),
            room: -1,
            update_order: 0,
            collision_layer: 2,
            layer_order: 0,
            even_update: false,
            mode: WeaponMode::Free,
            last_mode: WeaponMode::Free,
            rotation,
            last_rotation: rotation,
            rotation_speed: 0.0,
            set_rotation: None,
            tail_pos: Vec2::ZERO,
            throw_dir: (0, 0),
            vibrate: 0,
            exit_thrown_mode_speed: 30.0,
            override_exit_thrown_speed: 0.0,
            throw_mode_frames: -1,
            floor_bounce_frames: 0,
            closest_crit_dist: f32::MAX,
            in_front_of_objects: -1,
            blink: 0,
            forbidden_to_player: 0,
            last_outside_terrain_pos: None,
            loudness,
            sound_loop: ChunkDynamicSoundLoop::default(),
        })
    }

    #[must_use]
    pub fn chunk(&self) -> &BodyChunk {
        &self.body.chunks[0]
    }

    pub fn chunk_mut(&mut self) -> &mut BodyChunk {
        &mut self.body.chunks[0]
    }

    #[must_use]
    pub fn spear(&self) -> Option<&SpearParts> {
        match &self.kind {
            WeaponKind::Spear(s) => Some(s),
            WeaponKind::Rock => None,
        }
    }

    fn spear_mut(&mut self) -> Option<&mut SpearParts> {
        match &mut self.kind {
            WeaponKind::Spear(s) => Some(s),
            WeaponKind::Rock => None,
        }
    }

    /// `PlaceInRoom` (`Weapon`, then `Spear`'s): it joins room `room_index`'s update
    /// list and collision layer, sits in the middle of its tile and spins
    /// (`SetRandomSpin`); a spear saved stuck in a wall is stuck again.
    pub fn place_in_room(
        &mut self,
        object: &mut AbstractObject,
        room_index: i32,
        cx: &mut ModeChange<'_>,
    ) {
        self.room = room_index;
        *cx.update_seq += 1;
        self.update_order = *cx.update_seq;
        self.layer_order = *cx.update_seq;
        let at = Vec2::middle_of_tile(object.pos.tile);
        self.chunk_mut().hard_set_position(at);
        self.in_front_of_objects = -1;
        self.last_outside_terrain_pos = None;
        self.set_random_spin(cx.rng);
        if object.stuck_in_wall()
            && let Some(s) = self.spear_mut()
        {
            s.stuck_in_wall = Some(at);
            self.change_mode(WeaponMode::StuckInWall, object, cx);
        }
    }

    /// `SetRandomSpin`: a rock spins at up to 100 degrees a tick either way (one draw);
    /// a spear at 50 to 150 in a random direction (two) and is `spinning`.
    pub fn set_random_spin(&mut self, rng: &mut UnityRandom) {
        let gravity = 0.05f32.lerp(1.0, ROOM_GRAVITY);
        match &mut self.kind {
            WeaponKind::Rock => {
                self.rotation_speed = (-100.0f32).lerp(100.0, rng.value()) * gravity;
            }
            WeaponKind::Spear(s) => {
                let sign = if rng.value() < 0.5 { -1.0 } else { 1.0 };
                self.rotation_speed = sign * 50.0f32.lerp(150.0, rng.value()) * gravity;
                s.spinning = true;
            }
        }
    }

    /// `ChangeMode` (`Spear`'s part first, which runs even when the mode stays).
    pub fn change_mode(
        &mut self,
        new: WeaponMode,
        object: &mut AbstractObject,
        cx: &mut ModeChange<'_>,
    ) {
        let mode = self.mode;
        let throw_y = self.throw_dir.1;
        if let WeaponKind::Spear(s) = &mut self.kind {
            if new != WeaponMode::Thrown {
                s.spear_damage_bonus = 1.0;
            }
            if new == WeaponMode::StuckInWall
                && let ObjectData::Spear(abs) = &mut object.data
            {
                if abs.stuck_in_wall_cycles == 0 {
                    let sign = if throw_y == 0 { 1 } else { -1 };
                    abs.stuck_in_wall_cycles = cx.rng.range_i32(3, 7) * sign;
                }
                let stuck = s.stuck_in_wall.unwrap_or_default();
                let horizontal = abs.stuck_in_wall_cycles >= 0;
                for i in [-1.0f32, 1.0] {
                    let side = if horizontal {
                        Vec2::new(20.0 * i, 0.0)
                    } else {
                        Vec2::new(0.0, 20.0 * i)
                    };
                    if !cx.room.tile_at_pos(stuck + side).is_solid() {
                        self.set_rotation = Some(if horizontal {
                            Vec2::new(-i, 0.0)
                        } else {
                            Vec2::new(0.0, -i)
                        });
                        if !abs.explosive {
                            s.add_poles = true;
                        }
                        break;
                    }
                }
                if let Some(r) = self.set_rotation {
                    s.stuck_in_wall = Some(stuck.tile_middle() - r * 5.0);
                }
                if abs.explosive {
                    abs.stuck_in_wall_cycles = 0;
                }
                self.rotation_speed = 0.0;
            }
            if new != WeaponMode::Free {
                s.spinning = false;
            }
            if new != WeaponMode::StuckInWall && new != WeaponMode::StuckInCreature {
                s.stuck_in_wall = None;
            }
        }
        if new == mode {
            return;
        }
        let layer = if matches!(new, WeaponMode::Thrown | WeaponMode::StuckInWall) {
            0
        } else {
            2
        };
        if layer != self.collision_layer {
            self.collision_layer = layer;
            *cx.update_seq += 1;
            self.layer_order = *cx.update_seq;
        }
        if new != WeaponMode::Thrown {
            self.throw_mode_frames = -1;
        }
        let c = self.chunk_mut();
        c.collide_with_objects = new != WeaponMode::Carried;
        c.collide_with_terrain = matches!(new, WeaponMode::Free | WeaponMode::Thrown);
        c.go_through_floors = true;
        if new == WeaponMode::Free {
            self.set_random_spin(cx.rng);
        }
        self.closest_crit_dist = f32::MAX;
        self.mode = new;
    }

    /// `Update` of a rock or spear in its slot of the room's update list.
    pub fn update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        self.weapon_update(object, cx);
        if self.spear().is_some() {
            self.spear_update(object, cx);
        } else {
            self.rock_update(cx);
        }
    }

    /// `Weapon.Update` up to its `base.Update`, then that.
    fn weapon_update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        self.last_mode = self.mode;
        if self.vibrate > 0 {
            self.vibrate -= 1;
        }
        // `floorBounceFrames` is only set by belly-slide throws (step 4).
        self.last_rotation = self.rotation;
        match self.set_rotation.take() {
            Some(r) => self.rotation = r,
            None => {
                self.rotation =
                    Vec2::from_degrees(self.rotation.to_degrees() + self.rotation_speed);
            }
        }
        self.body.params.water_retardation_immunity = if self.mode == WeaponMode::Thrown {
            0.9
        } else {
            0.0
        };
        self.tail_pos = self.chunk().last_pos;
        match self.mode {
            WeaponMode::Carried => {
                if !self.body.held {
                    let room = Arc::clone(cx.room);
                    let mut change = ModeChange {
                        room: &room,
                        rng: cx.rng,
                        update_seq: cx.update_seq,
                    };
                    self.change_mode(WeaponMode::Free, object, &mut change);
                }
            }
            WeaponMode::Free
                if self.chunk().submersion(cx.water) > 0.0 && self.rotation_speed.abs() > 5.0 =>
            {
                self.rotation_speed *= 0.7;
            }
            // Thrown weapons are step 4.
            _ => {}
        }
        self.carryable_update(object, cx);
    }

    /// `PlayerCarryableItem.Update` with `PhysicalObject.Update` first.
    fn carryable_update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        // `WeatherInertia` draws only in blizzard rooms (Saint), none in the base game.
        self.body.update_with(cx.room, cx.water, &mut ());
        object.pos.tile = self.chunk().pos.tile();
        object.pos.room = cx.room_index;
        self.even_update = cx.eu;
        if self.blink > 0 {
            self.blink -= 1;
        }
        if self.forbidden_to_player > 0 {
            self.forbidden_to_player -= 1;
        }
        let c = &self.body.chunks[0];
        if cx.room.tile_at_pos(c.pos).is_solid() {
            let stuck_in_creature =
                self.spear().is_some() && self.mode == WeaponMode::StuckInCreature;
            if !self.body.held
                && c.collide_with_terrain
                && cx.room.tile_at_pos(c.last_pos).is_solid()
                && cx.room.tile_at_pos(c.last_last_pos).is_solid()
                && let Some(outside) = self.last_outside_terrain_pos
                && !stuck_in_creature
            {
                log::info!("Resetting {} to outside terrain", object.kind.value());
                let dir = Vec2::random_unit(cx.rng);
                let to = outside + dir * cx.rng.value();
                let c = self.chunk_mut();
                c.hard_set_position(to);
                c.vel = c.vel / 2.0;
            }
        } else {
            self.last_outside_terrain_pos = Some(c.pos);
        }
        let held = self.body.held;
        self.chunk_mut().collide_with_objects = !held;
    }

    /// The rest of `Rock.Update`: the skidding loop, and spin from rolling.
    fn rock_update(&mut self, cx: &mut WeaponTick<'_>) {
        self.sound_loop.sound = None;
        let c = self.chunk();
        let (speed, contact, vx, pos) = (c.vel.length(), c.contact(), c.vel.x, c.pos);
        if speed > 5.0 {
            self.sound_loop.sound = Some(if contact.y < 0 {
                SoundId::RockSkiddingOnGroundLoop
            } else {
                SoundId::RockThroughAirLoop
            });
            self.sound_loop.set_volume(speed.inverse_lerp(5.0, 15.0));
        }
        self.sound_loop.update(Some(self.room), pos, cx);
        if contact.y != 0 {
            self.rotation_speed = (self.rotation_speed * 2.0 + vx * 5.0) / 3.0;
        }
    }

    /// The rest of `Spear.Update`: its sound loop, the pivot, the poles of a wall-stuck
    /// spear, a free spear landing (it stops spinning on the ground, with one draw and
    /// `Spear_Stick_In_Ground`) and a stuck one held in place.
    fn spear_update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        let mode = self.mode;
        let Some(s) = self.spear_mut() else { return };
        if s.needle_fade_counter > 0 {
            s.needle_fade_counter -= 1;
        }
        self.sound_loop.sound = None;
        let c = self.chunk();
        let (speed, pos) = (c.vel.length(), c.pos);
        if speed > 5.0 {
            match mode {
                WeaponMode::Thrown => {
                    self.sound_loop.sound = Some(SoundId::SpearThrownThroughAirLoop);
                }
                WeaponMode::Free => {
                    self.sound_loop.sound = Some(SoundId::SpearSpinningThroughAirLoop);
                }
                _ => {}
            }
            self.sound_loop.set_volume(speed.inverse_lerp(5.0, 15.0));
        }
        self.sound_loop.update(Some(self.room), pos, cx);
        let Some(s) = self.spear_mut() else { return };
        s.last_pivot_at_tip = s.pivot_at_tip;
        s.pivot_at_tip = matches!(mode, WeaponMode::Thrown | WeaponMode::StuckInCreature);
        if s.add_poles && cx.ready_for_ai {
            let horizontal = object.spear().is_some_and(|a| a.stuck_in_wall_cycles >= 0);
            s.add_poles(Arc::make_mut(cx.room), horizontal);
        }
        match mode {
            WeaponMode::Free => self.free_spear(cx),
            WeaponMode::StuckInWall => {
                let at = self
                    .spear()
                    .and_then(|s| s.stuck_in_wall)
                    .unwrap_or_default();
                let c = self.chunk_mut();
                c.pos = at;
                c.vel *= 0.0;
            }
            // Thrown and stuck in a creature are steps 4 and 5.
            _ => {}
        }
    }

    /// `Spear.Update`'s `Free` branch.
    fn free_spear(&mut self, cx: &mut WeaponTick<'_>) {
        let c = self.chunk();
        let (pos, last, contact_y) = (c.pos, c.last_pos, c.contact().y);
        let Some(s) = self.spear_mut() else { return };
        if s.spinning {
            if pos.dist_less(last, 4.0 * ROOM_GRAVITY) {
                s.still_counter += 1;
            } else {
                s.still_counter = 0;
            }
            if contact_y < 0 || s.still_counter > 20 {
                s.spinning = false;
                self.rotation_speed = 0.0;
                self.rotation = Vec2::from_degrees((-50.0f32).lerp(50.0, cx.rng.value()) + 180.0);
                let c = self.chunk_mut();
                c.vel *= 0.0;
                cx.sounds
                    .play_on_object(SoundId::SpearStickInGround, pos, false, 1.0, 1.0, cx.rng);
            }
        } else if !last.dist_less(pos, 6.0) {
            self.set_random_spin(cx.rng);
        }
    }
}

impl SpearParts {
    /// `Spear.Update`'s `addPoles` block: the stuck tile and its open neighbours along
    /// the spear become horizontal (or, stuck vertically, vertical) poles; what they were
    /// is kept for `resetHorizontalBeamState`.
    fn add_poles(&mut self, room: &mut Room, horizontal: bool) {
        let stuck = self.stuck_in_wall.unwrap_or_default();
        let flag = if horizontal {
            TileFlags::HORIZONTAL_POLE
        } else {
            TileFlags::VERTICAL_POLE
        };
        let (x, y) = stuck.tile();
        self.was_horizontal_beam[1] = room.tile(x, y).flags.contains(flag);
        room.set_pole(x, y, flag, true);
        for k in [-1, 1] {
            let (nx, ny) = if horizontal { (x + k, y) } else { (x, y + k) };
            let tile = room.tile(nx, ny);
            self.was_horizontal_beam[usize::try_from(k + 1).unwrap_or(0)] =
                tile.flags.contains(flag);
            if !tile.is_solid() {
                room.set_pole(nx, ny, flag, true);
            }
        }
        self.add_poles = false;
        self.has_horizontal_beam_state = true;
    }

    /// `resetHorizontalBeamState` (called when the spear is pulled out, step 3): the
    /// tiles get their pole flags back and `stuckInWallCycles` becomes 0.
    pub fn reset_horizontal_beam_state(&mut self, room: &mut Room, spear: &mut AbstractSpear) {
        let cycles = spear.stuck_in_wall_cycles;
        if let Some(stuck) = self.stuck_in_wall {
            let flag = if cycles > 0 {
                TileFlags::HORIZONTAL_POLE
            } else {
                TileFlags::VERTICAL_POLE
            };
            let (x, y) = stuck.tile();
            room.set_pole(x, y, flag, self.was_horizontal_beam[1]);
            for k in [-1, 1] {
                let (nx, ny) = if cycles > 0 { (x + k, y) } else { (x, y + k) };
                if !room.tile(nx, ny).is_solid() {
                    let was = self.was_horizontal_beam[usize::try_from(k + 1).unwrap_or(0)];
                    room.set_pole(nx, ny, flag, was);
                }
            }
        }
        self.has_horizontal_beam_state = false;
        spear.stuck_in_wall_cycles = 0;
    }
}

impl ChunkDynamicSoundLoop {
    /// The `Volume` setter.
    pub fn set_volume(&mut self, v: f32) {
        self.last_vol = self.v;
        self.v = v;
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.v
    }

    /// `DynamicSoundLoop.Update` for an owner in `owner_room` whose chunk is at `pos`.
    pub fn update(&mut self, owner_room: Option<i32>, pos: Vec2, cx: &mut WeaponTick<'_>) {
        if self.emitter.is_some_and(|e| e.slated) {
            self.emitter = None;
        }
        if self.sound != self.last_sound {
            self.stop(cx.sounds);
            if self.sound.is_some() && self.v > 0.0 && owner_room.is_some() {
                self.start(pos, cx);
            }
            self.last_sound = self.sound;
        }
        if self.v == 0.0 && self.last_vol > 0.0 {
            self.stop(cx.sounds);
        } else if self.v > 0.0
            && self.last_vol == 0.0
            && self.sound.is_some()
            && owner_room.is_some()
        {
            self.start(pos, cx);
        }
        match &mut self.emitter {
            Some(e) if owner_room == Some(e.room) => {
                e.alive = true;
                cx.sounds.update_emitter(e.sound, self.v, self.p);
            }
            _ => self.last_sound = None,
        }
    }

    /// `Start`: a sound with a working trigger plays on the chunk as a loop
    /// (`Room.PlaySound(sound, chunk, loop: true, v, p)`), its emitter a new room object.
    fn start(&mut self, pos: Vec2, cx: &mut WeaponTick<'_>) {
        let Some(sound) = self.sound else { return };
        if cx.triggers.trigger(sound.name()).is_none() {
            return;
        }
        let before = cx.sounds.events().len();
        let id = cx
            .sounds
            .play_on_object(sound, pos, true, self.v, self.p, cx.rng);
        let playing = cx.room_index == cx.camera_room && cx.sounds.events().len() > before;
        *cx.update_seq += 1;
        self.emitter = Some(LoopEmitter {
            sound: id,
            room: cx.room_index,
            update_order: *cx.update_seq,
            alive: true,
            slated: false,
            still_playing: true,
            last_playing_tick: cx.tick,
            playing,
        });
    }

    /// `Stop`: the emitter is let go (`alive = false`); the port stops its sound at once.
    fn stop(&mut self, sounds: &mut Sounds) {
        if let Some(e) = self.emitter.take() {
            sounds.stop_emitter(e.sound);
        }
    }
}

#[cfg(test)]
#[path = "weapon_tests.rs"]
mod tests;
