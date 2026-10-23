//! `get_state`'s `shortcut_helper` on rooms of the installed game (skipped when it is
//! absent). `su_c04.txt`: two shortcuts, the creature den at (5, 28) opening right
//! (a wrong hole) and the pipe.

use super::Session;
use super::params::{InputParam, SetPlayerParams, StepParams};

#[test]
fn a_den_is_a_wrong_hole_that_shoves_the_player_out_of_its_mouth() {
    let Some(mut session) = Session::installed() else {
        return;
    };
    session.load("SU_C04", 1);
    let state = session.state().expect("state");
    let [h] = &state.shortcut_helper[..] else {
        panic!("{:?}", state.shortcut_helper);
    };
    assert_eq!(h.room, "SU_C04");
    assert_eq!(h.pushers.len(), 2);
    let den = h.pushers.iter().find(|p| p.tile == [5, 28]).expect("den");
    assert!(den.wrong_hole);
    assert_eq!(den.dir, [1, 0]);
    assert_eq!(den.push_pos, [130.0, 570.0]);
    assert!(h.pushers.iter().filter(|p| !p.wrong_hole).count() == 1);
    // On the den's push position the head is moved to `pushPos.x + 10 + rad` (19).
    session
        .set_player(&SetPlayerParams {
            tile: Some([6, 28]),
            ..SetPlayerParams::default()
        })
        .expect("moves");
    session
        .step(&StepParams {
            ticks: Some(1),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    let state = session.state().expect("state");
    let head = state.player.as_ref().expect("player").chunks[0].pos;
    assert!(head[0] > 145.0, "{head:?}");
}
