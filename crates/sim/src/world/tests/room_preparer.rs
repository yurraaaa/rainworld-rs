//! `RoomPreparer.Update`'s hand-offs from its thread (`RoomPreparer.cs`): the thread
//! started by the first call, at most one stage a call in the game's order, each taken
//! once, `done` the call after the thread ends; the thread's maps equal the synchronous
//! build of the same room.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::ShortcutMap;

use super::*;
use crate::tests::room;

/// A room with an entrance at (1, 2) whose tube runs down to a room exit at (1, 0).
const TUBE: [&str; 6] = [
    "#########",
    "#.......#",
    "#.......#",
    "#E......#",
    "#x#######",
    "#X#######",
];

fn job() -> PreparerJob {
    let mut unmapped = room(&TUBE);
    unmapped.unmap_shortcuts();
    PreparerJob {
        room: Arc::new(unmapped),
        index: 3,
        shelter_or_gate: false,
        lethal_water: false,
    }
}

/// Which stages one `update` handed over, by number.
fn numbers(stages: &[PreparerStage]) -> Vec<u8> {
    stages
        .iter()
        .map(|s| match s {
            PreparerStage::ShortcutsReady(_) => 1,
            PreparerStage::ReadyForAi(_) => 2,
        })
        .collect()
}

#[test]
fn the_thread_maps_what_the_synchronous_build_maps() {
    let job = job();
    assert!(!job.room.shortcuts_mapped());
    let sync_room = room(&TUBE);
    let sync = AiMap::new(&sync_room, 3, false, false);
    let mut p = RoomPreparer::new(3, 0, job);
    let mut maps = (None, None);
    while !p.done() {
        p.await_thread();
        for stage in p.update() {
            match stage {
                PreparerStage::ShortcutsReady(m) => maps.0 = Some(m),
                PreparerStage::ReadyForAi(m) => maps.1 = Some(*m),
            }
        }
    }
    let shortcuts = maps.0.expect("stage 1");
    assert_eq!(&shortcuts, sync_room.shortcut_map());
    assert_eq!(shortcuts, ShortcutMap::new(&room(&TUBE)));
    assert!(!shortcuts.shortcuts().is_empty());
    assert_eq!(maps.1.expect("stage 2"), sync);
}

#[test]
fn each_stage_comes_once_in_order_when_the_thread_requests_it() {
    let mut p = RoomPreparer::new(3, 0, job());
    assert!(!p.thread_started());
    // The first call starts the thread under the lock: nothing to take yet.
    assert!(p.update().is_empty());
    assert!(p.thread_started());
    p.await_thread();
    assert_eq!(p.requested_stage(), Some(1));
    assert_eq!(p.status(), PreparerStatus::MappingShortcuts);
    assert_eq!(numbers(&p.update()), [1]);
    assert_eq!(p.status(), PreparerStatus::MappingAi);
    p.await_thread();
    assert_eq!(p.requested_stage(), Some(2));
    assert_eq!(p.status(), PreparerStatus::AiReady);
    assert_eq!(numbers(&p.update()), [2]);
    assert!(p.handed_over_ready_for_ai());
    assert!(!p.done());
    p.await_thread();
    assert!(p.thread_finished());
    assert!(p.update().is_empty());
    assert!(p.done());
    assert!(p.update().is_empty());
    assert_eq!(p.calls(), 5);
}

#[test]
fn a_preparer_dropped_while_its_thread_waits_releases_it() {
    let mut p = RoomPreparer::new(3, 0, job());
    p.update();
    p.await_thread();
    drop(p);
    let mut q = RoomPreparer::new(3, 0, job());
    q.update();
    q.await_thread();
    q.update();
    q.await_thread();
    drop(q);
}
