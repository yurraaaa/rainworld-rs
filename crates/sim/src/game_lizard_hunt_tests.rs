//! One green lizard (and the player) in one room hunting and fleeing. Expected values
//! from `LizardAI.cs` (`Update`'s Hunt and Flee bodies, `AggressiveBehavior`,
//! `UnpleasantFallRisk`, `FallRisk`, `BitCreature`, `SocialEvent`),
//! `ThreatTracker.cs` (`FleeTo`, `GenerateRandomPath`, `EvaluateFlightDestThreat`,
//! `FindMostAttractiveExit`), `QuickConnectivity.QuickPath`,
//! `RainWorldGame.RequestHeavyAi` and `LizardBreeds` (green: `loungeDistance` 170,
//! `biteInFront` 40, `loungeTendensy` 1).

use rw_random::UnityRandom;

use crate::creature::lizard::{LizardBehavior, LizardGraphicsLink};
use crate::creature::{CreatureId, LizardAnimation, RepKey, WorldCoordinate};
use crate::game::lizard_perception::tests::{HALL, game};
use crate::vec2::TileExt;
use crate::world::{QuickConnectivity, SocialEventId};
use crate::{FloatExt, RainWorldGame};

/// The draws between `before` and `after`.
fn draws(before: &UnityRandom, after: &UnityRandom) -> usize {
    let mut p = before.clone();
    for n in 0..100_000 {
        if &p == after {
            return n;
        }
        p.value();
    }
    panic!("streams never meet");
}

impl RainWorldGame {
    /// Advances the stream until the next draw satisfies `want`.
    fn rng_until_next(&mut self, want: fn(f32) -> bool) {
        loop {
            let mut p = self.rng.clone();
            if want(p.value()) {
                return;
            }
            self.rng.value();
        }
    }

    /// A still lizard at `tile` facing the player (to its left), after `ticks` ticks, and
    /// the player's representation in its tracker.
    fn hunter(&mut self, tile: (i32, i32), ticks: usize) -> (CreatureId, RepKey) {
        let l = self.still_lizard(tile, -1.0, ticks);
        let k = self
            .world
            .lizard_ref(l)
            .ai
            .tracker
            .representation_for(self.player_id)
            .expect("tracks the player")
            .key;
        (l, k)
    }

    /// `FleeTo`'s tries from `pos` with no threats, on a copy of the stream: the stream
    /// after them and the first viable tile counted (the one that becomes the test
    /// destination: every viable path there is threat-free).
    fn flee_tries(
        &self,
        l: CreatureId,
        pos: WorldCoordinate,
    ) -> (UnityRandom, Option<WorldCoordinate>) {
        let map = self.world.realized(0).unwrap().ai_map.clone().unwrap();
        let template = self.world.creature(l).template;
        let (w, h) = (map.width(), map.height());
        let mut probe = self.rng.clone();
        let mut test = self.world.lizard_ref(l).ai.threat.test_flee_dest;
        let mut found = None;
        let mut scored = 0;
        for _ in 0..25 {
            let dx = probe.range_i32(0, 20) * if probe.value() < 0.5 { -1 } else { 1 };
            let dy = probe.range_i32(0, 20) * if probe.value() < 0.5 { -1 } else { 1 };
            let c = WorldCoordinate::new(
                0,
                (pos.tile.0 + dx).clamp(0, w - 1),
                (pos.tile.1 + dy).clamp(0, h - 1),
                -1,
            );
            if test.tile.float_dist(c.tile) > 3.0
                && map.world_coordinate_accessible_to_creature(c, &template)
            {
                let viable = self.world.path_finder(l).unwrap().coordinate_viable(c);
                if found.is_none() && viable {
                    found = Some(c);
                    test = c;
                }
                scored += 1;
            }
            if scored >= 5 {
                break;
            }
        }
        (probe, found)
    }

    fn distance_to_player(&self, l: CreatureId) -> f32 {
        let p = self.tracked_body(self.player_id).unwrap().main().pos;
        self.world.lizard_ref(l).head().pos.distance(p)
    }
}

