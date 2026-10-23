//! The circuits in a test room: `AIMapReady` grouping the placed objects into clusters and
//! building one circuit each after the wall mycelia, the update list running the circuit
//! first and the bits last, the circuit's push on the player and what the player takes of
//! it, and the circuits running while their system is frozen.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};

use super::{CircuitRef, RainWorldGame};
use crate::game::RoomObject;
use crate::superstructure::coral::turn::tests::{coral_game, coral_game_with};
use crate::{BitRef, CircuitPlace, CoralCircuit, SoundEvent, SoundId, Sounds, SystemId, Vec2};

/// Two overlapping circuits (distance 70 under 80 + 80) and one far off.
const PLACED: &str =
    "CoralCircuit><400><200><0~80, CoralCircuit><470><200><0~80, CoralCircuit><700><200><0~40";

fn place(x: f32, rad: f32) -> CircuitPlace {
    CircuitPlace {
        pos: Vec2::new(x, 0.0),
        rad,
    }
}

/// Clusters: the first place, then the places within `Rad + Rad` of one in the cluster,
/// found from the end of the rest, a place joining through a later member too.
#[test]
fn places_group_into_clusters_by_overlap() {
    let (a, b, c, d) = (
        place(0.0, 50.0),
        place(90.0, 50.0),
        place(500.0, 50.0),
        place(180.0, 50.0),
    );
    let clusters = CoralCircuit::clusters(vec![a, b, c, d]);
    assert_eq!(clusters, vec![vec![a, b, d], vec![c]]);
    assert_eq!(CoralCircuit::clusters(vec![]), Vec::<Vec<_>>::new());
    // Exactly `Rad + Rad` apart is not overlapping (`DistLess`).
    let clusters = CoralCircuit::clusters(vec![place(0.0, 50.0), place(100.0, 50.0)]);
    assert_eq!(clusters.len(), 2);
}

/// `AIMapReady` makes one circuit per cluster after the wall mycelia (their places in the
/// update list come first) and leaves the shared stream as an `SSSwarmers`-only room has
/// it: the constructor runs on its own seed.
#[test]
fn ai_map_ready_builds_a_circuit_per_cluster_after_the_wall_mycelia() {
    let placed = format!("WallMycelia><300><100><0~150, {PLACED}");
    let g = coral_game(&placed, &[], 1);
    let system = g.world.realized(0).unwrap().coral.as_ref().unwrap();
    assert_eq!(system.circuits.len(), 2);
    assert_eq!(system.circuits[0].places.len(), 2);
    assert_eq!(system.circuits[1].places.len(), 1);
    assert!(system.circuits[0].places[0].pos == Vec2::new(400.0, 200.0));
    let wall = system.wall_mycelia[0].update_order;
    let first = system.circuits[0].bits[0].update_order;
    assert!(wall < first && first < system.circuits[0].update_order);
    assert!(system.circuits[0].update_order < system.circuits[1].bits[0].update_order);
    assert!(system.circuits[1].bits.len() > 4);
    // Building one leaves the shared stream alone (the constructor runs on its own seed).
    let mut g = coral_game(&placed, &[], 1);
    let before = g.rng.state();
    g.add_coral_circuit(0, SystemId(1), vec![place(300.0, 60.0)]);
    assert_eq!(g.rng.state(), before);
    assert_eq!(
        g.world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .circuits
            .len(),
        3
    );
}

/// The list is walked from its end: the circuit, then the crabs, the connectors, and the
/// bits from the last to the first.
#[test]
fn the_circuit_updates_first_and_the_bits_last() {
    let g = coral_game(PLACED, &[], 1);
    let list = UpdateList(g.room_update_list(0));
    let c = &g
        .world
        .realized(0)
        .unwrap()
        .coral
        .as_ref()
        .unwrap()
        .circuits[0];
    let (bits, crabs, conns) = (
        c.bits.len() as u32,
        c.crabs.len() as u32,
        c.connectors.len() as u32,
    );
    assert!(bits > 0 && crabs > 0 && conns > 0);
    assert!(list.at(CircuitRef::Circuit(0)) < list.at(CircuitRef::Crab(0, crabs - 1)));
    assert!(list.at(CircuitRef::Crab(0, 0)) < list.at(CircuitRef::Connector(0, conns - 1)));
    assert!(list.at(CircuitRef::Connector(0, 0)) < list.at(CircuitRef::Bit(0, bits - 1)));
    assert!(list.at(CircuitRef::Bit(0, bits - 1)) < list.at(CircuitRef::Bit(0, 0)));
}

/// A room's update list, newest first.
struct UpdateList(Vec<(u64, RoomObject)>);

impl UpdateList {
    /// The place of a circuit object in the walk.
    fn at(&self, which: CircuitRef) -> usize {
        self.0
            .iter()
            .position(|(_, o)| *o == RoomObject::Circuit(which))
            .expect("in the list")
    }
}

/// The sound lines the circuit and the player play.
fn circuit_sounds() -> Sounds {
    let text = "Volume: 6\nVolume exponent: 1.8\n\
                Coral_Circuit_Break : CoralBreakA/vol=0.3\n\
                Coral_Circuit_Jump_Explosion : CoralSwimA/vol=0.4\n\
                Player_Coral_Circuit_Swim : CoralSwimA/vol=0.3\n";
    let names = ClipNames::new(["CoralBreakA".to_owned(), "CoralSwimA".to_owned()], None);
    Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()))
}

