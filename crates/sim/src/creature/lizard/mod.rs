//! The realized lizard (`Lizard`, 3 body chunks), shared by every breed; the breed's
//! numbers come from [`LizardBreedParams`]. Ported: the constructor, the body part of
//! `Update` with the `Creature` and `AirBreatherCreature` updates under it, `Act` with
//! the path following, the movement animations, gripping, swimming and the animations'
//! physics, shortcut travel, damage (`Violence`, `Stun`, `SpearStick`, the head shield,
//! hits in the mouth), biting, `CarryObject`, terrain impacts and `SpitOutOfShortCut`.
//! The AI is a stand-in ([`LizardAi`]: its path finder and the base AI's updates;
//! `LizardAI`'s behaviours are lizard step 7); the voice is step 5b. The graphics module
//! is `rw-graphics`' `LizardGraphics`; what it shares with the body is
//! [`LizardGraphicsLink`] (docs/DIVERGENCES.md D14).
//!
//! The updates that reach the rest of the game are `RainWorldGame`'s (`game_lizards.rs`,
//! `game_lizard_act.rs`, `game_lizard_hits.rs`). Behaviour:
//! `rainworld-re/notes/creature-model.md` sections 13 and 14, decompile `Lizard.cs`.

mod ai;
mod breed;
mod bubble;
mod graphics_link;
mod movement;

use rw_random::UnityRandom;

pub use ai::LizardAi;
pub use breed::{LizardBreedParams, SpeedMultiplier};
pub use bubble::LizardBubble;
pub use graphics_link::{
    BubbleHead, LightUpkeep, LizardGraphicsLink, LizardGraphicsTick, LizardHead, LizardSpawn,
    LizardView,
};
pub use movement::{AnimationOutcome, BodyPartDestination, MovementAnimation};

use super::{AbstractCreature, WorldCoordinate};
use crate::object::{BodyChunkConnection, ConnectionKind, PhysicalObject};
use crate::world::MovementConnection;
use crate::{BodyChunk, FloatExt, PhysicsParams, Vec2};

/// `Lizard.Animation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LizardAnimation {
    Standard,
    HearSound,
    PreyReSpotted,
    PreySpotted,
    FightingStance,
    ThreatSpotted,
    ThreatReSpotted,
    ShootTongue,
    Spit,
    PrepareToJump,
    Jumping,
    PrepareToLounge,
    Lounge,
    ShakePrey,
}

impl LizardAnimation {
    const ALL: [Self; 14] = [
        Self::Standard,
        Self::HearSound,
        Self::PreyReSpotted,
        Self::PreySpotted,
        Self::FightingStance,
        Self::ThreatSpotted,
        Self::ThreatReSpotted,
        Self::ShootTongue,
        Self::Spit,
        Self::PrepareToJump,
        Self::Jumping,
        Self::PrepareToLounge,
        Self::Lounge,
        Self::ShakePrey,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::HearSound => "HearSound",
            Self::PreyReSpotted => "PreyReSpotted",
            Self::PreySpotted => "PreySpotted",
            Self::FightingStance => "FightingStance",
            Self::ThreatSpotted => "ThreatSpotted",
            Self::ThreatReSpotted => "ThreatReSpotted",
            Self::ShootTongue => "ShootTongue",
            Self::Spit => "Spit",
            Self::PrepareToJump => "PrepareToJump",
            Self::Jumping => "Jumping",
            Self::PrepareToLounge => "PrepareToLounge",
            Self::Lounge => "Lounge",
            Self::ShakePrey => "ShakePrey",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.value() == value)
    }
}

