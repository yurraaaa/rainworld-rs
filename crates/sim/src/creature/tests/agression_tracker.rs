
use super::*;

#[test]
fn anger_steps_toward_its_target() {
    let a = AgressionTracker::new(0.001, 0.001);
    assert!((a.step_anger(0.1, 0.5) - 0.101).abs() < 1e-7);
    assert_eq!(a.step_anger(0.5, 0.5), 0.5);
    assert_eq!(a.step_anger(0.1, 0.1005), 0.1005);
    assert!((a.step_anger(0.5, 0.0) - 0.499).abs() < 1e-7);
    let mut a = a;
    a.highest_anger = 0.35;
    assert_eq!(a.utility(), 0.0);
}

/// `IncrementAnger` and `SetAnger` clamp to 0..1; a base of -1 keeps the base; a
/// representation not tracked changes nothing.
#[test]
fn anger_increments_and_sets_clamped() {
    let k = RepKey(3);
    let mut a = AgressionTracker::new(0.001, 0.001);
    a.add(k, 0.95, 0.8);
    a.increment_anger(Some(k), 0.1);
    assert_eq!(a.creatures[0].anger, 1.0);
    a.increment_anger(Some(RepKey(4)), -1.0);
    a.increment_anger(None, -1.0);
    assert_eq!(a.creatures[0].anger, 1.0);
    a.set_anger(Some(k), 0.0, -1.0);
    assert_eq!(
        (a.creatures[0].anger, a.creatures[0].base_anger),
        (0.0, 0.8)
    );
    a.set_anger(Some(k), -2.0, 0.0);
    assert_eq!(
        (a.creatures[0].anger, a.creatures[0].base_anger),
        (0.0, 0.0)
    );
    a.set_anger(Some(k), 3.0, 2.0);
    assert_eq!(
        (a.creatures[0].anger, a.creatures[0].base_anger),
        (1.0, 1.0)
    );
}