/// The player's chunks put at `at`, one chunk's velocity set.
fn put_player(g: &mut RainWorldGame, at: Vec2, vel: Vec2) {
    let p = g.player.as_mut().unwrap();
    for c in &mut p.body.chunks {
        c.pos = at;
        c.last_pos = at;
        c.vel = vel;
    }
}

/// The circuit's turn pushes the player's chunks off the bits near them and tells the
/// player of each: the circuit swim resistance is the largest overlap, the jump meter
/// counts the bits touched (each chunk's), and a bit pushed ahead of a moving chunk takes
/// its velocity.
#[test]
fn the_circuit_pushes_the_player_and_tells_it() {
    let mut g = coral_game(PLACED, &[], 1);
    let bit_at = {
        let c = &g
            .world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .circuits[0];
        c.bits[c.bits.len() / 2].stuck_pos
    };
    put_player(&mut g, bit_at - Vec2::new(4.0, 0.0), Vec2::new(2.0, 0.0));
    g.run_circuit_object(0, CircuitRef::Circuit(0));
    let p = g.player.as_ref().unwrap();
    assert!(p.circuit_swim_resistance() > 0.0 && p.circuit_swim_resistance() <= 1.0);
    assert!(p.circuit_jump_meter() >= 1.0, "{}", p.circuit_jump_meter());
    assert!(p.body.chunks[0].pos.x < bit_at.x - 4.0);
    let c = &g
        .world
        .realized(0)
        .unwrap()
        .coral
        .as_ref()
        .unwrap()
        .circuits[0];
    let moved = c.bits.iter().filter(|b| b.vel.x > 0.0).count();
    assert!(moved >= 1);
    // The next tick's chance follows the displaced count the bits' turns made (none yet).
    assert_eq!(c.disconnect_chance, 0.0);
}

/// The player's graphics module present: the first bit its main chunk touches becomes a
/// `swimBits` entry on one side, with the `Player_Coral_Circuit_Swim` sound; later bits
/// replace it only when nearer.
#[test]
fn the_player_remembers_the_bits_it_swims_through() {
    let mut g = coral_game_with(PLACED, &[], 1, circuit_sounds());
    g.player_graphics = true;
    let bit_at = {
        let c = &g
            .world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .circuits[0];
        c.bits[c.bits.len() / 2].stuck_pos
    };
    put_player(&mut g, bit_at - Vec2::new(4.0, 0.0), Vec2::new(2.0, 0.0));
    g.sounds.take_events();
    g.run_circuit_object(0, CircuitRef::Circuit(0));
    let p = g.player.as_ref().unwrap();
    let held: Vec<_> = p.swim_bits().into_iter().flatten().collect();
    assert!(!held.is_empty());
    for s in &held {
        assert_eq!((s.bit.room, s.bit.circuit), (0, 0));
        let bit = g.world.circuit_bit(s.bit).unwrap();
        assert_eq!(bit.pos, s.pos);
    }
    let swims = g
        .sounds
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, SoundEvent::Play(p) if p.id == SoundId::PlayerCoralCircuitSwim))
        .count();
    assert!(swims >= held.len() && !held.is_empty());
}

/// The circuit's turn is not gated by the system's `Frozen`.
#[test]
fn the_circuit_runs_while_the_system_is_frozen() {
    let mut g = coral_game(PLACED, &[], 1);
    {
        let s = g.world.realized_mut(0).unwrap().coral.as_mut().unwrap();
        s.age = 400;
        s.not_viewed_counter = 100;
        assert!(s.frozen());
    }
    let bit_at = {
        let c = &g
            .world
            .realized(0)
            .unwrap()
            .coral
            .as_ref()
            .unwrap()
            .circuits[0];
        c.bits[c.bits.len() / 2].stuck_pos
    };
    put_player(&mut g, bit_at - Vec2::new(4.0, 0.0), Vec2::new(2.0, 0.0));
    g.run_circuit_object(0, CircuitRef::Circuit(0));
    let c = &g
        .world
        .realized(0)
        .unwrap()
        .coral
        .as_ref()
        .unwrap()
        .circuits[0];
    assert!(c.bits.iter().any(|b| b.vel.x > 0.0));
    // A bit's turn runs too.
    let before = c.bits[0].clone();
    g.run_circuit_object(0, CircuitRef::Bit(0, 0));
    let after = &g
        .world
        .realized(0)
        .unwrap()
        .coral
        .as_ref()
        .unwrap()
        .circuits[0]
        .bits[0];
    assert_eq!(after.last_pos, before.pos);
}

/// A dead player-sized chunk beyond the places' reach plus 100 px is not touched.
#[test]
fn a_body_far_from_the_places_is_left_alone() {
    let mut g = coral_game(PLACED, &[], 1);
    put_player(&mut g, Vec2::new(40.0, 150.0), Vec2::new(2.0, 0.0));
    g.run_circuit_object(0, CircuitRef::Circuit(0));
    let p = g.player.as_ref().unwrap();
    assert_eq!(p.circuit_jump_meter(), 0.0);
    assert_eq!(p.circuit_swim_resistance(), 0.0);
    assert!(
        g.world
            .circuit_bit(BitRef {
                room: 0,
                circuit: 0,
                bit: 0
            })
            .is_some()
    );
}

/// A room that is abstractized takes its circuits' strands with it (the connectors' owners
/// are let go with the system's registry).
#[test]
fn abstractizing_the_room_frees_the_connectors_strands() {
    let mut g = coral_game(PLACED, &[], 1);
    assert!(g.world.mycelia().live() > 0);
    g.world.abstractize(0, &mut rw_random::UnityRandom::new(1));
    assert_eq!(g.world.mycelia().live(), 0);
}
