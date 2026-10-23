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
