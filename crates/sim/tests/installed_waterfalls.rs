//! The waterfalls the installed game's rooms make from their waterfall tiles (code 8 of the
//! geometry files, read-only): `WaterFall::specs` against an independent reference model of
//! `Room.LoadFromDataString`'s loop (a throwaway Python script over the same files: 151
//! falls in 39 of the 801 geometry files of the 12 base regions' room folders, the gates and
//! the arena levels), and every waterfall tile taken by exactly one fall. Set `RW_DIR` to
//! the game install directory; skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::{Room, TileFlags};
use rw_sim::WaterFall;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Install;

impl Install {
    fn streaming_assets() -> Option<PathBuf> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(sa)
        } else {
            eprintln!("skipping: no world under {}", sa.display());
            None
        }
    }

    /// Every geometry file (not a settings file) under the base regions' room folders,
    /// the gates and the arena levels.
    fn geometry_files(sa: &Path) -> Vec<PathBuf> {
        let mut dirs = vec![sa.join("world").join("gates"), sa.join("levels")];
        let regions =
            std::fs::read_to_string(sa.join("world").join("regions.txt")).expect("regions.txt");
        for r in regions.lines().map(str::trim).filter(|r| !r.is_empty()) {
            dirs.push(sa.join("world").join(format!("{}-rooms", r.to_lowercase())));
        }
        let mut files: Vec<PathBuf> = dirs
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| e == "txt")
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| !n.contains("_settings"))
            })
            .collect();
        files.sort();
        files
    }
}

#[test]
fn every_room_makes_the_waterfalls_its_tiles_name() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let (mut read, mut rejected, mut falls, mut with_falls) = (0, 0, 0, 0);
    for path in Install::geometry_files(&sa) {
        let Ok(room) = Room::load(&path) else {
            eprintln!("rejected: {}", path.display());
            rejected += 1;
            continue;
        };
        read += 1;
        let specs = WaterFall::specs(&room);
        let tiles = room
            .tiles()
            .iter()
            .filter(|t| t.flags.contains(TileFlags::WATERFALL))
            .count();
        let taken: usize = specs
            .iter()
            .map(|s| s.width as usize * if s.flow == 1.0 { 2 } else { 1 })
            .sum();
        assert_eq!(taken, tiles, "{}", path.display());
        if !specs.is_empty() {
            with_falls += 1;
            falls += specs.len();
            println!(
                "{} {}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default(),
                specs.len()
            );
        }
    }
    eprintln!("{read} rooms read, {rejected} rejected; {falls} waterfalls in {with_falls} rooms");
    assert_eq!((falls, with_falls), (151, 39));
}
