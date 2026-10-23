//! The installed game's `CoralStem` and `CoralStemWithNeurons` placed objects (read-only):
//! every `*_settings*.txt` under the base regions' room folders, gates and arena levels is
//! read, the objects are counted per campaign after `Filter` (`LoadPlacedObjects`), parsed
//! as `ResizableObjectData`, built into stems in their rooms (the chunks finite, the
//! segments fed their neurons) and run 100 ticks in view. Set `RW_DIR` to the game install
//! directory; skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::{
    ActiveMods, PlacedObject, PlacedObjectType, ResizableData, Room, RoomEffect, RoomEffectType,
    Timeline,
};
use rw_random::UnityRandom;
use rw_sim::creature::IdIssuer;
use rw_sim::world::AiMap;
use rw_sim::{
    CameraView, CoralStem, MyceliumEnv, MyceliumPool, NeuronCx, RoomExt, StemCx, StemTick,
    SystemState, Vec2,
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

    fn is_stem(o: &PlacedObject) -> bool {
        matches!(
            o.object_type(ActiveMods::NONE),
            Some(PlacedObjectType::CoralStem | PlacedObjectType::CoralStemWithNeurons)
        )
    }
}

/// The stem objects `Filter` leaves active for each campaign.
#[test]
fn filters_leave_each_campaign_its_stems() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    for timeline in [Timeline::White, Timeline::Yellow, Timeline::Red] {
        let (mut rooms, mut objects, mut with_neurons) = (std::collections::BTreeSet::new(), 0, 0);
        for path in Install::settings_files(&sa) {
            let text = std::fs::read_to_string(&path).expect("settings");
            let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
                continue;
            };
            let mut placed = PlacedObject::parse_list(value);
            PlacedObject::apply_filters(&mut placed, timeline, ActiveMods::NONE);
            let active: Vec<&PlacedObject> = placed
                .iter()
                .filter(|o| o.active && Install::is_stem(o))
                .collect();
            if !active.is_empty() {
                rooms.insert(Install::room_name(&path));
            }
            objects += active.len();
            with_neurons += active
                .iter()
                .filter(|o| {
                    o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::CoralStemWithNeurons)
                })
                .count();
        }
        eprintln!(
            "{timeline:?}: {} rooms, {objects} active stems ({with_neurons} with neurons)",
            rooms.len()
        );
        assert!(objects > 0);
    }
}

/// Every stem object of every room: parsed, built with the room's real geometry (the
/// order `AIMapReady` uses: the room's stems one after the other, sharing the pool and
/// the update counter), run in view for 100 ticks, newest segment first.
#[test]
fn every_stem_builds_and_runs() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let (mut objects, mut parsed, mut rejected, mut built, mut empty) = (0, 0, 0, 0, 0);
    let (mut rooms, mut segments, mut neurons, mut circles) =
        (std::collections::BTreeSet::new(), 0, 0, 0);
    let (mut chunks, mut in_walls) = (0, 0);
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
            continue;
        };
        let placed: Vec<PlacedObject> = PlacedObject::parse_list(value)
            .into_iter()
            .filter(Install::is_stem)
            .collect();
        if placed.is_empty() {
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
        let mut ids = IdIssuer {
            next_issued_id: 1000,
        };
        let mut seq = 0;
        let mut stems = Vec::new();
        let mut made_neurons = Vec::new();
        let mut placed_circles: Vec<rw_sim::ProjectedCircle> = Vec::new();
        for o in &placed {
            objects += 1;
            let Some(d) = ResizableData::parse(&o.data) else {
                rejected += 1;
                eprintln!("rejected: {} {:?}", path.display(), o.data);
                continue;
            };
            parsed += 1;
            let stream = rng.state();
            let mut cx = StemCx {
                neuron: NeuronCx {
                    room: &room,
                    system,
                    pool: &mut pool,
                    projector,
                    circles: placed_circles.iter().map(|c| c.update_order).collect(),
                    new_vine_system: false,
                    seq: &mut seq,
                },
                ai_map: &map,
                ids: &mut ids,
                index: stems.len() as u32,
            };
            let with_neurons =
                o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::CoralStemWithNeurons);
            let b = CoralStem::build(Vec2::from(o.pos), d.rad(), with_neurons, &mut cx, &mut rng);
            assert_eq!(
                rng.state(),
                stream,
                "{}: the stream is untouched",
                path.display()
            );
            built += 1;
            if b.stem.segments.is_empty() {
                empty += 1;
                eprintln!("no segments: {}", path.display());
            }
            segments += b.stem.segments.len();
            neurons += b.neurons.len();
            circles += b.circles.len();
            placed_circles.extend(b.circles);
            made_neurons.extend(b.neurons);
            stems.push(b.stem);
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
        let tick = StemTick {
            room: &room,
            ai_map: &map,
            water: None,
            wind: Vec2::new(0.3, -0.2),
            room_gravity: 1.0,
        };
        for stem in &mut stems {
            for _ in 0..100 {
                for k in (0..stem.segments.len()).rev() {
                    stem.segment_physics(k, false, &tick);
                    let neuron = stem.segments[k]
                        .neuron
                        .and_then(|o| made_neurons.iter_mut().find(|n| n.update_order == o));
                    stem.segment_after_physics(k, neuron, &mut pool, &env, &tick, &mut rng);
                }
            }
            for g in &stem.segments {
                for c in &g.body.chunks {
                    assert!(
                        c.pos.x.is_finite() && c.pos.y.is_finite(),
                        "{}",
                        path.display()
                    );
                    in_walls += usize::from(room.is_solid_at(c.pos));
                    chunks += 1;
                }
            }
        }
    }
    eprintln!(
        "coral stems: {} rooms, {objects} objects, {parsed} parsed, {rejected} rejected, {built} built ({empty} without segments), {segments} segments, {neurons} neurons, {circles} circles; {in_walls} of {chunks} chunks end in a Solid tile (the rod pulls after the collision)",
        rooms.len()
    );
    assert_eq!(rejected, 0);
    assert_eq!(objects, parsed);
    assert_eq!(empty, 0);
}
