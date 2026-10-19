//! The debug UI's actions on a running game process, as the window applies them
//! between frames, on the installed game (skipped without it).

use rw_assets::{AbstractObjectType, CreatureType};
use rw_menu::ProcessId;

use super::Session;
use super::params::{LoadProcessParams, ProcessParam};
use crate::debug_ui::{DebugAction, DebugView, GameView};

impl Session {
    fn game_process_at(room: &str) -> Option<Self> {
        let mut s = Self::installed()?;
        s.load_process(&LoadProcessParams {
            process: Some(ProcessParam::Game),
            room: Some(room.to_owned()),
            seed: Some(1),
        })
        .expect("game loads");
        s.settle_on(ProcessId::Game);
        Some(s)
    }

    fn apply(&mut self, action: DebugAction) -> Result<String, String> {
        action.apply(self.frontend_mut().expect("process mode"))
    }

    fn debug_view(&mut self) -> GameView {
        DebugView::of(self.frontend_mut().expect("process mode"))
            .game
            .expect("a game")
    }
}

#[test]
fn spawns_and_moves_apply_to_the_running_game() {
    let Some(mut s) = Session::game_process_at("SU_C04") else {
        return;
    };
    let view = s.debug_view();
    assert_eq!(
        (view.region.as_str(), view.camera_room.as_str()),
        ("SU", "SU_C04")
    );
    let tile = view.player_tile.expect("player in the camera's room");
    let reply = s
        .apply(DebugAction::SpawnCreature {
            kind: CreatureType::Fly,
            room: "SU_C04".to_owned(),
            tile,
        })
        .expect("spawns");
    assert!(
        reply.starts_with("Fly ") && reply.contains("realized"),
        "{reply}"
    );
    let view = s.debug_view();
    let fly = view.creatures.last().expect("the fly");
    assert_eq!(
        (fly.kind, fly.room.as_str(), fly.tile),
        (CreatureType::Fly, "SU_C04", tile)
    );
    assert!(fly.realized && fly.alive && !fly.in_den);
    let reply = s
        .apply(DebugAction::SpawnObject {
            kind: AbstractObjectType::Spear,
            room: "SU_C04".to_owned(),
            tile,
            explosive: false,
        })
        .expect("spawns");
    assert!(
        reply.starts_with("Spear ") && reply.contains("realized"),
        "{reply}"
    );
    let to = (tile.0 + 1, tile.1);
    s.apply(DebugAction::MovePlayer(to)).expect("moves");
    assert_eq!(s.debug_view().player_tile, Some(to));
    assert!(
        s.apply(DebugAction::SpawnCreature {
            kind: CreatureType::Fly,
            room: "NOWHERE".to_owned(),
            tile,
        })
        .is_err()
    );
    assert!(s.apply(DebugAction::MovePlayer((-5, -5))).is_err());
}

#[test]
fn load_room_restarts_the_world_in_that_room() {
    let Some(mut s) = Session::game_process_at("SU_C04") else {
        return;
    };
    s.apply(DebugAction::LoadRoom("SU_A07".to_owned()))
        .expect("loads");
    assert_eq!(s.frames(1, &[]).upcoming.as_deref(), Some("Game"));
    for _ in 0..400 {
        let state = s.frames(1, &[]);
        if state.upcoming.is_none() && state.fade_to_black == 0.0 {
            break;
        }
    }
    let view = s.debug_view();
    assert_eq!(
        (view.region.as_str(), view.camera_room.as_str()),
        ("SU", "SU_A07")
    );
    assert!(
        s.apply(DebugAction::LoadRoom("NO_ROOM".to_owned()))
            .is_err()
    );
}
