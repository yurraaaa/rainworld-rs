//! Realized rocks and spears: `Weapon` (81224) over `PlayerCarryableItem` (79261) and
//! `PhysicalObject` (78561), with `Rock` (246997) and `Spear` (270804) on top. Ported:
//! the constructors, `PlaceInRoom`, `ChangeMode`, `Thrown` (81682), `HitWall` (81812),
//! and `Update` in every mode but `StuckInCreature`: the floor bounce of a belly-slide
//! throw, a thrown weapon's turn, tumble, slope bounce and wall hit, a thrown spear
//! sticking in a wall with its poles, and the items' sound loops. Creature hits are
//! step 5 (the hooks are marked). Behaviour: `rainworld-re/notes/items.md` sections
//! 6-11.
//!
//! Mirrors: Weapon.cs, Rock.cs, Spear.cs, PlayerCarryableItem.cs, PhysicalObject.cs

use std::sync::Arc;

use rw_assets::{AbstractObjectType, Room, Slope, SoundTable, Terrain, TileFlags};
use rw_random::UnityRandom;

use super::{AbstractObject, AbstractSpear, ObjectData};
use crate::creature::CreatureId;
use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{
    BodyChunk, CosmeticKind, ImpactNoises, LoopEmitter, PhysicalObject, PhysicsParams, SoundId,
    Sounds, Spark, Vec2, Water,
};

/// A spark's colour (`new Color(1, 1, 1)`).
const WHITE: [f32; 4] = [1.0; 4];

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
    /// `stuckInObject` and its chunk, `stuckRotation` and `stuckBodyPart` while lodged
    /// in a creature.
    pub stuck_in: Option<StuckIn>,
    /// `stuckBodyPart`: kept from one lodging to the next, as the game keeps it (-1:
    /// none). While it is set, a new lodging keeps `stuckRotation` too.
    pub stuck_body_part: i32,
    /// `stuckRotation`, kept the same way.
    pub stuck_rotation: f32,
    pub pin_to_wall_counter: i32,
    /// `ChangeMode` pulled it out of a creature: the world plays
    /// `Spear_Dislodged_From_Creature` and drops the stick (`PulledOutOfStuckObject`).
    pub pulled_out: bool,
}

/// Where a spear is lodged in a creature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StuckIn {
    pub creature: CreatureId,
    pub chunk: usize,
    /// Degrees from the chunk's rotation (from the wall when pinned).
    pub rotation: f32,
    /// -1: no graphics body part.
    pub body_part: i32,
}

/// What kind of weapon.
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponKind {
    Rock,
    /// A `Spear`. An `ExplosiveSpear` is realized as one too (docs/divergences/ D14).
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
    /// Emitters `Stop` let go, still in the room until their next update destroys them.
    pub released: Vec<LoopEmitter>,
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
            released: Vec::new(),
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
    /// The player's creature and its `ThrowDirection` (a weapon it threw turns with it).
    pub player: Option<(CreatureId, i32)>,
    /// The abstract tiles of the room's spears stuck in walls (`Spear.Update` reads the
    /// room's entities; filled only for a thrown spear).
    pub stuck_spear_tiles: &'a [(i32, i32)],
    /// The room's `NoSpearStickZone`s: position and radius.
    pub no_stick_zones: &'a [(Vec2, f32)],
    /// A shelter whose door `IsClosing` or `IsOpening`.
    pub shelter_door_moving: bool,
    /// The sparks this update adds to the room, with their places in its update list.
    pub cosmetics: &'a mut Vec<(u64, CosmeticKind)>,
}

/// `Weapon.Thrown`'s arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Throw {
    pub by: CreatureId,
    /// The thrower's `mainBodyChunk.vel`.
    pub by_vel: Vec2,
    pub pos: Vec2,
    pub first_frame_trace_from_pos: Option<Vec2>,
    pub dir: (i32, i32),
    pub frc: f32,
    pub eu: bool,
}

/// A spear's `SetRandomSpin` draws: the direction, then the speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpearSpin {
    sign: f32,
    speed: f32,
}

