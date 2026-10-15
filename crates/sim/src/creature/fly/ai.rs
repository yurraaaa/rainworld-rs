//! `FlyAI`'s state, its `TrackedThreat`s and the flock behaviours flies share.

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use crate::creature::CreatureId;
use crate::{FloatExt, Vec2};

/// `FlyAI.Behavior`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyBehavior {
    Idle,
    Swarm,
    Drop,
    Flee,
    FleeFromRain,
    LeaveRoom,
    Burrow,
    Chain,
    Lure,
}

impl FlyBehavior {
    const ALL: [Self; 9] = [
        Self::Idle,
        Self::Swarm,
        Self::Drop,
        Self::Flee,
        Self::FleeFromRain,
        Self::LeaveRoom,
        Self::Burrow,
        Self::Chain,
        Self::Lure,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Swarm => "Swarm",
            Self::Drop => "Drop",
            Self::Flee => "Flee",
            Self::FleeFromRain => "FleeFromRain",
            Self::LeaveRoom => "LeaveRoom",
            Self::Burrow => "Burrow",
            Self::Chain => "Chain",
            Self::Lure => "Lure",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.value() == value)
    }
}

/// `FlyAI.DropStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropStatus {
    HaventDropped,
    Dropping,
    HasDropped,
}

impl DropStatus {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::HaventDropped => "HaventDropped",
            Self::Dropping => "Dropping",
            Self::HasDropped => "HasDropped",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [Self::HaventDropped, Self::Dropping, Self::HasDropped]
            .into_iter()
            .find(|d| d.value() == value)
    }
}

/// Index of a [`FlockBehavior`] in [`crate::world::World::flocks`].
pub type FlockId = usize;

/// `FlyAI.FlockBehavior` (185129): shared by the flies that adopt it; only its leader
/// advances it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlockBehavior {
    pub drop: bool,
    pub form_chains: bool,
    pub sin_cycle: f32,
    pub swarm_flight_sin_freq: f32,
    pub swarm_flight_sin_ampl: f32,
    pub swarm_flight_vertical_displace: f32,
    pub distance_to_others: f32,
    pub exaggerate_direction_of_others: f32,
    pub collective_movement: f32,
    pub flock_dir: Vec2,
    pub leader: CreatureId,
    pub life: f32,
    pub life_time: f32,
    pub dom: f32,
    /// `col`'s hue (`HSL2RGB(hue, 1, 0.5)`; unused by the game but drawn).
    pub hue: f32,
}

impl FlockBehavior {
    /// The constructor (185163): 14 draws, 15 when `collectiveMovement` is not 0.
    pub fn new(leader: CreatureId, rng: &mut UnityRandom) -> Self {
        let sin_cycle = rng.value();
        let drop = rng.value() < 0.5;
        let distance_to_others = 25.0 + rng.value() * 75.0;
        let hue = rng.value();
        let swarm_flight_vertical_displace = 200.0.lerp(320.0, rng.value());
        let swarm_flight_sin_freq = 1.0 / (30.0 + 120.0 * rng.value());
        let swarm_flight_sin_ampl = rng.value() * 80.0;
        let dir = Vec2::random_unit(rng);
        let flock_dir = dir * (rng.value() * 140.0);
        let exaggerate_direction_of_others = rng.value();
        let collective_movement = if rng.value() < 0.5 {
            0.0
        } else {
            rng.value().mathf_pow(10.0)
        };
        let form_chains = rng.value() < 0.5;
        let life_time = 200.0.lerp(800.0, rng.value());
        let dom = rng.value();
        Self {
            drop,
            form_chains,
            sin_cycle,
            swarm_flight_sin_freq,
            swarm_flight_sin_ampl,
            swarm_flight_vertical_displace,
            distance_to_others,
            exaggerate_direction_of_others,
            collective_movement,
            flock_dir,
            leader,
            life: 1.0,
            life_time,
            dom,
            hue,
        }
    }

    /// `Dominance`.
    #[must_use]
    pub fn dominance(&self) -> f32 {
        self.dom * self.life.inverse_lerp(0.0, 0.2)
    }

    /// `Update`, run by the leader only.
    pub fn update(&mut self) {
        self.sin_cycle += self.swarm_flight_sin_freq;
        self.life -= 1.0 / self.life_time;
    }
}

