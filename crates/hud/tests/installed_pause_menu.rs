//! `PauseMenu` on the installed game's fonts and strings (skipped without it), with
//! expectations from `Menu.PauseMenu` (Assembly-CSharp 528349..528745).

use std::path::PathBuf;

use rw_assets::{StreamingAssets, UnityResources};
use rw_hud::{
    Hud, HudSetup, PauseButton, PauseGame, PauseMenu, PauseRequest, RainCycleView, StartCondition,
};
use rw_menu::menu::MenuCx;
use rw_menu::{KeyboardKey, MenuShared, MenuSoundId, ProcessRequests};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Harness {
    shared: MenuShared,
    menu: PauseMenu,
    hud: Hud,
}

impl Harness {
    /// A pause menu opened in a story game (no menu microphone: the game runs).
    fn open(game: PauseGame) -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let data = root.join("RainWorld_Data");
        if !data.join("resources.assets").is_file() {
            eprintln!("skipping: no game under {}", root.display());
            return None;
        }
        let assets = StreamingAssets::new(data.join("StreamingAssets"));
        let resources = UnityResources::open(&data).expect("resources");
        let mut shared = MenuShared::load(assets, &resources, [1366.0, 768.0]).expect("fonts");
        shared.pre_switch(rw_menu::ProcessId::Game);
        let setup = HudSetup {
            screen_size: [1366.0, 768.0],
            max_food: 7,
            food_to_hibernate: 4,
            current_food: 0,
            karma: 0,
            karma_cap: 4,
            karma_reinforced: false,
            cycle_number: game.cycle_number,
            camera_room_shelter: false,
            rain: RainCycleView::STAND_IN,
            subregions: None,
            start: StartCondition::New,
            swarm_rooms: 0,
        };
        let mut hud = Hud::new(&setup, &shared.atlases);
        let menu = PauseMenu::new(&mut shared, &game, Some(&mut hud));
        Some(Self { shared, menu, hud })
    }

    fn tick(&mut self, keys: &[KeyboardKey]) -> Vec<PauseRequest> {
        for &k in keys {
            self.shared.input.set_key(k, true);
        }
        let mut requests = ProcessRequests::new(None, false, 0.0);
        let mut cx = MenuCx {
            shared: &mut self.shared,
            requests: &mut requests,
        };
        let asked = self.menu.update(&mut cx);
        for &k in keys {
            self.shared.input.set_key(k, false);
        }
        asked
    }

    /// One tick down, two up (a button clicks on release, a tick late).
    fn tap(&mut self, key: KeyboardKey) -> Vec<PauseRequest> {
        let mut asked = self.tick(&[key]);
        asked.extend(self.tick(&[]));
        asked.extend(self.tick(&[]));
        asked
    }

    fn signals(&self) -> Vec<String> {
        let menu = self.menu.menu();
        self.menu
            .buttons()
            .into_iter()
            .filter_map(|b| menu.button(b).map(|d| d.signal_text.clone()))
            .collect()
    }

    fn sounds(&mut self) -> Vec<MenuSoundId> {
        self.shared
            .game_mic_sounds
            .drain(..)
            .map(|(id, _)| id)
            .collect()
    }
}

const FIRST_CYCLE: PauseGame = PauseGame {
    story: true,
    cycle_number: 0,
    clock: 5000,
    red_is_out_of_cycles: false,
};

#[test]
fn buttons_at_the_bottom_right_continue_selected() {
    let Some(mut h) = Harness::open(FIRST_CYCLE) else {
        return;
    };
    // `ContinueAndExitButtonsXPos` = 1366 at 1366 wide: CONTINUE at x - 180.2, EXIT
    // 140 left of it, both at y 15, on the page at (1/3, 1/3).
    let menu = h.menu.menu();
    let pos: Vec<[f32; 2]> = h
        .menu
        .buttons()
        .into_iter()
        .map(|b| [menu.object(b).pos.x, menu.object(b).pos.y])
        .collect();
    assert_eq!(pos, [[1185.8, 15.0], [1045.8, 15.0]]);
    assert_eq!(h.signals(), ["CONTINUE", "EXIT"]);
    assert_eq!(menu.selected, None);
    // The constructor plays HUD_Pause_Game through the game's microphone.
    assert_eq!(h.sounds(), [MenuSoundId::HudPauseGame]);
    // Cycle 0: the plain pause text.
    assert!(h.hud.text_prompt.paused_mode && !h.hud.text_prompt.paused_warning_text);
    h.tick(&[]);
    let menu = h.menu.menu();
    assert_eq!(menu.selected, Some(h.menu.buttons()[0]));
}

