
use super::*;

/// `LizardInjuryTracker.Utility`: `SCurve(InverseLerp(0.2, 0.9, 1 - health), 0.01)`,
/// 0 down to 80 % health, 1 at 10 %.
#[test]
fn injury_utility() {
    assert_eq!(LizardAi::injury_utility(1.0), 0.0);
    assert_eq!(LizardAi::injury_utility(0.8), 0.0);
    assert_eq!(LizardAi::injury_utility(0.1), 1.0);
    // 60 %: InverseLerp gives 2/7, the steep S-curve (k 0.01) keeps it near 0.
    let x = (0.4f32 - 0.2) / 0.7 * 2.0;
    let expect = 0.01 * x / (0.01 - x + 1.0) * 0.5;
    assert!((LizardAi::injury_utility(0.6) - expect).abs() < 1e-6);
}

#[test]
fn behavior_names_round_trip() {
    for b in LizardBehavior::ALL {
        assert_eq!(LizardBehavior::from_value(b.value()), Some(b));
    }
    for c in LizardCommunication::ALL {
        assert_eq!(LizardCommunication::from_value(c.value()), Some(c));
    }
}