/// `FlyAI.TrackedThreat` (185067).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackedThreat {
    pub crit: CreatureId,
    pub crit_kind: CreatureType,
    pub ticks_since_seen: i32,
    pub delete_me_next_frame: bool,
    pub visual_contact: bool,
    pub currently_scared_of: bool,
    pub ticks_to_forget: i32,
    /// Unset (zero) until the first update sees the creature.
    pub last_seen_pos: Vec2,
}

impl TrackedThreat {
    pub fn new(crit: CreatureId, crit_kind: CreatureType) -> Self {
        Self {
            crit,
            crit_kind,
            ticks_since_seen: 0,
            delete_me_next_frame: false,
            visual_contact: true,
            ticks_to_forget: 50.0.lerp(250.0, FlyAi::fear(crit_kind)) as i32,
            currently_scared_of: false,
            last_seen_pos: Vec2::ZERO,
        }
    }

    /// `Severity`.
    #[must_use]
    pub fn severity(&self) -> f32 {
        FlyAi::fear(self.crit_kind)
            * 1.0.lerp(
                0.0,
                self.ticks_since_seen as f32 / self.ticks_to_forget as f32,
            )
    }
}

/// `FlyAI`'s fields (185188).
#[derive(Debug, Clone, PartialEq)]
pub struct FlyAi {
    pub local_goal: Vec2,
    pub following_dijkstra_map: i32,
    pub leave_room_dijkstra: i32,
    pub other_fly: Option<CreatureId>,
    pub lowest_other_pos: Vec2,
    pub flock: FlockId,
    pub threats: Vec<TrackedThreat>,
    pub afraid: f32,
    pub panic_flee: i32,
    pub flee_from_rain: bool,
    /// `dropStatus`: null until first set.
    pub drop_status: Option<DropStatus>,
    pub stuck_counter: i32,
    pub stuck_pos: Vec2,
    pub get_back_to_ground: i32,
    pub no_swarm_counter: i32,
    /// `luredCounter`: always 0 (no lures are ported).
    pub lured_counter: i32,
    /// `behavior`: null until the first `ChangeBehavior`.
    pub behavior: Option<FlyBehavior>,
}

impl FlyAi {
    /// The constructor (185291) after its flock: `fleeFromRain` when the rain is near.
    #[must_use]
    pub fn new(flock: FlockId, rain_approaching: f32) -> Self {
        Self {
            local_goal: Vec2::ZERO,
            following_dijkstra_map: -1,
            leave_room_dijkstra: -1,
            other_fly: None,
            lowest_other_pos: Vec2::ZERO,
            flock,
            threats: Vec::new(),
            afraid: 0.0,
            panic_flee: 0,
            flee_from_rain: rain_approaching < 0.3,
            drop_status: None,
            stuck_counter: 0,
            stuck_pos: Vec2::ZERO,
            get_back_to_ground: 0,
            no_swarm_counter: 0,
            lured_counter: 0,
            behavior: None,
        }
    }

    /// The Fly's `Afraid` relationship intensity to `kind` (126640-126646, passed on to
    /// descendant templates), 0 for the rest (Uncomfortable 0 by default).
    #[must_use]
    pub fn fear(kind: CreatureType) -> f32 {
        use CreatureType as T;
        match kind {
            T::Leech | T::SeaLeech | T::Spider | T::TentaclePlant | T::PoleMimic => 0.05,
            T::CicadaA | T::CicadaB => 0.7,
            T::JetFish => 0.6,
            T::Slugcat | T::SmallCentipede => 0.5,
            _ => 0.0,
        }
    }

    /// Whether the Fly's relationship to `kind` is `Afraid`.
    #[must_use]
    pub fn afraid_of(kind: CreatureType) -> bool {
        Self::fear(kind) > 0.0
    }

    /// `AfraidDistance`.
    #[must_use]
    pub fn afraid_distance(kind: CreatureType) -> f32 {
        if Self::afraid_of(kind) {
            50.0.lerp(600.0, Self::fear(kind))
        } else {
            0.0
        }
    }

    /// `Stuck`.
    #[must_use]
    pub fn stuck(&self) -> bool {
        self.stuck_counter >= 40
            && self.behavior != Some(FlyBehavior::Chain)
            && self.behavior != Some(FlyBehavior::Burrow)
    }

    /// `DoingSpecificAnimation`.
    #[must_use]
    pub fn doing_specific_animation(&self) -> bool {
        matches!(self.behavior, Some(FlyBehavior::Drop | FlyBehavior::Burrow))
    }
}
