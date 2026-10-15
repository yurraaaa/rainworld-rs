//! The pause menu in the game process (`RainWorldGame.Update`'s paused branch), on the
//! installed game.

use rw_menu::ProcessId;

use super::Session;
use super::params::LoadProcessParams;

impl Session {
    /// A game at SU_C04 with the fade from black over.
    fn game_at_su_c04() -> Option<Self> {
        let mut s = Self::installed()?;
        s.load_process(&LoadProcessParams {
            process: Some(super::params::ProcessParam::Game),
            room: Some("SU_C04".to_owned()),
            seed: Some(1),
        })
        .expect("game loads");
        s.settle_on(ProcessId::Game);
        Some(s)
    }

    /// Taps a key (one frame down, two up).
    pub(super) fn tap(&mut self, key: &str) -> super::reply::ProcessState {
        self.frames(1, &[key]);
        self.frames(2, &[])
    }
}

#[test]
fn paused_game_freezes_and_continue_resumes() {
    let Some(mut s) = Session::game_at_su_c04() else {
        return;
    };
    let before = s.frames(1, &[]).game_tick.expect("game");
    // The tick the menu opens still runs its rooms.
    let opened = s.frames(1, &["Escape"]);
    assert_eq!(opened.game_tick, Some(before + 1));
    let menu = opened.pause_menu.expect("paused");
    assert_eq!(
        menu.buttons,
        ["CONTINUE at (1186.13, 15.33)", "EXIT at (1046.13, 15.33)"]
    );
    // Fades in by 1/16 per tick; the clock stands still; Exit is greyed for 40 ticks.
    let s16 = s.frames(16, &[]);
    let menu = s16.pause_menu.expect("paused");
    assert_eq!((menu.counter, menu.black_fade), (16, 1.0));
    assert!(menu.buttons[1].ends_with("greyed"));
    assert_eq!(s16.game_tick, Some(before + 1));
    let s40 = s.frames(40, &[]);
    assert!(!s40.pause_menu.expect("paused").buttons[1].ends_with("greyed"));
    assert_eq!(s.hud_state().expect("hud").text_prompt.text, "Paused");
    // Escape again: out by 1/8 per tick, then the game runs on the same tick.
    let pressed = s.tap("Escape");
    let menu = pressed.pause_menu.expect("still fading");
    assert!(menu.want_to_continue);
    let resumed = s.frames(8, &[]);
    assert!(resumed.pause_menu.is_none());
    assert!(resumed.game_tick.expect("game") > before + 1);
}

#[test]
fn exit_goes_to_the_main_menu_frozen() {
    let Some(mut s) = Session::game_at_su_c04() else {
        return;
    };
    s.tap("Escape");
    s.frames(45, &[]);
    let left = s.tap("LeftArrow");
    assert_eq!(
        left.pause_menu.expect("paused").selected.as_deref(),
        Some("EXIT")
    );
    // No cycle has passed, so no confirmation: the switch starts and the menu shuts down,
    // the game staying paused through the fade.
    let exit = s.tap("Return");
    assert_eq!(exit.upcoming.as_deref(), Some("MainMenu"));
    let menu = exit.pause_menu.expect("kept until the switch");
    assert!(menu.shut_down);
    let tick = exit.game_tick;
    assert_eq!(s.frames(5, &[]).game_tick, tick);
    s.settle_on(ProcessId::MainMenu);
}
