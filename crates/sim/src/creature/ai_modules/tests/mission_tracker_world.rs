//! `LeaveRoom.Act` on the three-room abstract test world of `creature/abstract_ai/tests/world.rs`
//! (A: exit 0 to B, den 1; B: exit 0 to A, exit 1 to C, den 2; C: exit 0 to B, den 1;
//! the green's costs there). No room is realized, so the exit is the node it is at or
//! the first connection it can go to and come back from. Expected values from
//! `MissionTracker.cs` (`LeaveRoom.Act`) and `AbstractRoom.cs` (`ExitIndex`,
//! `ConnectionAndBackPossible`, `ConnectivityCost`).

use rw_random::UnityRandom;

use crate::creature::abstract_ai::world::tests::{green_out, world};
use crate::creature::{LeaveRoom, WorldCoordinate};

fn at(room: i32, node: i32) -> WorldCoordinate {
    WorldCoordinate::new(room, -1, -1, node)
}

/// At B's exit to A: through it into A at its exit 0 (`A.ExitIndex(B)`), whose only
/// other node, the den (cost 10, times 10 for a non-exit), is the destination. No
/// draws (the node is defined), no destination set yet, not abandoned.
#[test]
fn from_an_exit_it_heads_for_the_next_rooms_den() {
    let mut w = world();
    let l = green_out(&mut w, at(1, 0));
    let mut m = LeaveRoom::default();
    let mut rng = UnityRandom::new(7);
    let before = rng.clone();
    assert!(!w.leave_room_act(l, &mut m, &mut rng));
    assert_eq!(rng, before);
    assert_eq!(m.destination, Some(at(0, 1)));
    assert_eq!(m.dest_unreachable, 0);
}

/// In B's den (not an exit), the first connection there and back from it is exit 0
/// (den-to-0 cost 12): into A, its den again.
#[test]
fn from_a_den_it_takes_the_first_connection_back_and_forth() {
    let mut w = world();
    let l = green_out(&mut w, at(1, 2));
    let mut m = LeaveRoom::default();
    let mut rng = UnityRandom::new(7);
    assert!(!w.leave_room_act(l, &mut m, &mut rng));
    assert_eq!(m.destination, Some(at(0, 1)));
}

/// From C's exit into B at its exit 1 (`B.ExitIndex(C)`): of B's other nodes the exit
/// to A (cost 30 x 1) beats the den (20 x 10), so it goes on through it, to A's
/// entrance from B (node 0).
#[test]
fn an_exit_beyond_leads_on_to_the_room_after() {
    let mut w = world();
    let l = green_out(&mut w, at(2, 0));
    let mut m = LeaveRoom::default();
    let mut rng = UnityRandom::new(7);
    assert!(!w.leave_room_act(l, &mut m, &mut rng));
    assert_eq!(m.destination, Some(at(0, 0)));
}

/// With a destination: the abstract AI is sent there, and it abandons once its
/// position has the destination's room and node. No path finder: never unreachable.
#[test]
fn it_walks_to_its_destination_and_abandons_there() {
    let mut w = world();
    let l = green_out(&mut w, at(1, 0));
    let mut m = LeaveRoom {
        destination: Some(at(0, 1)),
        dest_unreachable: 0,
    };
    let mut rng = UnityRandom::new(7);
    assert!(!w.leave_room_act(l, &mut m, &mut rng));
    let dest = w.creature(l).ai.as_ref().map(|a| a.destination);
    assert_eq!(dest, Some(at(0, 1)));
    w.creature_mut(l).pos = WorldCoordinate::new(0, 5, 5, 1);
    assert!(w.leave_room_act(l, &mut m, &mut rng));
    assert_eq!(m.dest_unreachable, 0);
}

/// A position without a node is given one first (`DefineNodeOfLocalCoordinate`, one
/// draw in a room not ready for AI: `Random.Range(0, 3)` over B's three relevant
/// nodes) and the exit chosen from that node.
#[test]
fn an_undefined_node_is_defined_with_one_draw() {
    let mut w = world();
    let l = green_out(&mut w, WorldCoordinate::new(1, 5, 5, -1));
    let mut m = LeaveRoom::default();
    let mut rng = UnityRandom::new(7);
    let mut probe = rng.clone();
    let node = probe.range_i32(0, 3);
    assert!(!w.leave_room_act(l, &mut m, &mut rng));
    assert_eq!(rng, probe);
    // Nodes 0 and 2 lead into A (its den); node 1 into C, whose den is beyond its exit.
    let expect = if node == 1 { at(2, 1) } else { at(0, 1) };
    assert_eq!(m.destination, Some(expect));
}
