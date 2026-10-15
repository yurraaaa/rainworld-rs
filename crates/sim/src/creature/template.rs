//! `CreatureTemplate`s as `StaticWorld.InitStaticWorld` builds them, for the fields the
//! abstract world reads. A template inherits its ancestor's fields, then sets its own.
//!
//! Not ported: tile and connection resistances beyond the node types they map, the
//! relationships, lizard breed parameters and the cosmetic fields.

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
            .unwrap_or_else(|| Self::base(CreatureType::STANDARD_GROUND_CREATURE))
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
        self.kind == CreatureType::CICADA_A || self.kind == CreatureType::CICADA_B
    }

    /// `mappedNodeTypes[type]`: Exit and Den by the shortcut connection (not Den for
    /// scavengers, which map RegionTransportation instead), the border exits by their
    /// highways, BatHive for flies, only GarbageHoles for garbage worms.
    #[must_use]
    pub fn maps_node(&self, node: NodeType) -> bool {
        let k = self.kind;
        if k == CreatureType::GARBAGE_WORM {
            return node == NodeType::GarbageHoles;
        }
        match node {
            NodeType::Exit => self.uses_shortcuts,
            NodeType::Den => self.uses_shortcuts && k != CreatureType::SCAVENGER,
            NodeType::RegionTransportation => k == CreatureType::SCAVENGER,
            NodeType::SkyExit => self.sky_highway,
            NodeType::SeaExit => self.sea_highway,
            NodeType::SideExit => self.side_highway,
            NodeType::BatHive => k == CreatureType::FLY,
            NodeType::GarbageHoles => false,
        }
    }

    /// `RoomRealizer.GetCreaturePerformanceEstimation` (base game).
    #[must_use]
    pub fn performance_estimation(&self) -> f32 {
        let k = self.kind;
        match k.value() {
            "KingVulture" => 140.0,
            "Vulture" => 100.0,
            "Snail" | "DropBug" => 20.0,
            "Leech" | "Fly" => 10.0,
            "GarbageWorm" | "BigSpider" => 30.0,
            "BigEel" | "Scavenger" => 300.0,
            "Deer" | "DaddyLongLegs" | "BrotherLongLegs" => 200.0,
            "JetFish" => 25.0,
            "SpitterSpider" => 50.0,
            _ if self.is_cicada() => 25.0,
            _ if self.top_ancestor() == CreatureType::LIZARD_TEMPLATE => 50.0,
            _ => 10.0,
        }
    }
}

