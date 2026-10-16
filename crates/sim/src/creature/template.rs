//! `CreatureTemplate`s as `StaticWorld.InitStaticWorld` builds them, for the fields the
//! abstract world reads. A template inherits its ancestor's fields, then sets its own.
//!
//! The pathing data (resistances, swimming, `isTooCloseToTerrain`) is in [`super::pathing`].
//! Not ported: the relationships, lizard breed parameters and the cosmetic fields.

use rw_assets::{ActiveMods, CreatureType, NodeType};

/// A creature template.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureTemplate {
    pub kind: CreatureType,
    pub ancestor: Option<CreatureType>,
    /// `AI`: has an abstract and a realized AI.
    pub ai: bool,
    pub require_ai_map: bool,
    /// Counted per node instead of kept as creatures while abstract.
    pub quantified: bool,
    pub small_creature: bool,
    pub can_fly: bool,
    pub save_creature: bool,
    pub hibernate_off_screen: bool,
    pub off_screen_speed: f32,
    pub abstracted_laziness: i32,
    pub roam_in_room_chance: f32,
    pub roam_between_rooms_chance: f32,
    pub grasps: i32,
    pub body_size: f32,
    /// Allowed connections that map node types: the shortcut (Exit and Den nodes) and
    /// the sky, sea and side highways (their border exits).
    pub uses_shortcuts: bool,
    pub sky_highway: bool,
    pub sea_highway: bool,
    pub side_highway: bool,
    pub water_relationship: WaterRelationship,
}

/// `CreatureTemplate.Relationship.Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipType {
    DoesntTrack,
    Ignores,
    Eats,
    Afraid,
    StayOutOfWay,
    AgressiveRival,
    Attacks,
    Uncomfortable,
    Antagonizes,
    PlaysWith,
    SocialDependent,
    Pack,
}

impl RelationshipType {
    const ALL: [Self; 12] = [
        Self::DoesntTrack,
        Self::Ignores,
        Self::Eats,
        Self::Afraid,
        Self::StayOutOfWay,
        Self::AgressiveRival,
        Self::Attacks,
        Self::Uncomfortable,
        Self::Antagonizes,
        Self::PlaysWith,
        Self::SocialDependent,
        Self::Pack,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::DoesntTrack => "DoesntTrack",
            Self::Ignores => "Ignores",
            Self::Eats => "Eats",
            Self::Afraid => "Afraid",
            Self::StayOutOfWay => "StayOutOfWay",
            Self::AgressiveRival => "AgressiveRival",
            Self::Attacks => "Attacks",
            Self::Uncomfortable => "Uncomfortable",
            Self::Antagonizes => "Antagonizes",
            Self::PlaysWith => "PlaysWith",
            Self::SocialDependent => "SocialDependent",
            Self::Pack => "Pack",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.value() == value)
    }
}

/// `CreatureTemplate.Relationship`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relationship {
    pub kind: RelationshipType,
    pub intensity: f32,
}

impl Relationship {
    /// The slugcat's relationship to `kind`, for the types a room holds so far: Fly
    /// (Eats 0.5, 126784) and Overseer (Ignores 0, 126936).
    #[must_use]
    pub fn of_slugcat_to(kind: CreatureType) -> Option<Self> {
        let (kind, intensity) = match kind {
            CreatureType::Fly => (RelationshipType::Eats, 0.5),
            CreatureType::Overseer => (RelationshipType::Ignores, 0.0),
            _ => return None,
        };
        Some(Self { kind, intensity })
    }
}

/// `CreatureTemplate.WaterRelationship`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterRelationship {
    AirOnly,
    AirAndSurface,
    Amphibious,
    WaterOnly,
}

