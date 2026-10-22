//! Loads rooms from the installed game (read-only). Expected counts come from
//! `rainworld-re/tools/rw_room.py`. Set `RW_DIR` to the game install directory
//! (the one containing `RainWorld_Data`); skipped if the files are absent.

use std::path::PathBuf;

use rw_assets::{RainMask, Room, Shortcut, ShortcutType, Terrain, Tile};

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

/// Shortcut maps below were traced by hand on an ASCII dump of the level grids
/// (`shortcuts.md` section 1 rules), not from this crate.
#[test]
fn summit_shortcut_map() {
    let Some(sa) = streaming_assets() else { return };
    let room = Room::load_level(&sa, "Summit").expect("summit loads");
    let map = room.shortcut_map();
    // Room exits (13,16) and (34,16), each under its entrance; no dens.
    assert_eq!(map.nodes(), &[(13, 16), (34, 16)]);
    let table: Vec<_> = map
        .shortcuts()
        .iter()
        .map(|s| (s.start, s.kind, s.dest_node, s.length, s.path.clone()))
        .collect();
    assert_eq!(
        table,
        vec![
            (
                (13, 17),
                ShortcutType::RoomExit,
                Some(0),
                1,
                vec![(13, 17), (13, 16)]
            ),
            (
                (34, 17),
                ShortcutType::RoomExit,
                Some(1),
                1,
                vec![(34, 17), (34, 16)]
            ),
        ]
    );
    // Left and right are Solid, down is the exit tile (Solid): up.
    assert_eq!(room.shortcut_hole_direction(13, 17), (0, 1));
    assert_eq!(room.shortcut_hole_direction(34, 17), (0, 1));
}

#[test]
fn smallroom_shortcut_map() {
    let Some(sa) = streaming_assets() else { return };
    let room = Room::load_level(&sa, "SmallRoom").expect("smallroom loads");
    let map = room.shortcut_map();
    // Exits (33,25), (35,25); dens top row first: (14,25), (29,8), (21,2), (26,2).
    assert_eq!(
        map.nodes(),
        &[(33, 25), (35, 25), (14, 25), (29, 8), (21, 2), (26, 2)]
    );
    let starts: Vec<_> = map.shortcuts().iter().map(|s| s.start).collect();
    assert_eq!(
        starts,
        vec![
            (8, 29),
            (39, 29),
            (12, 26),
            (14, 26),
            (27, 26),
            (29, 26),
            (33, 26),
            (35, 26),
            (5, 25),
            (42, 25),
            (8, 23),
            (33, 23),
            (39, 23),
            (8, 18),
            (14, 18),
            (39, 18),
            (5, 16),
            (42, 16),
            (14, 12),
            (33, 12),
            (7, 10),
            (40, 10),
            (18, 9),
            (29, 9),
            (19, 3),
            (21, 3),
            (26, 3),
            (28, 3),
        ]
    );
    // Node shortcuts: one tile down into the node.
    for (start, kind, node) in [
        ((14, 26), ShortcutType::CreatureHole, 2),
        ((29, 9), ShortcutType::CreatureHole, 3),
        ((21, 3), ShortcutType::CreatureHole, 4),
        ((26, 3), ShortcutType::CreatureHole, 5),
        ((33, 26), ShortcutType::RoomExit, 0),
        ((35, 26), ShortcutType::RoomExit, 1),
    ] {
        let s = map.data_at(start).expect("indexed");
        assert_eq!((s.kind, s.dest_node, s.length), (kind, Some(node), 1));
        assert_eq!(s.dest, map.nodes()[node]);
    }
    // Every other entrance is a Normal pipe whose far end leads back.
    for s in map.shortcuts() {
        if s.dest_node.is_none() {
            assert_eq!(s.kind, ShortcutType::Normal, "{:?}", s.start);
            assert_eq!(map.data_at(s.dest).expect("indexed").dest, s.start);
        }
    }
    // Straight pipes with an Air path tile at (10,18) and (37,23).
    let straight: Vec<(i32, i32)> = (8..=14).map(|x| (x, 18)).collect();
    let s = map.data_at((8, 18)).expect("indexed");
    assert_eq!(
        (s.kind, s.length, &s.path),
        (ShortcutType::Normal, 6, &straight)
    );
    assert_eq!(map.data_at((33, 23)).expect("indexed").dest, (39, 23));
    // The winding pipe from (12,26).
    let s = map.data_at((12, 26)).expect("indexed");
    assert_eq!(
        s.path,
        vec![
            (12, 26),
            (12, 25),
            (11, 25),
            (10, 25),
            (9, 25),
            (9, 24),
            (9, 23),
            (8, 23)
        ]
    );
    // Hole directions: Air to the left / right, a Floor above, Air below.
    assert_eq!(room.shortcut_hole_direction(8, 18), (-1, 0));
    assert_eq!(room.shortcut_hole_direction(14, 18), (1, 0));
    assert_eq!(room.shortcut_hole_direction(12, 26), (0, 1));
    assert_eq!(room.shortcut_hole_direction(27, 26), (0, -1));
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

/// `RoomRain.GenerateShelterTex` over DS_A10 (48x35, water row 16), checked texel by
/// texel against its rules: red while nothing solid lies at or above the tile in its
/// column; blue on the surface row of a non-solid surface tile and on each row above
/// it up to the first solid tile (included), at most 20 rows up.
#[test]
fn ds_a10_rain_mask_follows_the_generator_rules() {
    let Some(sa) = streaming_assets() else { return };
    let room = Room::load(sa.join("world/ds-rooms/ds_a10.txt")).expect("ds_a10 loads");
    assert_eq!(room.water_level(), Some(16));
    let mask = RainMask::generate(&room);
    assert_eq!((mask.width(), mask.height()), (48, 35));
    let mut blue = 0;
    for x in 0..48 {
        let mut run_end = None;
        if !room.tile(x, 16).is_solid() {
            let mut y = 16;
            // The 20-row reach (row 36) lies past the top here.
            while y < 35 && !room.tile(x, y).is_solid() {
                y += 1;
            }
            run_end = Some(y);
        }
        for y in 0..35 {
            let open = (y..35).all(|above| !room.tile(x, above).is_solid());
            let water = run_end.is_some_and(|end| (16..=end).contains(&y));
            let t = mask.texel(x, y);
            assert_eq!(t[0] == 255, open, "red at ({x}, {y})");
            assert_eq!(t[2] == 255, water, "blue at ({x}, {y})");
            assert_eq!((t[1], t[3]), (0, 255));
            blue += usize::from(water);
        }
    }
    eprintln!("ds_a10: {blue} blue texels");
    assert!(blue > 0, "the surface marks some texels");
}
