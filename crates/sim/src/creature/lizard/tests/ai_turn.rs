//! One green lizard (and the player) in one room, its AI's decisions. Expected values
//! from `LizardAI.cs` (`DetermineBehavior`, `Update`'s behaviours, `IdleSpotScore`,
//! `TravelPreference`, `ReactToNoise`, `WantToStayInDenUntilEndOfCycle`),
//! `PreyTracker.cs`, `ThreatTracker.cs`, `DenFinder.cs`/`NodeFinder.cs` and
//! `AbstractSpaceNodeFinder.Update`.

use rw_random::UnityRandom;

use crate::creature::lizard::LizardBehavior;
use crate::creature::{
    CreatureId, NodeFinderStatus, RelationshipType, UtilityModule, WorldCoordinate,
};
use crate::game::lizard_perception::tests::{HALL, game};
use crate::world::{Legality, MovementConnection, MovementType, PathCost};
use crate::{Input, RainWorldGame};

impl RainWorldGame {
    /// Advances the stream until the next draws satisfy `want` (each a test on one
    /// `value()`), so a behaviour takes the branch under test.
    fn rng_until(&mut self, want: &[fn(f32) -> bool]) {
        loop {
            let mut p = self.rng.clone();
            if want.iter().all(|w| w(p.value())) {
                return;
            }
            self.rng.value();
        }
    }

    /// The lizard's AI left to decide nothing on its own (its comparer's highest none).
    fn quiet_lizard(&mut self, tile: (i32, i32)) -> CreatureId {
        let l = self.seeing_lizard(tile, 1.0, 5);
        self.world.lizard_at(l).ai.utility.highest = None;
        l
    }
}

/// `DetermineBehavior`: no highest module (or one under 0.05) is Idle at 0.05; the
/// highest module picks its behaviour; no draws.
#[test]
fn determine_behavior_follows_the_highest_module() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let raw = crate::creature::RawUtilities::default();
    let probe = g.rng.clone();
    assert_eq!(
        g.lizard_determine_behavior(0, l, &raw),
        LizardBehavior::Idle
    );
    assert_eq!(g.world.lizard_ref(l).ai.current_utility, 0.05);
    let cases = [
        (UtilityModule::ThreatTracker, LizardBehavior::Flee),
        (UtilityModule::RainTracker, LizardBehavior::EscapeRain),
        (UtilityModule::PreyTracker, LizardBehavior::Hunt),
        (UtilityModule::LizardInjuryTracker, LizardBehavior::Injured),
        (UtilityModule::AgressionTracker, LizardBehavior::Fighting),
        (
            UtilityModule::MissionTracker,
            LizardBehavior::ActingOutMission,
        ),
        (
            UtilityModule::NoiseTracker,
            LizardBehavior::InvestigateSound,
        ),
    ];
    for (m, b) in cases {
        let mut raw = crate::creature::RawUtilities::default();
        raw.set(m, 1.0);
        let ai = &mut g.world.lizard_at(l).ai;
        ai.utility.highest = ai.utility.trackers.iter().position(|t| t.module == m);
        // The smoothed trackers read their stored value.
        for t in &mut ai.utility.trackers {
            t.smoothed_utility = if t.module == m { t.weight } else { 0.0 };
        }
        assert_eq!(g.lizard_determine_behavior(0, l, &raw), b, "{m:?}");
        let w = g.world.lizard_ref(l).ai.utility.tracker(m).unwrap().weight;
        assert!((g.world.lizard_ref(l).ai.current_utility - w).abs() < 1e-6);
        // Under 0.05 it idles.
        let mut low = crate::creature::RawUtilities::default();
        low.set(m, 0.04 / w);
        for t in &mut g.world.lizard_at(l).ai.utility.trackers {
            t.smoothed_utility = if t.module == m { 0.04 } else { 0.0 };
        }
        assert_eq!(
            g.lizard_determine_behavior(0, l, &low),
            LizardBehavior::Idle
        );
    }
    assert_eq!(g.rng, probe, "no draws");
}

