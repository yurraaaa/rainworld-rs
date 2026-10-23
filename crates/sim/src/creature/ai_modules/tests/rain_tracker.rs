use super::*;

#[test]
fn story_rain_utility() {
    assert_eq!(RainTracker::utility(false, true, 5000), 0.0);
    assert_eq!(RainTracker::utility(false, true, 800), 1.0);
    assert_eq!(RainTracker::utility(true, true, 0), 0.0);
    // Halfway the S-curve is at its middle.
    assert!((RainTracker::utility(false, true, 2400) - 0.5).abs() < 1e-6);
    assert_eq!(RainTracker::utility(false, false, 80), 1.0);
}
