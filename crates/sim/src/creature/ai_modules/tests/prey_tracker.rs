use super::*;

/// `PreyTracker.Utility`'s distance part for the lizard (3, 70): 1 tile off,
/// `Lerp(1 - 0, 19.75 / 1, 0.5)` (`Mathf.Lerp` clamps only t).
#[test]
fn distance_term() {
    let p = PreyTracker::lizard(false);
    let q = 3.0f32.lerp(70.0, 0.25);
    assert!((p.distance_term(1.0) - (1.0 + (q - 1.0) * 0.5)).abs() < 1e-5);
    let d = 70.0;
    assert!((p.distance_term(d) - (0.0 + (q / d) * 0.5)).abs() < 1e-6);
}

#[test]
fn frustration_steps() {
    let mut p = PreyTracker::lizard(false);
    p.step_frustration(true);
    assert!((p.frustration - 0.0125).abs() < 1e-7);
    p.step_frustration(false);
    assert_eq!(p.frustration, 0.0);
    assert_eq!(p.ghost_generation_fac(25), 1.0);
    assert_eq!(p.ghost_generation_fac(50), 0.0);
}
