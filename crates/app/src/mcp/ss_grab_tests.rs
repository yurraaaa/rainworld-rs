//! `get_state`'s `deep_processing`, `deep_processing_lights` and `gravity_disruptors` on rooms
//! of the installed game (skipped when it is absent): `ss_a03` has one `GravityDisruptor`
//! and one `DeepProcessing` (`ss_a03_settings.txt`), `ss_e01` five quads and two lights.
//!
//! Mirrors: - (port-only: tests)

use super::Session;
use super::params::{InputParam, StepParams};

/// A disruptor with its twenty specks and the quad of its room, at full power.
#[test]
fn a_disruptor_is_reported_with_its_specks() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_A03", 1);
    session
        .step(&StepParams {
            ticks: Some(3),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    let state = session.state().expect("state");
    let [d] = &state.gravity_disruptors[..] else {
        panic!("{:?}", state.gravity_disruptors);
    };
    assert_eq!(d.room, "SS_A03");
    assert_eq!((d.specks.len(), d.streaks.len(), d.power), (20, 16, 1.0));
    assert_eq!(d.pos, [472.1744, 370.1522]);
    let [q] = &state.deep_processing[..] else {
        panic!("{:?}", state.deep_processing);
    };
    assert_eq!(
        (q.from_depth, q.to_depth, q.intensity),
        (0.2826087, 1.0, 0.5)
    );
    assert_eq!(q.power, 1.0);
}

/// Five quads and two lights in `ss_e01`, no disruptor; a tick in, each light's rad is
/// `Lerp(80, 800, seed / 100)` within the flicker's 10% (full power) and its alpha 0.5.
#[test]
fn quads_and_lights_are_reported() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_E01", 1);
    session
        .step(&StepParams {
            ticks: Some(2),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    let state = session.state().expect("state");
    let quads = state
        .deep_processing
        .iter()
        .filter(|q| q.room == "SS_E01")
        .count();
    assert_eq!(quads, 5);
    let lights: Vec<_> = state
        .deep_processing_lights
        .iter()
        .filter(|l| l.room == "SS_E01")
        .collect();
    assert_eq!(lights.len(), 2);
    assert!(state.gravity_disruptors.is_empty());
    for l in lights {
        let base = 80.0 + 720.0 * l.random_seed as f32 / 100.0;
        assert!(
            l.rad >= base * 0.9 && l.rad <= base * 1.1,
            "{} of {base}",
            l.rad
        );
        assert_eq!(l.alpha, 0.5);
    }
}
