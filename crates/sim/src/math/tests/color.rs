
use super::*;

#[test]
fn hsl_to_rgb_matches_the_sextants() {
    assert_eq!(Hsl::new(0.0, 1.0, 0.5).rgb(), [1.0, 0.0, 0.0]);
    assert_eq!(Hsl::new(1.0 / 3.0, 1.0, 0.5).rgb()[1], 1.0);
    assert_eq!(Hsl::new(0.5, 0.0, 0.25).rgb(), [0.25, 0.25, 0.25]);
}

#[test]
fn a_variation_makes_two_draws() {
    let mut rng = UnityRandom::new(5);
    let v = rng.wrapped_random_variation(0.32, 0.1, 0.6);
    let mut m = UnityRandom::new(5);
    m.value();
    m.value();
    assert_eq!(rng.state(), m.state());
    assert!((0.22..=0.42).contains(&v));
}