impl SpearSpin {
    pub fn draw(rng: &mut UnityRandom) -> Self {
        let sign = rng.value();
        Self {
            sign,
            speed: rng.value(),
        }
    }
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
    pub thrown_by: Option<CreatureId>,
    pub thrown_pos: Vec2,
    pub first_frame_trace_from_pos: Option<Vec2>,
    /// `meleeHitChunk`: a creature's chunk (`ThrowToGetFree`'s grabber) the next
    /// thrown trace hits whatever it crosses.
    pub melee_hit_chunk: Option<(CreatureId, usize)>,
    pub throw_dir: (i32, i32),
    pub change_dir_counter: i32,
    /// `doNotTumbleAtLowSpeed` (only Remix's upward bomb throw sets it).
    pub do_not_tumble_at_low_speed: bool,
    pub vibrate: i32,
    /// `exitThrownModeSpeed` (30).
    pub exit_thrown_mode_speed: f32,
    pub override_exit_thrown_speed: f32,
    pub throw_mode_frames: i32,
    pub floor_bounce_frames: i32,
    pub closest_crit_dist: f32,
    /// `thrownClosestToCreature`: the creature the throw passed closest to so far.
    pub thrown_closest_to_creature: Option<CreatureId>,
    /// The thrower and the creature of a missed `WeaponAttack` that `ChangeMode` told
    /// the room of, until the game hands it on.
    pub missed_attack: Option<(CreatureId, CreatureId)>,
    /// `inFrontOfObjects`: -1 unset, 1 in `Items`, 0 in `Background`.
    pub in_front_of_objects: i32,
    /// How many times `ChangeOverlap` moved the sprites (each time it changed
    /// `inFrontOfObjects`: `MoveObjectToContainer` to the top of `Items` or `Background`).
    pub overlap_moves: u32,
    /// The update-list counter at the last of those moves.
    pub overlap_seq: u64,
    pub blink: i32,
    pub forbidden_to_player: i32,
    pub last_outside_terrain_pos: Option<Vec2>,
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
                    stuck_in: None,
                    stuck_body_part: -1,
                    stuck_rotation: 0.0,
                    pin_to_wall_counter: 0,
                    pulled_out: false,
                };
                (WeaponKind::Spear(Box::new(parts)), 7.0)
            }
            ObjectData::Plain => (WeaponKind::Rock, 9.0),
        };
        let mut chunk = BodyChunk::new(Vec2::ZERO, 5.0, 0.07);
        chunk.loudness = loudness;
        Some(Self {
            kind,
            body: PhysicalObject::new(vec![chunk], WEAPON_PARAMS),
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
            thrown_by: None,
            thrown_pos: Vec2::ZERO,
            first_frame_trace_from_pos: None,
            melee_hit_chunk: None,
            throw_dir: (0, 0),
            change_dir_counter: 0,
            do_not_tumble_at_low_speed: false,
            vibrate: 0,
            exit_thrown_mode_speed: 30.0,
            override_exit_thrown_speed: 0.0,
            throw_mode_frames: -1,
            floor_bounce_frames: 0,
            closest_crit_dist: f32::MAX,
            thrown_closest_to_creature: None,
            missed_attack: None,
            in_front_of_objects: -1,
            overlap_moves: 0,
            overlap_seq: 0,
            blink: 0,
            forbidden_to_player: 0,
            last_outside_terrain_pos: None,
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

    pub(crate) fn spear_mut(&mut self) -> Option<&mut SpearParts> {
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

    /// `PlayerCarryableItem.Forbid`: 40 ticks out of the player's reach.
    pub fn forbid(&mut self) {
        self.forbidden_to_player = 40;
    }

    /// `ChangeOverlap`: in front of the other objects (`Items`) or behind them
    /// (`Background`).
    /// `seq` is the update-list counter now.
    pub fn change_overlap(&mut self, overlap: bool, seq: u64) {
        if self.in_front_of_objects != i32::from(overlap) {
            self.overlap_moves = self.overlap_moves.wrapping_add(1);
            self.overlap_seq = seq;
        }
        self.in_front_of_objects = i32::from(overlap);
    }

    /// Whether `Spear.PickedUp` pulls it out of a wall first (`hasHorizontalBeamState`).
    #[must_use]
    pub fn has_horizontal_beam_state(&self) -> bool {
        self.spear().is_some_and(|s| s.has_horizontal_beam_state)
    }

    /// `Spear.PickedUp`'s pull out of a wall: `resetHorizontalBeamState` (the tiles'
    /// poles put back), `stuckInWall` the zero vector, a vibration, terrain collision
    /// back and no cycles left in the wall.
    pub fn pull_out_of_wall(&mut self, object: &mut AbstractObject, room: &mut Room) {
        let Some(s) = self.spear_mut() else {
            return;
        };
        if let Some(abs) = object.spear_mut() {
            s.reset_horizontal_beam_state(room, abs);
        }
        s.stuck_in_wall = Some(Vec2::ZERO);
        self.vibrate = 20;
        self.chunk_mut().collide_with_terrain = true;
        if let Some(abs) = object.spear_mut() {
            abs.stuck_in_wall_cycles = 0;
        }
    }

    /// `NewRoom` (`Weapon`'s and `PlayerCarryableItem`'s).
    pub fn new_room(&mut self) {
        self.last_outside_terrain_pos = None;
        self.in_front_of_objects = -1;
    }

    /// `SetRandomSpin`: a rock spins at up to 100 degrees a tick either way (one draw);
    /// a spear at 50 to 150 in a random direction (two) and is `spinning`.
    pub fn set_random_spin(&mut self, rng: &mut UnityRandom) {
        let gravity = 0.05f32.lerp(1.0, ROOM_GRAVITY);
        match &mut self.kind {
            WeaponKind::Rock => {
                self.rotation_speed = (-100.0f32).lerp(100.0, rng.value()) * gravity;
            }
            WeaponKind::Spear(_) => self.spin_spear(SpearSpin::draw(rng)),
        }
    }

    /// A spear's `SetRandomSpin` from its two draws.
    fn spin_spear(&mut self, spin: SpearSpin) {
        let gravity = 0.05f32.lerp(1.0, ROOM_GRAVITY);
        if let WeaponKind::Spear(s) = &mut self.kind {
            let sign = if spin.sign < 0.5 { -1.0 } else { 1.0 };
            self.rotation_speed = sign * 50.0f32.lerp(150.0, spin.speed) * gravity;
            s.spinning = true;
        }
    }

    /// `ChangeMode` (`Spear`'s part first, which runs even when the mode stays).
    pub fn change_mode(
        &mut self,
        new: WeaponMode,
        object: &mut AbstractObject,
        cx: &mut ModeChange<'_>,
    ) {
        self.change_mode_spun(new, object, cx, None);
    }

    /// [`Self::change_mode`] with the spin of a spear going `Free` drawn earlier
    /// (`SpearOnBack.DropSpear` draws where the game does; the world applies it after).
    pub fn change_mode_spun(
        &mut self,
        new: WeaponMode,
        object: &mut AbstractObject,
        cx: &mut ModeChange<'_>,
        spin: Option<SpearSpin>,
    ) {
        let mode = self.mode;
        let throw_y = self.throw_dir.1;
        let mut overlap = None;
        if let WeaponKind::Spear(s) = &mut self.kind {
            if mode == WeaponMode::StuckInCreature {
                s.stuck_in = None;
                s.pulled_out = true;
                overlap = Some(true);
            } else if new == WeaponMode::StuckInCreature {
                overlap = Some(false);
            }
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
        if let Some(o) = overlap {
            self.change_overlap(o, *cx.update_seq);
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
            self.first_frame_trace_from_pos = None;
        }
        let c = self.chunk_mut();
        c.collide_with_objects = new != WeaponMode::Carried;
        c.collide_with_terrain = matches!(new, WeaponMode::Free | WeaponMode::Thrown);
        c.go_through_floors = true;
        if new == WeaponMode::Free {
            match spin {
                Some(spin) => self.spin_spear(spin),
                None => self.set_random_spin(cx.rng),
            }
        }
        // Leaving `Thrown` within 120 of a creature is a missed `WeaponAttack`.
        if self.closest_crit_dist < 120.0
            && self.mode == WeaponMode::Thrown
            && let (Some(thrower), Some(victim)) = (self.thrown_by, self.thrown_closest_to_creature)
        {
            self.missed_attack = Some((thrower, victim));
        }
        self.thrown_closest_to_creature = None;
        self.closest_crit_dist = f32::MAX;
        self.mode = new;
    }

    /// `HeavyWeapon`: true for a rock and a spear.
    #[must_use]
    pub fn heavy_weapon(&self) -> bool {
        true
    }

    /// `Thrown` (`Weapon`'s, then `Rock`'s or `Spear`'s up to its sound, which it
    /// returns): from `throw.pos` (`MoveFromOutsideMyUpdate`) at 40 px a tick along a
    /// sideways `dir` plus a part of the thrower's velocity and a lift (rock 3, spear
    /// 1.5), or at 40 along a vertical one; in `Items`, turned along `dir`, not spinning.
    pub fn thrown(
        &mut self,
        throw: Throw,
        object: &mut AbstractObject,
        cx: &mut ModeChange<'_>,
    ) -> SoundId {
        self.thrown_by = Some(throw.by);
        self.thrown_pos = throw.pos;
        self.throw_dir = throw.dir;
        self.first_frame_trace_from_pos = throw.first_frame_trace_from_pos;
        self.change_dir_counter = 3;
        self.change_overlap(true, *cx.update_seq);
        let even = self.even_update;
        let spear = self.spear().is_some();
        self.chunk_mut()
            .move_from_outside_my_update(even, throw.eu, throw.pos);
        let sound = if spear {
            SoundId::SlugcatThrowSpear
        } else {
            SoundId::SlugcatThrowRock
        };
        if let Some(s) = self.spear_mut() {
            s.always_stick_in_walls = false;
        }
        let (dx, dy) = throw.dir;
        let c = self.chunk_mut();
        if dx != 0 {
            c.vel.y = throw.by_vel.y * 0.5;
            c.vel.x = throw.by_vel.x * 0.2;
            c.vel.x += dx as f32 * 40.0 * throw.frc;
            c.vel.y += if spear { 1.5 } else { 3.0 };
        } else {
            if dy == 0 {
                self.change_mode(WeaponMode::Free, object, cx);
                return sound;
            }
            c.vel.x = throw.by_vel.x * 0.5;
            c.vel.y = dy as f32 * 40.0 * throw.frc;
        }
        self.override_exit_thrown_speed = if throw.frc >= 1.0 {
            0.0
        } else {
            self.exit_thrown_mode_speed.min(throw.frc * 20.0)
        };
        self.change_mode(WeaponMode::Thrown, object, cx);
        self.set_rotation = Some(Vec2::new(dx as f32, dy as f32));
        self.rotation_speed = 0.0;
        self.melee_hit_chunk = None;
        sound
    }

    /// A spear's `spearDamageBonus`.
    pub fn set_spear_damage_bonus(&mut self, bonus: f32) {
        if let Some(s) = self.spear_mut() {
            s.spear_damage_bonus = bonus;
        }
    }

    /// A spear's `alwaysStickInWalls = true`.
    pub fn set_always_stick_in_walls(&mut self) {
        if let Some(s) = self.spear_mut() {
            s.always_stick_in_walls = true;
        }
    }

    /// `throwDir` as a vector.
    #[must_use]
    pub fn throw_dir_vec(&self) -> Vec2 {
        Vec2::new(self.throw_dir.0 as f32, self.throw_dir.1 as f32)
    }

    /// `Update` of a rock or spear alone in its room (no creature to notify or hit,
    /// nothing else in collision layer 0): [`Self::update_start`],
    /// [`Self::thrown_update`] when thrown, then [`Self::update_finish`].
    pub fn update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        if self.update_start(object, cx) && self.thrown_update(object, cx) {
            self.traced_nothing();
        }
        self.update_finish(object, cx);
    }

    /// The trace against collision layer 1 found nothing to hit.
    pub fn traced_nothing(&mut self) {
        self.first_frame_trace_from_pos = None;
        self.forbidden_to_player = 40;
    }

    /// `Weapon.Update` up to its `Thrown` branch. True when the weapon is thrown: the
    /// game then tells the room's creatures of it, runs [`Self::thrown_update`], the
    /// trace against collision layer 1 and the walk over layer 0, then
    /// [`Self::update_finish`].
    pub fn update_start(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) -> bool {
        self.last_mode = self.mode;
        let exit_speed = self.exit_speed();
        if self.vibrate > 0 {
            self.vibrate -= 1;
        }
        if self.floor_bounce_frames > 0 && self.throw_dir.1 == 0 {
            self.floor_bounce(exit_speed, cx);
        }
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
        // `stuckObjects` with more than one stick (a spear in several creatures) is step 5.
        self.tail_pos = self.chunk().last_pos;
        match self.mode {
            WeaponMode::Carried => {
                if !self.body.held {
                    self.change_mode_in(WeaponMode::Free, object, cx);
                }
            }
            WeaponMode::Free
                if self.chunk().submersion(cx.water) > 0.0 && self.rotation_speed.abs() > 5.0 =>
            {
                self.rotation_speed *= 0.7;
            }
            WeaponMode::Thrown => return true,
            _ => {}
        }
        false
    }

    /// `exitThrownModeSpeed`, or the weak throw's override.
    fn exit_speed(&self) -> f32 {
        if self.override_exit_thrown_speed > 0.0 {
            self.override_exit_thrown_speed
        } else {
            self.exit_thrown_mode_speed
        }
    }

    /// The rest of `Update` after the layer-0 walk: `PlayerCarryableItem.Update` (with
    /// the physics), then `Rock.Update`'s or `Spear.Update`'s own part.
    pub fn update_finish(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        self.carryable_physics(cx);
        self.update_after_physics(object, cx);
    }

    /// [`Self::update_finish`] after the physics.
    pub fn update_after_physics(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        self.carryable_update(object, cx);
        self.update_own(object, cx);
    }

    /// `PhysicalObject.Update` at the start of `PlayerCarryableItem.Update`: the chunk
    /// moves. Returns its terrain impacts' noises (where, how strong); the game makes
    /// them as it meets the impacts, with nothing else drawn in between.
    pub fn carryable_physics(&mut self, cx: &mut WeaponTick<'_>) -> Vec<(Vec2, f32)> {
        // `WeatherInertia` draws only in blizzard rooms (Saint), none in the base game.
        let mut log = ImpactNoises::default();
        self.body.update_with(cx.room, cx.water, &mut log);
        log.0
    }

    /// `Rock.Update`'s or `Spear.Update`'s own part after the base.
    pub fn update_own(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        if self.spear().is_some() {
            self.spear_update(object, cx);
        } else {
            self.rock_update(cx);
        }
    }

    /// `ChangeMode` in an update, against the room it updates in.
    fn change_mode_in(
        &mut self,
        new: WeaponMode,
        object: &mut AbstractObject,
        cx: &mut WeaponTick<'_>,
    ) {
        let room = Arc::clone(cx.room);
        let mut change = ModeChange {
            room: &room,
            rng: cx.rng,
            update_seq: cx.update_seq,
        };
        self.change_mode(new, object, &mut change);
    }

    /// The floor bounce of a belly-slide throw (`floorBounceFrames`): a fast weapon
    /// touching a floor or ceiling springs off it with four sparks and `Weapon_Skid`, at
    /// least at its exit speed, and the frames halve.
    fn floor_bounce(&mut self, exit_speed: f32, cx: &mut WeaponTick<'_>) {
        self.floor_bounce_frames -= 1;
        let contact = self.chunk().contact();
        if contact.y == 0 || self.chunk().vel.x.abs() <= 5.0 {
            return;
        }
        let cy = f32::from(contact.y);
        let c = self.chunk_mut();
        c.vel.y = (c.vel.y.abs() + 1.0) * -cy + if contact.y < 0 { 4.5 } else { 0.0 };
        let (pos, vel) = (c.pos, c.vel);
        let contact_vec = Vec2::new(f32::from(contact.x), cy);
        let at = Vec2::new(pos.x, pos.tile_middle().y + cy * 10.0);
        for _ in 0..4 {
            let mut v = vel * cx.rng.value();
            v += Vec2::random_unit(cx.rng) * (cx.rng.value() * 4.0);
            v -= contact_vec * (4.0 * cx.rng.value());
            cx.add_spark(at, v, 6, 18);
        }
        cx.sounds
            .play_on_object(SoundId::WeaponSkid, pos, false, 1.0, 1.0, cx.rng);
        let c = self.chunk_mut();
        let sign = if c.vel.x >= 0.0 { 1.0 } else { -1.0 };
        c.vel.x = (exit_speed + 1.0).max(c.vel.x.abs()) * sign;
        self.floor_bounce_frames /= 2;
    }

    /// `Weapon.Update`'s `Thrown` branch after the creatures are told of the weapon, up
    /// to the layer-0 walk: the turn with a player thrower (one draw), the tumble back
    /// to `Free` under the exit speed or when the throw frames run out, then a
    /// belly-slide bounce up off a slope, the wall hit, or the trace against collision
    /// layer 1. True when the trace is due (the game's, with the player and the lizards
    /// in that layer).
    pub fn thrown_update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) -> bool {
        let exit_speed = self.exit_speed();
        if self.change_dir_counter > 0
            && let Some((player, dir)) = cx.player
            && self.thrown_by == Some(player)
            && dir == -self.throw_dir.0
        {
            self.change_dir_counter = 0;
            self.throw_dir = (dir, 0);
            let c = self.chunk_mut();
            c.vel.x = c.vel.x.abs() * dir as f32;
            let vx = c.vel.x;
            let nudge = Vec2::from_degrees(cx.rng.value() * 360.0) * 0.1;
            self.rotation = (self.rotation + nudge).normalized();
            self.set_rotation = Some(Vec2::from_degrees(if vx < 0.0 { 270.0 } else { 90.0 }));
        }
        self.change_dir_counter -= 1;
        if self.throw_mode_frames > 0 {
            self.throw_mode_frames -= 1;
        }
        if (self.chunk().vel.length() < exit_speed && !self.do_not_tumble_at_low_speed)
            || self.throw_mode_frames == 0
        {
            self.set_random_spin(cx.rng);
            self.change_mode_in(WeaponMode::Free, object, cx);
            self.forbidden_to_player = 10;
        }
        let c = self.chunk();
        let contact = c.contact();
        let touching = contact.x != 0 || contact.y != 0;
        let on_slope = cx.room.tile_at_pos(c.pos).terrain == Terrain::Slope;
        if self.floor_bounce_frames > 0 && self.throw_dir.0 != 0 && touching && on_slope {
            self.slope_bounce(exit_speed, object, cx);
        } else if (i32::from(contact.x), i32::from(contact.y)) == self.throw_dir {
            self.hit_wall(object, cx);
            self.floor_bounce_frames = 0;
        } else {
            return true;
        }
        false
    }

    /// The "vertical bounce": a belly-slide throw along a floor that meets a slope turns
    /// up (or down) it, thrown anew from the tile's middle, with four sparks and
    /// `Weapon_Skid`.
    fn slope_bounce(
        &mut self,
        exit_speed: f32,
        object: &mut AbstractObject,
        cx: &mut WeaponTick<'_>,
    ) {
        log::info!("vertical bounce");
        let spear = self.spear().is_some();
        let pos = self.chunk().pos;
        self.chunk_mut().vel.x = 0.5 * self.throw_dir.0 as f32;
        let (tx, ty) = pos.tile();
        let up = matches!(
            cx.room.identify_slope(tx, ty),
            Some(Slope::UpLeft | Slope::UpRight)
        );
        self.throw_dir = (0, if up { 1 } else { -1 });
        let extra = if !up {
            0.0
        } else if spear {
            5.0
        } else {
            10.0
        };
        let c = self.chunk_mut();
        let sign = if up { 1.0 } else { -1.0 };
        c.vel.y = sign * c.vel.length().max(exit_speed + 1.0 + extra);
        c.pos = pos.tile_middle();
        let (pos, vel) = (c.pos, c.vel);
        self.floor_bounce_frames = 0;
        self.change_mode_in(WeaponMode::Thrown, object, cx);
        self.thrown_pos = pos;
        self.throw_mode_frames = -1;
        self.set_rotation = Some(self.throw_dir_vec());
        self.rotation_speed = 0.0;
        for _ in 0..4 {
            let mut v = vel * cx.rng.value();
            v += Vec2::random_unit(cx.rng) * cx.rng.value();
            cx.add_spark(pos, v, 6, 18);
        }
        cx.sounds
            .play_on_object(SoundId::WeaponSkid, pos, false, 1.0, 1.0, cx.rng);
    }

    /// `HitWall`: seven sparks when the room is viewed, the sound (a spear's bounce or
    /// a rock's hit), a new spin and `Free`, out of the player's reach for 10 ticks.
    /// The camera's `ScreenMovement` bump is not ported (docs/divergences/ D7).
    pub fn hit_wall(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        let dir = self.throw_dir_vec();
        let c = self.chunk();
        let (pos, rad) = (c.pos, c.rad);
        if cx.room_index == cx.camera_room {
            for _ in 0..7 {
                let mut v = Vec2::from_degrees(cx.rng.value() * 360.0) * (10.0 * cx.rng.value());
                v += -dir * 10.0;
                cx.add_spark(pos + dir * (rad - 1.0), v, 2, 4);
            }
        }
        let sound = if self.spear().is_some() {
            SoundId::SpearBounceOffWall
        } else {
            SoundId::RockHitWall
        };
        cx.sounds
            .play_on_object(sound, pos, false, 1.0, 1.0, cx.rng);
        self.set_random_spin(cx.rng);
        self.change_mode_in(WeaponMode::Free, object, cx);
        self.forbidden_to_player = 10;
    }

    /// `WeaponDeflect`: halfway to `between`, shaking, `Free` and off along `dir` at half
    /// `bounce_speed`, spinning anew.
    pub fn weapon_deflect(
        &mut self,
        between: Vec2,
        dir: Vec2,
        bounce_speed: f32,
        object: &mut AbstractObject,
        cx: &mut ModeChange<'_>,
    ) {
        let c = self.chunk_mut();
        c.pos = c.pos.lerp(between, 0.5);
        self.vibrate = 20;
        self.change_mode(WeaponMode::Free, object, cx);
        self.chunk_mut().vel = dir * (bounce_speed * 0.5);
        self.set_random_spin(cx.rng);
    }

    /// `PlayerCarryableItem.Update` after `PhysicalObject.Update`.
    fn carryable_update(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
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
            WeaponMode::Thrown => self.thrown_spear(object, cx),
            // Stuck in a creature, and the impaled flies' loop after it, are step 5.
            _ => {}
        }
    }

    /// `Spear.Update`'s `Thrown` branch: half the gravity taken back, and a spear
    /// within 560 px of its throw (more with a damage bonus) that meets Solid ahead from
    /// an Air tile sticks in the wall a third of the time (always within 140 px or after
    /// a belly-slide throw): not where another spear is stuck, not in the room's outer
    /// tile ring by its unclamped abstract tile (an explosive spear is exempt), not in a
    /// `NoSpearStickZone`, and not while a shelter door moves.
    fn thrown_spear(&mut self, object: &mut AbstractObject, cx: &mut WeaponTick<'_>) {
        let explosive = object.spear().is_some_and(|a| a.explosive);
        self.chunk_mut().vel.y += 0.45;
        let Some(s) = self.spear() else { return };
        let (bonus, always) = (s.spear_damage_bonus, s.always_stick_in_walls);
        let c = self.chunk();
        let (pos, contact) = (c.pos, c.contact());
        let dir = self.throw_dir_vec();
        let near = self.thrown_pos.dist_less(pos, 560.0 * bonus.max(1.0));
        if !near
            || (i32::from(contact.x), i32::from(contact.y)) != self.throw_dir
            || cx.room.tile_at_pos(pos).terrain != Terrain::Air
            || cx.room.tile_at_pos(pos + dir * 20.0).terrain != Terrain::Solid
        {
            return;
        }
        let chance = if explosive { 0.8 } else { 0.33 };
        if !(cx.rng.value() < chance || self.thrown_pos.dist_less(pos, 140.0) || always) {
            return;
        }
        let tile = object.pos.tile;
        let mut stick = !cx.stuck_spear_tiles.contains(&tile);
        if stick && !explosive {
            let (w, h) = (cx.room.width(), cx.room.height());
            if tile.1 <= 0 || tile.1 >= h - 1 || tile.0 <= 0 || tile.0 >= w - 1 {
                stick = false;
                log::info!("Spear prevented from stabbing the edge of a room");
            } else if cx
                .no_stick_zones
                .iter()
                .any(|&(at, rad)| pos.tile_middle().dist_less(at, rad))
            {
                stick = false;
            }
        }
        if stick && cx.shelter_door_moving {
            stick = false;
        }
        if !stick {
            return;
        }
        if let Some(s) = self.spear_mut() {
            s.stuck_in_wall = Some(pos.tile_middle());
        }
        self.vibrate = 10;
        self.change_mode_in(WeaponMode::StuckInWall, object, cx);
        cx.sounds
            .play_on_object(SoundId::SpearStickInWall, pos, false, 1.0, 1.0, cx.rng);
        self.chunk_mut().collide_with_terrain = false;
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

impl WeaponTick<'_> {
    /// `room.AddObject(new Spark(pos, vel, white, null, standard, exceptional))`.
    fn add_spark(&mut self, pos: Vec2, vel: Vec2, standard: i32, exceptional: i32) {
        let spark = Spark::new(pos, vel, WHITE, standard, exceptional, self.rng);
        *self.update_seq += 1;
        self.cosmetics
            .push((*self.update_seq, CosmeticKind::Spark(spark)));
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

    /// The `Pitch` setter.
    pub fn set_pitch(&mut self, p: f32) {
        self.p = p;
    }

    /// `DynamicSoundLoop.Update` for an owner in `owner_room` whose chunk is at `pos`.
    pub fn update(&mut self, owner_room: Option<i32>, pos: Vec2, cx: &mut WeaponTick<'_>) {
        let mut lcx = LoopCx {
            room_index: cx.room_index,
            sounds: cx.sounds,
            rng: cx.rng,
            triggers: cx.triggers,
            camera_room: cx.camera_room,
            tick: cx.tick,
            update_seq: cx.update_seq,
        };
        self.update_in(owner_room, pos, &mut lcx);
    }

    /// `DynamicSoundLoop.Update` with what it reads of the room and game.
    pub fn update_in(&mut self, owner_room: Option<i32>, pos: Vec2, cx: &mut LoopCx<'_>) {
        if self.emitter.is_some_and(|e| e.slated) {
            self.emitter = None;
        }
        if self.sound != self.last_sound {
            self.stop();
            if self.sound.is_some() && self.v > 0.0 && owner_room.is_some() {
                self.start(pos, cx);
            }
            self.last_sound = self.sound;
        }
        if self.v == 0.0 && self.last_vol > 0.0 {
            self.stop();
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
    fn start(&mut self, pos: Vec2, cx: &mut LoopCx<'_>) {
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

    /// `Stop`: the emitter is let go (`alive = false`); it dies at its next update.
    pub fn stop(&mut self) {
        if let Some(mut e) = self.emitter.take() {
            e.alive = false;
            if !e.slated {
                self.released.push(e);
            }
        }
    }

    /// The emitter, held or let go, that joined the update list at `order`.
    pub fn emitter_mut(&mut self, order: u64) -> Option<&mut LoopEmitter> {
        self.emitter
            .iter_mut()
            .chain(self.released.iter_mut())
            .find(|e| e.update_order == order)
    }

    /// The emitters still in a room's update list.
    pub fn emitters(&self) -> impl Iterator<Item = &LoopEmitter> {
        self.emitter
            .iter()
            .chain(&self.released)
            .filter(|e| !e.slated)
    }

    /// Every emitter, held or let go, leaves the loop (its owner's graphics are gone):
    /// they stay in the room as they are.
    pub fn take_emitters(&mut self) -> Vec<LoopEmitter> {
        let mut all: Vec<LoopEmitter> = self.emitter.take().into_iter().collect();
        all.append(&mut self.released);
        all.retain(|e| !e.slated);
        all
    }
}

/// What a `DynamicSoundLoop` reads of its room and the game.
pub struct LoopCx<'a> {
    pub room_index: i32,
    pub sounds: &'a mut Sounds,
    pub rng: &'a mut UnityRandom,
    /// `soundLoader.workingTriggers`.
    pub triggers: &'a SoundTable,
    pub camera_room: i32,
    pub tick: u64,
    /// The world's `updateList` position counter (the emitter joins the room).
    pub update_seq: &'a mut u64,
}

#[cfg(test)]
#[path = "weapon_tests.rs"]
mod tests;