impl WaterRelationship {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::AirOnly => "AirOnly",
            Self::AirAndSurface => "AirAndSurface",
            Self::Amphibious => "Amphibious",
            Self::WaterOnly => "WaterOnly",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::AirOnly,
            Self::AirAndSurface,
            Self::Amphibious,
            Self::WaterOnly,
        ]
        .into_iter()
        .find(|w| w.value() == value)
    }

    /// `MovementLegalInRelationToWater`.
    #[must_use]
    pub fn movement_legal(self, deep_water: bool, water_surface: bool) -> bool {
        match self {
            Self::AirAndSurface => !deep_water,
            Self::AirOnly => !deep_water && !water_surface,
            Self::Amphibious => true,
            Self::WaterOnly => deep_water,
        }
    }
}

impl CreatureTemplate {
    /// The constructor's defaults for a template without an ancestor.
    const fn base(kind: CreatureType) -> Self {
        Self {
            kind,
            ancestor: None,
            ai: false,
            require_ai_map: false,
            quantified: false,
            small_creature: false,
            can_fly: false,
            save_creature: true,
            hibernate_off_screen: false,
            off_screen_speed: 1.0,
            abstracted_laziness: 1,
            roam_in_room_chance: 0.1,
            roam_between_rooms_chance: 0.1,
            grasps: 0,
            body_size: 1.0,
            uses_shortcuts: true,
            sky_highway: false,
            sea_highway: false,
            side_highway: false,
            water_relationship: WaterRelationship::Amphibious,
        }
    }

    /// A template inheriting `ancestor`'s fields.
    const fn child(kind: CreatureType, ancestor: &Self) -> Self {
        Self {
            kind,
            ancestor: Some(ancestor.kind),
            ..*ancestor
        }
    }

    /// `StaticWorld.GetCreatureTemplate`; unregistered types get the
    /// `StandardGroundCreature` stand-in the game fills empty slots with.
    #[must_use]
    pub fn of(kind: CreatureType, _mods: ActiveMods) -> Self {
        Self::BASE
            .iter()
            .find(|t| t.kind == kind)
            .copied()
            .unwrap_or_else(|| Self::base(CreatureType::StandardGroundCreature))
    }

    /// `TopAncestor().type`.
    #[must_use]
    pub fn top_ancestor(&self) -> CreatureType {
        let mut t = *self;
        while let Some(a) = t.ancestor {
            t = Self::of(a, ActiveMods::NONE);
        }
        t.kind
    }

    #[must_use]
    pub fn is_cicada(&self) -> bool {
        self.kind == CreatureType::CicadaA || self.kind == CreatureType::CicadaB
    }

    /// `mappedNodeTypes[type]`: Exit and Den by the shortcut connection (not Den for
    /// scavengers, which map RegionTransportation instead), the border exits by their
    /// highways, BatHive for flies, only GarbageHoles for garbage worms.
    #[must_use]
    pub fn maps_node(&self, node: NodeType) -> bool {
        let k = self.kind;
        if k == CreatureType::GarbageWorm {
            return node == NodeType::GarbageHoles;
        }
        match node {
            NodeType::Exit => self.uses_shortcuts,
            NodeType::Den => self.uses_shortcuts && k != CreatureType::Scavenger,
            NodeType::RegionTransportation => k == CreatureType::Scavenger,
            NodeType::SkyExit => self.sky_highway,
            NodeType::SeaExit => self.sea_highway,
            NodeType::SideExit => self.side_highway,
            NodeType::BatHive => k == CreatureType::Fly,
            NodeType::GarbageHoles => false,
        }
    }