/// Stranded, going home is Frustrated.
#[test]
fn stranded_escape_from_rain_is_frustrated() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let mut raw = crate::creature::RawUtilities::default();
    raw.set(UtilityModule::RainTracker, 1.0);
    let ai = &mut g.world.lizard_at(l).ai;
    ai.utility.highest = Some(2);
    ai.stranded = true;
    assert_eq!(
        g.lizard_determine_behavior(0, l, &raw),
        LizardBehavior::Frustrated
    );
}

/// `IdleSpotScore`: another room or off screen 50; a tile it cannot stand on, or the
/// forbidden spot, the most; a floor tile `10 / Lerp(visibility, 1, 0.9995) * 0.5`
/// (green), no threat or discomfort here.
#[test]
fn idle_spot_score() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    for _ in 0..40 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(
        g.lizard_idle_spot_score(0, l, WorldCoordinate::new(1, 5, 1, -1)),
        50.0
    );
    assert_eq!(
        g.lizard_idle_spot_score(0, l, WorldCoordinate::new(0, -1, -1, 0)),
        50.0
    );
    assert_eq!(
        g.lizard_idle_spot_score(0, l, WorldCoordinate::new(0, 5, 0, -1)),
        f32::MAX,
        "solid"
    );
    let floor = WorldCoordinate::new(0, 10, 1, -1);
    let map = g.world.realized(0).unwrap().ai_map.clone().unwrap();
    let vis = map.tile(10, 1).visibility as f32;
    assert_eq!(map.tile(10, 1).acc, crate::world::Accessibility::Floor);
    let expect = 10.0 / (vis + (1.0 - vis) * 0.9995) * 0.5;
    assert!(
        (g.lizard_idle_spot_score(0, l, floor) - expect).abs() < 1e-4,
        "{}",
        g.lizard_idle_spot_score(0, l, floor)
    );
    g.world.lizard_at(l).ai.forbidden_idle_spot = WorldCoordinate::new(0, 10, 1, 3);
    assert_eq!(
        g.lizard_idle_spot_score(0, l, floor),
        f32::MAX,
        "forbidden (node disregarded)"
    );
}

/// The Idle body near its spot, restless 40 or more: only the boredom roll (and the
/// voice's intensity on a hit).
#[test]
fn idle_near_its_spot_rolls_boredom() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let ai = &mut g.world.lizard_at(l).ai;
    ai.idle_counter = 500;
    ai.idle_restlessness = 60;
    let pos = g.world.creature(l).pos;
    g.world.set_destination(l, pos, &mut g.rng.clone());
    let mut probe = g.rng.clone();
    if probe.value() < 0.00125 {
        probe.value();
    }
    g.lizard_idle(0, l);
    assert_eq!(g.rng, probe);
    let ai = &g.world.lizard_ref(l).ai;
    assert_eq!(ai.idle_restlessness, 59);
    assert!(ai.idle_counter < 500);
    assert_eq!(ai.focus_creature, None);
}

/// Restless under 40 near its spot, the 1/600 roll comes first; a hit makes it
/// restless for `Range(1, 40)` ticks.
#[test]
fn idle_restlessness_roll() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let pos = g.world.creature(l).pos;
    g.world.set_destination(l, pos, &mut g.rng.clone());
    g.rng_until(&[|v| v < 0.001_666_666_7]);
    let ai = &mut g.world.lizard_at(l).ai;
    ai.idle_counter = 500;
    ai.idle_restlessness = 10;
    let mut probe = g.rng.clone();
    probe.value();
    let r = probe.range_i32(1, 40);
    if probe.value() < 0.00125 {
        probe.value();
    }
    g.lizard_idle(0, l);
    assert_eq!(g.rng, probe);
    assert_eq!(g.world.lizard_ref(l).ai.idle_restlessness, r);
}

