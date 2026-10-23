//! Placed `SSLightRod`, `ZapCoil`, `SuperStructureFuses`, `ZapCoilLight`, `DeepProcessing`,
//! `DeepProcessingLight` and `GravityDisruptor` data in the installed game's rooms
//! (read-only): every entry parses. Set `RW_DIR` to the game
//! install directory; skipped if absent.

use std::collections::BTreeSet;
use std::path::PathBuf;

use rw_assets::{
    ActiveMods, DeepProcessingData, GridRectData, LightFixtureData, LightFixtureType, PlacedObject,
    PlacedObjectType, SsLightRodData,
};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

/// Every `*_settings*.txt` under the base regions' room folders, gates and levels.
fn settings_files() -> Option<Vec<PathBuf>> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    let Ok(regions) = std::fs::read_to_string(sa.join("world").join("regions.txt")) else {
        eprintln!("skipping: no world under {}", sa.display());
        return None;
    };
    let mut dirs = vec![sa.join("world").join("gates"), sa.join("levels")];
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
    Some(files)
}

/// Every placed rod, coil, fuse grid and coil light parses; the rooms that hold them are
/// counted (rooms, gates and levels: 12 rod, 13 coil, 50 fuse, 19 coil-light).
#[test]
fn every_electric_object_parses() {
    let Some(files) = settings_files() else {
        return;
    };
    let mut rooms: [BTreeSet<String>; 4] = Default::default();
    let mut rejected = 0;
    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let text = std::fs::read_to_string(&path).expect("settings");
        for value in text
            .lines()
            .filter_map(|l| l.strip_prefix("PlacedObjects: "))
        {
            for o in PlacedObject::parse_list(value) {
                let (slot, ok) = match o.object_type(ActiveMods::NONE) {
                    Some(PlacedObjectType::SSLightRod) => {
                        (0, SsLightRodData::parse(&o.data).is_some())
                    }
                    Some(PlacedObjectType::ZapCoil) => (1, GridRectData::parse(&o.data).is_some()),
                    Some(PlacedObjectType::SuperStructureFuses) => {
                        (2, GridRectData::parse(&o.data).is_some())
                    }
                    Some(PlacedObjectType::LightFixture) => {
                        match LightFixtureData::parse(&o.data) {
                            Some(d) if d.kind == Some(LightFixtureType::ZapCoilLight) => (3, true),
                            Some(_) => continue,
                            None => (3, false),
                        }
                    }
                    _ => continue,
                };
                if ok {
                    rooms[slot].insert(name.clone());
                } else {
                    rejected += 1;
                }
            }
        }
    }
    eprintln!(
        "rod rooms {}, coil rooms {}, fuse rooms {}, coil-light rooms {}, {rejected} rejected",
        rooms[0].len(),
        rooms[1].len(),
        rooms[2].len(),
        rooms[3].len()
    );
    assert_eq!(rejected, 0);
    assert_eq!(
        [
            rooms[0].len(),
            rooms[1].len(),
            rooms[2].len(),
            rooms[3].len()
        ],
        [12, 13, 50, 19]
    );
}

/// Every placed `DeepProcessing` parses; the rooms (rooms, gates and levels) holding a
/// `DeepProcessing`, a `DeepProcessingLight` fixture and a `GravityDisruptor` are 18, 3
/// and 3 (the light fixtures are 4 objects in those 3 rooms).
#[test]
fn every_grab_object_parses() {
    let Some(files) = settings_files() else {
        return;
    };
    let mut rooms: [BTreeSet<String>; 3] = Default::default();
    let (mut rejected, mut lights) = (0, 0);
    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let text = std::fs::read_to_string(&path).expect("settings");
        for value in text
            .lines()
            .filter_map(|l| l.strip_prefix("PlacedObjects: "))
        {
            for o in PlacedObject::parse_list(value) {
                let slot = match o.object_type(ActiveMods::NONE) {
                    Some(PlacedObjectType::DeepProcessing) => {
                        if DeepProcessingData::parse(&o.data).is_none() {
                            rejected += 1;
                        }
                        0
                    }
                    Some(PlacedObjectType::LightFixture) => {
                        match LightFixtureData::parse(&o.data) {
                            Some(d) if d.kind == Some(LightFixtureType::DeepProcessing) => {
                                lights += 1;
                                1
                            }
                            Some(_) => continue,
                            None => {
                                rejected += 1;
                                continue;
                            }
                        }
                    }
                    Some(PlacedObjectType::GravityDisruptor) => 2,
                    _ => continue,
                };
                rooms[slot].insert(name.clone());
            }
        }
    }
    eprintln!(
        "deep processing rooms {}, light rooms {} ({lights} lights), disruptor rooms {}, {rejected} rejected",
        rooms[0].len(),
        rooms[1].len(),
        rooms[2].len()
    );
    assert_eq!(rejected, 0);
    assert_eq!(lights, 4);
    assert_eq!([rooms[0].len(), rooms[1].len(), rooms[2].len()], [18, 3, 3]);
}
