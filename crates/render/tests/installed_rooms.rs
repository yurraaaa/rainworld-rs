//! Shortcut and water geometry on rooms from the installed game (read-only). Set
//! `RW_DIR` to the game install directory; skipped if the files are absent. Expected
//! tubes were read off the level files' tile grids by hand.

use std::path::PathBuf;

use rw_assets::{Room, Shortcut};
use rw_render::{RoomDebugGeometry, RoomShape, WaterSurface};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn load(name: &str) -> Option<Room> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("levels").is_dir() {
        eprintln!("skipping: no levels dir under {}", sa.display());
        return None;
    }
    // Mapped, as a loaded room is from `ShortCutsReady` on.
    let room = Room::load_level(&sa, name).expect("room loads");
    room.map_shortcuts();
    Some(room)
}

/// Shapes of one kind from [`RoomDebugGeometry::debug_shapes`].
trait ShapeQuery {
    fn links(&self) -> Vec<((i32, i32), (i32, i32))>;
    fn shortcut_paths(&self) -> Vec<Vec<(i32, i32)>>;
    fn entrances(&self) -> Vec<((i32, i32), (i32, i32))>;
}

impl ShapeQuery for Room {
    fn shortcut_paths(&self) -> Vec<Vec<(i32, i32)>> {
        self.shortcut_map()
            .shortcuts()
            .iter()
            .map(|s| s.path.clone())
            .collect()
    }

    fn links(&self) -> Vec<((i32, i32), (i32, i32))> {
        self.debug_shapes()
            .into_iter()
            .filter_map(|s| match s {
                RoomShape::ShortcutLink { from, to } => Some((from, to)),
                _ => None,
            })
            .collect()
    }

    fn entrances(&self) -> Vec<((i32, i32), (i32, i32))> {
        self.debug_shapes()
            .into_iter()
            .filter_map(|s| match s {
                RoomShape::ShortcutEntrance { x, y, hole } => Some(((x, y), hole)),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn summit_tubes_drop_into_room_exits() {
    let Some(room) = load("summit") else { return };
    assert_eq!(
        room.shortcut_paths(),
        vec![vec![(13, 17), (13, 16)], vec![(34, 17), (34, 16)]]
    );
    assert_eq!(
        room.links(),
        vec![((13, 16), (13, 17)), ((34, 16), (34, 17))]
    );
    let mut e = room.entrances();
    e.sort_unstable();
    assert_eq!(e, vec![((13, 17), (0, 1)), ((34, 17), (0, 1))]);
}

#[test]
fn smallroom_tubes() {
    let Some(room) = load("smallroom") else {
        return;
    };
    let paths = room.shortcut_paths();
    assert_eq!(paths.len(), 28);
    // Entrance (12,26), open upwards, winds to the entrance at (8,23).
    assert_eq!(room.shortcut_hole_direction(12, 26), (0, 1));
    assert!(paths.contains(&vec![
        (12, 26),
        (12, 25),
        (11, 25),
        (10, 25),
        (9, 25),
        (9, 24),
        (9, 23),
        (8, 23),
    ]));
    // Straight pipe (14,18) -> (8,18), traced from both ends.
    let straight: Vec<(i32, i32)> = (8..=14).rev().map(|x| (x, 18)).collect();
    assert!(paths.contains(&straight));
    assert!(paths.contains(&straight.iter().rev().copied().collect()));
    assert_eq!(room.shortcut_hole_direction(14, 18), (1, 0));
    assert_eq!(room.shortcut_hole_direction(8, 18), (-1, 0));
    // Into the den at (14,25); entrances above slopes open downwards.
    assert!(paths.contains(&vec![(14, 26), (14, 25)]));
    assert_eq!(room.shortcut_hole_direction(27, 26), (0, -1));
    // Each step is drawn once and joins neighbouring tiles.
    let links = room.links();
    for (a, b) in &links {
        assert_eq!((a.0 - b.0).abs() + (a.1 - b.1).abs(), 1, "{a:?} {b:?}");
    }
    let mut dedup = links.clone();
    dedup.sort_unstable();
    dedup.dedup();
    assert_eq!(dedup.len(), links.len());
    // Every tube tile is on a traced path.
    for x in 0..room.width() {
        for y in 0..room.height() {
            if room.tile(x, y).shortcut != Shortcut::None {
                assert!(
                    paths.iter().any(|p| p.contains(&(x, y))),
                    "({x},{y}) not traced"
                );
            }
        }
    }
}

#[test]
fn waterreactor_surface_sits_mid_row() {
    let Some(room) = load("waterreactor") else {
        return;
    };
    assert_eq!(room.water_level(), Some(5));
    let surface = WaterSurface::for_room(&room).expect("room has water");
    assert_eq!(surface.span(), [-220.0, room.pixel_width() + 220.0]);
    assert!(surface.heights().all(|h| h == 110.0));
    assert!(WaterSurface::for_room(&load("summit").expect("summit")).is_none());
}
