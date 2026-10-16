//! The items Outskirts rooms place (room file line 5) and their random item settings, read
//! from the installed game (read-only) as the Survivor's world loads them. Expected
//! values are the files' own lines. Set `RW_DIR` to the game install directory; skipped
//! if absent.

use std::path::PathBuf;

use rw_assets::world::{
    GameRooms, LineFilter, Regions, RoomSource, StoryConditions, WorldData, WorldRequest,
};
use rw_assets::{ActiveMods, StreamingAssets, Timeline};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

fn su() -> Option<(WorldData, GameRooms)> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("world").join("regions.txt").is_file() {
        eprintln!("skipping: no world under {}", sa.display());
        return None;
    }
    let assets = StreamingAssets::new(sa);
    let regions = Regions::load(&assets).expect("regions");
    let request = WorldRequest {
        name: "SU".to_owned(),
        single_room: false,
        filter: LineFilter {
            timeline: Some(Timeline::White),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        },
    };
    let data = WorldData::load(&assets, &regions, &request).expect("SU loads");
    let rooms = GameRooms {
        assets,
        region: Some("su".to_owned()),
        timeline: Timeline::White,
    };
    Some((data, rooms))
}

#[test]
fn outskirts_file_items() {
    let Some((data, rooms)) = su() else { return };
    let mut census = Vec::new();
    for room in data.rooms.iter().filter(|r| !r.offscreen_den) {
        let loaded = rooms.load_room(room.file_name()).expect("room loads");
        let items = loaded.room.items();
        if items.is_empty() {
            continue;
        }
        let rocks = items.iter().filter(|i| i.kind == 0).count();
        let spears = items.iter().filter(|i| i.kind == 1).count();
        assert_eq!(rocks + spears, items.len(), "{}", room.name);
        census.push((room.name.clone(), rocks, spears));
    }
    census.sort();
    let want = [
        ("SU_A07", 1, 2),
        ("SU_A13", 5, 1),
        ("SU_A23", 3, 0),
        ("SU_A24", 0, 1),
        ("SU_A25", 2, 3),
        ("SU_A32", 0, 3),
        ("SU_A40", 0, 1),
        ("SU_A63", 1, 0),
        ("SU_B02", 1, 1),
        ("SU_B04", 2, 4),
        ("SU_C02", 0, 3),
    ];
    let want: Vec<_> = want.iter().map(|&(n, r, s)| (n.to_owned(), r, s)).collect();
    assert_eq!(census, want);

    // `su_a13.txt` line 5: `0,12,30|0,23,28|0,29,19|1,32,29|0,38,30|0,44,30|`, height 35.
    let a13 = rooms.load_room("SU_A13").expect("SU_A13");
    assert_eq!(a13.room.height(), 35);
    let tiles: Vec<_> = a13.room.items().iter().map(|i| i.tile(35)).collect();
    assert_eq!(
        tiles,
        [(11, 5), (22, 7), (28, 16), (31, 6), (37, 5), (43, 5)]
    );
}

#[test]
fn outskirts_random_item_settings() {
    let Some((data, rooms)) = su() else { return };
    let mut zero = Vec::new();
    for room in data.rooms.iter().filter(|r| !r.offscreen_den) {
        let s = rooms
            .load_room(room.file_name())
            .expect("room loads")
            .settings;
        assert_eq!(s.random_item_spear_chance, 0.2, "{}", room.name);
        if s.random_item_density == 0.0 {
            zero.push(room.name.clone());
        } else {
            assert_eq!(s.random_item_density, 0.5, "{}", room.name);
        }
    }
    zero.sort();
    assert_eq!(zero, ["SU_A41", "SU_A42", "SU_A43", "SU_A44", "SU_C04"]);
}
