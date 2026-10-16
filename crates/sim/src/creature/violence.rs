//! What `Creature.Violence` (77370) reads of a template, for every base-game
//! `CreatureTemplate.Type` as `StaticWorld.InitStaticWorld` and `LizardBreeds.BreedTemplate`
//! leave it (More Slugcats, Remix and Watcher off): the resistances, the kill limit,
//! `quickDeath`, `countsAsAKill` and the community `Killing` influences. Kept apart from
//! the template's movement and AI fields. Values: `rainworld-re/notes/creature-model.md`.

use rw_assets::CreatureType;

use super::CreatureTemplate;

/// `Creature.DamageType`, in registration order (the rows of `damageRestistances`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageType {
    Blunt,
    Stab,
    Bite,
    Water,
    Explosion,
    Electric,
    None,
}

impl DamageType {
    const ALL: [Self; 7] = [
        Self::Blunt,
        Self::Stab,
        Self::Bite,
        Self::Water,
        Self::Explosion,
        Self::Electric,
        Self::None,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Blunt => "Blunt",
            Self::Stab => "Stab",
            Self::Bite => "Bite",
            Self::Water => "Water",
            Self::Explosion => "Explosion",
            Self::Electric => "Electric",
            Self::None => "None",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.value() == value)
    }

    /// `Index` (`None` is registered too, so it has a row).
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }
}

/// `CreatureCommunities.CommunityID` (base game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Community {
    None,
    All,
    Scavengers,
    Lizards,
    Cicadas,
    GarbageWorms,
    Deer,
    JetFish,
}

impl Community {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::All => "All",
            Self::Scavengers => "Scavengers",
            Self::Lizards => "Lizards",
            Self::Cicadas => "Cicadas",
            Self::GarbageWorms => "GarbageWorms",
            Self::Deer => "Deer",
            Self::JetFish => "JetFish",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::None,
            Self::All,
            Self::Scavengers,
            Self::Lizards,
            Self::Cicadas,
            Self::GarbageWorms,
            Self::Deer,
            Self::JetFish,
        ]
        .into_iter()
        .find(|c| c.value() == value)
    }
}

/// A template's violence fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViolenceTemplate {
    pub base_damage_resistance: f32,
    pub base_stun_resistance: f32,
    /// `float.MaxValue` unless set.
    pub instant_death_damage_limit: f32,
    pub quick_death: bool,
    /// `damageRestistances[type, 0]` (damage) and `[type, 1]` (stun); 0 is none.
    pub damage_resistances: [[f32; 2]; 7],
    pub counts_as_a_kill: i32,
    pub community: Community,
    pub community_influence: f32,
}

impl ViolenceTemplate {
    /// The constructor's defaults.
    const DEFAULT: Self = Self {
        base_damage_resistance: 0.0,
        base_stun_resistance: 0.0,
        instant_death_damage_limit: f32::MAX,
        quick_death: true,
        damage_resistances: [[0.0; 2]; 7],
        counts_as_a_kill: 2,
        community: Community::All,
        community_influence: 0.5,
    };

    const fn res(dmg: f32, stun: f32, limit: f32) -> Self {
        Self {
            base_damage_resistance: dmg,
            base_stun_resistance: stun,
            instant_death_damage_limit: limit,
            ..Self::DEFAULT
        }
    }

    /// `LizardBreeds.BreedTemplate`: damage resistance twice the breed's toughness, its
    /// stun toughness, bites resisted 2.5 / 3, and the lizards' community.
    const fn lizard(toughness: f32, stun_toughness: f32) -> Self {
        let mut t = Self::res(toughness * 2.0, stun_toughness, f32::MAX);
        t.damage_resistances[2] = [2.5, 3.0];
        t.community = Community::Lizards;
        t.community_influence = 1.0;
        t
    }

    const fn with_kill(mut self, counts: i32) -> Self {
        self.counts_as_a_kill = counts;
        self
    }

    const fn with_community(mut self, community: Community, influence: f32) -> Self {
        self.community = community;
        self.community_influence = influence;
        self
    }

