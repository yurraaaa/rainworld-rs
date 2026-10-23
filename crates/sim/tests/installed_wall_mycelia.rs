//! The installed game's `WallMycelia` placed objects (read-only): the rooms whose
//! settings place them are counted (every `*_settings*.txt` under the base regions' room
//! folders, gates and arena levels), each room's objects are read as `ResizableObjectData`
//! and built into one `WallMycelia` (the strands' roots on walls, their points finite
//! while the strands run 100 ticks in view). Set `RW_DIR` to the game install directory;
//! skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::{ActiveMods, PlacedObject, PlacedObjectType, ResizableData, Room, Timeline};
use rw_random::UnityRandom;
use rw_sim::world::AiMap;
use rw_sim::{CameraView, MyceliumEnv, MyceliumPool, RoomExt, Vec2, WallMycelia, WallPlace};

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

/// The rooms whose `WallMycelia` objects `Filter` leaves active for each campaign's
/// timeline (`LoadPlacedObjects`): fewer than the 29 rooms that place them.
#[test]
fn filters_leave_each_campaign_its_rooms() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let mut counts = Vec::new();
    for timeline in [Timeline::White, Timeline::Yellow, Timeline::Red] {
        let (mut rooms, mut objects) = (0, 0);
        for path in Install::settings_files(&sa) {
            let text = std::fs::read_to_string(&path).expect("settings");
            let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
                continue;
            };
            let mut placed = PlacedObject::parse_list(value);
            PlacedObject::apply_filters(&mut placed, timeline, ActiveMods::NONE);
            let active = placed
                .iter()
                .filter(|o| {
                    o.active
                        && o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::WallMycelia)
                })
                .count();
            rooms += usize::from(active > 0);
            objects += active;
        }
        eprintln!("{timeline:?}: {rooms} rooms, {objects} active wall mycelia objects");
        counts.push((rooms, objects));
    }
    // SS_E05 and SS_D05 filter theirs out for the Survivor (observed in play).
    assert!(counts[0].0 < 29 && counts[0].0 > 0, "{counts:?}");
}

#[test]
fn every_wall_mycelia_room_builds_and_runs() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let (mut files, mut objects, mut parsed, mut rejected) = (0, 0, 0, 0);
    let mut rooms = std::collections::BTreeSet::new();
    let mut strands_total = 0;
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
            continue;
        };
        let mut places = Vec::new();
        for o in PlacedObject::parse_list(value) {
            if o.object_type(ActiveMods::NONE) != Some(PlacedObjectType::WallMycelia) {
                continue;
            }
            objects += 1;
            match ResizableData::parse(&o.data) {
                Some(d) => {
                    parsed += 1;
                    if o.active {
                        places.push(WallPlace {
                            pos: Vec2::from(o.pos),
                            rad: d.rad(),
                        });
                    }
                }
                None => {
                    rejected += 1;
                    eprintln!("rejected: {} {:?}", path.display(), o.data);
                }
            }
        }
        if objects == 0 || places.is_empty() {
            continue;
        }
        files += 1;
        rooms.insert(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .split("_settings")
                .next()
                .unwrap()
                .to_owned(),
        );
        let room = Install::geometry(&path).expect("geometry");
        let mut pool = MyceliumPool::default();
        pool.create_registry(0);
        let mut rng = UnityRandom::new(7);
        let wall = WallMycelia::new(1, places, &room, 0, &mut pool, &mut rng);
        assert!(!wall.mycelia.is_empty(), "{}", path.display());
        strands_total += wall.mycelia.len();
        for (i, root) in wall.roots.root_positions.iter().enumerate() {
            let on_wall = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .any(|&(dx, dy)| room.is_solid_at(*root + Vec2::new(dx, dy)));
            assert!(on_wall, "{} strand {i} at {root:?}", path.display());
            assert!((wall.roots.directions[i].length() - 1.0).abs() < 1e-4);
        }
        let map = AiMap::new(&room, 0, false, false);
        let env = MyceliumEnv {
            room: &room,
            room_index: 0,
            ai_map: Some(&map),
            camera: CameraView {
                room: 0,
                current: 0,
                next: 0,
            },
            cameras_changed_tick: 1,
            frozen: Some(false),
        };
        let mut wall = wall;
        for _ in 0..100 {
            wall.update(&mut pool, &env, &mut rng);
        }
        for (i, &id) in wall.mycelia.iter().enumerate() {
            let m = pool.get(id).expect("strand");
            assert!(m.length >= 20.0 - 1e-3 && m.length < 400.0, "{}", m.length);
            for p in &m.points {
                assert!(
                    p.iter().all(|v| v.x.is_finite() && v.y.is_finite()),
                    "{} strand {i}",
                    path.display()
                );
            }
            if !m.culled {
                assert_eq!(m.base(), wall.roots.root_positions[i]);
            }
        }
    }
    eprintln!(
        "wall mycelia: {} rooms, {files} settings files, {objects} objects, {parsed} parsed, {rejected} rejected, {strands_total} strands",
        rooms.len()
    );
    assert_eq!(rejected, 0);
    assert_eq!(rooms.len(), 29, "{rooms:?}");
    assert_eq!(objects, parsed);
    assert_eq!(objects, 121);
}