    /// `PreBakedPathingIndex`: the room files' connectivity layer of the template, its
    /// `preBakedPathingAncestor`'s for the templates without their own, -1 for none. The
    /// 15 own layers follow the type order of the `doPreBakedPathing` templates; DLC
    /// types register after the base ones, so the base indices hold with mods on (their
    /// own templates are not ported).
    #[must_use]
    pub fn pre_baked_pathing_index(&self) -> i32 {
        use CreatureType as T;
        match self.kind {
            T::StandardGroundCreature | T::Slugcat | T::LanternMouse => 0,
            T::PinkLizard | T::YellowLizard | T::RedLizard | T::BlackLizard => 1,
            T::GreenLizard | T::TubeWorm | T::VultureGrub | T::Hazer => 2,
            T::BlueLizard
            | T::CyanLizard
            | T::Centipede
            | T::RedCentipede
            | T::SmallCentipede
            | T::BigSpider
            | T::SpitterSpider
            | T::DropBug => 3,
            T::WhiteLizard | T::EggBug => 4,
            T::Salamander => 5,
            T::Fly
            | T::CicadaA
            | T::CicadaB
            | T::Centiwing
            | T::BigNeedleWorm
            | T::SmallNeedleWorm => 6,
            T::Leech | T::SeaLeech | T::JetFish => 7,
            T::Snail | T::Spider => 8,
            T::Vulture | T::KingVulture | T::TempleGuard => 9,
            T::BigEel => 10,
            T::Deer => 11,
            T::DaddyLongLegs | T::BrotherLongLegs => 12,
            T::MirosBird => 13,
            T::Scavenger => 14,
            _ => -1,
        }
    }

    /// `RoomRealizer.GetCreaturePerformanceEstimation` (base game).
    #[must_use]
    pub fn performance_estimation(&self) -> f32 {
        let k = self.kind;
        match k {
            CreatureType::KingVulture => 140.0,
            CreatureType::Vulture => 100.0,
            CreatureType::Snail | CreatureType::DropBug => 20.0,
            CreatureType::Leech | CreatureType::Fly => 10.0,
            CreatureType::GarbageWorm | CreatureType::BigSpider => 30.0,
            CreatureType::BigEel | CreatureType::Scavenger => 300.0,
            CreatureType::Deer | CreatureType::DaddyLongLegs | CreatureType::BrotherLongLegs => {
                200.0
            }
            CreatureType::JetFish => 25.0,
            CreatureType::SpitterSpider => 50.0,
            _ if self.is_cicada() => 25.0,
            _ if self.top_ancestor() == CreatureType::LizardTemplate => 50.0,
            _ => 10.0,
        }
    }
}