    /// The fields of base-game type `kind` (an unregistered type gets the
    /// `StandardGroundCreature` defaults).
    #[must_use]
    pub fn of(kind: CreatureType) -> Self {
        use Community as C;
        use CreatureType as T;
        let max = f32::MAX;
        match kind {
            T::Slugcat => Self::res(1.0, 1.0, 1.0),
            T::LizardTemplate => Self::DEFAULT.with_community(C::Lizards, 1.0),
            T::PinkLizard | T::BlackLizard | T::Salamander => Self::lizard(1.0, 1.0),
            T::GreenLizard => Self::lizard(2.5, 2.5),
            T::BlueLizard => Self::lizard(0.5, 0.5),
            T::YellowLizard => Self::lizard(0.8, 0.8),
            T::WhiteLizard | T::CyanLizard => Self::lizard(0.9, 0.9),
            T::RedLizard => {
                let mut t = Self::lizard(3.0, 3.0);
                t.damage_resistances[4][0] = 1.9;
                t
            }
            T::Fly => Self::res(0.15, 0.2, 1.0)
                .with_kill(0)
                .with_community(C::None, 0.5),
            T::Leech => Self::res(0.1, 0.1, 1.0)
                .with_kill(0)
                .with_community(C::None, 0.5),
            T::SeaLeech => Self::res(0.2, 0.2, 1.0)
                .with_kill(0)
                .with_community(C::None, 0.5),
            T::Snail => Self::res(0.4, 0.8, 1.0)
                .with_kill(1)
                .with_community(C::All, 0.25),
            T::Vulture => Self::res(8.5, 3.5, max).with_community(C::All, 0.25),
            T::KingVulture => Self::res(12.5, 6.5, max).with_community(C::All, 0.25),
            T::GarbageWorm => Self::res(1.0, 1.0, max).with_community(C::GarbageWorms, 1.0),
            T::LanternMouse => Self::res(0.5, 0.4, 1.0).with_community(C::All, 1.0),
            T::CicadaA | T::CicadaB => Self::res(0.8, 1.0, max).with_community(C::Cicadas, 1.0),
            T::Spider => Self::res(0.1, 0.1, 1.0)
                .with_kill(0)
                .with_community(C::None, 0.5),
            T::JetFish => Self::res(0.95, 0.9, 1.0).with_community(C::JetFish, 0.75),
            T::BigEel => Self::res(1000.0, 1000.0, max).with_community(C::None, 0.5),
            T::Deer => Self::res(200.0, 8.0, max).with_community(C::Deer, 0.85),
            T::TubeWorm => Self::res(0.3, 0.3, 1.2)
                .with_kill(1)
                .with_community(C::All, 0.15),
            T::DaddyLongLegs | T::BrotherLongLegs => {
                let (dmg, stun) = if kind == T::DaddyLongLegs {
                    (200.0, 200.0)
                } else {
                    (100.0, 2.0)
                };
                let mut t = Self::res(dmg, stun, max).with_community(C::None, 0.5);
                t.damage_resistances[4][0] = 0.03;
                t
            }
            T::TentaclePlant => Self::res(2.5, 0.8, max).with_community(C::All, 0.1),
            T::PoleMimic => Self::res(0.9, 0.5, 1.0)
                .with_kill(1)
                .with_community(C::All, 0.05),
            T::MirosBird => Self::res(7.0, 2.75, max).with_community(C::All, 0.1),
            T::TempleGuard => {
                Self::res(10_000_000.0, 10_000_000.0, max).with_community(C::None, 0.5)
            }
            T::Centipede => {
                let mut t = Self::res(0.9, 0.6, max).with_community(C::All, 0.25);
                t.quick_death = false;
                t.damage_resistances[5] = [102.0, 102.0];
                t
            }
            T::RedCentipede | T::Centiwing | T::SmallCentipede => {
                let (limit, kill, community) = if kind == T::SmallCentipede {
                    (1.0, 0, C::None)
                } else {
                    (max, 2, C::All)
                };
                let mut t = Self::res(1.0, 0.6, limit)
                    .with_kill(kill)
                    .with_community(community, 0.25);
                t.quick_death = false;
                t
            }
            T::Scavenger => Self::res(1.1, 1.3, 1.5).with_community(C::Scavengers, 1.0),
            T::Overseer => Self::res(0.9, 0.5, 1.0).with_kill(1),
            T::VultureGrub | T::Hazer => Self::res(0.3, 0.3, 0.6)
                .with_kill(0)
                .with_community(C::All, 0.0),
            T::EggBug => Self::res(0.15, 0.3, 0.7).with_community(C::All, 0.1),
            T::BigSpider => Self::res(0.6, 0.5, 1.0).with_community(C::All, 0.1),
            T::SpitterSpider => Self::res(1.1, 0.6, 1.0).with_community(C::All, 0.1),
            T::BigNeedleWorm => Self::res(0.4, 1.0, 1.2),
            T::SmallNeedleWorm => Self::res(0.2, 0.4, 1.2),
            T::DropBug => Self::res(0.95, 0.6, max),
            _ => Self::DEFAULT,
        }
    }

    /// `Creature.Violence`'s numbers before the stun and the damage are applied: the
    /// damage over `baseDamageResistance` and the stun `(damage * 30 + stunBonus)` over
    /// `baseStunResistance`, each divided by the type's resistance where it has one.
    /// `health_scale` is the health state's stun factor (1 without one).
    #[must_use]
    pub fn violence(
        &self,
        kind: DamageType,
        damage: f32,
        stun_bonus: f32,
        health_scale: f32,
    ) -> (f32, f32) {
        let mut dmg = damage / self.base_damage_resistance;
        let mut stun = (damage * 30.0 + stun_bonus) / self.base_stun_resistance * health_scale;
        let [d, s] = self.damage_resistances[kind.index()];
        if d > 0.0 {
            dmg /= d;
        }
        if s > 0.0 {
            stun /= s;
        }
        (dmg, stun)
    }
}

impl CreatureTemplate {
    /// The template's violence fields.
    #[must_use]
    pub fn violence(&self) -> ViolenceTemplate {
        ViolenceTemplate::of(self.kind)
    }

    /// `IsLizard`: the ancestor is `LizardTemplate`.
    #[must_use]
    pub fn is_lizard(&self) -> bool {
        self.ancestor == Some(CreatureType::LizardTemplate)
    }
}
