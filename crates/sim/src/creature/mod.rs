//! The abstract creature layer (`AbstractWorldEntity`, `AbstractCreature`,
//! `AbstractCreatureAI`): every creature of the loaded world, the player included, with
//! its place in the world (`WorldCoordinate`), its state and its abstract AI. The world
//! keeps them in an arena ([`crate::world::World::creature`]) with per-room lists.
//!
//! Ported: the constructors with all their random draws for every base-game type, the
//! world-load population ([`population`]), the overseers' abstract AI ([`overseer`]),
//! dens, `Move` and the realize/abstractize hooks. Other types' AI behaviour is not
//! ported: they stay in their dens (docs/DIVERGENCES.md D14). Design:
//! `rainworld-re/notes/creature-model.md`.

pub(crate) mod ai;
pub mod overseer;
pub(crate) mod population;
pub mod template;

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use crate::FloatExt;

pub use ai::{AbstractCreatureAi, AiKind, CarriedItem, ItemKind};
pub use overseer::{OverseerAbstractAi, OverseerBody, PlayerGuideState};
pub use template::CreatureTemplate;

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

    /// `CompareDisregardingTile`.
    #[must_use]
    pub fn same_room_and_node(self, other: Self) -> bool {
        self.room == other.room && self.node == other.node
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureState {
    pub kind: StateKind,
    pub alive: bool,
    pub health: f32,
}

impl CreatureState {
    /// Whether the state is a `HealthState` (or a subclass).
    #[must_use]
    pub fn has_health(self) -> bool {
        !matches!(self.kind, StateKind::Player | StateKind::NoHealth)
    }
}

/// What is realized of a creature.
#[derive(Debug, Clone, PartialEq)]
pub enum RealizedCreature {
    /// The player's body (`RainWorldGame` keeps it).
    Player,
    Overseer(OverseerBody),
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
    pub in_den: bool,
    pub time_spent_here: i32,
    /// `evenUpdate`.
    pub even_update: bool,
    pub distance_to_my_node: i32,
    pub slated_for_deletion: bool,
    pub realized: Option<RealizedCreature>,
    /// Objects held by `CreatureGripStick`s (scavenger gear, a drop bug's item).
    pub carried: Vec<CarriedItem>,
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
        let kind = match top.value() {
            "Slugcat" => StateKind::Player,
            "LizardTemplate" => StateKind::Lizard,
            "Fly" | "Leech" | "Spider" => StateKind::NoHealth,
            "GarbageWorm" => StateKind::GarbageWorm,
            "LanternMouse" => StateKind::Mouse,
            "Vulture" => StateKind::Vulture,
            "DaddyLongLegs" => StateKind::Daddy,
            "VultureGrub" => StateKind::VultureGrub,
            "BigNeedleWorm" | "SmallNeedleWorm" => StateKind::NeedleWorm,
            "Centipede" if template.kind != CreatureType::SMALL_CENTIPEDE => StateKind::Centipede,
            _ => StateKind::Health,
        };
        CreatureState {
            kind,
            alive: true,
            health: 1.0,
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
            Some(RealizedCreature::Overseer(b)) => Some(b),
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
