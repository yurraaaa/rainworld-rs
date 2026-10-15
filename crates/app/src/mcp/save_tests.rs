//! Saves through the MCP tools, on a constructed save in temporary directories (never
//! the game's): `import_save`, then Continue from the slugcat select, then the exit
//! save. Skipped without the installed game.

use std::path::Path;

use rw_assets::{SlugcatName, StreamingAssets, UnityResources};
use rw_menu::ProcessId;
use rw_save::user_data::UserDataFile;
use rw_save::{CampaignStart, PlayerProgression};
use rw_sim::{DiscoveryTexture, UnityTexture};

use super::Session;
use super::params::{ImportSaveParams, LoadProcessParams, ProcessParam};
use crate::GameDir;
use crate::saves::SaveConfig;

const SAVE: &str = "SAVE STATE<progDivB>SAV STATE NUMBER<svB>White<svA>SEED<svB>4321<svA>\
    DENPOS<svB>SU_S01<svA>LASTVDENPOS<svB>SU_S01<svA>CYCLENUM<svB>5<svA>FOOD<svB>3<svA>\
    NEXTID<svB>2500<svA>DEATHPERSISTENTSAVEDATA<svB>REINFORCEDKARMA<dpB>0<dpA>KARMA<dpB>2<dpA>\
    KARMACAP<dpB>5<dpA>GHOSTS<dpB><dpA>TUTMESSAGES<dpB><dpA>METERSSHOWN<dpB><dpA>DEATHS<dpB>1\
    <dpA>SURVIVES<dpB>4<dpA>QUITS<dpB>0<dpA>CHATLOGS<dpB><dpA><svA>FORCEPUPS<svB>0<svA>\
    <progDivA>MAP_Rivulet<progDivB>SU<progDivB>AAAA<progDivA>MISCPROG<progDivB>CURRENTSLUGCAT\
    <mpdB>White<mpdA>INTEGERS<mpdB>0,0,0,0,0,0,0,0,0,0,0<mpdA><progDivA>";

const WHITE: CampaignStart<'static> = CampaignStart {
    slugcat: SlugcatName::White,
    den: "SU_C04",
    vanilla: true,
};

fn stored(path: &Path, assets: &StreamingAssets) -> (i32, i32, Vec<String>) {
    let mut p = PlayerProgression::load(path, SaveConfig::salt(assets)).expect("loads");
    assert!(!p.game_tinkered_with, "checksum");
    let lines = p.prog_lines();
    let state = p
        .peek_save_state(WHITE)
        .expect("reads")
        .expect("a White save");
    (
        state.death_persistent.karma,
        state.death_persistent.quits,
        lines,
    )
}

#[test]
fn import_continue_and_exit() {
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

    let report = s
        .import_save(&ImportSaveParams::default())
        .expect("imports");
    assert_eq!(report.save_states, ["White"]);
    assert_eq!(report.backup, None);
    let sav = port.path().join("sav");

    s.load_process(&LoadProcessParams {
        process: Some(ProcessParam::SlugcatSelect),
        ..LoadProcessParams::default()
    })
    .expect("process loads");
    s.settle_on(ProcessId::SlugcatSelect);
    assert_eq!(s.process_state().unwrap().save.saved, ["White"]);
    // The Survivor's page is selected (CURRENTSLUGCAT): START continues.
    let mut signalled = false;
    for _ in 0..80 {
        if s.frames(1, &["Z"]).upcoming.is_some() {
            signalled = true;
            break;
        }
    }
    assert!(signalled, "START never signalled");
    s.settle_on(ProcessId::Game);
    let state = s.process_state().unwrap();
    let story = state.story.expect("a game");
    assert_eq!(
        (
            story.den.as_str(),
            story.cycle,
            story.karma,
            story.karma_cap
        ),
        ("SU_S01", 5, 2, 5)
    );
    assert_eq!(story.player_room.as_deref(), Some("SU_S01"));
    assert_eq!(story.food_in_stomach, 3);
    assert_ne!(story.player_tile, Some([0, 0]));
    let current = state.save.current.expect("a save state");
    assert!(current.loaded);
    assert_eq!((current.karma, current.quits), (2, 0));

    // Starting saved it as a death and a quit; lines the port does not own stay.
    let (karma, quits, lines) = stored(&sav, &assets);
    assert_eq!((karma, quits), (1, 1));
    assert!(lines[0].contains("DEATHPERSISTENTSAVEDATA<svB>REDSDEATH<dpA>"));
    assert!(lines[0].contains("CHATLOGS<dpB><dpA><svA>FORCEPUPS<svB>0<svA><svA>"));
    assert!(
        lines
            .iter()
            .any(|l| l == "MAP_Rivulet<progDivB>SU<progDivB>AAAA")
    );

    // An exit within 5 s is neither a death nor a quit.
    let ctx = &mut s.frontend_mut().expect("process").ctx;
    ctx.exit_game(true, true, 100);
    assert_eq!(stored(&sav, &assets).0, 2);
    assert_eq!(stored(&sav, &assets).1, 0);

    // The pause menu's Exit after 30 s warns, asks, and saves both.
    let ran = s.frames(1210, &[]);
    assert!(ran.game_tick.expect("game") > 1200);
    s.tap("Escape");
    s.frames(45, &[]);
    s.tap("LeftArrow");
    let asked = s.tap("Return");
    assert_eq!(asked.upcoming, None);
    let menu = asked.pause_menu.expect("paused");
    assert_eq!(menu.selected.as_deref(), Some("NO_EXIT"));
    s.frames(45, &[]);
    s.tap("LeftArrow");
    let exit = s.tap("Return");
    assert_eq!(exit.upcoming.as_deref(), Some("MainMenu"));
    let (karma, quits, lines) = stored(&sav, &assets);
    assert_eq!((karma, quits), (1, 1));
    // `warnedAboutKarmaLossOnExit` is saved with the progression.
    assert!(
        lines
            .iter()
            .any(|l| l.contains("INTEGERS<mpdB>0,0,0,0,0,0,1,0,0,0,0<mpdA>"))
    );
    s.settle_on(ProcessId::MainMenu);
}

