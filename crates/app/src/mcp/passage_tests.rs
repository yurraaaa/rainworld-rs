//! The end-game meters on a sleep screen after a continued cycle, on a constructed save
//! in temporary directories: `CycleCompleted`'s tracker changes, the meters, a passage
//! fulfilled on the screen, its token and the disabled PASSAGE, the meters shown, and
//! `Revert` before the next cycle. Skipped without the installed game.

use std::path::Path;

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_menu::ProcessId;
use rw_save::user_data::UserDataFile;
use rw_save::win_state::{EndgameId, Progress};
use rw_save::{CampaignStart, PlayerProgression};

use super::Session;
use super::params::{ImportSaveParams, LoadProcessParams, ProcessParam, ScreenshotParams};
use super::reply::KarmaScreenState;
use crate::GameDir;
use crate::saves::SaveConfig;

/// Fed in SU_S01; the Survivor fulfilled and unused, the Saint two cycles from it, the
/// Survivor's meter shown before.
pub(super) const SAVE: &str = "SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA>SEED<svB>4321<svA>\
    DENPOS<svB>SU_S01<svA>LASTVDENPOS<svB>SU_S01<svA>CYCLENUM<svB>5<svA>FOOD<svB>4<svA>\
    NEXTID<svB>2500<svA>DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>2<dpA>\
    KARMACAP<dpB>5<dpA>GHOSTS<dpB><dpA>WINSTATE<dpB>Survivor<egA>0<egA>5<wsA>\
    Saint<egA>0<egA>10<wsA>Hunter<egA>0<egA>6<wsA>\
    Traveller<egA>0<egA>0.1.0.0.0.0.0.0.0.0.0.0.<wsA>Chieftain<egA>0<egA>0.3<wsA><dpA>\
    TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB>Survivor<dpA>DEATHS<dpB>1<dpA>SURVIVES<dpB>4<dpA>\
    QUITS<dpB>0<dpA><svA><progDivA>MISCPROG<progDivB>CURRENTSLUGCAT<mpdB>White<mpdA>\
    INTEGERS<mpdB>0,4,4,0,0,0,0,0,0,0,0<mpdA><progDivA>";

pub(super) const WHITE: CampaignStart<'static> = CampaignStart {
    slugcat: SlugcatName::White,
    den: "SU_C04",
    vanilla: true,
};

/// A session continuing [`SAVE`], with the port's save directory.
fn continued() -> Option<(Session, StreamingAssets, tempfile::TempDir)> {
    continued_with(SAVE)
}

/// A session continuing `save`.
pub(super) fn continued_with(save: &str) -> Option<(Session, StreamingAssets, tempfile::TempDir)> {
    let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
    if !assets.file("levels").is_dir() {
        eprintln!("skipping: no game under {}", assets.path().display());
        return None;
    }
    let Some(salt) = SaveConfig::salt(&assets) else {
        eprintln!("skipping: no checksum salt");
        return None;
    };
    let game = tempfile::tempdir().unwrap();
    let port = tempfile::tempdir().unwrap();
    let mut file = UserDataFile::new();
    file.set("save", salt.md5_sum(save) + save);
    std::fs::write(game.path().join("sav"), file.to_bytes()).unwrap();
    let resources = UnityResources::open(assets.data_dir()).expect("Unity resources");
    let config = SaveConfig {
        dir: Some(port.path().to_owned()),
        game_saves: Some(game.path().to_owned()),
    };
    let mut s = Session::new(assets.clone(), std::sync::Arc::new(resources), config);
    s.import_save(&ImportSaveParams::default())
        .expect("imports");
    s.load_process(&LoadProcessParams {
        process: Some(ProcessParam::SlugcatSelect),
        ..LoadProcessParams::default()
    })
    .expect("process loads");
    s.settle_on(ProcessId::SlugcatSelect);
    for _ in 0..80 {
        if s.frames(1, &["Z"]).upcoming.is_some() {
            break;
        }
    }
    s.settle_on(ProcessId::Game);
    Some((s, assets, port))
}