/// Field values from `InitStaticWorld` (line numbers of the 1.11.8 decompile in
/// `rainworld-re/notes/creature-model.md`).
impl CreatureTemplate {
    const STANDARD: Self = Self {
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::StandardGroundCreature)
    };
    const SLUGCAT: Self = Self {
        grasps: 2,
        off_screen_speed: 0.1,
        ..Self::base(CreatureType::Slugcat)
    };
    const LIZARD: Self = Self {
        off_screen_speed: 0.3,
        grasps: 1,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 2.0,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::LizardTemplate)
    };
    const FLY: Self = Self {
        abstracted_laziness: 100,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 0.1,
        body_size: 0.1,
        quantified: true,
        grasps: 1,
        small_creature: true,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::Fly)
    };
    const LEECH: Self = Self {
        abstracted_laziness: 100,
        require_ai_map: true,
        off_screen_speed: 0.1,
        body_size: 0.1,
        quantified: true,
        grasps: 1,
        small_creature: true,
        water_relationship: WaterRelationship::WaterOnly,
        ..Self::base(CreatureType::Leech)
    };
    const SNAIL: Self = Self {
        off_screen_speed: 0.05,
        body_size: 0.7,
        ai: true,
        require_ai_map: true,
        ..Self::base(CreatureType::Snail)
    };
    const VULTURE: Self = Self {
        abstracted_laziness: 10,
        ai: true,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 0.8,
        body_size: 6.0,
        grasps: 1,
        hibernate_off_screen: true,
        uses_shortcuts: false,
        sky_highway: true,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::Vulture)
    };
    const GARBAGE_WORM: Self = Self {
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.0,
        grasps: 1,
        save_creature: false,
        uses_shortcuts: false,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::GarbageWorm)
    };
    const LANTERN_MOUSE: Self = Self {
        off_screen_speed: 0.3,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 0.7,
        grasps: 1,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::LanternMouse)
    };
    const CICADA: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 0.5,
        body_size: 0.6,
        grasps: 1,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::CicadaA)
    };
    const SPIDER: Self = Self {
        abstracted_laziness: 100,
        require_ai_map: true,
        off_screen_speed: 0.1,
        body_size: 0.1,
        quantified: true,
        grasps: 1,
        small_creature: true,
        ..Self::base(CreatureType::Spider)
    };
    const JET_FISH: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        off_screen_speed: 2.0,
        body_size: 0.8,
        grasps: 1,
        water_relationship: WaterRelationship::WaterOnly,
        ..Self::base(CreatureType::JetFish)
    };
    const BIG_EEL: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        body_size: 100.0,
        grasps: 1,
        hibernate_off_screen: true,
        uses_shortcuts: false,
        sea_highway: true,
        water_relationship: WaterRelationship::WaterOnly,
        ..Self::base(CreatureType::BigEel)
    };
    const DEER: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        body_size: 10.0,
        hibernate_off_screen: true,
        uses_shortcuts: false,
        side_highway: true,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::Deer)
    };
    const TUBE_WORM: Self = Self {
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.0,
        body_size: 0.25,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::TubeWorm)
    };
    const DADDY_LONG_LEGS: Self = Self {
        abstracted_laziness: 400,
        roam_between_rooms_chance: 0.05,
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.02,
        body_size: 5.5,
        grasps: 1,
        ..Self::base(CreatureType::DaddyLongLegs)
    };
    const TENTACLE_PLANT: Self = Self {
        ai: true,
        off_screen_speed: 0.0,
        body_size: 2.0,
        grasps: 1,
        uses_shortcuts: false,
        ..Self::base(CreatureType::TentaclePlant)
    };
    const MIROS_BIRD: Self = Self {
        abstracted_laziness: 10,
        ai: true,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 1.5,
        body_size: 7.0,
        grasps: 1,
        hibernate_off_screen: true,
        uses_shortcuts: false,
        side_highway: true,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::MirosBird)
    };
    const TEMPLE_GUARD: Self = Self {
        abstracted_laziness: 20000,
        roam_in_room_chance: -1.0,
        roam_between_rooms_chance: -1.0,
        ai: true,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 0.8,
        body_size: 7.0,
        grasps: 1,
        uses_shortcuts: false,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::TempleGuard)
    };
    const CENTIPEDE: Self = Self {
        off_screen_speed: 0.3,
        grasps: 2,
        abstracted_laziness: 150,
        ai: true,
        require_ai_map: true,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::Centipede)
    };
    const SCAVENGER: Self = Self {
        off_screen_speed: 0.75,
        grasps: 4,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 1.2,
        hibernate_off_screen: true,
        roam_between_rooms_chance: -1.0,
        roam_in_room_chance: -1.0,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::Scavenger)
    };
    /// No connection resistances: no node type is mapped.
    const OVERSEER: Self = Self {
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.0,
        body_size: 0.3,
        save_creature: false,
        uses_shortcuts: false,
        ..Self::base(CreatureType::Overseer)
    };
    /// No connection resistances: no node type is mapped.
    const VULTURE_GRUB: Self = Self {
        off_screen_speed: 0.0,
        body_size: 0.2,
        uses_shortcuts: false,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::VultureGrub)
    };
    const EGG_BUG: Self = Self {
        off_screen_speed: 0.3,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 0.4,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::EggBug)
    };
    const BIG_SPIDER: Self = Self {
        off_screen_speed: 0.3,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 1.1,
        grasps: 1,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::BigSpider)
    };
    const BIG_NEEDLE_WORM: Self = Self {
        abstracted_laziness: 10,
        roam_between_rooms_chance: 0.07,
        ai: true,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 1.5,
        body_size: 0.7,
        grasps: 1,
        water_relationship: WaterRelationship::AirOnly,
        ..Self::base(CreatureType::BigNeedleWorm)
    };
    const DROP_BUG: Self = Self {
        abstracted_laziness: 200,
        roam_between_rooms_chance: 0.025,
        off_screen_speed: 0.1,
        ai: true,
        require_ai_map: true,
        body_size: 1.2,
        grasps: 1,
        water_relationship: WaterRelationship::AirAndSurface,
        ..Self::base(CreatureType::DropBug)
    };

    /// Every base-game template, in `InitStaticWorld` order.
    const BASE: [Self; 47] = [
        Self::STANDARD,
        Self::SLUGCAT,
        Self::LIZARD,
        Self::child(CreatureType::PinkLizard, &Self::LIZARD),
        Self::child(CreatureType::GreenLizard, &Self::LIZARD),
        Self::child(CreatureType::BlueLizard, &Self::LIZARD),
        Self::child(CreatureType::YellowLizard, &Self::LIZARD),
        Self::child(CreatureType::WhiteLizard, &Self::LIZARD),
        Self::child(CreatureType::RedLizard, &Self::LIZARD),
        Self::child(CreatureType::BlackLizard, &Self::LIZARD),
        Self {
            water_relationship: WaterRelationship::Amphibious,
            ..Self::child(CreatureType::Salamander, &Self::LIZARD)
        },
        Self::child(CreatureType::CyanLizard, &Self::LIZARD),
        Self::FLY,
        Self::LEECH,
        Self {
            body_size: 0.15,
            ..Self::child(CreatureType::SeaLeech, &Self::LEECH)
        },
        Self::SNAIL,
        Self::VULTURE,
        Self {
            body_size: 8.5,
            ..Self::child(CreatureType::KingVulture, &Self::VULTURE)
        },
        Self::GARBAGE_WORM,
        Self::LANTERN_MOUSE,
        Self::CICADA,
        Self::child(CreatureType::CicadaB, &Self::CICADA),
        Self::SPIDER,
        Self::JET_FISH,
        Self::BIG_EEL,
        Self::DEER,
        Self::TUBE_WORM,
        Self::DADDY_LONG_LEGS,
        Self {
            body_size: 2.5,
            off_screen_speed: 0.1,
            ..Self::child(CreatureType::BrotherLongLegs, &Self::DADDY_LONG_LEGS)
        },
        Self::TENTACLE_PLANT,
        Self {
            ai: false,
            require_ai_map: true,
            body_size: 1.4,
            ..Self::child(CreatureType::PoleMimic, &Self::TENTACLE_PLANT)
        },
        Self::MIROS_BIRD,
        Self::TEMPLE_GUARD,
        Self::CENTIPEDE,
        Self {
            body_size: 8.5,
            ..Self::child(CreatureType::RedCentipede, &Self::CENTIPEDE)
        },
        Self {
            roam_between_rooms_chance: -1.0,
            abstracted_laziness: 1000,
            body_size: 0.2,
            ..Self::child(CreatureType::SmallCentipede, &Self::CENTIPEDE)
        },
        Self {
            can_fly: true,
            body_size: 1.2,
            ..Self::child(CreatureType::Centiwing, &Self::CENTIPEDE)
        },
        Self::SCAVENGER,
        Self::OVERSEER,
        Self::VULTURE_GRUB,
        Self {
            water_relationship: WaterRelationship::Amphibious,
            ..Self::child(CreatureType::Hazer, &Self::VULTURE_GRUB)
        },
        Self::EGG_BUG,
        Self::BIG_SPIDER,
        Self {
            body_size: 1.2,
            ..Self::child(CreatureType::SpitterSpider, &Self::BIG_SPIDER)
        },
        Self::BIG_NEEDLE_WORM,
        Self {
            small_creature: true,
            body_size: 0.15,
            roam_between_rooms_chance: 0.05,
            off_screen_speed: 3.2,
            ..Self::child(CreatureType::SmallNeedleWorm, &Self::BIG_NEEDLE_WORM)
        },
        Self::DROP_BUG,
    ];
}