#[test]
fn map_discovery_loaded_and_saved() {
    let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
    if !assets.file("levels").is_dir() {
        eprintln!("skipping: no game under {}", assets.path().display());
        return;
    }
    let Some(salt) = SaveConfig::salt(&assets) else {
        eprintln!("skipping: no checksum salt");
        return;
    };
    // SU's map is 1189 x 1710: a 169 x 244 discovery texture, one corner pixel seen.
    let mut texture = DiscoveryTexture(UnityTexture::black(169, 244));
    texture.0.set(2, 3, [1.0, 1.0, 1.0]);
    let png = texture.to_base64_png();
    let save = SAVE.replace(
        "MAP_Rivulet<progDivB>SU<progDivB>AAAA",
        &format!("MAP<progDivB>SU<progDivB>{png}<progDivA>MAPUPDATE<progDivB>SU<progDivB>7"),
    );
    let game = tempfile::tempdir().unwrap();
    let port = tempfile::tempdir().unwrap();
    let mut file = UserDataFile::new();
    file.set("save", salt.md5_sum(&save) + &save);
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
    let mut signalled = false;
    for _ in 0..80 {
        if s.frames(1, &["Z"]).upcoming.is_some() {
            signalled = true;
            break;
        }
    }
    assert!(signalled, "START never signalled");
    s.settle_on(ProcessId::Game);
    s.frames(10, &[]);

    let frontend = s.frontend_mut().expect("process");
    let sim = frontend.game().expect("a game").scene.game().sim();
    let loaded = sim.map_discovery().texture("SU").expect("SU loaded");
    assert_eq!((loaded.0.width(), loaded.0.height()), (169, 244));
    assert!(loaded.0.r(2, 3) > 0.5, "the stored pixel");

    // The pause menu's Exit (before 30 s, without asking) saves the map.
    s.tap("Escape");
    s.frames(45, &[]);
    s.tap("LeftArrow");
    let exit = s.tap("Return");
    assert_eq!(exit.upcoming.as_deref(), Some("MainMenu"));
    let (_, _, lines) = stored(&port.path().join("sav"), &assets);
    let maps: Vec<&String> = lines.iter().filter(|l| l.starts_with("MAP<")).collect();
    assert_eq!(maps.len(), 1, "{maps:?}");
    let rewritten = maps[0]
        .strip_prefix("MAP<progDivB>SU<progDivB>")
        .expect("SU");
    assert_ne!(rewritten, png, "the shelter was discovered");
    let saved = DiscoveryTexture::from_base64_png(rewritten).expect("decodes");
    assert!(saved.0.r(2, 3) > 0.5);
    s.settle_on(ProcessId::MainMenu);
}
