//! The abstract creature layer (`AbstractWorldEntity`, `AbstractCreature`,
//! `AbstractCreatureAI`): every creature of the loaded world, the player included, with
//! its place in the world (`WorldCoordinate`), its state and its abstract AI. The world
//! keeps them in an arena ([`crate::world::World::creature`]) with per-room lists.
//!
//! Ported: the constructors with all their random draws for every base-game type, the
//! world-load population ([`population`]), the overseers' abstract AI ([`overseer`]),
//! the plain `AbstractCreatureAI` (`world/abstract_ai.rs`) and the other subclasses
//! ([`ai_kinds`], `world/abstract_ai_kinds.rs`) for every type but the scavenger (which
//! stays in its den, docs/divergences/ D14), the relationships, dens, `Move` and the
//! realize/abstractize hooks. Design:
//! `rainworld-re/notes/creature-model.md`.
//!
//! Mirrors: AbstractWorldEntity.cs, AbstractCreature.cs, AbstractCreatureAI.cs

pub mod agression_tracker;
pub(crate) mod ai;
pub mod ai_kinds;
pub mod danger;
pub mod death;
pub mod den_finder;
pub mod fly;
pub mod grasp;
pub mod lizard;
pub mod looker;
pub mod mission_tracker;
pub mod noise_tracker;
pub mod overseer;
pub mod path_finder;
pub mod pathing;
pub(crate) mod population;
pub mod prey_tracker;
pub mod rain_tracker;
mod relationship_tracker;
mod relationships;
pub mod social_memory;
mod state_save;
pub mod stuck_tracker;
pub mod template;
mod template_abstract;
pub mod threat_tracker;
pub mod tracker;
pub mod utility_comparer;
pub mod violence;
pub mod vision;

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use crate::FloatExt;

pub use agression_tracker::{AgressionTracker, AngerRelationship};
pub use ai::{AbstractCreatureAi, AiKind, CarriedItem, ItemKind};
pub use danger::{CreatureDanger, TrackedRep};
pub use death::{CommunityInfluence, SessionDeaths, SessionKill};
pub use den_finder::{DenFinder, DenSearch, NodeFinderStatus};
pub use fly::{
    DropStatus, FlockBehavior, FlockId, Fly, FlyAi, FlyBehavior, FlyView, HoverEmitter,
    MovementMode, TrackedThreat,
};
pub use grasp::{GrabRequest, Grasp, GripStick, Shareability};
pub use lizard::{LUNG_CAPACITY, Lizard, LizardAnimation, LizardBreedParams};
pub use looker::{CreatureLooker, LookCandidate, LookCandidateParts};
pub use mission_tracker::{LeaveRoom, Mission, MissionData, MissionKind, MissionTracker};
pub use noise_tracker::{HeardNoise, NoiseKind, NoiseTracker, TheorizedSource};
pub use overseer::{OverseerAbstractAi, OverseerAi, OverseerBody, PlayerGuideState};
pub use path_finder::{PathFinder, PathFinders, PatherKind};
pub use prey_tracker::{PreyTracker, TrackedPrey};
pub use rain_tracker::RainTracker;
pub use relationship_tracker::{
    DynamicRelationship, LizardTrackState, RelationshipTracker, TrackedCreatureState,
    TrackingModule,
};
pub use social_memory::{Relationship as SocialRelationship, SocialMemory};
pub use state_save::{StateParts, StateSave};
pub use stuck_tracker::{DiscomfortTracker, MoveBacklog, StuckTracker};
pub use template::{CreatureTemplate, Relationship, RelationshipType, WaterRelationship};
pub use template_abstract::{AbstractCost, AbstractMovement};
pub use threat_tracker::{ThreatCreature, ThreatPoint, ThreatTracker};
pub use tracker::{
    BestGhost, CreatureRep, ElaborateRep, Ghost, GhostCloneCx, GhostParams, RepKey, RepKind,
    RepReading, Sighting, Tracker,
};
pub use utility_comparer::{
    FloatTween, FloatTweenBasic, RawUtilities, TweenType, UtilityComparer, UtilityModule,
    UtilityTracker,
};
pub use violence::{Community, DamageType, ViolenceTemplate};
pub use vision::{ChunkVisibility, CreatureVision, Eyes, LizardSight, PlayerVisibility};

/// `EntityID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    pub spawner: i32,
    pub number: i32,
    /// `altSeed`, -1 when unset.
    pub alt_seed: i32,
}

impl EntityId {
    #[must_use]
    pub const fn new(spawner: i32, number: i32) -> Self {
        Self {
            spawner,
            number,
            alt_seed: -1,
        }
    }