/// A realized `Lizard` with its stand-in AI.
#[derive(Debug, Clone, PartialEq)]
pub struct Lizard {
    pub params: &'static LizardBreedParams,
    /// Head (0), middle (1) and hips (2); rods 0-1 and 1-2, and the stiffness push 0-2
    /// (`BodyStiff`).
    pub body: PhysicalObject,
    /// `room`: `None` in a shortcut or removed.
    pub room: Option<i32>,
    /// Its place in the room's `updateList` (and `physicalObjects[1]`).
    pub update_order: u64,
    pub slated: bool,
    pub stun: i32,
    pub dead: bool,
    pub shortcut_delay: i32,
    pub entering_shortcut: Option<(i32, i32)>,
    pub in_shortcut: bool,
    pub new_to_room_invinsibility: i32,
    /// `Creature.lastCoord` (the default coordinate until the first update).
    pub last_coord: WorldCoordinate,
    pub sticks_respawned: bool,
    /// `evenUpdate`.
    pub even_update: bool,
    /// `AirBreatherCreature.lungs`.
    pub lungs: f32,
    pub animation: LizardAnimation,
    pub time_in_animation: i32,
    pub time_to_remain_in_animation: i32,
    pub bite_delay: i32,
    /// `jawOpen`, read through [`Lizard::jaw_open`].
    jaw_open: f32,
    pub last_jaw_open: f32,
    pub jaw_forced_shut: f32,
    pub spawn_data_evil: f32,
    pub grabbed_attack_counter: i32,
    pub turned_by_rock_direction: i32,
    pub turned_by_rock_counter: i32,
    pub bubble: i32,
    pub bubble_intensity: f32,
    pub lounge_delay: i32,
    pub post_lounge_stun: i32,
    pub lounge_dir: Vec2,
    pub apply_gravity: bool,
    pub in_allowed_terrain_counter: i32,
    pub swim: f32,
    pub bite_control_reset: bool,
    pub effect_color: [f32; 3],
    /// Its `LizardGraphics` module's shared state, while it has one (its room is
    /// viewed and the app builds graphics).
    pub graphics: Option<LizardGraphicsLink>,
    pub ai: LizardAi,
    /// `shortCutRadAdd`.
    pub short_cut_rad_add: f32,
    pub upcoming_connections: Vec<MovementConnection>,
    pub following_connection: Option<MovementConnection>,
    pub last_following_connection: Option<MovementConnection>,
    pub commited_to_drop_connection: Option<MovementConnection>,
    pub movement_animation: Option<MovementAnimation>,
    pub grip_point: Option<Vec2>,
    pub time_spent_trying_this_move: i32,
    pub desperation_smoother: f32,
    pub limbs_aim_for: Vec2,
    pub straighten_out_needed: f32,
    pub narrow_upcoming: bool,
    pub climb_upcoming: bool,
    pub shortcut_upcoming: bool,
    pub body_wiggle: f32,
    /// `bodyWiggleCounter` (0-100 through [`Lizard::set_body_wiggle_counter`]).
    pub body_wiggle_counter: i32,
    /// `NPCTransportationDestination`.
    pub npc_transportation_destination: Option<WorldCoordinate>,
}

/// `Lizard`'s physics scalars as the constructor sets them (`Update` changes them
/// every tick).
const PARAMS: PhysicsParams = PhysicsParams {
    gravity: 0.9,
    air_friction: 0.999,
    bounce: 0.1,
    surface_friction: 0.5,
    impact_threshold: 1.0,
    buoyancy: 1.06,
    water_friction: 0.92,
    water_retardation_immunity: 0.0,
};

/// `AirBreatherCreature`'s `lungCapacity`: the lizards keep the template default.
pub const LUNG_CAPACITY: f32 = 520.0;