impl Session {
    /// Through the pipe from the player's room to `to`, holding the arrow into it.
    fn pipe_to(&mut self, to: &str) {
        let game = self.frontend_mut().unwrap().game_mut().expect("the game");
        let sim = game.scene.game().sim();
        let world = sim.world();
        let src = sim.player_pos().room;
        let dst = world.room_index(to).expect("the room");
        let node = world.abstract_room(src).unwrap().exit_index(dst);
        let room = &world.realized(src).unwrap().room;
        let entrance = room
            .shortcut_map()
            .leading_to_node(usize::try_from(node).unwrap())
            .unwrap()
            .start;
        let key = match room.shortcut_hole_direction(entrance.0, entrance.1) {
            (1, _) => "LeftArrow",
            (-1, _) => "RightArrow",
            (_, 1) => "DownArrow",
            _ => "UpArrow",
        };
        let at = rw_sim::Vec2::from(rw_assets::tile_centre(entrance.0, entrance.1));
        game.scene.game_mut().set_player(at, rw_sim::Vec2::ZERO);
        for _ in 0..400 {
            self.frames(1, &[key]);
            let sim = self
                .frontend_mut()
                .unwrap()
                .game()
                .unwrap()
                .scene
                .game()
                .sim();
            if sim.player_pos().room == dst && !sim.player().unwrap().in_shortcut() {
                return;
            }
        }
        panic!("never reached {to}");
    }

    /// Out of the start shelter and back (only then does a fed slugcat sleep), onto the
    /// spawn position, then still until the sleep screen starts.
    pub(super) fn sleep(&mut self) -> KarmaScreenState {
        self.pipe_to("SU_A22");
        self.pipe_to("SU_S01");
        let game = self.frontend_mut().unwrap().game_mut().unwrap();
        let sim = game.scene.game().sim();
        let shelter = sim.player_pos().room;
        let spawn = sim.shelter_door(shelter).unwrap().player_spawn_pos;
        let at = rw_sim::Vec2::from(rw_assets::tile_centre(spawn.0, spawn.1));
        game.scene.game_mut().set_player(at, rw_sim::Vec2::ZERO);
        for _ in 0..3000 {
            if let Some(screen) = self.frames(1, &[]).karma_screen {
                return screen;
            }
        }
        panic!("never slept");
    }

    pub(super) fn screen(&mut self) -> KarmaScreenState {
        self.frames(1, &[]).karma_screen.expect("the sleep screen")
    }

    pub(super) fn process_shot(&mut self, dir: &Path, name: &str) {
        let params = ScreenshotParams {
            save_path: Some(
                dir.join(format!("{name}.png"))
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..ScreenshotParams::default()
        };
        self.screenshot(&params).expect("screenshot");
    }
}

#[test]
fn a_passage_fulfilled_on_the_sleep_screen() {
    let Some((mut s, assets, port)) = continued() else {
        return;
    };
    let screen = s.sleep();
    // `CycleCompleted` with the Survivor fulfilled: Saint +2 (peaceful) to 12, the
    // Hunter kept (nothing eaten), SU visited, the Chieftain set to the scavengers' like
    // (0); a Monk tracker added at 0 shows nothing.
    let meters: Vec<(&str, &str, bool)> = screen
        .meters
        .iter()
        .map(|m| (m.passage.as_str(), m.kind.as_str(), m.fulfilled_now))
        .collect();
    assert_eq!(
        meters,
        [
            ("Survivor", "notch", false),
            ("Saint", "float", true),
            ("Hunter", "float", false),
            ("Traveller", "notch", false),
            ("Chieftain", "float", false),
        ]
    );
    // The Saint's meter was never shown: the watch is forced, and it is saved shown.
    assert!(screen.force_watch_animation);
    let sav = port.path().join("sav");
    let mut p = PlayerProgression::load(&sav, SaveConfig::salt(&assets)).expect("loads");
    let state = p.peek_save_state(WHITE).expect("reads").expect("a save");
    assert_eq!(
        state.death_persistent.meters_ever_shown(),
        ["Survivor", "Saint"]
    );
    // The unused Survivor's token and PASSAGE at once, greyed out for good; the Saint's
    // token waits for its meter.
    let passages: Vec<&str> = screen.tokens.iter().map(|t| t.passage.as_str()).collect();
    assert_eq!(passages, ["Survivor", "Saint"]);
    assert!(screen.buttons[2].starts_with("PASSAGE at (70.33, 15.33) greyed"));

    let mut done = None;
    for _ in 0..1500 {
        let state = s.screen();
        if state.animation_done && !state.buttons[0].ends_with("greyed") {
            done = Some(state);
            break;
        }
    }
    let done = done.expect("the buttons came on");
    assert!(done.animation_done);
    assert!(done.meters.iter().all(|m| m.powered_on));
    assert_eq!(done.meters[1].show_as_fulfilled, 1.0);
    assert!(done.tokens.iter().all(|t| t.fade > 0.9));
    assert!(done.buttons[2].ends_with("greyed"));

    // CONTINUE reverts the progression: the next cycle's state is the stored one, its
    // trackers shown as saved.
    s.tap("Return");
    s.settle_on(ProcessId::Game);
    let f = s.frontend_mut().unwrap();
    let state = f
        .ctx
        .menu
        .saves
        .current_save_state
        .as_ref()
        .expect("loaded");
    let saint = state
        .death_persistent
        .win_state
        .trackers
        .iter()
        .find(|t| t.id == EndgameId::Saint)
        .expect("the Saint");
    assert_eq!(
        saint.progress,
        Progress::Integer {
            progress: 12,
            last_shown: 12
        }
    );
    assert!(state.loaded_next_issued_id.is_some());
}

#[test]
#[ignore = "writes screenshots; run with PASSAGE_SHOTS=<dir>"]
fn passage_screenshots() {
    let Some(dir) = std::env::var_os("PASSAGE_SHOTS") else {
        return;
    };
    let dir = Path::new(&dir);
    let Some((mut s, _assets, _port)) = continued() else {
        return;
    };
    s.sleep();
    s.frames(20, &[]);
    s.process_shot(dir, "1_screen");
    let mut taken = 1;
    for frame in 0..2000 {
        let state = s.screen();
        let m = &state.meters;
        let due = match taken {
            1 => m[0].energy > 0.5,
            2 => m[1].meter_animation > 0.5,
            3 => m[1].show_as_fulfilled > 0.4,
            _ => state.animation_done && state.tokens.iter().all(|t| t.fade > 0.95),
        };
        if due {
            taken += 1;
            let name = [
                "2_meters_on",
                "3_saint_filling",
                "4_saint_fulfilled",
                "5_tokens_passage",
            ][taken - 2];
            eprintln!("{name} at {frame}: {m:?}");
            s.process_shot(dir, name);
            if taken == 5 {
                return;
            }
        }
    }
}

impl Session {
    /// Killed, then the map button once the game over allows it: the death screen.
    fn die(&mut self) {
        let game = self.frontend_mut().unwrap().game_mut().expect("the game");
        game.scene.game_mut().sim_mut().kill_player();
        self.frames(45, &[]);
        self.frames(1, &["Space"]);
        self.settle_on(ProcessId::DeathScreen);
    }

