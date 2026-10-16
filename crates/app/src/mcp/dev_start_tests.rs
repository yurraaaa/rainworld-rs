//! A dev start (a room on the command line) has its own save state in memory: a fed
//! slugcat sleeping in a shelter and a starving one reach the sleep and starve screens,
//! and the port's save directory is never written (D16). Skipped without the installed
//! game.

use std::path::{Path, PathBuf};

use rw_assets::{StreamingAssets, UnityResources};
use rw_menu::ProcessId;
use rw_save::user_data::UserDataFile;

use super::Session;
use super::params::{LoadProcessParams, ProcessParam};
use super::passage_tests::SAVE;
use crate::GameDir;
use crate::saves::SaveConfig;

/// Every file under a directory, with its contents.
#[derive(Debug, PartialEq, Eq)]
struct DirSnapshot(Vec<(PathBuf, Vec<u8>)>);

impl DirSnapshot {
    fn of(dir: &Path) -> Self {
        let mut files = Vec::new();
        Self::collect(dir, &mut files);
        files.sort();
        Self(files)
    }

    fn collect(dir: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                Self::collect(&path, files);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                files.push((path, bytes));
            }
        }
    }
}

impl Session {
    /// A dev start in SU_S01, with a saved Survivor in the port's save directory (a
    /// temporary one), and that directory before the start.
    fn dev_start() -> Option<(Self, tempfile::TempDir, DirSnapshot)> {
        let assets = StreamingAssets::new(GameDir::from_env().streaming_assets());
        if !assets.file("levels").is_dir() {
            eprintln!("skipping: no game under {}", assets.path().display());
            return None;
        }
        let Some(salt) = SaveConfig::salt(&assets) else {
            eprintln!("skipping: no checksum salt");
            return None;
        };
        let port = tempfile::tempdir().unwrap();
        let mut file = UserDataFile::new();
        file.set("save", salt.md5_sum(SAVE) + SAVE);
        std::fs::write(port.path().join("sav"), file.to_bytes()).unwrap();
        let before = DirSnapshot::of(port.path());
        let resources = UnityResources::open(assets.data_dir()).expect("Unity resources");
        let config = SaveConfig {
            dir: Some(port.path().to_owned()),
            game_saves: Some(port.path().join("no-game-saves")),
        };
        let mut s = Self::new(assets, std::sync::Arc::new(resources), config);
        s.load_process(&LoadProcessParams {
            process: Some(ProcessParam::Game),
            room: Some("SU_S01".to_owned()),
            seed: None,
        })
        .expect("process loads");
        s.settle_on(ProcessId::Game);
        Some((s, port, before))
    }
}

#[test]
fn a_dev_start_sleeps_on_the_sleep_screen_without_saving() {
    let Some((mut s, port, before)) = Session::dev_start() else {
        return;
    };
    let game = s.frontend_mut().unwrap().game_mut().expect("the game");
    game.scene.game_mut().sim_mut().set_food_in_stomach(4);
    s.sleep();
    assert_eq!(s.frames(1, &[]).process.as_deref(), Some("SleepScreen"));
    // The dev start's own state (a new game's), not the saved cycle 5's.
    let saves = &s.frontend_mut().unwrap().ctx.menu.saves;
    assert!(saves.path().is_none());
    let state = saves.current_save_state.as_ref().expect("a save state");
    assert_eq!(
        (state.cycle_number, state.den_position.as_str()),
        (1, "SU_S01")
    );
    assert_eq!(DirSnapshot::of(port.path()), before, "the save dir changed");
}

#[test]
fn a_dev_start_starves_on_the_starve_screen_without_saving() {
    let Some((mut s, port, before)) = Session::dev_start() else {
        return;
    };
    s.starve();
    assert!(s.frames(1, &[]).karma_screen.is_some());
    assert_eq!(DirSnapshot::of(port.path()), before, "the save dir changed");
}