impl Lizard {
    /// The `Lizard` constructor: three chunks at (200, 500), the rods and the push,
    /// `GoThroughFloors`, the `Mean:x` spawn flag, then `effectColor` under the ID's
    /// seed with the global state restored (no net draw). `None` for a breed without
    /// data.
    #[must_use]
    pub fn new(c: &AbstractCreature, rng: &mut UnityRandom) -> Option<Self> {
        let params = LizardBreedParams::of(c.kind())?;
        let at = Vec2::new(200.0, 500.0);
        let mut chunks: Vec<BodyChunk> = (0..3)
            .map(|_| BodyChunk::new(at, params.chunk_rad(), params.chunk_mass()))
            .collect();
        for chunk in &mut chunks {
            chunk.go_through_floors = true;
        }
        let rod = params.rod_length();
        let mut body = PhysicalObject::new(chunks, PARAMS);
        body.connections = vec![
            BodyChunkConnection::new(0, 1, rod, ConnectionKind::Normal, 0.95, 0.5),
            BodyChunkConnection::new(1, 2, rod, ConnectionKind::Normal, 0.95, 0.5),
            BodyChunkConnection::new(
                0,
                2,
                rod * (1.0 + params.body_stiffnes),
                ConnectionKind::Push,
                1.0 - 0.9f32.lerp(0.5, params.body_stiffnes),
                0.5,
            ),
        ];
        let spawn_data_evil = c.mean_flag().unwrap_or(0.0);
        let saved = rng.state();
        rng.init_state(c.id.random_seed());
        let effect_color = params.effect_color(rng);
        rng.set_state(saved);
        Some(Self {
            params,
            body,
            room: None,
            update_order: 0,
            slated: false,
            stun: 0,
            dead: !c.state.alive,
            shortcut_delay: 0,
            entering_shortcut: None,
            in_shortcut: false,
            new_to_room_invinsibility: 0,
            last_coord: WorldCoordinate::new(0, 0, 0, 0),
            sticks_respawned: false,
            even_update: false,
            lungs: 1.0,
            animation: LizardAnimation::Standard,
            time_in_animation: 0,
            time_to_remain_in_animation: 0,
            bite_delay: 0,
            jaw_open: 0.0,
            last_jaw_open: 0.0,
            jaw_forced_shut: 0.0,
            spawn_data_evil,
            grabbed_attack_counter: 0,
            turned_by_rock_direction: 0,
            turned_by_rock_counter: 0,
            bubble: 0,
            bubble_intensity: 0.0,
            lounge_delay: 0,
            post_lounge_stun: 0,
            lounge_dir: Vec2::ZERO,
            apply_gravity: false,
            in_allowed_terrain_counter: 0,
            swim: 0.0,
            bite_control_reset: false,
            effect_color,
            graphics: None,
            ai: LizardAi::default(),
            short_cut_rad_add: 0.0,
            upcoming_connections: Vec::new(),
            following_connection: None,
            last_following_connection: None,
            commited_to_drop_connection: None,
            movement_animation: None,
            grip_point: None,
            time_spent_trying_this_move: 0,
            desperation_smoother: 0.0,
            limbs_aim_for: Vec2::ZERO,
            straighten_out_needed: 0.0,
            narrow_upcoming: false,
            climb_upcoming: false,
            shortcut_upcoming: false,
            body_wiggle: 0.0,
            body_wiggle_counter: 0,
            npc_transportation_destination: None,
        })
    }

    /// `mainBodyChunk` (chunk 0).
    #[must_use]
    pub fn head(&self) -> &BodyChunk {
        &self.body.chunks[0]
    }

    pub fn head_mut(&mut self) -> &mut BodyChunk {
        &mut self.body.chunks[0]
    }

    #[must_use]
    pub fn chunks(&self) -> &[BodyChunk] {
        &self.body.chunks
    }