/// Injured without a den: the lerp to 1 by 0.01 and the 1/80 pain roll (its voice at
/// `1 - health`); one draw on a miss.
#[test]
fn injured_rolls_pain() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    if let Some(a) = g.world.creature_mut(l).ai.as_mut() {
        a.den_position = None;
    }
    g.rng_until(&[|v| v >= 0.0125]);
    g.world.lizard_at(l).ai.run_speed = 0.0;
    let mut probe = g.rng.clone();
    probe.value();
    g.lizard_behave(0, l, LizardBehavior::Injured);
    assert_eq!(g.rng, probe);
    assert!((g.world.lizard_ref(l).ai.run_speed - 0.01).abs() < 1e-7);
}

/// Frustrated, far from its destination and not on the 1/10 roll: four rolls (re-spot,
/// wiggle, growl); it runs (lerp to 1 by half) and focuses on its prey.
#[test]
fn frustrated_rolls() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    if let Some(a) = g.world.creature_mut(l).ai.as_mut() {
        a.destination = WorldCoordinate::new(0, 2, 1, -1);
    }
    g.rng_until(&[
        |v| v >= 0.1,
        |v| v >= 0.0125,
        |v| v >= 0.0125,
        |v| v >= 0.0125,
    ]);
    g.world.lizard_at(l).ai.run_speed = 0.0;
    let mut probe = g.rng.clone();
    for _ in 0..4 {
        probe.value();
    }
    g.lizard_frustrated(0, l);
    assert_eq!(g.rng, probe);
    assert_eq!(g.world.lizard_ref(l).ai.run_speed, 0.5);
}

/// Frustrated on the wiggle roll: `Max(wiggle, (int)(value * 100))`, one more draw.
#[test]
fn frustrated_wiggle() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    if let Some(a) = g.world.creature_mut(l).ai.as_mut() {
        a.destination = WorldCoordinate::new(0, 2, 1, -1);
    }
    g.rng_until(&[|v| v >= 0.1, |v| v >= 0.0125, |v| v < 0.0125]);
    g.world.lizard_at(l).set_body_wiggle_counter(0);
    let mut probe = g.rng.clone();
    probe.value();
    probe.value();
    probe.value();
    let w = (probe.value() * 100.0) as i32;
    if probe.value() < 0.0125 {
        probe.value();
    }
    g.lizard_frustrated(0, l);
    assert_eq!(g.rng, probe);
    assert_eq!(g.world.lizard_ref(l).body_wiggle_counter, w.clamp(0, 100));
}

/// The other bodies make no draws: Travelling without a migration eases to 0.5,
/// InvestigateSound without a sound eases to 0.6, ActingOutMission without a mission
/// eases to 0.6 by a twentieth, Fighting without a target changes nothing.
#[test]
fn draw_free_bodies() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let probe = g.rng.clone();
    g.world.lizard_at(l).ai.run_speed = 0.0;
    g.lizard_behave(0, l, LizardBehavior::Travelling);
    assert!((g.world.lizard_ref(l).ai.run_speed - 0.005).abs() < 1e-7);
    g.world.lizard_at(l).ai.noise.sound_to_examine = None;
    g.world.lizard_at(l).ai.run_speed = 0.0;
    g.lizard_behave(0, l, LizardBehavior::InvestigateSound);
    assert!((g.world.lizard_ref(l).ai.run_speed - 0.12).abs() < 1e-6);
    g.world.lizard_at(l).ai.run_speed = 1.0;
    g.lizard_behave(0, l, LizardBehavior::ActingOutMission);
    assert!((g.world.lizard_ref(l).ai.run_speed - 0.98).abs() < 1e-6);
    g.world.lizard_at(l).ai.agression.highest_target = None;
    g.lizard_behave(0, l, LizardBehavior::Fighting);
    assert!((g.world.lizard_ref(l).ai.run_speed - 0.98).abs() < 1e-6);
    assert_eq!(g.rng, probe);
}

