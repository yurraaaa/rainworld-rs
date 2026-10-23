
use super::*;

/// `EstimatedChanceOfFinding` (`Tracker.cs` 62-77): 1 in sight; a fresh simple
/// representation (generation 1, 0 ticks) is `n = 2.5`; at `n = 45` the other branch.
#[test]
fn chance_of_finding() {
    let seen = Sighting {
        visual_contact: true,
        lowest_generation: 1,
        ticks_since_seen: 500,
    };
    assert_eq!(seen.estimated_chance_of_finding(), 1.0);
    let fresh = Sighting {
        visual_contact: false,
        ticks_since_seen: 0,
        ..seen
    };
    let n: f32 = 2.5;
    let expected = 1.0 / -(1.0 + std::f32::consts::E.powf(-(n / 12.0 - 5.0))) + 1.007;
    let got = fresh.estimated_chance_of_finding();
    assert!((got - expected).abs() < 1e-6, "{got} vs {expected}");
    assert!((got - 0.998_77).abs() < 1e-4);
    // n = (0 + 180) / 4 = 45: 30 / 38.
    let far = Sighting {
        lowest_generation: 0,
        ticks_since_seen: 180,
        ..fresh
    }
    .estimated_chance_of_finding();
    assert!((far - 30.0 / 38.0).abs() < 1e-6);
}

/// `TicksSinceSeen` counts only past the corners seen around (10 for lizards).
#[test]
fn ticks_since_seen_after_corners() {
    let mut r = CreatureRep::new(
        RepKey(0),
        CreatureId(1),
        WorldCoordinate::new(3, 4, 5, 2),
        1.0,
        RepKind::Simple {
            forget_when_not_visible: false,
        },
    );
    assert_eq!(r.last_seen_coord, WorldCoordinate::new(3, 4, 5, -1));
    r.ticks_since_seen = 9;
    assert_eq!(r.ticks_since_seen(10), 0);
    r.ticks_since_seen = 25;
    assert_eq!(r.ticks_since_seen(10), 15);
    assert_eq!(r.lowest_generation(), 1);
}

/// `MaxGhosts`: `maxGhosts * chance^0.25` clamped to 2..maxGhosts.
#[test]
fn max_ghosts() {
    let mut rng = UnityRandom::new(1);
    let p = Tracker::lizard().ghosts;
    let e = ElaborateRep::new(
        WorldCoordinate::new(0, 1, 1, -1),
        Vec2::ZERO,
        Vec2::ZERO,
        5,
        p,
        &mut rng,
    );
    assert_eq!(e.max_ghosts_now(1.0), 5);
    assert_eq!(e.max_ghosts_now(0.5), 4);
    assert_eq!(e.max_ghosts_now(0.0), 2);
}

/// The ghost constructor draws its move buffer, once.
#[test]
fn ghost_constructor_draws_once() {
    let mut rng = UnityRandom::new(7);
    let mut probe = UnityRandom::new(7);
    let g = Ghost::new(
        0,
        WorldCoordinate::new(2, 3, 4, 1),
        Vec2::ZERO,
        Vec2::ZERO,
        Tracker::lizard().ghosts,
        &mut rng,
    );
    assert_eq!(g.move_buffer, probe.value());
    assert_eq!(rng.state(), probe.state());
    assert!(!g.stopped);
    assert_eq!(g.push_forward, 5);
    assert_eq!(g.coord.node, -1);
}