    /// Unfed in the shelter when the cycle's rain is over: the door closes on too little
    /// food.
    fn starve(&mut self) {
        let game = self.frontend_mut().unwrap().game_mut().expect("the game");
        let sim = game.scene.game_mut().sim_mut();
        sim.set_food_in_stomach(0);
        let rain = &mut sim.world_mut().rain_cycle;
        rain.timer = rain.cycle_length + 1;
        let shelter = sim.player_pos().room;
        let spawn = sim.shelter_door(shelter).unwrap().player_spawn_pos;
        let at = rw_sim::Vec2::from(rw_assets::tile_centre(spawn.0, spawn.1));
        game.scene.game_mut().set_player(at, rw_sim::Vec2::ZERO);
        for _ in 0..3000 {
            if self.frames(1, &[]).process.as_deref() == Some("StarveScreen") {
                self.settle_on(ProcessId::StarveScreen);
                return;
            }
        }
        panic!("never starved");
    }

    /// Shots `<name>_<frame>` at each listed frame, then the map held for 60 frames.
    fn screen_shots(&mut self, dir: &Path, name: &str, at: &[u32]) {
        let mut frame = 0;
        for &f in at {
            self.frames(f - frame, &[]);
            frame = f;
            self.process_shot(dir, &format!("{name}_{f:04}"));
        }
        self.frames(60, &["Space"]);
        self.process_shot(dir, &format!("{name}_map"));
    }
}

#[test]
#[ignore = "writes screenshots; run with DEPTH_SHOTS=<dir>"]
fn depth_screen_screenshots() {
    let Some(dir) = std::env::var_os("DEPTH_SHOTS") else {
        return;
    };
    let dir = Path::new(&dir);
    let Some((mut s, _assets, _port)) = continued() else {
        return;
    };
    s.sleep();
    s.screen_shots(dir, "sleep", &[1, 200, 600]);

    let Some((mut s, _assets, _port)) = continued() else {
        return;
    };
    s.frames(10, &[]);
    s.die();
    s.screen_shots(dir, "death", &[1, 300]);

    let reinforced = SAVE.replace("REINFORCEDKARMA<dpB>0", "REINFORCEDKARMA<dpB>1");
    let Some((mut s, _assets, _port)) = continued_with(&reinforced) else {
        return;
    };
    s.frames(10, &[]);
    s.die();
    s.screen_shots(dir, "flower", &[1, 150, 200, 250, 300, 350, 400, 500]);

    let Some((mut s, _assets, _port)) = continued() else {
        return;
    };
    s.frames(10, &[]);
    s.starve();
    s.screen_shots(dir, "starve", &[1, 300]);
}
