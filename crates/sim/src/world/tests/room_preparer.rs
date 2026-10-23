//! `RoomPreparer.Update`'s hand-offs under a schedule: at most one stage a call, the
//! thread started by the first call, `done` the call after the thread ends.
//!
//! Mirrors: - (port-only: tests)

use super::*;

/// The call numbers (from 1) at which `calls` calls hand over each stage.
fn stage_calls(schedule: LoadingSchedule, calls: u32) -> Vec<(u32, PreparerStage)> {
    let mut p = RoomPreparer::new(3);
    let mut at = Vec::new();
    for call in 1..=calls {
        at.extend(p.update(schedule).into_iter().map(|s| (call, s)));
    }
    at
}

#[test]
fn the_game_minimum_hands_stage_one_on_call_two_and_stage_two_on_call_three() {
    let mut p = RoomPreparer::new(3);
    let s = LoadingSchedule::GAME_MINIMUM;
    assert!(p.update(s).is_empty());
    assert_eq!(p.status(), PreparerStatus::MappingShortcuts);
    assert_eq!(p.update(s), vec![PreparerStage::ShortcutsReady]);
    assert_eq!(p.status(), PreparerStatus::MappingAi);
    assert_eq!(p.update(s), vec![PreparerStage::ReadyForAi]);
    assert_eq!((p.status().value(), p.done()), (5, false));
    assert!(p.update(s).is_empty());
    assert!(p.done());
    assert_eq!(p.calls(), 4);
}

#[test]
fn a_longer_schedule_waits_its_calls() {
    let s = LoadingSchedule::Deferred {
        shortcuts_calls: 2,
        ai_calls: 3,
    };
    assert_eq!(
        stage_calls(s, 8),
        vec![
            (3, PreparerStage::ShortcutsReady),
            (6, PreparerStage::ReadyForAi)
        ]
    );
}

#[test]
fn immediate_hands_both_stages_on_the_first_call() {
    let mut p = RoomPreparer::new(3);
    assert_eq!(
        p.update(LoadingSchedule::Immediate),
        vec![PreparerStage::ShortcutsReady, PreparerStage::ReadyForAi]
    );
    assert!(p.done());
}