#[test]
fn fades_in_and_out_and_ignores_escape_for_ten_ticks() {
    let Some(mut h) = Harness::open(FIRST_CYCLE) else {
        return;
    };
    h.sounds();
    // Escape held from the first tick does nothing while `counter <= 10`.
    for _ in 0..10 {
        h.tick(&[KeyboardKey::Escape]);
    }
    assert!(!h.menu.want_to_continue());
    for _ in 0..6 {
        h.tick(&[]);
    }
    assert_eq!((h.menu.counter, h.menu.black_fade), (16, 1.0));
    assert_eq!(h.menu.mic_volume(), 0.5);
    assert!((h.menu.mic_time_speed(1.0) - 0.7).abs() < 1e-6);
    assert!(h.tick(&[KeyboardKey::Escape]).is_empty());
    assert!(h.menu.want_to_continue());
    assert_eq!(h.menu.black_fade, 0.875);
    assert_eq!(h.sounds(), [MenuSoundId::HudUnpauseGame]);
    let mut asked = Vec::new();
    for _ in 0..7 {
        asked.extend(h.tick(&[]));
    }
    assert_eq!(asked, [PauseRequest::ContinuePaused]);
}

#[test]
fn exit_greyed_for_forty_ticks() {
    let Some(mut h) = Harness::open(FIRST_CYCLE) else {
        return;
    };
    let exit = h.menu.buttons()[1];
    for _ in 0..39 {
        h.tick(&[]);
    }
    assert!(h.menu.menu().behavior(exit).expect("button").greyed_out);
    h.tick(&[]);
    assert!(!h.menu.menu().behavior(exit).expect("button").greyed_out);
}

#[test]
fn exit_in_the_first_cycle_leaves_at_once() {
    let Some(mut h) = Harness::open(FIRST_CYCLE) else {
        return;
    };
    for _ in 0..40 {
        h.tick(&[]);
    }
    h.tap(KeyboardKey::LeftArrow);
    h.sounds();
    assert_eq!(h.tap(KeyboardKey::Return), [PauseRequest::ExitToMenu]);
    assert_eq!(
        h.sounds(),
        [MenuSoundId::ButtonPressInit, MenuSoundId::HudExitGame]
    );
}

#[test]
fn exit_late_in_a_later_cycle_asks_first() {
    // `clock > 1200` after cycle 0: quitting counts as a loss.
    let Some(mut h) = Harness::open(PauseGame {
        cycle_number: 3,
        clock: 1201,
        ..FIRST_CYCLE
    }) else {
        return;
    };
    assert!(h.menu.pause_warning_active());
    let prompt = &h.hud.text_prompt;
    assert!(prompt.paused_mode && prompt.paused_warning_text);
    assert_eq!(h.shared.saves.misc.warned_about_karma_loss_on_exit(), 1);
    for _ in 0..40 {
        h.tick(&[]);
    }
    h.tap(KeyboardKey::LeftArrow);
    assert!(h.tap(KeyboardKey::Return).is_empty());
    assert_eq!(
        h.signals(),
        [PauseButton::YesExit.signal(), PauseButton::NoExit.signal()]
    );
    let menu = h.menu.menu();
    let no = h.menu.buttons()[1];
    assert_eq!(menu.selected, Some(no));
    // The question sits left of NO: its left edge 40 px plus its width before NO's x.
    let label = h.menu.confirm_message().expect("confirmation");
    let width = menu.label_text_width(label);
    assert_eq!(menu.object(label).pos.x, 1045.8 - width - 40.0);
    // NO puts CONTINUE and EXIT back.
    assert!(h.tap(KeyboardKey::Return).is_empty());
    assert_eq!(h.signals(), ["CONTINUE", "EXIT"]);
    assert!(h.menu.confirm_message().is_none());
}