/// Seeing the player it eats, the prey tracker takes it (once the dynamic relationship
/// turns to Eats): `MostAttractivePrey`, and a utility of
/// `Min(Lerp(1 - InverseLerp(3, 70, d), Lerp(3, 70, 0.25) / d, 0.5), chance) *
/// intensity^0.75 * reachable` (in sight: chance 1).
#[test]
fn the_seen_player_becomes_prey() {
    let mut g = game(&HALL, (3, 1));
    let l = g.seeing_lizard((10, 1), -1.0, 40);
    let ai = &g.world.lizard_ref(l).ai;
    let rep = ai.tracker.representation_for(g.player_id).expect("tracked");
    let rel = rep.current_relationship().unwrap();
    assert_eq!(rel.kind, RelationshipType::Eats);
    assert_eq!(ai.prey.current, Some(rep.key));
    let key = rep.key;
    let mut t = ai.tracker.clone();
    let u = g.world.prey_utility(l, &mut t);
    let guess = g
        .world
        .rep_best_guess(l, t.rep_mut(key).unwrap(), g.world.path_finder(l));
    let pos = g.world.creature(l).pos;
    let d = crate::math::vec2::TileExt::float_dist(pos.tile, guess.tile);
    let near = 1.0 - ((d - 3.0) / 67.0).clamp(0.0, 1.0);
    let q = 3.0 + 67.0 * 0.25;
    let dist_term = near + (q / d - near) * 0.5;
    let chance = t.chance_of_finding(t.rep(key).unwrap());
    let p = ai.prey.prey.iter().find(|p| p.rep == key).unwrap().clone();
    let reach = g.world.prey_reachable(l, &mut t, &p);
    let expect = dist_term.min(chance) * crate::FloatExt::mathf_pow(rel.intensity, 0.75) * reach;
    assert!((u - expect).abs() < 1e-5, "{u} vs {expect}");
    assert!(u > 0.0);
}

/// `ThreatTracker.NewRoom`: one draw, a random node of the room to flee to.
#[test]
fn threat_tracker_new_room_draws_a_node() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let nodes = g.world.abstract_room(0).unwrap().nodes.len() as i32;
    let mut probe = g.rng.clone();
    let node = probe.range_i32(0, nodes);
    let mut rng = g.rng.clone();
    g.world.threat_tracker_new_room(l, 0, &mut rng);
    assert_eq!(rng, probe);
    let th = &g.world.lizard_ref(l).ai.threat;
    assert_eq!(
        (th.map_room, th.saved_flee_dest),
        (0, WorldCoordinate::new(0, -1, -1, node))
    );
}

/// The pather's threat terms: with a threat point of severity 1 on the destination
/// tile, `threat^3 * 2 * fear^0.5`; afraid above 0.2, a drop into a more threatened
/// area costs 100 more.
#[test]
fn threat_travel_cost() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    let dest = WorldCoordinate::new(0, 10, 1, -1);
    let ai = &mut g.world.lizard_at(l).ai;
    ai.threat.add_point(None, dest, 1.0);
    ai.threat.current_threat = 0.5;
    ai.fear = 0.25;
    let start = WorldCoordinate::new(0, 10, 3, -1);
    let c = MovementConnection::new(MovementType::DropToFloor, start, dest, 2);
    let f = g.world.threat_of_tile(l, dest);
    let map = g.world.realized(0).unwrap().ai_map.clone().unwrap();
    let vis = map.tile(10, 1).visibility as f32 / (map.width() * map.height()) as f32;
    // 1^1.5 * 10 / max(1, 0) = 10, below 15: no visibility term.
    assert!((f - 10.0).abs() < 1e-5, "{f} {vis}");
    let cost = g
        .world
        .lizard_threat_travel_cost(l, &c, PathCost::new(1.0, Legality::Allowed));
    let expect = 1.0 + 1000.0 * 2.0 * 0.5 + 100.0;
    assert!(
        (cost.resistance - expect).abs() < 1e-2,
        "{}",
        cost.resistance
    );
    g.world.lizard_at(l).ai.fear = 0.2;
    let cost = g
        .world
        .lizard_threat_travel_cost(l, &c, PathCost::new(1.0, Legality::Allowed));
    let expect = 1.0 + 1000.0 * 2.0 * 0.2f32.sqrt();
    assert!(
        (cost.resistance - expect).abs() < 1e-2,
        "{}",
        cost.resistance
    );
}

