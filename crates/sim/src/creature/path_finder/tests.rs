//! The path finder's pieces with expected values from `PathFinder.cs`,
//! `PathfinderResourceDivider`, `PathCost` and `QuickConnectivity`.

use rw_assets::{ActiveMods, CreatureType};
use rw_random::UnityRandom;

use super::mapper::PathfinderResourceDivider;
use super::{PathingCell, WorldCoordinate};
use crate::creature::CreatureTemplate;
use crate::tests::room;
use crate::world::{AiMap, Legality, PathCost, QuickConnectivity};

#[test]
fn a_pathing_cell_draws_both_costs() {
    let mut rng = UnityRandom::new(3);
    let mut expected = rng.clone();
    let h = 123_456.0 * expected.value();
    let c = 123_456.0 * expected.value();
    let cell = PathingCell::new(WorldCoordinate::new(0, 1, 1, -1), &mut rng);
    assert_eq!(rng, expected, "two draws");
    assert_eq!(cell.heuristic.resistance, h);
    assert_eq!(cell.cost_to_goal.resistance, c);
    assert_eq!(cell.generation, -1);
    assert!(!cell.reachable && !cell.possible_to_get_back_from);
}

#[test]
fn the_divider_shares_out_last_frames_requests() {
    let mut d = PathfinderResourceDivider::default();
    // Under the budget (50) last frame: everything asked for.
    assert_eq!(d.request_pathfinder_updates(10), 10);
    assert_eq!(d.request_pathfinder_updates(90), 90);
    d.update();
    // 100 asked last frame: each gets its share of 50, floored, at least 1.
    assert_eq!(d.request_pathfinder_updates(10), 5);
    assert_eq!(d.request_pathfinder_updates(1), 1);
    // The accessibility budget is 500.
    assert_eq!(d.request_accessibility_updates(600), 600);
    d.update();
    assert_eq!(d.request_accessibility_updates(30), 25);
}

#[test]
fn path_costs_compare_by_legality_first() {
    let cheap_unwanted = PathCost::new(1.0, Legality::Unwanted);
    let dear_allowed = PathCost::new(100.0, Legality::Allowed);
    assert!(dear_allowed.less_or_equal(cheap_unwanted));
    assert!(!cheap_unwanted.less_or_equal(dear_allowed));
    assert!(
        PathCost::new(2.0, Legality::Allowed).less_or_equal(PathCost::new(2.0, Legality::Allowed))
    );
    // `+` keeps the worse legality.
    let sum = dear_allowed + cheap_unwanted;
    assert_eq!(sum.legality, Legality::Unwanted);
    assert_eq!(sum.resistance, 101.0);
}

#[test]
fn quick_connectivity_counts_generations_from_the_goal() {
    let r = room(&["##########", "#........#", "#........#", "##########"]);
    let map = AiMap::new(&r, 0, false, false);
    let t = CreatureTemplate::of(CreatureType::GreenLizard, ActiveMods::NONE);
    assert_eq!(QuickConnectivity::check(&map, &t, (2, 1), (2, 1), 100), 0);
    // Along the floor from the goal (6, 1): each expanded tile is one generation; the
    // tile next to the start is expanded in the fourth.
    let n = QuickConnectivity::check(&map, &t, (2, 1), (6, 1), 100);
    assert_eq!(n, 3);
    // Not reached within the generations allowed.
    assert_eq!(QuickConnectivity::check(&map, &t, (2, 1), (6, 1), 2), -1);
}
