
use super::*;

/// `Utility`: `Lerp(-0.01, 1.01, threat)` clamped; `CurrentThreat` maps
/// `chance^0.75 * severity` from 0.05..0.5.
#[test]
fn utility_and_current_threat() {
    let mut t = ThreatTracker::new(3);
    assert_eq!(t.utility(), 0.0);
    t.current_threat = 0.5;
    assert!((t.utility() - 0.5).abs() < 1e-6);
    let c = ThreatCreature {
        rep: RepKey(0),
        severity: 0.5,
        point: 0,
    };
    assert_eq!(c.current_threat(1.0, false), 1.0);
    assert_eq!(c.current_threat(1.0, true), 0.0);
    assert!(
        (c.current_threat(0.5f32.mathf_pow(1.0 / 0.75), false) - (0.25f32 - 0.05) / 0.45).abs()
            < 1e-5
    );
}
