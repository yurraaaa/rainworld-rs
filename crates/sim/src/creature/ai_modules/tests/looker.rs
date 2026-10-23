use super::*;

impl LookCandidate {
    fn eats_half(key: u32, tile: (i32, i32), visual: bool) -> Self {
        Self::new(LookCandidateParts {
            key: RepKey(key),
            creature: CreatureId(key as usize),
            listed: true,
            best_guess: WorldCoordinate::new(1, tile.0, tile.1, -1),
            visual_field: visual,
            in_view: true,
            lowest_generation: 1,
            ticks_since_seen: 0,
            body_size: 1.0,
            scaryness: 1.0,
            relationship: Relationship {
                kind: RelationshipType::Eats,
                intensity: 0.5,
            },
            main_pos: Some(Vec2::ZERO),
            main_vel: Some(Vec2::ZERO),
            interest: 1.0,
            grabbed: false,
        })
    }
}

/// `ReevaluateLookObject` (`CreatureLooker.cs` 54-104) without a favourite draws its
/// coin, then one value per creature in the room when the coin lands under 0.5; with
/// a favourite only the coin.
#[test]
fn reevaluate_draws() {
    let cands = [
        LookCandidate::eats_half(0, (5, 5), true),
        LookCandidate::eats_half(1, (9, 5), true),
    ];
    let at = WorldCoordinate::new(1, 2, 5, -1);
    let mut probe = UnityRandom::new(3);
    let coin = probe.value() < 0.5;
    let mut rng = UnityRandom::new(3);
    let mut l = CreatureLooker::new(0.3, 160);
    l.reevaluate(&cands, None, 0.0, at, &mut rng);
    let mut expected = UnityRandom::new(3);
    for _ in 0..if coin { 3 } else { 1 } {
        expected.value();
    }
    assert_eq!(rng.state(), expected.state());
    let mut rng = UnityRandom::new(3);
    l.reevaluate(&cands, Some(RepKey(1)), 4.0, at, &mut rng);
    let mut one = UnityRandom::new(3);
    one.value();
    assert_eq!(rng.state(), one.state());
    // Favoured four times over and as close as the other: creature 1 wins.
    assert_eq!(l.look_creature, Some(RepKey(1)));
}

/// The score: `1 + Lerp(1, 1, .5) * 0.5 * 1 * 1 = 1.5` for an Eats-0.5 creature in
/// sight, still, divided by `Lerp(distance, 1, 0.6)`: at 3 tiles 1.5 / 1.8.
#[test]
fn score_by_distance() {
    let c = LookCandidate::eats_half(0, (5, 5), true);
    let at = WorldCoordinate::new(1, 2, 5, -1);
    let s = c.score(at, None, false, 0.0);
    assert!((s - 1.5 / 1.8).abs() < 1e-6, "{s}");
    assert!(c.guessed.get());
}

/// The `VisualContact` getter clears the field of a creature out of the room.
#[test]
fn getter_clears_out_of_view() {
    let mut c = LookCandidate::eats_half(0, (5, 5), true);
    c.in_view = false;
    assert!((c.chance_of_finding() - 1.0).abs() < 1e-6);
    assert!(!c.visual_contact());
    assert!(!c.visual_field.get());
    assert!(c.chance_of_finding() < 1.0);
}
