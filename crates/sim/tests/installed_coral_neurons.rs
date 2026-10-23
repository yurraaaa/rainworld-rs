//! The installed game's `CoralNeuron` placed objects (read-only): every `*_settings*.txt`
//! under the base regions' room folders, gates and arena levels is read, the objects are
//! counted per campaign after `Filter` (`LoadPlacedObjects`), parsed as
//! `ResizableObjectData`, built into neurons in their rooms (the segments finite, the ends
//! rooted on walls where a Solid tile is beside them) and run 100 ticks in view. Set
//! `RW_DIR` to the game install directory; skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::{
    ActiveMods, PlacedObject, PlacedObjectType, ResizableData, Room, RoomEffect, RoomEffectType,
    Timeline,
};
use rw_random::UnityRandom;
use rw_sim::world::AiMap;
use rw_sim::{
    CameraView, CoralNeuron, MyceliumEnv, MyceliumPool, NeuronCx, RoomExt, SystemState, Vec2,
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

    fn geometry(settings: &Path) -> Option<Room> {
        let name = settings.file_name()?.to_str()?;
        let stem = &name[..name.find("_settings")?];
        Room::load(settings.with_file_name(format!("{stem}.txt"))).ok()
    }

    fn room_name(settings: &Path) -> String {
        settings
            .file_name()
            .unwrap()
            .to_string_lossy()
            .split("_settings")
            .next()
            .unwrap()
            .to_owned()
    }
}

/// The `CoralNeuron` objects `Filter` leaves active for each campaign.
#[test]
fn filters_leave_each_campaign_its_neurons() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    for timeline in [Timeline::White, Timeline::Yellow, Timeline::Red] {
        let (mut rooms, mut objects) = (std::collections::BTreeSet::new(), 0);
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
                        && o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::CoralNeuron)
                })
                .count();
            if active > 0 {
                rooms.insert(Install::room_name(&path));
            }
            objects += active;
        }
        eprintln!(
            "{timeline:?}: {} rooms, {objects} active neurons",
            rooms.len()
        );
        assert!(objects > 0);
    }
}

/// Every neuron object of every room: parsed, built with the room's real geometry, run in
/// view for 100 ticks.
#[test]
fn every_neuron_builds_and_runs() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let (mut objects, mut parsed, mut rejected, mut built, mut too_short) = (0, 0, 0, 0, 0);
    let (mut rooms, mut rooted_ends, mut circles) = (std::collections::BTreeSet::new(), 0, 0);
    let mut lengths = Vec::new();
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
            continue;
        };
        let neurons: Vec<PlacedObject> = PlacedObject::parse_list(value)
            .into_iter()
            .filter(|o| o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::CoralNeuron))
            .collect();
        if neurons.is_empty() {
            continue;
        }
        let room = Install::geometry(&path).expect("geometry");
        let map = AiMap::new(&room, 0, false, false);
        let projector = text
            .lines()
            .find_map(|l| l.strip_prefix("Effects: "))
            .is_some_and(|v| {
                RoomEffect::parse_list(v).iter().any(|e| {
                    e.kind == RoomEffectType::SuperStructureProjector.value() && e.amount > 0.0
                })
            });
        let mut pool = MyceliumPool::default();
        let system = pool.create_registry();
        let mut rng = UnityRandom::new(7);
        let mut seq = 0;
        let mut made = Vec::new();
        let mut placed_circles = Vec::new();
        for o in &neurons {
            objects += 1;
            let Some(d) = ResizableData::parse(&o.data) else {
                rejected += 1;
                eprintln!("rejected: {} {:?}", path.display(), o.data);
                continue;
            };
            parsed += 1;
            let handle = Vec2::new(d.handle[0], d.handle[1]);
            let (pos, length) = (Vec2::from(o.pos), handle.length());
            lengths.push(length);
            let mut cx = NeuronCx {
                room: &room,
                system,
                pool: &mut pool,
                projector,
                circles: placed_circles
                    .iter()
                    .map(|c: &rw_sim::ProjectedCircle| c.update_order)
                    .collect(),
                seq: &mut seq,
            };
            match CoralNeuron::build(length, Some(pos), Some(pos + handle), &mut cx, &mut rng) {
                Some(b) => {
                    built += 1;
                    rooted_ends += usize::from(b.neuron.geometry.root_dir_a.is_some())
                        + usize::from(b.neuron.geometry.root_dir_b.is_some());
                    circles += b.circles.len();
                    placed_circles.extend(b.circles);
                    made.push(b.neuron);
                }
                None => {
                    too_short += 1;
                    eprintln!("too short ({length}): {}", path.display());
                }
            }
        }
        rooms.insert(Install::room_name(&path));
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
            system: Some(SystemState {
                id: system,
                frozen: false,
            }),
        };
        for neuron in &mut made {
            for end in [neuron.geometry.root_dir_a, neuron.geometry.root_dir_b]
                .into_iter()
                .flatten()
            {
                assert!((end.length() - 1.0).abs() < 1e-4);
            }
            for _ in 0..100 {
                neuron.update(Vec2::new(0.3, -0.2), &mut pool, &env, &mut rng);
            }
            for s in &neuron.geometry.segments {
                assert!(
                    s.iter().all(|v| v.x.is_finite() && v.y.is_finite()),
                    "{}",
                    path.display()
                );
                assert!(
                    !room.is_solid_at(s[0])
                        || neuron.geometry.root_dir_a.is_some()
                        || neuron.geometry.root_dir_b.is_some(),
                    "{} segment inside a wall",
                    path.display()
                );
            }
        }
    }
    lengths.sort_by(f32::total_cmp);
    eprintln!(
        "coral neurons: {} rooms, {objects} objects, {parsed} parsed, {rejected} rejected, {built} built, {too_short} too short, {rooted_ends} rooted ends, {circles} circles; lengths {:?}..{:?}",
        rooms.len(),
        lengths.first(),
        lengths.last()
    );
    assert_eq!(rejected, 0);
    assert_eq!(too_short, 0);
    assert_eq!(objects, parsed);
}