/// Field values from `InitStaticWorld` (line numbers of the 1.11.8 decompile in
/// `rainworld-re/notes/creature-model.md`).
impl CreatureTemplate {
    const STANDARD: Self = Self::base(CreatureType::STANDARD_GROUND_CREATURE);
    const SLUGCAT: Self = Self {
        grasps: 2,
        off_screen_speed: 0.1,
        ..Self::base(CreatureType::SLUGCAT)
    };
    const LIZARD: Self = Self {
        off_screen_speed: 0.3,
        grasps: 1,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 2.0,
        ..Self::base(CreatureType::LIZARD_TEMPLATE)
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
        ..Self::base(CreatureType::FLY)
    };
    const LEECH: Self = Self {
        abstracted_laziness: 100,
        require_ai_map: true,
        off_screen_speed: 0.1,
        body_size: 0.1,
        quantified: true,
        grasps: 1,
        small_creature: true,
        ..Self::base(CreatureType::LEECH)
    };
    const SNAIL: Self = Self {
        off_screen_speed: 0.05,
        body_size: 0.7,
        ai: true,
        require_ai_map: true,
        ..Self::base(CreatureType::SNAIL)
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
        ..Self::base(CreatureType::VULTURE)
    };
    const GARBAGE_WORM: Self = Self {
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.0,
        grasps: 1,
        save_creature: false,
        uses_shortcuts: false,
        ..Self::base(CreatureType::GARBAGE_WORM)
    };
    const LANTERN_MOUSE: Self = Self {
        off_screen_speed: 0.3,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 0.7,
        grasps: 1,
        ..Self::base(CreatureType::LANTERN_MOUSE)
    };
    const CICADA: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        can_fly: true,
        off_screen_speed: 0.5,
        body_size: 0.6,
        grasps: 1,
        ..Self::base(CreatureType::CICADA_A)
    };
    const SPIDER: Self = Self {
        abstracted_laziness: 100,
        require_ai_map: true,
        off_screen_speed: 0.1,
        body_size: 0.1,
        quantified: true,
        grasps: 1,
        small_creature: true,
        ..Self::base(CreatureType::SPIDER)
    };
    const JET_FISH: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        off_screen_speed: 2.0,
        body_size: 0.8,
        grasps: 1,
        ..Self::base(CreatureType::JET_FISH)
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
        ..Self::base(CreatureType::BIG_EEL)
    };
    const DEER: Self = Self {
        abstracted_laziness: 100,
        ai: true,
        require_ai_map: true,
        body_size: 10.0,
        hibernate_off_screen: true,
        uses_shortcuts: false,
        side_highway: true,
        ..Self::base(CreatureType::DEER)
    };
    const TUBE_WORM: Self = Self {
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.0,
        body_size: 0.25,
        ..Self::base(CreatureType::TUBE_WORM)
    };
    const DADDY_LONG_LEGS: Self = Self {
        abstracted_laziness: 400,
        roam_between_rooms_chance: 0.05,
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.02,
        body_size: 5.5,
        grasps: 1,
        ..Self::base(CreatureType::DADDY_LONG_LEGS)
    };
    const TENTACLE_PLANT: Self = Self {
        ai: true,
        off_screen_speed: 0.0,
        body_size: 2.0,
        grasps: 1,
        uses_shortcuts: false,
        ..Self::base(CreatureType::TENTACLE_PLANT)
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
        ..Self::base(CreatureType::MIROS_BIRD)
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
        ..Self::base(CreatureType::TEMPLE_GUARD)
    };
    const CENTIPEDE: Self = Self {
        off_screen_speed: 0.3,
        grasps: 2,
        abstracted_laziness: 150,
        ai: true,
        require_ai_map: true,
        ..Self::base(CreatureType::CENTIPEDE)
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
        ..Self::base(CreatureType::SCAVENGER)
    };
    /// No connection resistances: no node type is mapped.
    const OVERSEER: Self = Self {
        ai: true,
        require_ai_map: true,
        off_screen_speed: 0.0,
        body_size: 0.3,
        save_creature: false,
        uses_shortcuts: false,
        ..Self::base(CreatureType::OVERSEER)
    };
    /// No connection resistances: no node type is mapped.
    const VULTURE_GRUB: Self = Self {
        off_screen_speed: 0.0,
        body_size: 0.2,
        uses_shortcuts: false,
        ..Self::base(CreatureType::VULTURE_GRUB)
    };
    const EGG_BUG: Self = Self {
        off_screen_speed: 0.3,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 0.4,
        ..Self::base(CreatureType::EGG_BUG)
    };
    const BIG_SPIDER: Self = Self {
        off_screen_speed: 0.3,
        abstracted_laziness: 50,
        ai: true,
        require_ai_map: true,
        body_size: 1.1,
        grasps: 1,
        ..Self::base(CreatureType::BIG_SPIDER)
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
        ..Self::base(CreatureType::BIG_NEEDLE_WORM)
    };
    const DROP_BUG: Self = Self {
        abstracted_laziness: 200,
        roam_between_rooms_chance: 0.025,
        off_screen_speed: 0.1,
        ai: true,
        require_ai_map: true,
        body_size: 1.2,
        grasps: 1,
        ..Self::base(CreatureType::DROP_BUG)
    };

    /// Every base-game template, in `InitStaticWorld` order.
    const BASE: [Self; 47] = [
        Self::STANDARD,
        Self::SLUGCAT,
        Self::LIZARD,
        Self::child(CreatureType::PINK_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::GREEN_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::BLUE_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::YELLOW_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::WHITE_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::RED_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::BLACK_LIZARD, &Self::LIZARD),
        Self::child(CreatureType::SALAMANDER, &Self::LIZARD),
        Self::child(CreatureType::CYAN_LIZARD, &Self::LIZARD),
        Self::FLY,
        Self::LEECH,
        Self {
            body_size: 0.15,
            ..Self::child(CreatureType::SEA_LEECH, &Self::LEECH)
        },
        Self::SNAIL,
        Self::VULTURE,
        Self {
            body_size: 8.5,
            ..Self::child(CreatureType::KING_VULTURE, &Self::VULTURE)
        },
        Self::GARBAGE_WORM,
        Self::LANTERN_MOUSE,
        Self::CICADA,
        Self::child(CreatureType::CICADA_B, &Self::CICADA),
        Self::SPIDER,
        Self::JET_FISH,
        Self::BIG_EEL,
        Self::DEER,
        Self::TUBE_WORM,
        Self::DADDY_LONG_LEGS,
        Self {
            body_size: 2.5,
            off_screen_speed: 0.1,
            ..Self::child(CreatureType::BROTHER_LONG_LEGS, &Self::DADDY_LONG_LEGS)
        },
        Self::TENTACLE_PLANT,
        Self {
            ai: false,
            require_ai_map: true,
            body_size: 1.4,
            ..Self::child(CreatureType::POLE_MIMIC, &Self::TENTACLE_PLANT)
        },
        Self::MIROS_BIRD,
        Self::TEMPLE_GUARD,
        Self::CENTIPEDE,
        Self {
            body_size: 8.5,
            ..Self::child(CreatureType::RED_CENTIPEDE, &Self::CENTIPEDE)
        },
        Self {
            roam_between_rooms_chance: -1.0,
            abstracted_laziness: 1000,
            body_size: 0.2,
            ..Self::child(CreatureType::SMALL_CENTIPEDE, &Self::CENTIPEDE)
        },
        Self {
            can_fly: true,
            body_size: 1.2,
            ..Self::child(CreatureType::CENTIWING, &Self::CENTIPEDE)
        },
        Self::SCAVENGER,
        Self::OVERSEER,
        Self::VULTURE_GRUB,
        Self::child(CreatureType::HAZER, &Self::VULTURE_GRUB),
        Self::EGG_BUG,
        Self::BIG_SPIDER,
        Self {
            body_size: 1.2,
            ..Self::child(CreatureType::SPITTER_SPIDER, &Self::BIG_SPIDER)
        },
        Self::BIG_NEEDLE_WORM,
        Self {
            small_creature: true,
            body_size: 0.15,
            roam_between_rooms_chance: 0.05,
            off_screen_speed: 3.2,
            ..Self::child(CreatureType::SMALL_NEEDLE_WORM, &Self::BIG_NEEDLE_WORM)
        },
        Self::DROP_BUG,
    ];
}