/// Hunt without prey: `runSpeed` a tenth of the way to `Max(hunger, 0.3)`, and one draw
/// for the blood-lust voice (1/80, missed here).
#[test]
fn hunt_without_prey_draws_once() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((20, 1), 1.0, 0);
    let ai = &mut g.world.lizard_at(l).ai;
    ai.prey.current = None;
    ai.hunger = 0.0;
    ai.run_speed = 0.0;
    g.rng_until_next(|v| v >= 0.0125);
    let mut probe = g.rng.clone();
    probe.value();
    g.lizard_hunt(l);
    assert_eq!(g.rng, probe);
    assert!((g.world.lizard_ref(l).ai.run_speed - 0.03).abs() < 1e-7);
}

/// `AggressiveBehavior` at a seen target with the lounge delay running: no casual
/// target, the destination the target's guess, the jaw `InverseLerp(400, 90 +
/// biteInFront, distance)`, the focus the target, no lounge roll and, tracked for
/// fewer than 120 ticks, no bite.
#[test]
fn aggressive_behavior_opens_the_jaw_as_the_target_nears() {
    let mut g = game(&HALL, (3, 1));
    let (l, k) = g.hunter((12, 1), 40);
    let lz = g.world.lizard_at(l);
    lz.lounge_delay = 10;
    lz.animation = LizardAnimation::Standard;
    lz.ai.casual_aggression_target = Some(k);
    let probe = g.rng.clone();
    g.lizard_aggressive_behavior(l, k, 1.0);
    let d = g.distance_to_player(l);
    let lz = g.world.lizard_ref(l);
    assert_eq!(lz.ai.casual_aggression_target, None);
    assert_eq!(lz.ai.focus_creature, Some(k));
    let expect = d.inverse_lerp(400.0, 130.0);
    assert!(
        (lz.jaw_open(false) - expect).abs() < 1e-6,
        "{}",
        lz.jaw_open(false)
    );
    let age = lz.ai.tracker.rep(k).unwrap().age;
    assert!(age <= 120);
    assert_eq!(lz.ai.attempt_bite_frames, 0);
    assert_eq!(g.rng, probe, "no draws");
    let dest = g.world.creature(l).ai.as_ref().unwrap().destination;
    assert_eq!(dest.room, 0);
    assert_eq!(dest.tile.1, 1, "the player's floor");
}

/// With the delay out, the lounge roll (one draw against `loungeTendensy` 1) and the
/// ground for it (floor under the head, three legs gripping, within 170 px) make it
/// crouch: `PrepareToLounge`, its direction toward the target. Two gripping legs (no
/// graphics module) never lounge.
#[test]
fn aggressive_behavior_crouches_to_lounge() {
    let mut g = game(&HALL, (3, 1));
    let (l, k) = g.hunter((10, 1), 40);
    assert!(
        g.distance_to_player(l) < 170.0,
        "{}",
        g.distance_to_player(l)
    );
    let lz = g.world.lizard_at(l);
    lz.lounge_delay = 0;
    lz.animation = LizardAnimation::Standard;
    lz.graphics = None;
    let before = g.rng.clone();
    g.lizard_aggressive_behavior(l, k, 1.0);
    assert_eq!(g.world.lizard_ref(l).animation, LizardAnimation::Standard);
    assert_eq!(draws(&before, &g.rng), 1, "the roll only");
    g.world.lizard_at(l).graphics = Some(LizardGraphicsLink {
        legs_grabbing: 4,
        ..LizardGraphicsLink::default()
    });
    g.lizard_aggressive_behavior(l, k, 1.0);
    let lz = g.world.lizard_ref(l);
    assert_eq!(lz.animation, LizardAnimation::PrepareToLounge);
    let p = g.tracked_body(g.player_id).unwrap().main().pos;
    let dir = lz.head().pos.direction_to(p);
    assert!((lz.lounge_dir - dir).length() < 1e-5);
}

