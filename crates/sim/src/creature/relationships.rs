//! `CreatureTemplate.relationships` as `StaticWorld` builds them: each template starts
//! from its root template's default relationship (the constructor's argument; a child
//! clones its ancestor's array), then `InitStaticWorldRelationships` (126578) runs its
//! `EstablishRelationship` calls in order, each reaching the descendants of both types
//! (126962), then makes the egg bug afraid of every type by body size and every type
//! ignore overseers; last, the cyan lizard's Eats intensities are scaled by 0.3.
//! Base game only (the DLC tables are not ported).

use std::sync::OnceLock;

use rw_assets::{ActiveMods, CreatureType};

use super::{CreatureTemplate, Relationship, RelationshipType};
use crate::FloatExt;

use CreatureType as T;
use RelationshipType as R;

/// The relationships of every base template to every base type, by `Index`.
struct RelationshipTable {
    types: Vec<CreatureType>,
    cells: Vec<Vec<Relationship>>,
}

impl RelationshipTable {
    fn get() -> &'static Self {
        static TABLE: OnceLock<RelationshipTable> = OnceLock::new();
        TABLE.get_or_init(Self::build)
    }

    fn build() -> Self {
        let types = CreatureType::registered(ActiveMods::NONE);
        let cells = types
            .iter()
            .map(|&t| {
                let root = CreatureTemplate::of(t, ActiveMods::NONE).top_ancestor();
                vec![Self::default_of(root); types.len()]
            })
            .collect();
        let mut table = Self { types, cells };
        for &(a, b, kind, intensity) in ESTABLISHED {
            table.establish(a, b, Relationship { kind, intensity });
        }
        for b in table.types.clone() {
            let size = CreatureTemplate::of(b, ActiveMods::NONE).body_size;
            let afraid = Relationship {
                kind: R::Afraid,
                intensity: size.inverse_lerp(0.0, 8.0),
            };
            table.establish(T::EggBug, b, afraid);
        }
        let ignores = Relationship {
            kind: R::Ignores,
            intensity: 0.0,
        };
        for a in table.types.clone() {
            table.establish(a, T::Overseer, ignores);
        }
        if let Some(i) = table.index(T::CyanLizard) {
            for r in &mut table.cells[i] {
                if r.kind == R::Eats {
                    r.intensity *= 0.3;
                }
            }
        }
        table
    }

    fn index(&self, t: CreatureType) -> Option<usize> {
        self.types.iter().position(|&x| x == t)
    }

    /// `EstablishRelationship(a, b, relationship)`.
    fn establish(&mut self, a: CreatureType, b: CreatureType, r: Relationship) {
        let (Some(i), Some(j)) = (self.index(a), self.index(b)) else {
            return;
        };
        self.cells[i][j] = r;
        for t in self.types.clone() {
            let ancestor = CreatureTemplate::of(t, ActiveMods::NONE).ancestor;
            if ancestor == Some(a) {
                self.establish(t, b, r);
            }
            if ancestor == Some(b) {
                self.establish(a, t, r);
            }
        }
    }

    /// The `defaultRelationship` of a root template's constructor (`InitStaticWorld`).
    fn default_of(root: CreatureType) -> Relationship {
        let (kind, intensity) = match root {
            T::Fly | T::CicadaA => (R::Uncomfortable, 0.0),
            T::Snail => (R::Uncomfortable, 0.5),
            T::TempleGuard | T::BigNeedleWorm => (R::Uncomfortable, 1.0),
            T::LanternMouse | T::EggBug => (R::Afraid, 0.1),
            T::Leech
            | T::Spider
            | T::BigEel
            | T::DaddyLongLegs
            | T::TentaclePlant
            | T::MirosBird
            | T::Centipede
            | T::DropBug => (R::Eats, 1.0),
            T::Scavenger => (R::Ignores, 0.1),
            _ => (R::Ignores, 0.0),
        };
        Relationship { kind, intensity }
    }
}

impl CreatureTemplate {
    /// `CreatureRelationship(other)`: this template's relationship to `other`.
    #[must_use]
    pub fn relationship_to(&self, other: CreatureType) -> Relationship {
        let table = RelationshipTable::get();
        match (table.index(self.kind), table.index(other)) {
            (Some(i), Some(j)) => table.cells[i][j],
            _ => RelationshipTable::default_of(self.top_ancestor()),
        }
    }

