//! Placed `LightFixture` data in the installed game's rooms (read-only): every entry
//! parses; the census per kind and the red lights that read the room's electric power.
//! Set `RW_DIR` to the game install directory; skipped if absent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rw_assets::{
    ActiveMods, LightFixtureData, LightFixtureType, PlacedObject, PlacedObjectType, RoomEffectType,
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

    /// The `BrokenZeroG` amount of a settings file's `Effects:` line, 0 without one.
    fn broken_zero_g(text: &str) -> f32 {
        let Some(line) = text.lines().find_map(|l| l.strip_prefix("Effects: ")) else {
            return 0.0;
        };
        line.split(", ")
            .filter_map(|e| {
                let mut f = e.split('-');
                (f.next()? == RoomEffectType::BrokenZeroG.value())
                    .then(|| f.next()?.parse::<f32>().ok())
                    .flatten()
            })
            .fold(0.0, f32::max)
    }
}

/// `RainWorld.LoadResources` loads `Atlases/LightMask0` as the atlas "LightMask0", the
/// `RedlightLight`'s element.
#[test]
fn light_mask_0_texture_loads() {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let data = root.join("RainWorld_Data");
    if !data.is_dir() {
        eprintln!("skipping: no game under {}", root.display());
        return;
    }
    let resources = rw_assets::UnityResources::open(&data).expect("resources.assets");
    let image = resources
        .texture_image("Atlases/LightMask0")
        .expect("LightMask0");
    let texture: rw_assets::unity::Texture2D =
        resources.load("Atlases/LightMask0").expect("LightMask0");
    eprintln!(
        "LightMask0: {}x{}, filter {} wrap {:?}, centre {:?}, corner {:?}",
        image.width(),
        image.height(),
        texture.filter_mode,
        texture.wrap,
        image.pixel(image.width() / 2, image.height() / 2),
        image.pixel(0, 0),
    );
}

/// Every `LightFixture` of the base game's settings files parses (the game would throw
/// on any that does not). The census is over every settings file, unreachable rooms
/// included.
#[test]
fn every_placed_light_fixture_parses() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let (mut read, mut rejected) = (0, 0);
    let (mut gravity_dependent, mut rooms_with) = (0, 0);
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let broken = Install::broken_zero_g(&text);
        let mut here = false;
        for value in text
            .lines()
            .filter_map(|l| l.strip_prefix("PlacedObjects: "))
        {
            for o in PlacedObject::parse_list(value)
                .iter()
                .filter(|o| o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::LightFixture))
            {
                let Some(d) = LightFixtureData::parse(&o.data) else {
                    rejected += 1;
                    continue;
                };
                read += 1;
                here = true;
                let name = d.kind.map_or("(unregistered)", LightFixtureType::value);
                *kinds.entry(name.to_owned()).or_default() += 1;
                gravity_dependent += usize::from(
                    d.kind == Some(LightFixtureType::RedLight) && broken > 0.0 && d.random_seed > 0,
                );
            }
        }
        rooms_with += usize::from(here);
    }
    eprintln!(
        "{read} light fixtures read, {rejected} rejected, in {rooms_with} settings files; \
         {gravity_dependent} red lights gravity-dependent (BrokenZeroG > 0, seed > 0); {kinds:?}"
    );
    assert_eq!(rejected, 0);
}