/// Over 120 ticks tracked with the jaw ready the bite frames count; not ready, more than
/// five frames end in `JawsSnapShut` toward the target (jaw forced shut, `biteDelay`
/// set) and the count restarts.
#[test]
fn bite_frames_and_the_snap() {
    let mut g = game(&HALL, (3, 1));
    let (l, k) = g.hunter((10, 1), 40);
    let lz = g.world.lizard_at(l);
    lz.lounge_delay = 10;
    lz.ai.tracker.rep_mut(k).unwrap().age = 121;
    lz.bite_delay = 0;
    lz.jaw_forced_shut = 0.0;
    lz.animation = LizardAnimation::Lounge;
    lz.set_jaw_open(0.85, false);
    // Leaping, the jaw opens by 0.2 to 1: ready, but the target is out of reach.
    g.lizard_aggressive_behavior(l, k, 1.0);
    assert_eq!(g.world.lizard_ref(l).ai.attempt_bite_frames, 1);
    let lz = g.world.lizard_at(l);
    lz.animation = LizardAnimation::Standard;
    lz.ai.attempt_bite_frames = 6;
    lz.bite_delay = 3;
    g.lizard_aggressive_behavior(l, k, 1.0);
    let lz = g.world.lizard_ref(l);
    assert_eq!(lz.ai.attempt_bite_frames, 0);
    assert_eq!(lz.jaw_forced_shut, 1.0);
    assert_eq!(lz.bite_delay, lz.params.bite_delay);
}

/// On the floor of a closed room no fall is unpleasant or a risk, and `BitCreature`
/// makes it shake its prey (forced).
#[test]
fn bit_creature_shakes_on_safe_ground() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((20, 1), 1.0, 5);
    let tile = g.world.creature(l).pos.tile;
    assert!(!g.lizard_unpleasant_fall_risk(l, tile));
    assert!(!g.lizard_fall_risk(l, tile));
    g.lizard_bit_creature(l);
    assert_eq!(g.world.lizard_ref(l).animation, LizardAnimation::ShakePrey);
}

/// `RequestHeavyAi` gates the update of a lizard that fled last update: granted, the
/// update runs (its room clock counts); asked again in the same tick it is in the
/// last-run list, so the update returns at once. Other behaviours never ask.
#[test]
fn a_fleeing_lizard_needs_a_heavy_ai_slot() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((20, 1), 1.0, 5);
    g.world.lizard_at(l).ai.behavior = LizardBehavior::Idle;
    let t = g.world.lizard_ref(l).ai.lizard_time_in_room;
    g.lizard_ai_update(0, l);
    assert_eq!(g.world.lizard_ref(l).ai.heavy_ai_granted, None);
    assert_eq!(g.world.lizard_ref(l).ai.lizard_time_in_room, t + 1);
    g.heavy_ai.end_of_tick();
    g.world.lizard_at(l).ai.behavior = LizardBehavior::Flee;
    g.lizard_ai_update(0, l);
    assert_eq!(g.world.lizard_ref(l).ai.heavy_ai_granted, Some(true));
    assert_eq!(g.world.lizard_ref(l).ai.lizard_time_in_room, t + 2);
    g.world.lizard_at(l).ai.behavior = LizardBehavior::Flee;
    let probe = g.rng.clone();
    g.lizard_ai_update(0, l);
    assert_eq!(g.world.lizard_ref(l).ai.heavy_ai_granted, Some(false));
    assert_eq!(g.world.lizard_ref(l).ai.lizard_time_in_room, t + 2);
    assert_eq!(g.rng, probe, "nothing ran");
}