    /// `EntityID ==`: the game compares the numbers only.
    #[must_use]
    pub fn same_entity(self, other: Self) -> bool {
        self.number == other.number
    }

    /// `EntityID.FromString` (`ID.spawner.number`); `None` where the game would throw.
    #[must_use]
    pub fn from_save_string(s: &str) -> Option<Self> {
        let mut parts = s.split('.').skip(1);
        let spawner = parts.next()?.trim().parse().ok()?;
        let number = parts.next()?.trim().parse().ok()?;
        Some(Self::new(spawner, number))
    }

    /// `RandomSeed`.
    #[must_use]
    pub fn random_seed(self) -> i32 {
        if self.alt_seed > -1 {
            self.alt_seed
        } else {
            self.number
        }
    }
}

/// `RainWorldGame.nextIssuedId` and `GetNewID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdIssuer {
    pub next_issued_id: i32,
}

impl IdIssuer {
    /// `GetNewID(spawner)` (`GetNewID()` is spawner -1).
    pub fn new_id(&mut self, spawner: i32) -> EntityId {
        self.next_issued_id += 1;
        EntityId::new(spawner, self.next_issued_id)
    }
}

/// `WorldCoordinate` (rooms by global index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoordinate {
    pub room: i32,
    pub tile: (i32, i32),
    /// `abstractNode`, -1 when undefined.
    pub node: i32,
}

impl WorldCoordinate {
    #[must_use]
    pub const fn new(room: i32, x: i32, y: i32, node: i32) -> Self {
        Self {
            room,
            tile: (x, y),
            node,
        }
    }

    #[must_use]
    pub fn tile_defined(self) -> bool {
        self.tile.0 != -1 || self.tile.1 != -1
    }

    #[must_use]
    pub fn node_defined(self) -> bool {
        self.node > -1
    }

    /// `WashNode`: the node left undefined.
    #[must_use]
    pub fn wash_node(self) -> Self {
        Self { node: -1, ..self }
    }

    /// `WorldCoordinate + IntVector2` (`AddIntVector`): the tile moved, the node kept.
    #[must_use]
    pub fn add_tile(self, d: (i32, i32)) -> Self {
        Self {
            tile: (self.tile.0 + d.0, self.tile.1 + d.1),
            ..self
        }
    }

    /// `CompareDisregardingNode`.
    #[must_use]
    pub fn same_room_and_tile(self, other: Self) -> bool {
        self.room == other.room && self.tile == other.tile
    }

    /// `CompareDisregardingTile`.
    #[must_use]
    pub fn same_room_and_node(self, other: Self) -> bool {
        self.room == other.room && self.node == other.node
    }

    /// `Custom.ManhattanDistance` of two coordinates: `int.MaxValue` across rooms.
    #[must_use]
    pub fn manhattan_distance(self, other: Self) -> i32 {
        if self.room != other.room {
            return i32::MAX;
        }
        (self.tile.0 - other.tile.0).abs() + (self.tile.1 - other.tile.1).abs()
    }
}

/// Index of a creature in the world's arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CreatureId(pub usize);

/// `AbstractCreature.Personality`, drawn from the ID's seed with the global state saved
/// and restored (no net draw).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Personality {
    pub sympathy: f32,
    pub energy: f32,
    pub bravery: f32,
    pub nervous: f32,
    pub aggression: f32,
    pub dominance: f32,
}

impl Personality {
    pub fn new(id: EntityId, rng: &mut UnityRandom) -> Self {
        let saved = rng.state();
        rng.init_state(id.random_seed());
        let sympathy = rng.value().push_from_half(1.5);
        let energy = rng.value().push_from_half(1.5);
        let bravery = rng.value().push_from_half(1.5);
        let a = rng.value();
        let nervous = a.lerp(energy.lerp(1.0 - bravery, 0.5), rng.value().mathf_pow(0.25));
        let a = rng.value();
        let aggression = a.lerp(
            (energy + bravery) / 2.0 * (1.0 - sympathy),
            rng.value().mathf_pow(0.25),
        );
        let a = rng.value();
        let dominance = a.lerp(
            (energy + bravery + aggression) / 3.0,
            rng.value().mathf_pow(0.25),
        );
        rng.set_state(saved);
        Self {
            sympathy,
            energy,
            bravery,
            nervous: nervous.push_from_half(2.5),
            aggression: aggression.push_from_half(2.5),
            dominance,
        }
    }
}

/// Which `CreatureState` subclass the constructor builds (by top ancestor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Player,
    Lizard,
    NoHealth,
    GarbageWorm,
    Mouse,
    Vulture,
    Daddy,
    VultureGrub,
    NeedleWorm,
    Centipede,
    Health,
}

