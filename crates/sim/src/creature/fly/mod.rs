//! The realized bat fly: `Fly` (one body chunk), its `FlyAI` with the shared
//! `FlockBehavior`s, and the room's `FliesRoomAI`. Base game, Remix off: no safari
//! control, no lures (`FlyLure` is not ported), no challenge limits.
//!
//! Behaviour: `rainworld-re/notes/creature-model/overseers-and-flies.md` (realized flies) and the decompile
//! lines given on each item. The graphics (`FlyGraphics`) are `rw-graphics`', called
//! through [`crate::PlayerObserver`].
//!
//! Mirrors: Fly.cs, FlyAI.cs, FliesRoomAI.cs

mod ai;
mod ai_update;
mod room_ai;
mod update;

use rw_random::UnityRandom;

pub use ai::{DropStatus, FlockBehavior, FlockId, FlyAi, FlyBehavior, TrackedThreat};
pub use room_ai::FliesRoomAi;
pub(crate) use room_ai::FlyRoomTick;
pub use update::PlayerBody;
pub(crate) use update::{FlyRun, FlyTick};

use super::{CreatureId, EntityId, WorldCoordinate};
use crate::object::PhysicalObject;
use crate::{BodyChunk, PhysicsParams, Vec2};

/// `Fly.MovementMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementMode {
    BatFlight,
    SwarmFlight,
    Passive,
    Burrow,
    Panic,
    Hang,
}

impl MovementMode {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::BatFlight => "BatFlight",
            Self::SwarmFlight => "SwarmFlight",
            Self::Passive => "Passive",
            Self::Burrow => "Burrow",
            Self::Panic => "Panic",
            Self::Hang => "Hang",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::BatFlight,
            Self::SwarmFlight,
            Self::Passive,
            Self::Burrow,
            Self::Panic,
            Self::Hang,
        ]
        .into_iter()
        .find(|m| m.value() == value)
    }
}

/// The hover loop's `ChunkSoundEmitter`.
pub use crate::sound::LoopEmitter as HoverEmitter;

/// A realized `Fly` with its `FlyAI`.
#[derive(Debug, Clone, PartialEq)]
pub struct Fly {
    pub body: PhysicalObject,
    /// `room`: `None` in a shortcut, in a hive or removed.
    pub room: Option<i32>,
    /// Its place in the room's `updateList`.
    pub update_order: u64,
    pub slated: bool,
    pub stun: i32,
    pub dead: bool,
    pub shortcut_delay: i32,
    pub entering_shortcut: Option<(i32, i32)>,
    pub short_cut_rad_add: f32,
    pub in_shortcut: bool,
    pub new_to_room_invinsibility: i32,
    /// `Creature.lastCoord` (the default coordinate until the first update).
    pub last_coord: WorldCoordinate,
    pub sticks_respawned: bool,
    pub flap: f32,
    pub flap_speed: f32,
    pub flap_depth: f32,
    pub last_flap_depth: f32,
    pub drown: f32,
    /// `movMode`: null until the AI's first update.
    pub mov_mode: Option<MovementMode>,
    pub eaten: i32,
    pub burrow_or_hang_spot: Option<Vec2>,
    pub dir: Vec2,
    pub wall_avoidance: f32,
    pub chain_behavior_variation: i32,
    pub bites: i32,
    pub ever_been_caught_by_player: bool,
    /// `evenUpdate`: the `eu` of its last update (false before the first).
    pub even_update: bool,
    pub hover: Option<HoverEmitter>,
    pub ai: FlyAi,
    /// Whether it has a graphics module (built by `AddObject` or `NowViewed` in a viewed
    /// room, dropped by `NoLongerViewed`).
    pub graphics: bool,
}

/// The Fly's physics scalars (184589).
const PARAMS: PhysicsParams = PhysicsParams {
    g: 0.9,
    room_gravity: 1.0,
    air_friction: 0.98,
    bounce: 0.1,
    surface_friction: 0.5,
    impact_threshold: 1.0,
    buoyancy: 0.94,
    water_friction: 0.9,
    water_retardation_immunity: 0.0,
};

impl Fly {
    /// The `Fly` constructor (184579): one chunk (rad 6, mass 0.05), `flap` and
    /// `flapSpeed` draws, then `FlyAI`'s with its `FlockBehavior`. `flock` is the id the
    /// new flock will have.
    pub fn new(
        me: CreatureId,
        flock: FlockId,
        rain_approaching: f32,
        rng: &mut UnityRandom,
    ) -> (Self, FlockBehavior) {
        let mut chunk = BodyChunk::new(Vec2::ZERO, 6.0, 0.05);
        chunk.go_through_floors = true;
        let body = PhysicalObject::new(vec![chunk], PARAMS);
        let flap = rng.value();
        let flap_speed = rng.value() + 0.1;
        let behavior = FlockBehavior::new(me, rng);
        let fly = Self {
            body,
            room: None,
            update_order: 0,
            slated: false,
            stun: 0,
            dead: false,
            shortcut_delay: 0,
            entering_shortcut: None,
            short_cut_rad_add: 0.0,
            in_shortcut: false,
            new_to_room_invinsibility: 0,
            last_coord: WorldCoordinate::new(0, 0, 0, 0),
            sticks_respawned: false,
            flap,
            flap_speed,
            flap_depth: 0.0,
            last_flap_depth: 0.0,
            drown: 0.0,
            mov_mode: None,
            eaten: 0,
            burrow_or_hang_spot: None,
            dir: Vec2::ZERO,
            wall_avoidance: 0.0,
            chain_behavior_variation: 0,
            bites: 3,
            ever_been_caught_by_player: false,
            even_update: false,
            hover: None,
            ai: FlyAi::new(flock, rain_approaching),
            graphics: false,
        };
        (fly, behavior)
    }

    /// `mainBodyChunk` (`firstChunk`).
    #[must_use]
    pub fn chunk(&self) -> &BodyChunk {
        &self.body.chunks[0]
    }

    pub fn chunk_mut(&mut self) -> &mut BodyChunk {
        &mut self.body.chunks[0]
    }

    #[must_use]
    pub fn pos(&self) -> Vec2 {
        self.body.chunks[0].pos
    }

    /// `Stunned`.
    #[must_use]
    pub fn stunned(&self) -> bool {
        self.stun >= 10
    }

    /// `Consious`.
    #[must_use]
    pub fn conscious(&self) -> bool {
        !self.stunned() && !self.dead
    }

    /// `CollideWithTerrain`'s setter.
    pub fn set_collide_with_terrain(&mut self, on: bool) {
        for c in &mut self.body.chunks {
            c.collide_with_terrain = on;
        }
    }
}

/// What `FlyGraphics` reads of a fly and the creatures around it.
#[derive(Debug, Clone, Copy)]
pub struct FlyView<'a> {
    pub id: EntityId,
    pub fly: &'a Fly,
    /// `grasps[0].grabbed.firstChunk.pos`.
    pub grasp_pos: Option<Vec2>,
    /// `grabbedBy[0].grabber.mainBodyChunk.pos`, and whether that grabber is a fly (else
    /// the slugcat).
    pub grabber: Option<(Vec2, bool)>,
    /// `firstChunk.submersion`.
    pub submersion: f32,
}
