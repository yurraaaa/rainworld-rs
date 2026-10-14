//! Loads rooms from the installed game (read-only). Expected counts come from
//! `rainworld-re/tools/rw_room.py`. Set `RW_DIR` to the game install directory
//! (the one containing `RainWorld_Data`); skipped if the files are absent.

use std::path::PathBuf;

use rw_assets::{Room, Shortcut, Terrain, Tile};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn streaming_assets() -> Option<PathBuf> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if sa.join("levels").is_dir() {
        Some(sa)
    } else {
        eprintln!("skipping: no levels dir under {}", sa.display());
        None
    }
}

fn count(room: &Room, pred: impl Fn(&Tile) -> bool) -> usize {
    room.tiles().iter().filter(|t| pred(t)).count()
}

fn terrain_counts(room: &Room) -> [usize; 5] {
    [
        Terrain::Air,
        Terrain::Solid,
        Terrain::Slope,
        Terrain::Floor,
        Terrain::ShortcutEntrance,
    ]
    .map(|k| count(room, |t| t.terrain == k))
}

#[test]
fn summit_matches_reference() {
    let Some(sa) = streaming_assets() else { return };
    let room = Room::load_level(&sa, "Summit").expect("summit loads");

    assert_eq!(room.name(), "Summit");
    assert_eq!((room.width(), room.height()), (48, 35));
    assert_eq!(room.water_level(), None);
    assert!(!room.water_in_front());
    // File `-220,-30` -> y = -(800 - 700 - 30) = -70.
    assert_eq!(room.cameras(), &[[-220.0, -70.0]]);
    assert_eq!(terrain_counts(&room), [1410, 262, 4, 2, 2]);
    assert_eq!(count(&room, |t| t.horizontal_pole()), 12);
    assert_eq!(count(&room, |t| t.vertical_pole()), 0);
    assert_eq!(count(&room, |t| t.wall_behind()), 4);
    // Flags 3 x4 and 4 x2; the two room exits also carry 3.
    assert_eq!(count(&room, |t| t.shortcut == Shortcut::RoomExit), 2);
    assert_eq!(count(&room, |t| t.shortcut == Shortcut::Path), 2);
    // Symmetric layout: every column mirrors its partner.
    for x in 0..room.width() {
        for y in 0..room.height() {
            assert_eq!(
                room.tile(x, y).terrain,
                room.tile(room.width() - 1 - x, y).terrain
            );
        }
    }
}

#[test]
fn smallroom_matches_reference() {
    let Some(sa) = streaming_assets() else { return };
    let room = Room::load_level(&sa, "SmallRoom").expect("smallroom loads");

    assert_eq!((room.width(), room.height()), (48, 35));
    assert_eq!(room.water_level(), None);
    assert_eq!(room.cameras().len(), 1);
    assert_eq!(terrain_counts(&room), [454, 1168, 10, 20, 28]);
    assert_eq!(count(&room, |t| t.vertical_pole()), 44);
    assert_eq!(count(&room, |t| t.horizontal_pole()), 12);
    assert_eq!(count(&room, |t| t.wall_behind()), 189);
    // Flag 3 on 227 tiles, 4 on 2, 5 on 4; every 4/5 tile also has 3.
    assert_eq!(count(&room, |t| t.shortcut != Shortcut::None), 227);
    assert_eq!(count(&room, |t| t.shortcut == Shortcut::RoomExit), 2);
    assert_eq!(count(&room, |t| t.shortcut == Shortcut::CreatureDen), 4);
}

#[test]
fn every_level_parses() {
    let Some(sa) = streaming_assets() else { return };
    let mut parsed = 0;
    for entry in std::fs::read_dir(sa.join("levels")).expect("levels dir readable") {
        let path = entry.expect("dir entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !name.ends_with(".txt")
            || name.ends_with("_settings.txt")
            || name.ends_with("_arena.txt")
        {
            continue;
        }
        if let Err(e) = Room::load(&path) {
            panic!("{}: {e}", path.display());
        }
        parsed += 1;
    }
    eprintln!("parsed {parsed} level files");
    assert!(parsed > 0);
}
