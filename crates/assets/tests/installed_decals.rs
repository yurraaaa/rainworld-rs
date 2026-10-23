//! Placed `CustomDecal` and `Rainbow` data in the installed game's settings files
//! (read-only), the decal PNGs they name and the `Rainbow` texture. Set `RW_DIR` to the
//! game install directory; skipped if absent.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rw_assets::{CustomDecalData, PlacedObject, RainbowData, StreamingAssets, UnityResources};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Install;

impl Install {
    fn root() -> PathBuf {
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from)
    }

    fn streaming_assets() -> Option<PathBuf> {
        let sa = Self::root().join("RainWorld_Data").join("StreamingAssets");
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

    /// The placed objects of type `kind` in every settings file: (file, object).
    fn objects(sa: &Path, kind: &str) -> Vec<(PathBuf, PlacedObject)> {
        let mut out = Vec::new();
        for path in Self::settings_files(sa) {
            let text = std::fs::read_to_string(&path).expect("settings");
            for value in text
                .lines()
                .filter_map(|l| l.strip_prefix("PlacedObjects: "))
            {
                out.extend(
                    PlacedObject::parse_list(value)
                        .into_iter()
                        .filter(|o| o.kind == kind)
                        .map(|o| (path.clone(), o)),
                );
            }
        }
        out
    }
}

/// Every placed `CustomDecal` parses and names a decal file that exists and decodes.
#[test]
fn every_custom_decal_parses_and_its_image_loads() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let assets = StreamingAssets::new(sa.clone());
    let (mut read, mut rejected, mut with_vertices) = (0, 0, 0);
    let mut images = BTreeSet::new();
    let mut rooms = BTreeSet::new();
    for (path, o) in Install::objects(&sa, "CustomDecal") {
        match CustomDecalData::parse(&o.data) {
            Some(d) => {
                read += 1;
                with_vertices += usize::from(o.data.split('~').count() >= 20);
                images.insert(d.image_name);
                rooms.insert(path);
            }
            None => {
                rejected += 1;
                eprintln!("rejected {}: {:?}", path.display(), o.data);
            }
        }
    }
    let mut missing = Vec::new();
    for image in &images {
        if CustomDecalData::load_image(&assets, image).is_err() {
            missing.push(image.clone());
        }
    }
    eprintln!(
        "{read} CustomDecals read ({with_vertices} with vertices), {rejected} rejected, in {} \
         settings files, {} distinct images, {} missing: {missing:?}",
        rooms.len(),
        images.len(),
        missing.len()
    );
    assert_eq!(rejected, 0);
    assert!(missing.is_empty());
    assert!(read > 900);
}

/// Every placed `Rainbow` parses; the `Rainbow` texture is in the game's resources.
#[test]
fn every_rainbow_parses_and_the_texture_loads() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let objects = Install::objects(&sa, "Rainbow");
    let mut read = 0;
    for (path, o) in &objects {
        let d = RainbowData::parse(&o.data)
            .unwrap_or_else(|| panic!("{}: {:?}", path.display(), o.data));
        assert_eq!(d.fades.len(), 6);
        read += 1;
    }
    eprintln!("{read} of {} Rainbows read", objects.len());
    assert_eq!(read, objects.len());
    assert!(read >= 24);
    let resources =
        UnityResources::open(Install::root().join("RainWorld_Data")).expect("resources.assets");
    for name in ["Atlases/Rainbow", "Atlases/rainbow"] {
        let image = resources.texture_image(name).expect(name);
        let texture: rw_assets::unity::Texture2D = resources.load(name).expect(name);
        eprintln!(
            "{name}: {}x{}, filter {} wrap {:?}",
            image.width(),
            image.height(),
            texture.filter_mode,
            texture.wrap
        );
    }
}
