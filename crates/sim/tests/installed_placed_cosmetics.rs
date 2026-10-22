//! The installed game's `CosmeticSlimeMold`, `CosmeticSlimeMold2`, `SlimeMold` and
//! `DeadTokenStalk` placed objects (read-only): every line parses as
//! `ResizableObjectData` would, each slime mold builds on its room's geometry, and each
//! dead stalk updates 200 ticks with two shared draws a tick (more on a new pose) and
//! finite positions. Set `RW_DIR` to the game install directory; skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::{ActiveMods, PlacedObject, PlacedObjectType, ResizableData, Room};
use rw_random::UnityRandom;
use rw_sim::{CosmeticSlimeMold, TokenStalk, Vec2};

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

    /// The geometry file beside a settings file (`x_settings-white.txt` -> `x.txt`).
    fn geometry(settings: &Path) -> Option<Room> {
        let name = settings.file_name()?.to_str()?;
        let stem = &name[..name.find("_settings")?];
        Room::load(settings.with_file_name(format!("{stem}.txt"))).ok()
    }
}

#[derive(Default, Debug)]
struct Census {
    molds: usize,
    molds2: usize,
    items: usize,
    item_patches: usize,
    stalks: usize,
    rejected: usize,
    dots: usize,
}

#[test]
fn placed_slime_molds_and_dead_stalks() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let mut c = Census::default();
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
            continue;
        };
        let objects = PlacedObject::parse_list(value);
        let kinds = [
            PlacedObjectType::CosmeticSlimeMold,
            PlacedObjectType::CosmeticSlimeMold2,
            PlacedObjectType::SlimeMold,
            PlacedObjectType::DeadTokenStalk,
        ];
        if !objects.iter().any(|o| {
            o.object_type(ActiveMods::NONE)
                .is_some_and(|t| kinds.contains(&t))
        }) {
            continue;
        }
        let room = Install::geometry(&path)
            .unwrap_or_else(|| panic!("no geometry for {}", path.display()));
        let mut rng = UnityRandom::new(1);
        for o in &objects {
            let pos = Vec2::from(o.pos);
            match o.object_type(ActiveMods::NONE) {
                Some(
                    t
                    @ (PlacedObjectType::CosmeticSlimeMold | PlacedObjectType::CosmeticSlimeMold2),
                ) => {
                    let Some(d) = ResizableData::parse(&o.data) else {
                        c.rejected += 1;
                        continue;
                    };
                    let through = t == PlacedObjectType::CosmeticSlimeMold2;
                    let before = rng.state();
                    let mold = CosmeticSlimeMold::new(0, &room, pos, d.rad(), through, &mut rng);
                    assert_eq!(rng.state(), before);
                    assert!(
                        mold.positions
                            .iter()
                            .all(|p| p.x.is_finite() && p.y.is_finite())
                    );
                    c.dots += mold.positions.len();
                    if through {
                        c.molds2 += 1;
                    } else {
                        c.molds += 1;
                    }
                }
                Some(PlacedObjectType::SlimeMold) => {
                    c.items += 1;
                    if let Some(rad) = CosmeticSlimeMold::placed_item_rad(pos, &mut rng) {
                        let mold = CosmeticSlimeMold::new(0, &room, pos, rad, false, &mut rng);
                        c.dots += mold.positions.len();
                        c.item_patches += 1;
                    }
                }
                Some(PlacedObjectType::DeadTokenStalk) => {
                    let Some(d) = ResizableData::parse(&o.data) else {
                        c.rejected += 1;
                        continue;
                    };
                    let base = pos + Vec2::from(d.handle);
                    let mut s = TokenStalk::new(0, pos, base, &mut rng);
                    assert!(!s.coord.is_empty(), "{}", path.display());
                    for _ in 0..200 {
                        let before = rng.state();
                        s.update(&room, &mut rng);
                        let mut two = UnityRandom::from_state(before);
                        two.value();
                        two.value();
                        let mut six = UnityRandom::from_state(two.state());
                        for _ in 0..4 {
                            six.value();
                        }
                        assert!(rng.state() == two.state() || rng.state() == six.state());
                    }
                    assert!(
                        s.coord
                            .iter()
                            .all(|k| k.pos.x.is_finite() && k.pos.y.is_finite())
                            && s.head.x.is_finite(),
                        "{}",
                        path.display()
                    );
                    c.stalks += 1;
                }
                _ => {}
            }
        }
    }
    eprintln!("{c:?}");
    assert_eq!(c.rejected, 0);
    assert!(c.molds > 0 && c.molds2 > 0 && c.stalks >= 46 && c.items > 0);
}
