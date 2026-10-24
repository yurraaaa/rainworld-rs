//! `SleepAndDeathScreen` on `KarmaLadderScreen` with
//! the installed game's fonts, strings and scene images (skipped without it), from a
//! constructed data package.

use std::path::PathBuf;

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_hud::{LadderPhase, SleepAndDeathScreen, SleepDeathScreenDataPackage};
use rw_menu::menu::{Kind, MenuCx};
use rw_menu::{KeyboardKey, MenuShared, MenuSoundId, ProcessId, ProcessRequests};
use rw_save::{PlayerSessionRecord, SaveState, WatchedScreen};
use rw_sim::{Input, MapDiscovery, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Harness {
    shared: MenuShared,
    screen: SleepAndDeathScreen,
    /// The switch the screen asked for.
    switched: Option<ProcessId>,
}

fn shared() -> Option<MenuShared> {
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
    shared.pre_switch(ProcessId::SleepScreen);
    Some(shared)
}

/// The Survivor's package: karma (`karma`, 5), 7 food pips, 4 to hibernate.
fn package(food: i32, karma: i32, reinforced: bool) -> SleepDeathScreenDataPackage {
    SleepDeathScreenDataPackage {
        food,
        karma: (karma, 5),
        karma_reinforced: reinforced,
        player_room: -1,
        player_pos: Vec2::ZERO,
        map_data: None,
        slugcat: SlugcatName::White,
        max_food: 7,
        food_to_hibernate: 4,
        session_record: PlayerSessionRecord::default(),
        start_malnourished: false,
        goal_malnourished: false,
        discovery: MapDiscovery::default(),
    }
}

impl Harness {
    fn open(id: ProcessId, package: SleepDeathScreenDataPackage) -> Option<Self> {
        let mut shared = shared()?;
        let mut save = SaveState::new(SlugcatName::White, 0);
        save.death_persistent.karma = package.karma.0;
        save.death_persistent.karma_cap = package.karma.1;
        shared.saves.current_save_state = Some(save);
        let screen = SleepAndDeathScreen::new(&mut shared, id, package);
        Some(Self {
            shared,
            screen,
            switched: None,
        })
    }

    fn tick_with(&mut self, keys: &[KeyboardKey], input: Input) {
        for &k in keys {
            self.shared.input.set_key(k, true);
        }
        let mut requests = ProcessRequests::new(self.switched, false, 0.0);
        let mut cx = MenuCx {
            shared: &mut self.shared,
            requests: &mut requests,
        };
        self.screen.update(&mut cx, input);
        self.switched = requests.upcoming();
        for &k in keys {
            self.shared.input.set_key(k, false);
        }
    }

    fn tick(&mut self) {
        self.tick_with(&[], Input::NONE);
    }

    /// Ticks until the ladder has moved and come to rest; the ticks it took.
    fn until_rest(&mut self) -> usize {
        for n in 1..2000 {
            self.tick();
            let l = self.screen.ladder();
            if l.started_animating && l.all_animation_done() && l.phase == LadderPhase::Resting {
                return n;
            }
        }
        panic!("the ladder never rested");
    }

    fn greyed(&self) -> [bool; 2] {
        let b = self.screen.buttons();
        [b[0], b[1]].map(|b| self.screen.menu().behavior(b).expect("button").greyed_out)
    }
}

#[test]
fn buttons_bottom_right_and_the_info_label_at_the_top() {
    let Some(h) = Harness::open(ProcessId::SleepScreen, package(1, 3, false)) else {
        return;
    };
    let menu = h.screen.menu();
    // `ContinueAndExitButtonsXPos` is 1366 at 1366 wide: CONTINUE 180 left of it, EXIT
    // 320, both at `Max(SafeScreenOffset.y, 15)`.
    let [cont, exit] = h.screen.buttons()[..] else {
        panic!("no PASSAGE without a save state");
    };
    assert_eq!(menu.object(cont).pos, Vec2::new(1186.0, 15.0));
    assert_eq!(menu.object(exit).pos, Vec2::new(1046.0, 15.0));
    assert_eq!(menu.button(cont).unwrap().signal_text, "CONTINUE");
    assert_eq!(menu.button(exit).unwrap().signal_text, "EXIT");
    assert_eq!(menu.my_sound_loop, Some(MenuSoundId::SleepScreenLoop));
    // The sleep screen's ladder starts a notch below the saved karma, at (350, 450).
    let ladder = h.screen.ladder();
    assert_eq!(ladder.display_karma, (2, 5));
    assert_eq!(ladder.pos, Vec2::new(350.0, 450.0));
    assert_eq!(ladder.symbols.len(), 6);
}

#[test]
fn first_sleep_screens_wait_for_the_ladder() {
    let Some(mut h) = Harness::open(ProcessId::SleepScreen, package(1, 3, false)) else {
        return;
    };
    // `watchedSleepScreens` 0 < 2: counted, and the buttons wait.
    assert_eq!(h.shared.saves.misc.watched(WatchedScreen::Sleep), 1);
    assert!(h.screen.force_watch_animation());
    h.tick();
    assert_eq!(h.greyed(), [true, true]);
    // The food meter counts the four hibernation pips down, then the ladder climbs.
    let ticks = h.until_rest();
    assert_eq!(h.screen.ladder().display_karma, (3, 5));
    assert!(ticks > 266, "{ticks}");
    h.tick();
    assert_eq!(h.greyed(), [false, false]);
    // CONTINUE (selected first): the next cycle, loaded.
    h.tick_with(&[KeyboardKey::Return], Input::NONE);
    h.tick();
    h.tick();
    assert_eq!(h.switched, Some(ProcessId::Game));
    assert_eq!(
        h.shared.start_game_condition,
        Some(rw_menu::StoryGameInitCondition::Load)
    );
}

#[test]
fn a_watched_screen_lets_the_buttons_be() {
    let Some(mut shared) = shared() else {
        return;
    };
    shared.saves.misc.set_watched(WatchedScreen::Death, 2);
    let mut h = Harness {
        screen: SleepAndDeathScreen::new(&mut shared, ProcessId::DeathScreen, package(2, 3, false)),
        shared,
        switched: None,
    };
    assert_eq!(h.shared.saves.misc.watched(WatchedScreen::Death), 3);
    assert!(!h.screen.force_watch_animation());
    h.tick();
    assert_eq!(h.greyed(), [false, false]);
    // EXIT: the main menu.
    h.tick_with(&[KeyboardKey::LeftArrow], Input::NONE);
    h.tick();
    h.tick_with(&[KeyboardKey::Return], Input::NONE);
    h.tick();
    h.tick();
    assert_eq!(h.switched, Some(ProcessId::MainMenu));
}

#[test]
fn validation_skips_the_forced_watch() {
    let Some(mut shared) = shared() else {
        return;
    };
    shared.options.validation = true;
    let screen =
        SleepAndDeathScreen::new(&mut shared, ProcessId::SleepScreen, package(1, 3, false));
    assert_eq!(shared.saves.misc.watched(WatchedScreen::Sleep), 1);
    assert!(!screen.force_watch_animation());
}

#[test]
fn death_drops_a_notch_after_the_meter() {
    let Some(mut h) = Harness::open(ProcessId::DeathScreen, package(2, 3, false)) else {
        return;
    };
    assert_eq!(
        h.screen.menu().my_sound_loop,
        Some(MenuSoundId::DeathScreenLoop)
    );
    assert_eq!(h.screen.ladder().display_karma, (3, 5));
    // `DeathUpdate`: `eatCircleDelay` 80 runs out, then `GoToKarma(karma - 1)`.
    for _ in 0..80 {
        h.tick();
    }
    assert!(!h.screen.ladder().started_animating);
    h.tick();
    assert!(h.screen.ladder().started_animating);
    assert_eq!(h.screen.ladder().move_to_karma, 2);
    h.until_rest();
    assert_eq!(h.screen.ladder().display_karma, (2, 5));
}

#[test]
fn reinforced_death_shows_the_flower_and_counts_it() {
    let Some(mut h) = Harness::open(ProcessId::DeathScreen, package(0, 3, true)) else {
        return;
    };
    let misc = &h.shared.saves.misc;
    assert_eq!(misc.watched(WatchedScreen::DeathWithFlower), 1);
    let scene = h.screen.scene();
    for _ in 0..81 {
        h.tick();
    }
    // `FoodCountDownDone` starts the scene's timer (the flower's fade-in).
    let rw_menu::menu::Kind::Scene(s) = &h.screen.menu().object(scene).kind else {
        panic!("the death scene");
    };
    assert_eq!(s.timer, 0);
    h.until_rest();
    let l = h.screen.ladder();
    assert_eq!(l.display_karma, (3, 5));
    assert!(!l.reinforced);
}

#[test]
fn a_malnourished_sleep_warns_and_is_counted() {
    let mut p = package(0, 3, false);
    p.goal_malnourished = true;
    let Some(mut h) = Harness::open(ProcessId::SleepScreen, p) else {
        return;
    };
    let label = h.screen.starved_label().expect("the starving label");
    assert_eq!(
        h.screen.menu().label_text(label),
        Some("You are starving. Your game has not been saved.")
    );
    assert_eq!(h.shared.saves.misc.watched(WatchedScreen::Malnourish), 1);
    // `eatCircles = maxFood`, the survival limit at the shown count.
    assert_eq!(h.screen.hud().food_meter.eat_circles, 7);
    h.tick();
}

#[test]
fn the_map_button_freezes_and_shows_the_map_once_moved() {
    let Some(mut h) = Harness::open(ProcessId::DeathScreen, package(0, 3, false)) else {
        return;
    };
    let map = Input {
        map: true,
        ..Input::NONE
    };
    // Before the ladder has shown its movement the map stays hidden, but the buttons
    // grey out while it is held, and the frame rate rises over the unfinished ladder.
    h.tick_with(&[], map);
    assert!(!h.screen.reveal_map());
    assert_eq!(h.greyed(), [true, true]);
    assert_eq!(h.screen.frames_per_second(), 200);
    h.until_rest();
    assert!(h.screen.map_toggle());
    h.tick_with(&[], map);
    assert!(h.screen.reveal_map());
    assert_eq!(h.screen.frames_per_second(), 40);
    // `fadeOutIllustration`: `LerpAndTick(0, 1, 0.02, 0.025)`.
    assert!((h.screen.fade_out_illustration() - 0.045).abs() < 1e-6);
    // In depth (no `flatmode.txt`) the death scene's layers 0, 2 and 3 dim with it
    // (`SleepAndDeathScreen.Update`): `Lerp(1, 0.1 | 0.25 | 0.5, fadeOutIllustration)`, applied next tick.
    let menu = h.screen.menu();
    let Kind::Scene(scene) = &menu.object(h.screen.scene()).kind else {
        panic!("not a scene");
    };
    let set: Vec<Option<f32>> = scene
        .depth_illustrations
        .iter()
        .take(4)
        .map(|&l| menu.illustration(l).expect("a layer").set_alpha)
        .collect();
    let f = 0.045;
    assert_eq!(set[0], Some(1.0 + (0.1 - 1.0) * f));
    assert_eq!(set[1], None);
    assert_eq!(set[2], Some(1.0 + (0.25 - 1.0) * f));
    assert_eq!(set[3], Some(1.0 + (0.5 - 1.0) * f));
}