/// `FleeTo` without threats: five reevaluations (the budget is not short), each try
/// four draws (x range, sign, y range, sign) on a tile clamped into the room; a try
/// counts when the tile is accessible and more than 3 tiles from the test destination,
/// and the fifth counted ends the search. Every viable straight path is threat-free
/// (0), so the first viable tile becomes the test destination, and (the saved one, a
/// node, not viable: the most) the saved destination, with 20 anti-flicker ticks.
#[test]
fn flee_to_draws_four_per_try() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((15, 1), 1.0, 60);
    let pos = g.world.creature(l).pos;
    let (probe, found) = loop {
        let (probe, found) = g.flee_tries(l, pos);
        if found.is_some() {
            break (probe, found);
        }
        g.rng.value();
    };
    let mut rng = g.rng.clone();
    let dest = g
        .world
        .threat_flee_to(l, pos, 5, 20, false, false, None, &mut rng);
    assert_eq!(rng, probe);
    let th = &g.world.lizard_ref(l).ai.threat;
    let found = found.expect("a viable tile");
    assert_eq!(th.test_flee_dest, found);
    assert_eq!(th.saved_flee_dest, found);
    assert_eq!(th.anti_flicker_counter, 20);
    assert_eq!(dest, found);
    assert_eq!(th.flee_threat, 0.0);
}

/// The Flee body with a threat point on its tile (`ThreatOfTile` over 1): the panic
/// jumps to 40 (then up by 2), the destination `FleeTo`'s, `runSpeed` halfway to
/// `CombinedFear^0.1`.
#[test]
fn the_flee_body_panics_on_a_threatened_tile() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((15, 1), 1.0, 60);
    let pos = g.world.creature(l).pos;
    let ai = &mut g.world.lizard_at(l).ai;
    ai.threat.add_point(None, pos, 1.0);
    ai.fear = 0.5;
    ai.run_speed = 0.0;
    ai.panic = 0;
    assert!(g.world.threat_of_tile(l, pos) > 1.0);
    g.lizard_flee(l);
    let ai = &g.world.lizard_ref(l).ai;
    assert_eq!(ai.panic, 40);
    let to = 0.5f32.mathf_pow(0.1);
    assert!((ai.run_speed - to * 0.5).abs() < 1e-6);
    let flee = ai.flee_to.expect("fled");
    assert_eq!(g.world.creature(l).ai.as_ref().unwrap().destination, flee);
    g.lizard_flee(l);
    assert_eq!(g.world.lizard_ref(l).ai.panic, 42);
}

/// `GenerateRandomPath` of one step: one draw per allowed outgoing connection of the
/// tile, the path its start tile.
#[test]
fn generate_random_path_draws_per_connection() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((15, 1), 1.0, 60);
    let pos = g.world.creature(l).pos;
    let map = g.world.realized(0).unwrap().ai_map.clone().unwrap();
    let t = g.world.creature(l).template;
    let allowed = map
        .tile(pos.tile.0, pos.tile.1)
        .outgoing_paths
        .iter()
        .filter(|c| {
            t.connection_resistance(c.kind).allowed()
                && t.accessibility_resistance(map.tile_at(c.dest).acc)
                    .allowed()
        })
        .count();
    assert!(allowed > 0);
    let mut rng = g.rng.clone();
    let path = g.world.threat_generate_random_path(l, pos, 1, &mut rng);
    assert_eq!(path, Some(vec![pos.tile]));
    assert_eq!(draws(&g.rng, &rng), allowed);
    let off = WorldCoordinate::new(0, 5, 0, -1);
    assert_eq!(
        g.world.threat_generate_random_path(l, off, 3, &mut rng),
        None,
        "solid"
    );
}

/// `QuickPath` along a floor: the tiles walked from the start before the goal; the
/// start itself is a path of one; beyond the distance none.
#[test]
fn quick_path_along_the_floor() {
    let g = game(&HALL, (3, 1));
    let map = g.world.realized(0).unwrap().ai_map.clone().unwrap();
    let t = crate::creature::CreatureTemplate::of(
        rw_assets::CreatureType::GreenLizard,
        rw_assets::ActiveMods::NONE,
    );
    let mut path = Vec::new();
    let n = QuickConnectivity::quick_path(&map, &t, (5, 1), (12, 1), 40, 500, true, &mut path);
    assert_eq!(n, 7);
    assert_eq!(
        &path[..n],
        &[(5, 1), (6, 1), (7, 1), (8, 1), (9, 1), (10, 1), (11, 1)]
    );
    assert_eq!(
        QuickConnectivity::quick_path(&map, &t, (5, 1), (5, 1), 40, 500, true, &mut path),
        1
    );
    assert_eq!(
        QuickConnectivity::quick_path(&map, &t, (5, 1), (12, 1), 6, 500, true, &mut path),
        0
    );
}