    /// Whether this template eats `other` (`CreatureRelationship(...).type == Eats`).
    #[must_use]
    pub fn eats(&self, other: CreatureType) -> bool {
        self.relationship_to(other).kind == R::Eats
    }
}

/// `InitStaticWorldRelationships`' `EstablishRelationship` calls, in order.
const ESTABLISHED: &[(CreatureType, CreatureType, RelationshipType, f32)] = &[
    (T::LizardTemplate, T::LizardTemplate, R::AgressiveRival, 0.1),
    (T::LizardTemplate, T::Slugcat, R::SocialDependent, 0.5),
    (T::LizardTemplate, T::Vulture, R::Afraid, 0.9),
    (T::LizardTemplate, T::KingVulture, R::Afraid, 1.0),
    (T::LizardTemplate, T::TubeWorm, R::Eats, 0.025),
    (T::LizardTemplate, T::Scavenger, R::Eats, 0.8),
    (T::LizardTemplate, T::CicadaA, R::Eats, 0.05),
    (T::LizardTemplate, T::LanternMouse, R::Eats, 0.3),
    (T::LizardTemplate, T::BigSpider, R::Eats, 0.35),
    (T::LizardTemplate, T::EggBug, R::Eats, 0.45),
    (T::LizardTemplate, T::JetFish, R::Eats, 0.1),
    (T::LizardTemplate, T::BigEel, R::Afraid, 1.0),
    (T::LizardTemplate, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::LizardTemplate, T::Centipede, R::Eats, 0.8),
    (T::LizardTemplate, T::BigNeedleWorm, R::Eats, 0.25),
    (T::LizardTemplate, T::DropBug, R::Eats, 0.2),
    (T::LizardTemplate, T::SmallNeedleWorm, R::Eats, 0.3),
    (T::LizardTemplate, T::RedCentipede, R::Afraid, 0.9),
    (T::LizardTemplate, T::TentaclePlant, R::Afraid, 0.2),
    (T::LizardTemplate, T::Hazer, R::Eats, 0.15),
    (T::LizardTemplate, T::Leech, R::Uncomfortable, 0.5),
    (T::LizardTemplate, T::Spider, R::Afraid, 0.1),
    (T::LizardTemplate, T::MirosBird, R::Afraid, 1.0),
    (T::PinkLizard, T::PinkLizard, R::AgressiveRival, 0.4),
    (T::PinkLizard, T::GreenLizard, R::AgressiveRival, 0.15),
    (T::PinkLizard, T::BlueLizard, R::AgressiveRival, 0.2),
    (T::PinkLizard, T::WhiteLizard, R::AgressiveRival, 0.25),
    (T::GreenLizard, T::PinkLizard, R::AgressiveRival, 0.2),
    (T::GreenLizard, T::GreenLizard, R::AgressiveRival, 0.8),
    (T::GreenLizard, T::BigSpider, R::Eats, 0.6),
    (T::GreenLizard, T::WhiteLizard, R::AgressiveRival, 0.05),
    (T::GreenLizard, T::BlueLizard, R::Eats, 0.25),
    (T::GreenLizard, T::EggBug, R::Eats, 0.1),
    (T::GreenLizard, T::KingVulture, R::Afraid, 0.5),
    (T::GreenLizard, T::RedCentipede, R::Afraid, 0.5),
    (T::GreenLizard, T::Spider, R::Ignores, 0.0),
    (T::BlueLizard, T::PinkLizard, R::AgressiveRival, 0.3),
    (T::BlueLizard, T::BigSpider, R::Afraid, 0.5),
    (T::BlueLizard, T::BlueLizard, R::AgressiveRival, 0.45),
    (T::BlueLizard, T::GreenLizard, R::Afraid, 0.25),
    (T::BlueLizard, T::WhiteLizard, R::Afraid, 0.1),
    (T::BlueLizard, T::CyanLizard, R::Afraid, 0.2),
    (T::BlueLizard, T::CicadaA, R::Eats, 0.7),
    (T::WhiteLizard, T::PinkLizard, R::AgressiveRival, 0.15),
    (T::WhiteLizard, T::GreenLizard, R::AgressiveRival, 0.05),
    (T::WhiteLizard, T::BlueLizard, R::AgressiveRival, 0.35),
    (T::WhiteLizard, T::WhiteLizard, R::AgressiveRival, 0.25),
    (T::WhiteLizard, T::CicadaA, R::Eats, 0.7),
    (T::WhiteLizard, T::EggBug, R::Eats, 0.7),
    (T::RedLizard, T::Vulture, R::Attacks, 0.4),
    (T::RedLizard, T::KingVulture, R::Attacks, 0.2),
    (T::RedLizard, T::MirosBird, R::Attacks, 0.4),
    (T::RedLizard, T::DaddyLongLegs, R::Afraid, 0.2),
    (T::RedLizard, T::RedLizard, R::Attacks, 1.0),
    (T::RedLizard, T::Spider, R::Ignores, 0.0),
    (T::CyanLizard, T::CyanLizard, R::AgressiveRival, 0.075),
    (T::CyanLizard, T::BlueLizard, R::Eats, 0.15),
    (T::BlackLizard, T::BlackLizard, R::AgressiveRival, 0.05),
    (T::YellowLizard, T::YellowLizard, R::Pack, 0.2),
    (T::Salamander, T::JetFish, R::Eats, 0.5),
    (T::Fly, T::Leech, R::Afraid, 0.05),
    (T::Fly, T::CicadaA, R::Afraid, 0.7),
    (T::Fly, T::JetFish, R::Afraid, 0.6),
    (T::Fly, T::Slugcat, R::Afraid, 0.5),
    (T::Fly, T::SmallCentipede, R::Afraid, 0.5),
    (T::Fly, T::Spider, R::Afraid, 0.05),
    (T::Fly, T::TentaclePlant, R::Afraid, 0.05),
    (T::LanternMouse, T::LanternMouse, R::PlaysWith, 0.01),
    (T::LanternMouse, T::Fly, R::Eats, 0.5),
    (T::LanternMouse, T::LizardTemplate, R::Afraid, 0.7),
    (T::LanternMouse, T::Slugcat, R::Afraid, 0.3),
    (T::LanternMouse, T::BigSpider, R::Afraid, 0.6),
    (T::LanternMouse, T::DropBug, R::Afraid, 0.7),
    (T::LanternMouse, T::SmallCentipede, R::Ignores, 0.0),
    (T::LanternMouse, T::RedCentipede, R::Afraid, 1.0),
    (T::LanternMouse, T::TentaclePlant, R::Afraid, 0.5),
    (T::LanternMouse, T::Vulture, R::Afraid, 0.7),
    (T::LanternMouse, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::LanternMouse, T::BigEel, R::Afraid, 1.0),
    (T::LanternMouse, T::MirosBird, R::Afraid, 1.0),
    (T::LanternMouse, T::Centipede, R::Afraid, 0.5),
    (T::LanternMouse, T::Leech, R::Afraid, 0.7),
    (T::Snail, T::Snail, R::Ignores, 0.0),
    (T::Snail, T::BrotherLongLegs, R::Afraid, 0.8),
    (T::Snail, T::BigEel, R::Afraid, 1.0),
    (T::Snail, T::DaddyLongLegs, R::Afraid, 0.8),
    (T::Snail, T::MirosBird, R::Afraid, 0.5),
    (T::Vulture, T::CicadaA, R::Eats, 0.1),
    (T::Vulture, T::Slugcat, R::Eats, 0.3),
    (T::Vulture, T::LizardTemplate, R::Eats, 0.5),
    (T::Vulture, T::GreenLizard, R::Ignores, 0.0),
    (T::Vulture, T::Centipede, R::Eats, 0.5),
    (T::Vulture, T::RedCentipede, R::Afraid, 0.2),
    (T::Vulture, T::Scavenger, R::Eats, 0.8),
    (T::Vulture, T::VultureGrub, R::Eats, 0.01),
    (T::Vulture, T::JetFish, R::Eats, 0.5),
    (T::Vulture, T::BigSpider, R::Eats, 0.5),
    (T::Vulture, T::EggBug, R::Eats, 0.3),
    (T::Vulture, T::BigNeedleWorm, R::Eats, 0.15),
    (T::Vulture, T::SmallNeedleWorm, R::Eats, 0.05),
    (T::Vulture, T::DropBug, R::Eats, 0.4),
    (T::Vulture, T::BigEel, R::Afraid, 0.3),
    (T::Vulture, T::DaddyLongLegs, R::Afraid, 0.2),
    (T::Vulture, T::MirosBird, R::Afraid, 0.1),
    (T::Vulture, T::LanternMouse, R::Eats, 0.05),
    (T::Vulture, T::TentaclePlant, R::Afraid, 0.1),
    (T::Vulture, T::PoleMimic, R::Afraid, 0.1),
    (T::KingVulture, T::GreenLizard, R::Eats, 0.55),
    (T::KingVulture, T::RedCentipede, R::Eats, 0.55),
    (T::CicadaA, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::CicadaA, T::CicadaA, R::PlaysWith, 1.0),
    (T::CicadaA, T::CicadaB, R::PlaysWith, 1.0),
    (T::CicadaA, T::LizardTemplate, R::Afraid, 0.3),
    (T::CicadaA, T::GreenLizard, R::Antagonizes, 0.1),
    (T::CicadaA, T::Slugcat, R::SocialDependent, 1.0),
    (T::CicadaA, T::JetFish, R::AgressiveRival, 0.5),
    (T::CicadaA, T::Centipede, R::Afraid, 0.15),
    (T::CicadaA, T::Centiwing, R::Afraid, 0.35),
    (T::CicadaA, T::SmallCentipede, R::Eats, 0.15),
    (T::CicadaA, T::Vulture, R::Afraid, 0.7),
    (T::CicadaA, T::KingVulture, R::Afraid, 0.8),
    (T::CicadaA, T::WhiteLizard, R::Afraid, 0.7),
    (T::CicadaA, T::BlueLizard, R::Afraid, 0.7),
    (T::CicadaA, T::Fly, R::Eats, 0.2),
    (T::CicadaA, T::Leech, R::Eats, 0.4),
    (T::CicadaA, T::BigSpider, R::Afraid, 0.5),
    (T::CicadaA, T::BigNeedleWorm, R::Afraid, 0.15),
    (T::CicadaA, T::SmallNeedleWorm, R::Eats, 0.1),
    (T::CicadaA, T::DropBug, R::Afraid, 0.5),
    (T::CicadaA, T::RedCentipede, R::Afraid, 0.8),
    (T::CicadaA, T::TentaclePlant, R::Afraid, 0.5),
    (T::CicadaA, T::MirosBird, R::Afraid, 0.7),
    (T::CicadaA, T::BigEel, R::Afraid, 0.7),
    (T::CicadaA, T::Spider, R::Afraid, 0.1),
    (T::CicadaB, T::Leech, R::Eats, 0.2),
    (T::CicadaB, T::Slugcat, R::SocialDependent, 1.0),
    (T::CicadaB, T::Fly, R::Eats, 0.4),
    (T::Leech, T::Leech, R::Ignores, 0.0),
    (T::Leech, T::TentaclePlant, R::Ignores, 0.0),
    (T::Leech, T::PoleMimic, R::Ignores, 0.0),
    (T::Leech, T::CicadaA, R::Eats, 0.2),
    (T::Leech, T::Snail, R::Eats, 0.5),
    (T::Leech, T::GarbageWorm, R::Ignores, 0.0),
    (T::Leech, T::Salamander, R::Afraid, 0.3),
    (T::Leech, T::JetFish, R::Afraid, 0.6),
    (T::Leech, T::BigSpider, R::Eats, 0.3),
    (T::Leech, T::EggBug, R::Eats, 0.3),
    (T::Leech, T::BigNeedleWorm, R::Eats, 0.15),
    (T::Leech, T::SmallNeedleWorm, R::Eats, 0.15),
    (T::Leech, T::DropBug, R::Eats, 0.4),
    (T::Leech, T::Hazer, R::Ignores, 0.0),
    (T::Leech, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::Leech, T::BrotherLongLegs, R::Afraid, 1.0),
    (T::Leech, T::BigEel, R::Afraid, 1.0),
    (T::Leech, T::Spider, R::Ignores, 0.0),
    (T::Spider, T::Spider, R::Ignores, 0.0),
    (T::Spider, T::BigSpider, R::Ignores, 0.0),
    (T::Spider, T::Leech, R::Ignores, 0.0),
    (T::Spider, T::GarbageWorm, R::Ignores, 0.0),
    (T::Spider, T::TentaclePlant, R::Ignores, 0.0),
    (T::JetFish, T::Slugcat, R::SocialDependent, 1.0),
    (T::JetFish, T::Fly, R::Eats, 0.2),
    (T::JetFish, T::Leech, R::Eats, 0.2),
    (T::JetFish, T::Vulture, R::Afraid, 1.0),
    (T::JetFish, T::KingVulture, R::Afraid, 1.0),
    (T::JetFish, T::CicadaA, R::Eats, 0.1),
    (T::JetFish, T::LizardTemplate, R::Afraid, 0.2),
    (T::JetFish, T::Salamander, R::Afraid, 0.5),
    (T::JetFish, T::BigEel, R::Afraid, 1.0),
    (T::JetFish, T::DropBug, R::Afraid, 0.2),
    (T::JetFish, T::TentaclePlant, R::Afraid, 0.2),
    (T::JetFish, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::JetFish, T::MirosBird, R::Afraid, 1.0),
    (T::JetFish, T::Centipede, R::Afraid, 0.2),
    (T::BigEel, T::BigEel, R::Ignores, 0.0),
    (T::BigEel, T::GarbageWorm, R::Ignores, 0.0),
    (T::BigEel, T::TentaclePlant, R::Ignores, 0.0),
    (T::BigEel, T::TubeWorm, R::Eats, 0.1),
    (T::BigEel, T::Leech, R::Eats, 0.1),
    (T::BigEel, T::Spider, R::Ignores, 0.0),
    (T::BigEel, T::VultureGrub, R::Eats, 0.01),
    (T::BigEel, T::Fly, R::Ignores, 1.0),
    (T::DaddyLongLegs, T::Slugcat, R::Eats, 1.0),
    (T::DaddyLongLegs, T::LizardTemplate, R::Eats, 1.0),
    (T::DaddyLongLegs, T::CicadaA, R::Eats, 1.0),
    (T::DaddyLongLegs, T::CicadaB, R::Eats, 1.0),
    (T::DaddyLongLegs, T::DaddyLongLegs, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::BrotherLongLegs, R::Eats, 1.0),
    (T::DaddyLongLegs, T::BigEel, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::TubeWorm, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::Fly, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::GarbageWorm, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::Leech, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::BigNeedleWorm, R::Eats, 1.0),
    (T::DaddyLongLegs, T::SmallNeedleWorm, R::Eats, 0.15),
    (T::DaddyLongLegs, T::DropBug, R::Eats, 0.1),
    (T::DaddyLongLegs, T::TentaclePlant, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::PoleMimic, R::Ignores, 0.0),
    (T::BrotherLongLegs, T::DaddyLongLegs, R::Ignores, 0.0),
    (T::BrotherLongLegs, T::TempleGuard, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::TempleGuard, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::Spider, R::Ignores, 0.0),
    (T::DaddyLongLegs, T::VultureGrub, R::Eats, 0.1),
    (T::Slugcat, T::LizardTemplate, R::Afraid, 1.0),
    (T::Slugcat, T::Fly, R::Eats, 0.5),
    (T::Slugcat, T::Vulture, R::Afraid, 1.0),
    (T::Slugcat, T::BigEel, R::Afraid, 1.0),
    (T::Slugcat, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::Slugcat, T::TentaclePlant, R::Afraid, 1.0),
    (T::Slugcat, T::MirosBird, R::Afraid, 0.8),
    (T::Slugcat, T::Centipede, R::Afraid, 0.3),
    (T::Slugcat, T::Centiwing, R::Afraid, 0.3),
    (T::Slugcat, T::SmallCentipede, R::Ignores, 0.3),
    (T::Slugcat, T::BigSpider, R::Afraid, 0.5),
    (T::Slugcat, T::SpitterSpider, R::Afraid, 0.65),
    (T::Slugcat, T::DropBug, R::Afraid, 0.5),
    (T::Slugcat, T::RedCentipede, R::Afraid, 1.0),
    (T::TentaclePlant, T::TentaclePlant, R::Ignores, 0.0),
    (T::TentaclePlant, T::PoleMimic, R::Ignores, 0.0),
    (T::TentaclePlant, T::Leech, R::Ignores, 0.0),
    (T::TentaclePlant, T::MirosBird, R::Afraid, 0.8),
    (T::TentaclePlant, T::BigEel, R::Afraid, 1.0),
    (T::TentaclePlant, T::DaddyLongLegs, R::Afraid, 0.6),
    (T::TentaclePlant, T::Deer, R::Afraid, 0.2),
    (T::TentaclePlant, T::GarbageWorm, R::Ignores, 0.0),
    (T::TentaclePlant, T::Spider, R::Ignores, 0.0),
    (T::MirosBird, T::MirosBird, R::Antagonizes, 0.1),
    (T::MirosBird, T::Slugcat, R::Eats, 0.6),
    (T::MirosBird, T::DaddyLongLegs, R::Afraid, 0.7),
    (T::MirosBird, T::SmallCentipede, R::Ignores, 0.0),
    (T::MirosBird, T::Fly, R::Ignores, 0.0),
    (T::MirosBird, T::Leech, R::Ignores, 0.0),
    (T::MirosBird, T::Spider, R::Ignores, 0.0),
    (T::MirosBird, T::Centipede, R::Eats, 0.8),
    (T::MirosBird, T::BigSpider, R::Eats, 0.5),
    (T::MirosBird, T::RedCentipede, R::Afraid, 0.7),
    (T::MirosBird, T::GarbageWorm, R::Ignores, 0.0),
    (T::MirosBird, T::TentaclePlant, R::Ignores, 0.0),
    (T::MirosBird, T::Deer, R::Ignores, 0.0),
    (T::MirosBird, T::Vulture, R::Ignores, 0.0),
    (T::MirosBird, T::CicadaA, R::Eats, 0.3),
    (T::MirosBird, T::BigEel, R::Afraid, 0.3),
    (T::MirosBird, T::TubeWorm, R::Ignores, 0.0),
    (T::MirosBird, T::Snail, R::Ignores, 0.0),
    (T::MirosBird, T::TempleGuard, R::Ignores, 1.0),
    (T::MirosBird, T::VultureGrub, R::Eats, 0.1),
    (T::TempleGuard, T::TempleGuard, R::Pack, 0.0),
    (T::GarbageWorm, T::BrotherLongLegs, R::Afraid, 0.8),
    (T::GarbageWorm, T::DaddyLongLegs, R::Afraid, 0.8),
    (T::GarbageWorm, T::MirosBird, R::Afraid, 0.8),
    (T::GarbageWorm, T::TentaclePlant, R::Afraid, 0.5),
    (T::GarbageWorm, T::BigEel, R::Afraid, 1.0),
    (T::SmallCentipede, T::GarbageWorm, R::Ignores, 0.0),
    (T::Centipede, T::GarbageWorm, R::Ignores, 0.0),
    (T::RedCentipede, T::GarbageWorm, R::Ignores, 0.0),
    (T::Scavenger, T::Vulture, R::Afraid, 0.75),
    (T::Scavenger, T::KingVulture, R::Afraid, 0.95),
    (T::Scavenger, T::Slugcat, R::SocialDependent, 1.0),
    (T::Scavenger, T::Scavenger, R::Pack, 0.5),
    (T::Scavenger, T::LanternMouse, R::Uncomfortable, 0.1),
    (T::Scavenger, T::LizardTemplate, R::Afraid, 0.5),
    (T::Scavenger, T::Snail, R::Uncomfortable, 0.6),
    (T::Scavenger, T::GarbageWorm, R::Uncomfortable, 0.8),
    (T::Scavenger, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::Scavenger, T::BrotherLongLegs, R::Afraid, 0.8),
    (T::Scavenger, T::Centipede, R::Afraid, 0.4),
    (T::Scavenger, T::RedCentipede, R::Afraid, 0.95),
    (T::Scavenger, T::SmallCentipede, R::Ignores, 0.0),
    (T::Scavenger, T::TentaclePlant, R::Afraid, 0.65),
    (T::Scavenger, T::PoleMimic, R::Afraid, 0.3),
    (T::Scavenger, T::MirosBird, R::Afraid, 0.7),
    (T::Scavenger, T::BigSpider, R::Afraid, 0.45),
    (T::Scavenger, T::BigNeedleWorm, R::Uncomfortable, 0.5),
    (T::Scavenger, T::SmallNeedleWorm, R::Ignores, 0.0),
    (T::Scavenger, T::DropBug, R::Afraid, 0.55),
    (T::Scavenger, T::Leech, R::Uncomfortable, 0.5),
    (T::Scavenger, T::Spider, R::Afraid, 0.1),
    (T::Scavenger, T::BigEel, R::Afraid, 1.0),
    (T::Centipede, T::Centipede, R::Ignores, 0.0),
    (T::Centipede, T::TentaclePlant, R::Afraid, 0.5),
    (T::Centipede, T::PoleMimic, R::Ignores, 0.0),
    (T::Centipede, T::BigSpider, R::Eats, 0.25),
    (T::Centipede, T::Leech, R::Uncomfortable, 0.5),
    (T::Centipede, T::Spider, R::Afraid, 0.1),
    (T::Centipede, T::Snail, R::Eats, 0.4),
    (T::Centipede, T::BigEel, R::Afraid, 1.0),
    (T::Centipede, T::Deer, R::Ignores, 0.0),
    (T::Centipede, T::TubeWorm, R::Eats, 0.1),
    (T::Centipede, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::Centipede, T::MirosBird, R::Afraid, 1.0),
    (T::Centipede, T::TempleGuard, R::Ignores, 0.0),
    (T::Centipede, T::VultureGrub, R::Eats, 0.1),
    (T::Centipede, T::Fly, R::Ignores, 0.0),
    (T::RedCentipede, T::Leech, R::Ignores, 0.0),
    (T::RedCentipede, T::DaddyLongLegs, R::Eats, 1.0),
    (T::RedCentipede, T::MirosBird, R::Eats, 1.0),
    (T::SmallCentipede, T::Fly, R::Eats, 0.5),
    (T::SmallCentipede, T::Vulture, R::Afraid, 0.5),
    (T::BigSpider, T::Slugcat, R::Eats, 0.4),
    (T::BigSpider, T::LizardTemplate, R::Eats, 0.3),
    (T::BigSpider, T::BlueLizard, R::Eats, 0.5),
    (T::BigSpider, T::GreenLizard, R::Afraid, 0.4),
    (T::BigSpider, T::RedLizard, R::Afraid, 0.4),
    (T::BigSpider, T::Vulture, R::Afraid, 1.0),
    (T::BigSpider, T::LanternMouse, R::Eats, 0.8),
    (T::BigSpider, T::Centipede, R::Eats, 0.11),
    (T::BigSpider, T::SmallCentipede, R::Eats, 0.2),
    (T::BigSpider, T::VultureGrub, R::Eats, 0.1),
    (T::BigSpider, T::Scavenger, R::Eats, 0.45),
    (T::BigSpider, T::TentaclePlant, R::Afraid, 0.6),
    (T::BigSpider, T::PoleMimic, R::Afraid, 0.3),
    (T::BigSpider, T::Centiwing, R::Eats, 0.4),
    (T::BigSpider, T::CicadaA, R::Eats, 0.4),
    (T::BigSpider, T::CicadaB, R::Eats, 0.4),
    (T::BigSpider, T::BigEel, R::Afraid, 1.0),
    (T::BigSpider, T::BigNeedleWorm, R::Eats, 0.3),
    (T::BigSpider, T::SmallNeedleWorm, R::Eats, 0.15),
    (T::BigSpider, T::DropBug, R::Eats, 0.25),
    (T::BigSpider, T::MirosBird, R::Afraid, 0.9),
    (T::BigSpider, T::RedCentipede, R::Afraid, 0.8),
    (T::BigSpider, T::Leech, R::Uncomfortable, 0.5),
    (T::BigSpider, T::EggBug, R::Eats, 0.2),
    (T::BigSpider, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::DropBug, T::RedLizard, R::Afraid, 0.4),
    (T::DropBug, T::Vulture, R::Afraid, 1.0),
    (T::DropBug, T::TentaclePlant, R::Afraid, 0.6),
    (T::DropBug, T::PoleMimic, R::Afraid, 0.3),
    (T::DropBug, T::BigEel, R::Afraid, 1.0),
    (T::DropBug, T::DaddyLongLegs, R::Afraid, 1.0),
    (T::DropBug, T::SmallNeedleWorm, R::Eats, 0.1),
    (T::DropBug, T::SmallCentipede, R::Eats, 0.1),
    (T::DropBug, T::DropBug, R::Ignores, 0.25),
    (T::DropBug, T::MirosBird, R::Afraid, 0.9),
    (T::DropBug, T::RedCentipede, R::Afraid, 0.8),
    (T::DropBug, T::GarbageWorm, R::Ignores, 0.0),
    (T::DropBug, T::Spider, R::Afraid, 0.1),
    (T::DropBug, T::Deer, R::Eats, 0.5),
    (T::DropBug, T::TubeWorm, R::Eats, 0.1),
    (T::DropBug, T::Fly, R::Ignores, 0.0),
    (T::BigNeedleWorm, T::BigNeedleWorm, R::Attacks, 0.9),
    (T::BigNeedleWorm, T::MirosBird, R::Afraid, 0.9),
    (T::BigNeedleWorm, T::DaddyLongLegs, R::Afraid, 0.9),
    (T::BigNeedleWorm, T::BigEel, R::Afraid, 0.9),
    (T::BigNeedleWorm, T::Vulture, R::Afraid, 0.5),
    (T::BigNeedleWorm, T::KingVulture, R::Afraid, 0.6),
    (T::BigNeedleWorm, T::TentaclePlant, R::Afraid, 0.3),
    (T::BigNeedleWorm, T::VultureGrub, R::Ignores, 0.0),
    (T::BigNeedleWorm, T::Fly, R::Ignores, 0.0),
    (T::BigNeedleWorm, T::RedCentipede, R::Afraid, 0.8),
    (T::EggBug, T::Fly, R::Ignores, 0.0),
    (T::Scavenger, T::Overseer, R::Attacks, 1.0),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(a: CreatureType, b: CreatureType) -> Relationship {
        CreatureTemplate::of(a, ActiveMods::NONE).relationship_to(b)
    }

    #[test]
    fn cicadas_eat_flies() {
        // `(CicadaA, Fly, Eats 0.2)` reaches CicadaB (ancestor CicadaA); a later
        // `(CicadaB, Fly, Eats 0.4)` sets its own.
        assert_eq!(rel(T::CicadaA, T::Fly).kind, R::Eats);
        assert!((rel(T::CicadaA, T::Fly).intensity - 0.2).abs() < 1e-6);
        assert_eq!(rel(T::CicadaB, T::Fly).kind, R::Eats);
        assert!((rel(T::CicadaB, T::Fly).intensity - 0.4).abs() < 1e-6);
        // `(CicadaA, Leech, Eats 0.4)` reaches SeaLeech (ancestor Leech).
        assert_eq!(rel(T::CicadaA, T::SeaLeech).kind, R::Eats);
    }

    #[test]
    fn defaults_overseers_egg_bugs_and_the_slugcat() {
        // Leech's constructor default is Eats 1; nothing sets Leech -> Deer.
        assert_eq!(rel(T::Leech, T::Deer).kind, R::Eats);
        // The last loop: every type ignores overseers, the scavenger's Attacks undone.
        assert_eq!(rel(T::Scavenger, T::Overseer).kind, R::Ignores);
        // The egg bug fears a vulture by `InverseLerp(0, 8, 6)`.
        let r = rel(T::EggBug, T::Vulture);
        assert_eq!(r.kind, R::Afraid);
        assert!((r.intensity - 0.75).abs() < 1e-6);
        assert_eq!(
            Some(rel(T::Slugcat, T::Fly)),
            Relationship::of_slugcat_to(T::Fly)
        );
        // Lizards eat centipedes (`LizardTemplate`'s call reaches every breed); the
        // cyan's is scaled by 0.3.
        assert_eq!(rel(T::GreenLizard, T::Centipede).kind, R::Eats);
        assert!((rel(T::CyanLizard, T::Centipede).intensity - 0.24).abs() < 1e-6);
    }
}
