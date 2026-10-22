//! Placed `LightSource` data in the installed game's rooms (read-only): every entry
//! parses, and Outskirts has the lights the files list. Set `RW_DIR` to the game install
//! directory; skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::world::{
    GameRooms, LineFilter, Regions, RoomSource, StoryConditions, WorldData, WorldRequest,
};
use rw_assets::{
    ActiveMods, LightColorType, LightSourceData, PlacedObject, PlacedObjectType, StreamingAssets,
    Timeline,
};

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

    /// Every `*_settings*.txt` under the base regions' room folders, gates and arena
    /// levels.
    fn settings_files(sa: &Path) -> Vec<PathBuf> {
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
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_settings"))
            })
            .collect();
        files.sort();
        files
    }

    fn is_light(o: &PlacedObject) -> bool {
        o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::LightSource)
    }
}

/// Every placed `LightSource` of the base game's settings files parses (the game would
/// throw on any that does not), with colour types the game registers.
#[test]
fn every_placed_light_parses() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let (mut lights, mut env) = (0, 0);
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        for value in text
            .lines()
            .filter_map(|l| l.strip_prefix("PlacedObjects: "))
        {
            for o in PlacedObject::parse_list(value)
                .iter()
                .filter(|o| Install::is_light(o))
            {
                let d = LightSourceData::parse(&o.data)
                    .unwrap_or_else(|| panic!("{}: {:?}", path.display(), o.data));
                assert!(d.color_type.is_some(), "{}", path.display());
                lights += 1;
                env += usize::from(d.color_type == Some(LightColorType::Environment));
            }
        }
    }
    eprintln!("{lights} placed lights, {env} coloured by the environment");
    assert!(lights > 500);
}

/// Outskirts' lights (survey `lights-camera-shake.md` 1.1): `su_a33` 1, `su_a34` 2,
/// `su_a35` 1, `su_a41` 1, `su_b04` 1, `su_b09` 1 (white), `su_b13` 3.
#[test]
fn outskirts_lights() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
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
    let mut census = Vec::new();
    for room in data.rooms.iter().filter(|r| !r.offscreen_den) {
        let Ok(loaded) = rooms.load_room(room.file_name()) else {
            continue;
        };
        let lights: Vec<LightSourceData> = loaded
            .settings
            .placed_objects_for(Timeline::White, ActiveMods::NONE)
            .iter()
            .filter(|o| o.active && Install::is_light(o))
            .filter_map(|o| LightSourceData::parse(&o.data))
            .collect();
        if lights.is_empty() {
            continue;
        }
        let white = lights
            .iter()
            .filter(|d| d.color_type == Some(LightColorType::White))
            .count();
        census.push((room.name.clone(), lights.len(), white));
    }
    census.sort();
    let want = [
        ("SU_A33", 1, 0),
        ("SU_A34", 2, 0),
        ("SU_A35", 1, 0),
        ("SU_A41", 1, 0),
        ("SU_B04", 1, 0),
        ("SU_B09", 1, 1),
        ("SU_B13", 3, 0),
    ];
    let want: Vec<_> = want.iter().map(|&(n, l, w)| (n.to_owned(), l, w)).collect();
    assert_eq!(census, want);
}
