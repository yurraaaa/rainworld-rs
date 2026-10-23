//! Placed `LightBeam` and `SpotLight` data in the installed game's rooms (read-only):
//! every entry parses. Set `RW_DIR` to the game install directory; skipped if absent.

use std::path::PathBuf;

use rw_assets::{ActiveMods, LightBeamData, PlacedObject, PlacedObjectType, QuadData};

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

/// Every placed `LightBeam` (20 in the survey, 9 rooms) and `SpotLight` (13) parses; all
/// beams have `sun` and no blink.
#[test]
fn every_beam_and_spot_parses() {
    let Some(files) = settings_files() else {
        return;
    };
    let (mut beams, mut spots, mut rejected) = (0, 0, 0);
    for path in files {
        let text = std::fs::read_to_string(&path).expect("settings");
        for value in text
            .lines()
            .filter_map(|l| l.strip_prefix("PlacedObjects: "))
        {
            for o in PlacedObject::parse_list(value) {
                match o.object_type(ActiveMods::NONE) {
                    Some(PlacedObjectType::LightBeam) => match LightBeamData::parse(&o.data) {
                        Some(d) => {
                            assert!(d.sun && !d.night_light && d.blink_rate == 0.0);
                            beams += 1;
                        }
                        None => rejected += 1,
                    },
                    Some(PlacedObjectType::SpotLight) => match QuadData::parse(&o.data) {
                        Some(_) => spots += 1,
                        None => rejected += 1,
                    },
                    _ => {}
                }
            }
        }
    }
    eprintln!("{beams} beams, {spots} spot lights, {rejected} rejected");
    assert_eq!(rejected, 0);
    assert!(beams >= 20 && spots >= 13);
}
