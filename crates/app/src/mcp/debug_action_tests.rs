//! The debug UI's actions on a running game process, as the window applies them
//! between frames, on the installed game (skipped without it).

use rw_assets::{AbstractObjectType, CreatureType};
use rw_menu::ProcessId;

use super::Session;
use super::params::{LoadProcessParams, ProcessParam};
use crate::debug_ui::{DebugAction, DebugView, GameView};
use crate::dev_spawn::RainTimerEdit;

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

/// "Go to its room" for a creature in another room: the room is loaded, the
/// player arrives there with the camera, and the same world (that creature, the game
/// clock) goes on.
#[test]
fn teleport_goes_to_a_creatures_room_in_the_same_world() {
    let Some(mut s) = Session::game_process_at("SU_C04") else {
        return;
    };
    let view = s.debug_view();
    let creature = view
        .creatures
        .iter()
        .find(|c| c.room != view.camera_room && view.rooms.contains(&c.room))
        .expect("a creature in another room")
        .clone();
    let tick = s.frames(1, &[]).game_tick.expect("game");
    let reply = s
        .apply(DebugAction::Teleport(creature.room.clone()))
        .expect("teleports");
    assert!(
        reply.starts_with(&format!("teleporting to {}", creature.room)),
        "{reply}"
    );
    let mut arrived = false;
    for _ in 0..120 {
        s.frames(1, &[]);
        if s.debug_view().camera_room == creature.room {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "never reached {}", creature.room);
    assert!(s.frames(1, &[]).game_tick.expect("game") > tick);
    let view = s.debug_view();
    assert!(view.player_tile.is_some() || view.camera_room == creature.room);
    assert!(view.creatures.iter().any(|c| c.id == creature.id));
    assert!(
        s.apply(DebugAction::Teleport("NO_ROOM".to_owned()))
            .is_err()
    );
}

#[test]
fn rain_timer_actions_edit_the_timer_in_the_running_game() {
    let Some(mut s) = Session::game_process_at("SU_A07") else {
        return;
    };
    let length = s.debug_view().rain.cycle_length;
    let reply = s
        .apply(DebugAction::RainTimer(RainTimerEdit::until_rain(
            length,
            30 * 40,
        )))
        .expect("sets");
    assert!(reply.contains("1200 until rain"), "{reply}");
    assert_eq!(s.debug_view().rain.time_until_rain, 1200);
    s.apply(DebugAction::RainTimer(RainTimerEdit {
        paused: Some(true),
        ..RainTimerEdit::default()
    }))
    .expect("holds");
    let held = s.debug_view().rain.timer;
    s.frames(10, &[]);
    let rain = s.debug_view().rain;
    assert_eq!((rain.timer, rain.dev_paused), (held, true));
    s.apply(DebugAction::RainTimer(RainTimerEdit {
        restart: true,
        ..RainTimerEdit::default()
    }))
    .expect("restarts");
    assert_eq!(s.debug_view().rain.timer, 0);
}