/// `FindMostAttractiveExit` in a room without exits: -1, no draws.
#[test]
fn no_exit_is_attractive_without_exits() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((15, 1), 1.0, 60);
    let mut rng = g.rng.clone();
    let nodes = &g.world.abstract_room(0).unwrap().nodes;
    assert!(nodes.iter().all(|n| n.kind != rw_assets::NodeType::Exit));
    assert_eq!(g.world.threat_find_most_attractive_exit(l, &mut rng), -1);
    assert_eq!(rng, g.rng);
}

/// `SocialEvent`'s fear branch: an attack on a creature it fears that is not a threat
/// creature (nor holds it) counts `sqrt(offence) * 0.1`; as a threat creature 0.7 more
/// times `LerpMap(distance to its DangerPos, 120, 320, 1, 0.1)`.
#[test]
fn attacking_what_it_fears_relieves_it() {
    let mut g = game(&HALL, (3, 1));
    let me = g.still_lizard((20, 1), -1.0, 5);
    let other = g.still_lizard((12, 1), 1.0, 40);
    let offence = SocialEventId::LethalAttack.lizard_offence();
    assert_eq!(
        g.world
            .lizard_fear_relief(me, SocialEventId::LethalAttack, other, offence),
        (1.0, 0.1)
    );
    let k = g
        .world
        .lizard_ref(me)
        .ai
        .tracker
        .representation_for(other)
        .expect("sees the other")
        .key;
    let ai = &mut g.world.lizard_at(me).ai;
    let point = ai
        .threat
        .add_point(None, WorldCoordinate::new(0, 12, 1, -1), 0.5);
    ai.threat.creatures.push(crate::creature::ThreatCreature {
        rep: k,
        severity: 0.5,
        point,
    });
    let d = g
        .world
        .lizard_ref(me)
        .head()
        .pos
        .distance(g.world.lizard_ref(other).head().pos);
    let expect = 0.1 + 0.7 * d.lerp_map(120.0, 320.0, 1.0, 0.1);
    let (o, fac) =
        g.world
            .lizard_fear_relief(me, SocialEventId::NonLethalAttackAttempt, other, 0.05);
    assert_eq!(o, 0.05);
    assert!((fac - expect).abs() < 1e-6);
}

/// `focusCreature` is a reference the AI keeps: when the tracker drops the
/// representation the focus still reads it (its `VisualContact`, the realized body), so
/// a crouch on a seen target goes on.
#[test]
fn the_focus_outlives_its_place_in_the_tracker() {
    let mut g = game(&HALL, (3, 1));
    let (l, k) = g.hunter((10, 1), 40);
    g.world.lizard_at(l).ai.focus_creature = Some(k);
    let seen = g.lizard_focus(l).expect("focus").visual;
    assert!(seen, "the hunter sees the player");
    let held = g.ai_held_reps(l);
    let t = &mut g.world.lizard_at(l).ai.tracker;
    let i = t.index_of(k).expect("listed");
    t.remove_rep(i, &held);
    assert!(g.world.lizard_ref(l).ai.tracker.index_of(k).is_none());
    let f = g.lizard_focus(l).expect("still the focus");
    assert_eq!(f.creature, g.player_id);
    assert!(f.visual);
    assert!(f.chunks.is_some(), "the realized body");
    let lz = g.world.lizard_at(l);
    lz.ai.behavior = LizardBehavior::Hunt;
    lz.animation = LizardAnimation::PrepareToLounge;
    g.lizard_act_animation(l);
    assert_eq!(
        g.world.lizard_ref(l).animation,
        LizardAnimation::PrepareToLounge
    );
}
