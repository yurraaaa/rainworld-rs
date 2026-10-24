use rw_assets::{ActiveMods, CreatureType as T};

use crate::creature::CreatureTemplate;

/// `dangerousToPlayer` as `InitStaticWorld` and the lizard breeds'
/// `danger` (`LizardBreeds.BreedTemplate`) leave it; children inherit (`CreatureTemplate.ctor`).
#[test]
fn dangerous_to_player_per_template() {
    let table = [
        (T::StandardGroundCreature, 0.0),
        (T::Slugcat, 0.0),
        (T::LizardTemplate, 0.0),
        (T::PinkLizard, 0.45),
        (T::GreenLizard, 0.45),
        (T::BlueLizard, 0.35),
        (T::YellowLizard, 0.4),
        (T::WhiteLizard, 0.5),
        (T::RedLizard, 0.8),
        (T::BlackLizard, 0.45),
        (T::Salamander, 0.4),
        (T::CyanLizard, 0.65),
        (T::Fly, 0.0),
        (T::Leech, 0.05),
        (T::SeaLeech, 0.07),
        (T::Snail, 0.0),
        (T::Vulture, 0.7),
        (T::KingVulture, 0.9),
        (T::GarbageWorm, 0.0),
        (T::LanternMouse, 0.0),
        (T::CicadaA, 0.0),
        (T::CicadaB, 0.0),
        (T::Spider, 0.1),
        (T::JetFish, 0.0),
        (T::BigEel, 0.8),
        (T::Deer, 0.0),
        (T::TubeWorm, 0.0),
        (T::DaddyLongLegs, 0.85),
        (T::BrotherLongLegs, 0.75),
        (T::TentaclePlant, 0.5),
        (T::PoleMimic, 0.5),
        (T::MirosBird, 0.6),
        (T::TempleGuard, 0.0),
        (T::Centipede, 0.3),
        (T::RedCentipede, 0.9),
        (T::SmallCentipede, 0.0),
        (T::Centiwing, 0.4),
        (T::Scavenger, 0.65),
        (T::Overseer, 0.0),
        (T::VultureGrub, 0.0),
        (T::Hazer, 0.0),
        (T::EggBug, 0.0),
        (T::BigSpider, 0.35),
        (T::SpitterSpider, 0.5),
        (T::BigNeedleWorm, 0.5),
        (T::SmallNeedleWorm, 0.0),
        (T::DropBug, 0.4),
    ];
    assert_eq!(table.len(), 47);
    for (kind, danger) in table {
        let t = CreatureTemplate::of(kind, ActiveMods::NONE);
        assert_eq!(t.kind, kind);
        assert_eq!(t.dangerous_to_player, danger, "{kind:?}");
    }
}