/// A den finder without a den in a room without dens: the creature's node defined
/// (draws) at the constructor; the search starts the next update and fails after its
/// floods (one node, then the empty check): no accessible den.
#[test]
fn den_finder_without_dens_finds_none() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    if let Some(a) = g.world.creature_mut(l).ai.as_mut() {
        a.den_position = None;
    }
    g.world.lizard_at(l).ai.den_finder.status = NodeFinderStatus::Working;
    let mut rng = UnityRandom::new(3);
    g.world.den_finder_update(l, &mut rng);
    let left = g
        .world
        .lizard_ref(l)
        .ai
        .den_finder
        .search
        .as_ref()
        .expect("started")
        .updates_left;
    assert!(left >= 2);
    for _ in 0..left {
        assert_eq!(
            g.world.lizard_ref(l).ai.den_finder.status,
            NodeFinderStatus::Working
        );
        g.world.den_finder_update(l, &mut rng);
    }
    let df = &g.world.lizard_ref(l).ai.den_finder;
    assert_eq!(df.status, NodeFinderStatus::NoAccessible);
    assert!(df.search.is_none());
    assert_eq!(
        g.world.creature(l).ai.as_ref().and_then(|a| a.den_position),
        None
    );
}

/// `WantToStayInDenUntilEndOfCycle` of a realized lizard: escaping the rain or
/// injured.
#[test]
fn wants_to_stay_in_den_by_behavior() {
    let mut g = game(&HALL, (3, 1));
    let l = g.quiet_lizard((20, 1));
    g.world.rain_cycle.timer = 0;
    for (b, want) in [
        (LizardBehavior::Idle, false),
        (LizardBehavior::EscapeRain, true),
        (LizardBehavior::Injured, true),
        (LizardBehavior::Hunt, false),
    ] {
        g.world.lizard_at(l).ai.behavior = b;
        assert_eq!(g.world.wants_to_stay_in_den(l), want, "{b:?}");
    }
}

/// A lizard left alone idles and walks: within a few hundred ticks its runSpeed has
/// been 0.25 and its head has moved.
#[test]
fn an_idle_lizard_walks_on_its_own() {
    let mut g = game(&HALL, (2, 1));
    let l = g.seeing_lizard((20, 1), 1.0, 5);
    let start = g.world.lizard_ref(l).head().pos;
    let mut walked = false;
    let mut moved = 0.0f32;
    for _ in 0..400 {
        g.tick(Input::NONE, &mut ());
        let ai = &g.world.lizard_ref(l).ai;
        walked |= ai.run_speed > 0.2;
        moved = moved.max(g.world.lizard_ref(l).head().pos.distance(start));
    }
    assert!(walked);
    assert!(moved > 20.0, "{moved}");
}

/// A hall whose only node is a den (node 0) in the left wall.
const DEN_HALL: [&str; 6] = [
    "##############################",
    "#............................#",
    "#............................#",
    "##...........................#",
    "DE...........................#",
    "##############################",
];

/// `CheckOriginalDenReachability` in a single-room world (no spawner's den): a lizard
/// not made in a den keeps `spawnDen` (-1, -1, -1, -1), which `TryAssigningDen` turns
/// down (no node), so the reachable den at node 0 of room 0 is not made its den this
/// way; that den itself would be taken.
#[test]
fn no_spawn_den_assigns_no_den() {
    let mut g = game(&DEN_HALL, (10, 1));
    let l = g.quiet_lizard((20, 1));
    assert_eq!(g.world.creature(l).spawn_den, None);
    if let Some(a) = g.world.creature_mut(l).ai.as_mut() {
        a.den_position = None;
    }
    g.world.check_original_den_reachability(l);
    assert_eq!(
        g.world.creature(l).ai.as_ref().and_then(|a| a.den_position),
        None
    );
    let den = WorldCoordinate::new(0, -1, -1, 0);
    g.world.try_assigning_den(l, den);
    assert_eq!(
        g.world.creature(l).ai.as_ref().and_then(|a| a.den_position),
        Some(den)
    );
}