/// `CreatureState`: alive or dead, and health where the state has it (1 when created).
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureState {
    pub kind: StateKind,
    pub alive: bool,
    pub health: f32,
    /// `PlayerState.foodInStomach` (0 for other states); the base game never takes it
    /// below 0 (only Jolly Co-op's `SubtractFood` and More Slugcats' pups can).
    pub food_in_stomach: u32,
    /// `PlayerState.quarterFoodPoints`.
    pub quarter_food_points: i32,
    /// `socialMemory`: an empty one for templates with `socialMemory`; a save's `Social`
    /// entry gives one to any creature.
    pub social_memory: Option<SocialMemory>,
    /// The subclass's own saved fields.
    pub parts: StateParts,
    /// Saved fields nothing in the port reads.
    pub save: StateSave,
}

impl CreatureState {
    /// Whether the state is a `HealthState` (or a subclass): not the player's, the
    /// garbage worm's, the vulture grub's or a `NoHealthState`.
    #[must_use]
    pub fn has_health(&self) -> bool {
        !matches!(
            self.kind,
            StateKind::Player
                | StateKind::NoHealth
                | StateKind::GarbageWorm
                | StateKind::VultureGrub
        )
    }
}

/// What is realized of a creature.
#[derive(Debug, Clone, PartialEq)]
pub enum RealizedCreature {
    /// The player's body (`RainWorldGame` keeps it).
    Player,
    Overseer(Box<OverseerBody>),
    Fly(Box<Fly>),
    Lizard(Box<Lizard>),
}

/// The base-game creature flags of `setCustomFlags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnFlag {
    Night,
    Ignorecycle,
    PreCycle,
}

impl SpawnFlag {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Night => "Night",
            Self::Ignorecycle => "Ignorecycle",
            Self::PreCycle => "PreCycle",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [Self::Night, Self::Ignorecycle, Self::PreCycle]
            .into_iter()
            .find(|f| f.value() == value)
    }
}

/// `AbstractCreature`.
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractCreature {
    pub template: CreatureTemplate,
    pub id: EntityId,
    pub pos: WorldCoordinate,
    pub personality: Personality,
    pub state: CreatureState,
    pub ai: Option<AbstractCreatureAi>,
    pub remain_in_den_counter: i32,
    pub spawn_den: Option<WorldCoordinate>,
    /// `spawnData` of the spawner (unrecognized flags are kept here as the game keeps
    /// them; `setCustomFlags` reads none of the vanilla ones).
    pub spawn_data: Option<String>,
    pub night_creature: bool,
    pub ignore_cycle: bool,
    /// `preCycle` (the `PreCycle` spawn flag): with no precycles in the base game it
    /// keeps the creature in its den.
    pub pre_cycle: bool,
    pub in_den: bool,
    pub time_spent_here: i32,
    /// `evenUpdate`.
    pub even_update: bool,
    pub distance_to_my_node: i32,
    pub slated_for_deletion: bool,
    pub realized: Option<RealizedCreature>,
    /// Objects held by `CreatureGripStick`s (scavenger gear, a drop bug's item).
    pub carried: Vec<CarriedItem>,
    /// The realized creature's `grasps` (empty while abstract).
    pub grasps: Vec<Option<Grasp>>,
    /// The realized creature's `grabbedBy`.
    pub grabbed_by: Vec<Grasp>,
    /// `stuckObjects` between creatures, in the order they were made.
    pub sticks: Vec<GripStick>,
    /// The realized creature's `killTag` (cleared when it is realized anew) and
    /// `killTagCounter`.
    pub kill_tag: Option<CreatureId>,
    pub kill_tag_counter: i32,
}

impl AbstractCreature {
    #[must_use]
    pub fn kind(&self) -> CreatureType {
        self.template.kind
    }

    /// The state the constructor builds.
    #[must_use]
    pub fn state_for(template: &CreatureTemplate) -> CreatureState {
        let top = template.top_ancestor();
        let kind = match top {
            CreatureType::Slugcat => StateKind::Player,
            CreatureType::LizardTemplate => StateKind::Lizard,
            CreatureType::Fly | CreatureType::Leech | CreatureType::Spider => StateKind::NoHealth,
            CreatureType::GarbageWorm => StateKind::GarbageWorm,
            CreatureType::LanternMouse => StateKind::Mouse,
            CreatureType::Vulture => StateKind::Vulture,
            CreatureType::DaddyLongLegs => StateKind::Daddy,
            CreatureType::VultureGrub => StateKind::VultureGrub,
            CreatureType::BigNeedleWorm | CreatureType::SmallNeedleWorm => StateKind::NeedleWorm,
            CreatureType::Centipede if template.kind != CreatureType::SmallCentipede => {
                StateKind::Centipede
            }
            _ => StateKind::Health,
        };
        CreatureState {
            kind,
            alive: true,
            health: 1.0,
            food_in_stomach: 0,
            quarter_food_points: 0,
            social_memory: template.social_memory().then(SocialMemory::default),
            parts: StateParts::of(kind),
            save: StateSave::default(),
        }
    }

