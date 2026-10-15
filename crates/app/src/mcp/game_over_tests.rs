//! The game over in a continued story (`Player.Die` → `GameOver` → the text prompt's
//! game-over mode → `GoToDeathScreen`), on a constructed save in temporary directories.
//! Skipped without the installed game.

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_menu::ProcessId;
use rw_save::user_data::UserDataFile;
use rw_save::{CampaignStart, PlayerProgression};

use super::Session;
use super::params::{ImportSaveParams, LoadProcessParams, ProcessParam};
use crate::GameDir;
use crate::saves::SaveConfig;

const SAVE: &str = "SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA>SEED<svB>4321<svA>\
    DENPOS<svB>SU_S01<svA>LASTVDENPOS<svB>SU_S01<svA>CYCLENUM<svB>5<svA>FOOD<svB>3<svA>\
    NEXTID<svB>2500<svA>DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>2<dpA>\
    KARMACAP<dpB>5<dpA>GHOSTS<dpB><dpA>TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>1\
    <dpA>SURVIVES<dpB>4<dpA>QUITS<dpB>0<dpA><svA><progDivA>MISCPROG<progDivB>CURRENTSLUGCAT\
    <mpdB>White<mpdA>INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA><progDivA>";

const WHITE: CampaignStart<'static> = CampaignStart {
    slugcat: SlugcatName::White,
    den: "SU_C04",
    vanilla: true,
};

#[test]
fn death_waits_for_a_map_press_then_saves_and_switches() {
    let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
    if !assets.file("levels").is_dir() {
        eprintln!("skipping: no game under {}", assets.path().display());
        return;
    }
    let Some(salt) = SaveConfig::salt(&assets) else {
        eprintln!("skipping: no checksum salt");
        return;
    };
    let game = tempfile::tempdir().unwrap();
    let port = tempfile::tempdir().unwrap();
    let mut file = UserDataFile::new();
    file.set("save", salt.md5_sum(SAVE) + SAVE);
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
    s.frames(10, &[]);

    let process = s.frontend_mut().unwrap().game_mut().expect("the game");
    assert!(!process.scene.game_mut().respawn(), "no R in a story");
    process.scene.game_mut().sim_mut().kill_player();
    // `GameOver` during the rooms' update: the prompt is in game-over mode from the next
    // HUD update.
    s.frames(1, &[]);
    let prompt = s.hud_state().expect("hud").text_prompt;
    assert!(prompt.game_over);
    // `ShowPauseMenu` refuses during game over.
    let paused = s.tap("Escape");
    assert!(paused.pause_menu.is_none());
    // `restartNotAllowed` 40 (3 ticks spent above): Space held through it goes nowhere,
    // and `lastAnyKey` starts true, so the first tick after it only notes the button.
    let held = s.frames(38, &["Space"]);
    assert_eq!(held.upcoming, None);
    assert_eq!(
        s.hud_state().expect("hud").text_prompt.text,
        "Game Over - Press SPACE to restart"
    );
    assert_eq!(s.frames(1, &["Space"]).upcoming, None);
    assert_eq!(s.frames(1, &[]).upcoming, None);

    // `CommunicateWithUpcomingProcess`: the death screen's food, room and position are
    // the prompt's; its map shows one karma less (2 → 1, not reinforced).
    let f = s.frontend_mut().unwrap();
    let package = f
        .manager
        .current()
        .and_then(|p| p.screen_package(ProcessId::DeathScreen, &f.ctx.menu.saves, &assets))
        .expect("a package");
    let prompt = &f.game().unwrap().scene.hud().unwrap().text_prompt;
    assert_eq!(prompt.food_in_stomach, 3);
    assert_eq!(
        (package.food, package.player_room, package.player_pos),
        (
            prompt.food_in_stomach,
            prompt.death_room,
            rw_sim::Vec2::from(prompt.death_pos)
        )
    );
    assert_eq!(package.karma, (2, 5));
    assert_eq!(package.map_data.expect("map").current_karma, 1);
    // The rising edge: `GoToDeathScreen`.
    let pressed = s.frames(1, &["Space"]);
    assert_eq!(pressed.upcoming.as_deref(), Some("DeathScreen"));

    // `SessionEnded(survived: false)` from the in-memory state (karma 2, 1 death, no
    // quit): one karma and a death more, the start's quit undone.
    let sav = port.path().join("sav");
    let mut p = PlayerProgression::load(&sav, SaveConfig::salt(&assets)).expect("loads");
    let state = p.peek_save_state(WHITE).expect("reads").expect("a save");
    let d = &state.death_persistent;
    assert_eq!((d.karma, d.deaths, d.quits), (1, 2, 0));
    s.settle_on(ProcessId::DeathScreen);
}
