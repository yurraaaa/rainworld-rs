use super::*;

fn id(n: i32) -> EntityId {
    EntityId::new(-1, n)
}

/// One heavy AI alone runs every tick (nothing waits, so the list clears at each
/// tick's end); asked twice in one tick, the second is turned away without counting
/// as delayed.
#[test]
fn a_lone_heavy_ai_runs_every_tick() {
    let mut h = HeavyAiThrottle::default();
    let mut runs = Vec::new();
    for _ in 0..3 {
        runs.push(h.request(id(1)));
        runs.push(h.request(id(1)));
        assert_eq!(h.delayed_except_last_run, 0);
        h.end_of_tick();
    }
    assert_eq!(runs, [true, false, true, false, true, false]);
}

/// Three a tick; the fourth is delayed, which keeps the three listed (and skipped)
/// next tick, when the fourth runs and nothing waits, so the list clears.
#[test]
fn three_a_tick_and_the_delayed_keep_the_list() {
    let mut h = HeavyAiThrottle::default();
    let first: Vec<bool> = (1..=4).map(|n| h.request(id(n))).collect();
    assert_eq!(first, [true, true, true, false]);
    assert_eq!(h.delayed_except_last_run, 1);
    h.end_of_tick();
    assert_eq!(h.last_run, [id(1), id(2), id(3)]);
    let second: Vec<bool> = (1..=4).map(|n| h.request(id(n))).collect();
    assert_eq!(second, [false, false, false, true]);
    h.end_of_tick();
    assert!(h.last_run.is_empty());
    let third: Vec<bool> = (1..=4).map(|n| h.request(id(n))).collect();
    assert_eq!(third, [true, true, true, false]);
}