    /// `Quantify`: a living quantified creature with stuck objects (the game's test reads
    /// `stuckObjects.Count == 0` as false, so a free fly does not quantify).
    #[must_use]
    pub fn quantify(&self) -> bool {
        self.template.quantified
            && self.state.alive
            && (!self.carried.is_empty() || !self.sticks.is_empty())
    }

    /// `setCustomFlags`' base-game flags: `nightCreature` and `ignoreCycle` are cleared,
    /// then the spawn data's `{...}` list sets `Night`, `Ignorecycle` and `PreCycle`. The
    /// others (DLC flags, `Seed`, `NamedAttr`, numbers) are kept in `spawn_data` only;
    /// the region's `globalCreatureFlags` are not read (no vanilla region sets them).
    pub fn set_custom_flags(&mut self) {
        self.night_creature = false;
        self.ignore_cycle = false;
        let Some(data) = self.spawn_data.as_deref() else {
            return;
        };
        let Some(list) = data
            .strip_prefix('{')
            .and_then(|d| d.get(..d.len().saturating_sub(1)))
        else {
            return;
        };
        for flag in list.split([',', '|']) {
            match SpawnFlag::from_value(flag.split(':').next().unwrap_or("")) {
                Some(SpawnFlag::Night) => {
                    self.night_creature = true;
                    self.ignore_cycle = false;
                }
                Some(SpawnFlag::Ignorecycle) => self.ignore_cycle = true,
                Some(SpawnFlag::PreCycle) => self.pre_cycle = true,
                None => {}
            }
        }
    }

    /// `PacifiedBecauseCarried`: a grip stick where `B == this` equals its `carry`.
    #[must_use]
    pub fn pacified_because_carried(&self, me: CreatureId) -> bool {
        self.sticks
            .iter()
            .any(|s| (s.b == crate::item::Entity::Creature(me)) == s.carry)
    }

    /// The realized object's `slatedForDeletetion`.
    #[must_use]
    pub fn realized_slated(&self) -> bool {
        match &self.realized {
            Some(RealizedCreature::Fly(f)) => f.slated,
            Some(RealizedCreature::Lizard(l)) => l.slated,
            _ => false,
        }
    }

    /// The realized fly.
    #[must_use]
    pub fn fly(&self) -> Option<&Fly> {
        match &self.realized {
            Some(RealizedCreature::Fly(f)) => Some(f.as_ref()),
            _ => None,
        }
    }

    pub fn fly_mut(&mut self) -> Option<&mut Fly> {
        match &mut self.realized {
            Some(RealizedCreature::Fly(f)) => Some(f.as_mut()),
            _ => None,
        }
    }

    /// `realizedCreature != null`.
    #[must_use]
    pub fn is_realized(&self) -> bool {
        self.realized.is_some()
    }

    /// The realized overseer's body.
    #[must_use]
    pub fn overseer_body(&self) -> Option<&OverseerBody> {
        match &self.realized {
            Some(RealizedCreature::Overseer(b)) => Some(b.as_ref()),
            _ => None,
        }
    }

    #[must_use]
    pub fn overseer_ai(&self) -> Option<&OverseerAbstractAi> {
        match &self.ai.as_ref()?.kind {
            AiKind::Overseer(o) => Some(o),
            _ => None,
        }
    }

    pub fn overseer_ai_mut(&mut self) -> Option<&mut OverseerAbstractAi> {
        match &mut self.ai.as_mut()?.kind {
            AiKind::Overseer(o) => Some(o),
            _ => None,
        }
    }
}

/// `Custom.PushFromHalf`.
trait PushFromHalf {
    fn push_from_half(self, exponent: f32) -> f32;
}

impl PushFromHalf for f32 {
    fn push_from_half(self, exponent: f32) -> f32 {
        if self == 0.5 {
            0.5
        } else if self < 0.5 {
            self.lerp_map_pow(0.0, 0.5, 0.0, 0.5, exponent)
        } else {
            self.lerp_map_pow(1.0, 0.5, 1.0, 0.5, exponent)
        }
    }
}
