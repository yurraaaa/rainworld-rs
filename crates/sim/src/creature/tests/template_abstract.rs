
use rw_assets::ActiveMods;

use super::*;

fn cost(kind: CreatureType, m: AbstractMovement) -> AbstractCost {
    CreatureTemplate::of(kind, ActiveMods::NONE).abstract_resistance(m)
}

fn allowed(resistance: f32) -> AbstractCost {
    AbstractCost {
        resistance,
        allowed: true,
    }
}

fn stow(kind: CreatureType) -> bool {
    CreatureTemplate::of(kind, ActiveMods::NONE).stow_food_in_den()
}

fn sm(kind: CreatureType) -> bool {
    CreatureTemplate::of(kind, ActiveMods::NONE).social_memory()
}

#[test]
fn social_memory_per_template() {
    use CreatureType as T;
    assert!(sm(T::GreenLizard) && sm(T::KingVulture) && sm(T::CicadaB));
    assert!(sm(T::JetFish) && sm(T::Scavenger) && sm(T::BigNeedleWorm));
    assert!(!sm(T::SmallNeedleWorm) && !sm(T::Slugcat) && !sm(T::DropBug));
    assert!(!sm(T::Fly) && !sm(T::Overseer) && !sm(T::LanternMouse));
}

/// The abstract lists of `InitStaticWorld` (125287..) for the roaming types.
#[test]
fn abstract_connection_costs_follow_init_static_world() {
    use AbstractMovement as M;
    use CreatureType as T;
    assert_eq!(cost(T::KingVulture, M::SkyHighway), allowed(1.0));
    assert!(!cost(T::Vulture, M::ShortCut).allowed);
    // Cicada B clears its list: Cicada A's.
    assert_eq!(cost(T::CicadaB, M::ShortCut), allowed(1.0));
    assert_eq!(cost(T::CicadaB, M::BetweenRooms), allowed(10.0));
    assert_eq!(cost(T::Scavenger, M::OffScreenMovement), allowed(1.2));
    assert_eq!(cost(T::Scavenger, M::RegionTransportation), allowed(400.0));
    assert!(!cost(T::TempleGuard, M::BetweenRooms).allowed);
    assert_eq!(cost(T::SmallNeedleWorm, M::BetweenRooms), allowed(1.0));
    assert_eq!(cost(T::DropBug, M::BetweenRooms), allowed(5.0));
    assert_eq!(cost(T::Snail, M::ShortCut), allowed(0.2));
    assert_eq!(cost(T::BigEel, M::SeaHighway), allowed(100_000.0));
    assert_eq!(cost(T::Deer, M::SideHighway), allowed(100_000.0));
    assert_eq!(cost(T::MirosBird, M::SideHighway), allowed(100.0));
    assert!(!cost(T::GarbageWorm, M::OffScreenMovement).allowed);
}

#[test]
fn stow_food_in_den_per_template() {
    use CreatureType as T;
    assert!(stow(T::CicadaB) && stow(T::PoleMimic) && stow(T::SmallNeedleWorm));
    assert!(!stow(T::Centipede) && !stow(T::Centiwing) && !stow(T::Deer));
    assert!(!stow(T::Scavenger) && !stow(T::Snail));
}
