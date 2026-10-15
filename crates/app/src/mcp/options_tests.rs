//! The options menu through the MCP tools, in temporary directories (never the port's
//! or the game's): from the main menu's OPTIONS, the import of save slot 1 from a copy
//! of the game's save (the installed game's own `sav` when there is one), and the
//! options file written on BACK. Set `RW_OPTIONS_SHOTS` to a directory to keep the
//! screenshots. Skipped without the installed game.

use std::path::Path;

use rw_assets::{StreamingAssets, UnityResources};
use rw_menu::ProcessId;
use rw_save::user_data::UserDataFile;
use rw_save::{Options, Platform};

use super::Session;
use super::params::{LoadProcessParams, ProcessParam, ScreenshotParams};
use crate::GameDir;
use crate::saves::SaveConfig;

impl Session {
    fn shot_to(&mut self, dir: Option<&Path>, name: &str) {
        let params = ScreenshotParams {
            save_path: dir.map(|d| d.join(format!("{name}.png")).to_string_lossy().into_owned()),
            ..ScreenshotParams::default()
        };
        self.screenshot(&params).expect("screenshot");
    }
}

#[test]
fn options_menu_imports_the_selected_slot() {
    let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
    if !assets.file("levels").is_dir() {
        eprintln!("skipping: no game under {}", assets.path().display());
        return;
    }
    let Some(salt) = SaveConfig::salt(&assets) else {
        eprintln!("skipping: no checksum salt");
        return;
    };
    let shots = std::env::var_os("RW_OPTIONS_SHOTS").map(std::path::PathBuf::from);
    let game = tempfile::tempdir().unwrap();
    let port = tempfile::tempdir().unwrap();
    // A copy of the game's slot 1 (read only), else a made-up one.
    let real = assets
        .data_dir()
        .parent()
        .and_then(|g| Platform::current().game_save_dir_here(g))
        .map(|d| d.join("sav"))
        .filter(|p| p.is_file());
    let source = match &real {
        Some(path) => std::fs::read(path).unwrap(),
        None => {
            let text = "MISCPROG<progDivB>CURRENTSLUGCAT<mpdB>White<mpdA><progDivA>";
            let mut file = UserDataFile::new();
            file.set("save", salt.md5_sum(text) + text);
            file.to_bytes()
        }
    };
    std::fs::write(game.path().join("sav"), &source).unwrap();
    let resources = UnityResources::open(assets.data_dir()).expect("Unity resources");
    let config = SaveConfig {
        dir: Some(port.path().to_owned()),
        game_saves: Some(game.path().to_owned()),
    };
    let mut s = Session::new(assets.clone(), std::sync::Arc::new(resources), config);
    s.load_process(&LoadProcessParams {
        process: Some(ProcessParam::MainMenu),
        ..LoadProcessParams::default()
    })
    .expect("process loads");
    s.settle_on(ProcessId::MainMenu);
    for _ in 0..4 {
        s.tap("DownArrow");
    }
    s.tap("Return");
    s.settle_on(ProcessId::OptionsMenu);
    s.frames(20, &[]);
    s.shot_to(shots.as_deref(), "options-1-menu");

    // BACK is selected; right is the importer.
    s.tap("RightArrow");
    let state = s.tap("Return");
    let dialog = state.options.dialog.expect("a confirmation");
    assert!(dialog.starts_with("Replace save slot 1"), "{dialog}");
    s.frames(10, &[]);
    s.shot_to(shots.as_deref(), "options-2-confirm");
    let state = s.tap("Return");
    let report = state.options.dialog.expect("a report");
    assert_eq!(report, "Imported save slot 1 from Rain World.");
    assert_eq!(std::fs::read(port.path().join("sav")).unwrap(), source);
    assert_eq!(state.save.path.as_deref(), port.path().join("sav").to_str());
    s.frames(10, &[]);
    s.shot_to(shots.as_deref(), "options-3-imported");
    s.tap("Return");

    // Again: the port's copy is moved to a backup first.
    s.tap("Return");
    s.tap("Return");
    let backups: Vec<_> = std::fs::read_dir(port.path().join("backup"))
        .unwrap()
        .collect();
    assert_eq!(backups.len(), 1);
    s.tap("Return");

    // Escape saves the options in the port's directory and leaves.
    s.tap("Escape");
    s.settle_on(ProcessId::MainMenu);
    let options = Options::load(port.path()).unwrap();
    assert_eq!(options.save_slot, 0);
    assert!(port.path().join(Options::LOCAL_FILE).is_file());
    // The game's directory is untouched: only the copy we put there.
    let names: Vec<_> = std::fs::read_dir(game.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["sav"]);
}
