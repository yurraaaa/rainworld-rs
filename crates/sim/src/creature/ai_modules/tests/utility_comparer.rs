use super::*;

/// `UtilityComparer.Update`: the threat smoother rises halfway, falls by 0.0025 a
/// tick; prey lerps by 0.15 and weighs 0.6; the highest is strictly the largest,
/// the first on a tie.
#[test]
fn lizard_comparer_smooths_and_picks_the_highest() {
    let mut c = UtilityComparer::lizard(CreatureType::GreenLizard);
    let mut raw = RawUtilities::default();
    c.update(&raw);
    assert_eq!(c.highest_module(), Some(UtilityModule::ThreatTracker));
    assert_eq!(c.highest_utility(&raw), 0.0);
    raw.set(UtilityModule::ThreatTracker, 1.0);
    raw.set(UtilityModule::PreyTracker, 1.0);
    c.update(&raw);
    assert_eq!(
        c.tracker(UtilityModule::ThreatTracker)
            .unwrap()
            .smoothed_utility,
        0.5
    );
    let prey = c
        .tracker(UtilityModule::PreyTracker)
        .unwrap()
        .smoothed_utility;
    assert!((prey - 0.6 * 0.15).abs() < 1e-6);
    raw.set(UtilityModule::ThreatTracker, 0.0);
    c.update(&raw);
    let threat = c
        .tracker(UtilityModule::ThreatTracker)
        .unwrap()
        .smoothed_utility;
    assert!((threat - 0.4975).abs() < 1e-6);
    assert!(
        (c.smoothed_non_weighted(UtilityModule::PreyTracker, &raw)
            - (0.09 + (0.6 - 0.09) * 0.15) / 0.6)
            .abs()
            < 1e-6
    );
    // Unsmoothed trackers read the utility now.
    raw.set(UtilityModule::RainTracker, 1.0);
    assert!((c.smoothed_non_weighted(UtilityModule::RainTracker, &raw) - 1.0).abs() < 1e-6);
    c.update(&raw);
    assert_eq!(c.highest_module(), Some(UtilityModule::RainTracker));
    assert!((c.highest_utility(&raw) - 0.9).abs() < 1e-6);
}

#[test]
fn tick_tween_stops_at_the_target() {
    let t = FloatTweenBasic {
        kind: TweenType::Tick,
        speed: 0.3,
    };
    assert_eq!(t.tween(0.0, 0.2), 0.2);
    assert_eq!(t.tween(1.0, 0.0), 0.7);
}
