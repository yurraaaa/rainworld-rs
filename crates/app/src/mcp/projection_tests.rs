//! `get_state`'s `projectors`, `scan_lines` and `star_matrices` on rooms of the installed
//! game (skipped when it is absent): `ss_f02` has two `ProjectedStars` objects and
//! `ProjectedScanLines` 0.24 (`ss_f02_settings.txt`), `ss_e07` a `SuperStructureProjector`
//! of amount 1 and `ss_e06` `ProjectedScanLines` 0.71.

use super::Session;
use super::params::{InputParam, StepParams};

impl Session {
    fn step_projection_ticks(&mut self, ticks: u32) {
        self.step(&StepParams {
            ticks: Some(ticks),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    }
}

/// The star matrices are made with the room, the scan lines when the camera comes to it
/// (the head of the first of the loading ticks `load` steps).
#[test]
fn star_matrices_and_scan_lines_are_reported() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_F02", 1);
    let state = session.state().expect("state");
    assert_eq!(state.star_matrices.len(), 2);
    assert!(state.projectors.is_empty());
    session.step_projection_ticks(2);
    let state = session.state().expect("state");
    let [lines] = &state.scan_lines[..] else {
        panic!("{:?}", state.scan_lines);
    };
    assert_eq!(lines.room, "SS_F02");
    // Lerp(W * H, max(W, H), 0.5) / 2000 * 10 * 0.2391304, in tiles.
    assert!(lines.ideal > 0);
    assert_eq!(lines.lines.len(), lines.ideal);
    let stars = &state.star_matrices;
    assert!(stars.iter().all(|s| s.stars > 20 && s.sprites > s.stars));
}

#[test]
fn a_projector_is_reported_with_its_glyphs() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SS_E07", 1);
    session.step_projection_ticks(2);
    let state = session.state().expect("state");
    let [p] = &state.projectors[..] else {
        panic!("{:?}", state.projectors);
    };
    assert_eq!((p.room.as_str(), p.amount), ("SS_E07", 1.0));
    assert_eq!(p.ideal_glyphs, 520);
    assert!(p.glyphs >= 260 && p.single_glyphs + p.glyph_matrices == p.glyphs);
    assert!(p.visible && p.parts >= p.glyphs);
    // One cursor is added a tick up to the ideal 5 (three loading ticks, then two).
    assert_eq!((p.ideal_cursors, p.cursors.len()), (5, 5));
}
