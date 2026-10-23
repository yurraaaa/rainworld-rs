//! `RoomCamera.MoveCamera` to another room is applied by the camera's next update
//! (`applyPosChangeWhenTextureIsLoaded`), where `ChangeRoom` makes the shortcut graphics
//! anew; the installed game's `SU_A07` and a realized neighbour (skipped without it).

use crate::mcp::Session;
use crate::mcp::params::{InputParam, LoadRoomParams, StepParams, TeleportPlayerParams};

fn one_tick(s: &mut Session) -> crate::mcp::reply::State {
    s.step(&StepParams {
        ticks: Some(1),
        input: Some(InputParam::default()),
    })
    .expect("steps")
}

#[test]
fn the_room_change_applies_one_update_after_the_move() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.load_room(&LoadRoomParams {
        name: "SU_A07".to_owned(),
        seed: Some(1),
        spawn_tile: None,
        slugcat: None,
    })
    .expect("room loads");
    let before = s
        .step(&StepParams {
            ticks: Some(40),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    assert!(before.camera_move.is_none());
    let target = before
        .realized_rooms
        .iter()
        .map(|r| r.room.clone())
        .find(|r| *r != before.room)
        .expect("a realized neighbour");
    s.teleport_player(&TeleportPlayerParams {
        room: target.clone(),
        node: None,
    })
    .expect("teleports");
    for _ in 0..80 {
        let state = one_tick(&mut s);
        let Some(mv) = state.camera_move else {
            continue;
        };
        // The tick the vessel left the lobby: the sim's camera room changed, the camera
        // (and its shortcut graphics) still show the old room.
        assert_eq!(state.room, target);
        assert_eq!(
            mv.room,
            s.scene_mut().expect("scene").game().sim().camera_room()
        );
        let old = state.shortcut_graphics.expect("graphics").room;
        assert_ne!(old, mv.room);
        let next = one_tick(&mut s);
        assert!(next.camera_move.is_none());
        let graphics = next.shortcut_graphics.expect("graphics");
        assert_eq!(graphics.room, mv.room);
        return;
    }
    panic!("the camera never moved");
}