    /// `TotalMass`.
    #[must_use]
    pub fn total_mass(&self) -> f32 {
        self.body.chunks.iter().map(|c| c.mass).sum()
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

    /// `BodyChunk.Rotation` of chunk `i`: the constructor's connections leave chunk 0
    /// and 1 turned toward chunk 2, and chunk 2 toward chunk 0.
    #[must_use]
    pub fn chunk_rotation(&self, i: usize) -> Vec2 {
        let other = if i == 2 { 0 } else { 2 };
        (self.body.chunks[i].pos - self.body.chunks[other].pos).normalized()
    }

    /// `JawOpen`'s getter, without its side effects on the stored value (a held object
    /// shuts the jaw; there is no tongue on the ported breeds).
    #[must_use]
    pub fn jaw_open(&self, holding: bool) -> f32 {
        let mut j = if holding { 0.0 } else { self.jaw_open };
        if self.jaw_forced_shut > 0.0 {
            j = (1.0 - self.jaw_forced_shut).mathf_pow(4.0).min(j);
        }
        j
    }

    /// `JawOpen`'s getter as the game runs it: it also stores what it returns.
    pub fn read_jaw_open(&mut self, holding: bool) -> f32 {
        self.jaw_open = self.jaw_open(holding);
        self.jaw_open
    }

    /// `JawOpen`'s setter: clamped, and shut while something is held.
    pub fn set_jaw_open(&mut self, value: f32, holding: bool) {
        self.jaw_open = if holding { 0.0 } else { value.clamp(0.0, 1.0) };
    }

    /// The stored `jawOpen` field, which some code writes directly.
    pub fn jaw_open_field_mut(&mut self) -> &mut f32 {
        &mut self.jaw_open
    }

    /// `JawReadyForBite`.
    pub fn jaw_ready_for_bite(&mut self, holding: bool) -> bool {
        self.read_jaw_open(holding) > 0.9 && self.bite_delay == 0
    }

    /// `BodyStiff`: unconscious, or conscious without a movement animation.
    #[must_use]
    pub fn body_stiff(&self) -> bool {
        !self.conscious() || self.movement_animation.is_none()
    }

    /// `ShortCutColor`.
    #[must_use]
    pub fn shortcut_color(&self) -> [f32; 3] {
        self.effect_color
    }

    /// `HitInMouth`: from below or level, the direction (bent a tenth toward up) within
    /// `Lerp(-15, 11, JawOpen)` degrees of the head's front.
    pub fn hit_in_mouth(&mut self, direction: Vec2, holding: bool) -> bool {
        if direction.y > 0.0 {
            return false;
        }
        let d = direction.slerp(Vec2::new(0.0, 1.0), 0.1);
        let front = -self.chunk_rotation(0);
        d.angle_to(front).abs() < (-15.0f32).lerp(11.0, self.read_jaw_open(holding))
    }

    /// `HitHeadShield`'s test (the caller plays `Lizard_Head_Shield_Deflect`): not in
    /// the mouth, and within `headShieldAngle + 20 * JawOpen` degrees of the front.
    pub fn hits_head_shield(&mut self, direction: Vec2, holding: bool) -> bool {
        let angle = direction.angle_to(-self.chunk_rotation(0));
        if self.hit_in_mouth(direction, holding) {
            return false;
        }
        angle < self.params.head_shield_angle + 20.0 * self.read_jaw_open(holding)
    }

    /// `PhysicalObject.WeightedPush(a, b, dir, frc)`.
    pub fn weighted_push(&mut self, a: usize, b: usize, dir: Vec2, frc: f32) {
        let share =
            self.body.chunks[b].mass / (self.body.chunks[a].mass + self.body.chunks[b].mass);
        self.body.chunks[a].vel += dir * (frc * share);
        self.body.chunks[b].vel -= dir * (frc * (1.0 - share));
    }
}

impl AbstractCreature {
    /// The `Mean:x` entry of `unrecognizedFlags` (`spawnDataEvil`), read as the
    /// `Lizard` constructor does: the last parsable one.
    #[must_use]
    pub fn mean_flag(&self) -> Option<f32> {
        let data = self.spawn_data.as_deref()?;
        let list = data.strip_prefix('{')?;
        let list = list.get(..list.len().saturating_sub(1))?;
        // `Mean` is a spawn-data key of the world file, not one of `setCustomFlags`'.
        list.split([',', '|'])
            .filter_map(|flag| {
                let mut parts = flag.split(':');
                let key = parts.next()?;
                let value = parts.next()?;
                (key == "Mean").then(|| value.trim().parse::<f32>().ok())?
            })
            .next_back()
    }

    /// The realized lizard.
    #[must_use]
    pub fn lizard(&self) -> Option<&Lizard> {
        match &self.realized {
            Some(super::RealizedCreature::Lizard(l)) => Some(l.as_ref()),
            _ => None,
        }
    }

    pub fn lizard_mut(&mut self) -> Option<&mut Lizard> {
        match &mut self.realized {
            Some(super::RealizedCreature::Lizard(l)) => Some(l.as_mut()),
            _ => None,
        }
    }
}
