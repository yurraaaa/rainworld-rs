
use super::*;

/// `LeaveRoom(0.4, 1000)`: utility 0.4 from the first `Update`; abandoned at the
/// 1000th (the counter goes 999 .. 0, abandoned below 1) and removed that update.
#[test]
fn a_mission_is_forgotten_after_its_counter() {
    let mut t = MissionTracker::default();
    t.add(Mission::leave_room(0.4, 1000, false), false);
    assert_eq!(t.utility(), 0.0);
    t.update();
    assert_eq!(t.utility(), 0.4);
    assert_eq!(
        t.most_relevant().map(Mission::kind),
        Some(MissionKind::LeaveRoom)
    );
    for _ in 1..999 {
        t.update();
    }
    assert_eq!(t.missions[0].forget_counter, 1);
    assert_eq!(t.utility(), 0.4);
    t.update();
    assert!(t.missions.is_empty());
    assert_eq!(t.utility(), 0.0);
    assert!(t.most_relevant().is_none());
}

/// In a single-room world the mission is abandoned as it is made: added, then gone
/// at the next `Update` without ever being the most relevant.
#[test]
fn single_room_world_abandons_at_once() {
    let mut t = MissionTracker::default();
    t.add(Mission::leave_room(0.4, 1000, true), false);
    assert_eq!(t.missions.len(), 1);
    t.update();
    assert!(t.missions.is_empty());
    assert_eq!(t.utility(), 0.0);
}

/// A second `LeaveRoom` replaces the first (its utility 0.4 is not above the
/// tracker's 0.4); the old one, the most relevant, is still the one acted on until
/// the next `Update`. A mission that may coexist is simply added.
#[test]
fn a_new_mission_of_a_kind_replaces_the_old() {
    let mut t = MissionTracker::default();
    t.add(Mission::leave_room(0.4, 1000, false), false);
    t.update();
    let first = t.most_relevant;
    t.add(Mission::leave_room(0.4, 1000, false), false);
    assert_eq!(t.missions.len(), 1);
    assert_ne!(Some(t.missions[0].serial), first);
    assert_eq!(t.most_relevant().map(|m| m.serial), first);
    assert!(t.most_relevant().unwrap().delete_me_next_frame);
    t.update();
    assert_eq!(t.most_relevant, Some(t.missions[0].serial));
    assert!(t.detached.is_none());
    t.add(Mission::leave_room(0.4, 1000, false), true);
    assert_eq!(t.missions.len(), 2);
}

/// Before any `Update` the tracker's utility is 0, so an existing mission of the
/// kind (0.4 > 0) refuses the new one.
#[test]
fn a_mission_above_the_utility_refuses_a_new_one() {
    let mut t = MissionTracker::default();
    t.add(Mission::leave_room(0.4, 1000, false), false);
    t.add(Mission::leave_room(0.4, 1000, false), false);
    assert_eq!(t.missions.len(), 1);
    assert_eq!(t.missions[0].serial, 1);
}
