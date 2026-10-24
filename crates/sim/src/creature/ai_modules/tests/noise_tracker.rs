use super::*;

/// `NoiseTracker.TheorizedSource.Update`: a tick older, the strength `Max(10, 0.6x)`, destroyed
/// once older than the forget time, and the representation let go when deleted.
#[test]
fn source_update() {
    let mut t = NoiseTracker::default();
    let i = t.add_source(Vec2::new(10.0, 20.0), Some(RepKey(4)));
    let s = &mut t.sources[i];
    s.last_sound_strength = 150.0;
    s.update(3, false);
    assert_eq!((s.age, s.last_sound_strength), (1, 90.0));
    assert_eq!(s.creature_rep, Some(RepKey(4)));
    s.update(3, true);
    s.update(3, false);
    assert!(!s.slated_for_deletion);
    assert_eq!(s.creature_rep, None);
    s.update(3, false);
    assert!(s.slated_for_deletion);
    assert!((s.last_sound_strength - 19.44).abs() < 1e-3);
    s.update(3, false);
    s.update(3, false);
    assert_eq!(s.last_sound_strength, 10.0);
    assert_eq!(
        s.unknown_match(Vec2::new(10.0, 50.0)),
        30.0 + 200.0 + 6.0 / 5.0
    );
}

/// `Utility` (`NoiseTracker`) and the fade at the end of `Update`.
#[test]
fn utility_and_fade() {
    let mut t = NoiseTracker {
        mysterious_noises: 5.0,
        ..NoiseTracker::default()
    };
    assert_eq!(t.utility(), 0.0);
    t.sound_to_examine = Some(0);
    assert!((t.utility() - (4.0f32 / 19.0).sqrt()).abs() < 1e-6);
    t.add_mysterious(0.0);
    assert_eq!(t.mysterious_noise_counter, 200);
    t.mysterious_noises = 0.01;
    t.mysterious_noise_counter = 1;
    t.fade_mysterious_noises();
    assert_eq!((t.mysterious_noise_counter, t.mysterious_noises), (0, 0.01));
    t.fade_mysterious_noises();
    assert!((t.mysterious_noises - (0.01 - 0.0125)).abs() < 1e-7);
    t.fade_mysterious_noises();
    assert!((t.mysterious_noises - (0.01 - 0.0125)).abs() < 1e-7);
}

/// `HowInterestingIsThisNoiseToMe` (`NoiseTracker`).
#[test]
fn interest() {
    assert_eq!(NoiseKind::Kin.interest(4.0), 0.4);
    assert_eq!(NoiseKind::Rock.interest(4.0), 20.0);
    assert_eq!(NoiseKind::Other.interest(4.0), 4.0);
}
