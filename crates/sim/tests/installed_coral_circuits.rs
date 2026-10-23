//! The installed game's `CoralCircuit` placed objects (read-only): every `*_settings*.txt`
//! under the base regions' room folders, gates and arena levels is read, the objects are
//! counted per campaign after `Filter` (`LoadPlacedObjects`), parsed as
//! `ResizableObjectData`, grouped into clusters and built into circuits in their rooms with
//! the real geometry (the order `AIMapReady` uses: the room's clusters one after the other,
//! sharing the pool and the update counter), then run 100 ticks in view. Set `RW_DIR` to
//! the game install directory; skipped if absent.

use std::path::{Path, PathBuf};

use rw_assets::{ActiveMods, PlacedObject, PlacedObjectType, ResizableData, Room, Timeline};
use rw_random::UnityRandom;
use rw_sim::world::AiMap;
use rw_sim::{
    BitCx, CameraView, CircuitCx, CircuitPlace, CoralCircuit, MyceliumEnv, MyceliumPool, Sounds,
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

    fn is_circuit(o: &PlacedObject) -> bool {
        o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::CoralCircuit)
    }
}

/// The circuit objects `Filter` leaves active for each campaign.
#[test]
fn filters_leave_each_campaign_its_circuits() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    for timeline in [Timeline::White, Timeline::Yellow, Timeline::Red] {
        let (mut rooms, mut objects, mut parsed, mut clusters) =
            (std::collections::BTreeSet::new(), 0, 0, 0);
        for path in Install::settings_files(&sa) {
            let text = std::fs::read_to_string(&path).expect("settings");
            let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
                continue;
            };
            let mut placed = PlacedObject::parse_list(value);
            PlacedObject::apply_filters(&mut placed, timeline, ActiveMods::NONE);
            let places: Vec<CircuitPlace> = placed
                .iter()
                .filter(|o| o.active && Install::is_circuit(o))
                .filter_map(|o| {
                    objects += 1;
                    ResizableData::parse(&o.data).map(|d| CircuitPlace {
                        pos: Vec2::from(o.pos),
                        rad: d.rad(),
                    })
                })
                .collect();
            if !places.is_empty() {
                rooms.insert(Install::room_name(&path));
            }
            parsed += places.len();
            clusters += CoralCircuit::clusters(places).len();
        }
        eprintln!(
            "{timeline:?}: {} rooms, {objects} active circuits ({parsed} parsed) in {clusters} clusters",
            rooms.len()
        );
        assert!(objects > 0);
        assert_eq!(objects, parsed);
    }
}

/// Every circuit object of every room: parsed, clustered, built with the room's real
/// geometry, run in view for 100 ticks (bits last to first after the circuit's push, then
/// the connectors and crabs), everything finite.
#[test]
fn every_circuit_builds_and_runs() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let (mut objects, mut parsed, mut rejected) = (0, 0, 0);
    let (mut rooms, mut clusters, mut tiles, mut bits) =
        (std::collections::BTreeSet::new(), 0, 0, 0);
    let (mut connectors, mut crabs, mut strands, mut empty, mut linked) = (0, 0, 0, 0, 0);
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
            continue;
        };
        let placed: Vec<PlacedObject> = PlacedObject::parse_list(value)
            .into_iter()
            .filter(Install::is_circuit)
            .collect();
        if placed.is_empty() {
            continue;
        }
        let room = Install::geometry(&path).expect("geometry");
        let map = AiMap::new(&room, 0, false, false);
        let mut places = Vec::new();
        for o in &placed {
            objects += 1;
            match ResizableData::parse(&o.data) {
                Some(d) => {
                    parsed += 1;
                    places.push(CircuitPlace {
                        pos: Vec2::from(o.pos),
                        rad: d.rad(),
                    });
                }
                None => {
                    rejected += 1;
                    eprintln!("rejected: {} {:?}", path.display(), o.data);
                }
            }
        }
        let mut pool = MyceliumPool::default();
        let system = pool.create_registry();
        let mut rng = UnityRandom::new(7);
        let mut seq = 0;
        let mut circuits = Vec::new();
        for cluster in CoralCircuit::clusters(places) {
            let stream = rng.state();
            let c = CoralCircuit::build(
                cluster,
                &mut CircuitCx {
                    room: &room,
                    ai_map: &map,
                    system,
                    pool: &mut pool,
                    seq: &mut seq,
                },
                &mut rng,
            );
            assert_eq!(
                rng.state(),
                stream,
                "{}: the stream is untouched",
                path.display()
            );
            clusters += 1;
            tiles += c.tiles.len();
            bits += c.bits.len();
            connectors += c.connectors.len();
            crabs += c.crabs.len();
            strands += c
                .connectors
                .iter()
                .map(|x| x.connections.len())
                .sum::<usize>();
            empty += usize::from(c.tiles.is_empty());
            linked += c.bits.iter().map(|b| b.neighbors.len()).sum::<usize>();
            circuits.push(c);
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
        let mut sounds = Sounds::default();
        for _ in 0..100 {
            for c in &mut circuits {
                c.finish_update();
                for k in (0..c.crabs.len()).rev() {
                    let CoralCircuit { bits, crabs, .. } = c;
                    crabs[k].update(bits, &mut rng);
                }
                for x in c.connectors.iter().rev() {
                    x.update(&c.bits, &mut pool, &env, &mut rng);
                }
                for i in (0..c.bits.len()).rev() {
                    c.update_bit(
                        i,
                        &mut BitCx {
                            room: &room,
                            ai_map: &map,
                            sounds: &mut sounds,
                            rng: &mut rng,
                        },
                    );
                }
            }
        }
        for c in &circuits {
            for b in &c.bits {
                assert!(
                    b.pos.x.is_finite() && b.pos.y.is_finite() && b.rotat.x.is_finite(),
                    "{}",
                    path.display()
                );
                assert!(
                    b.in_position(),
                    "{}: untouched bits stay put",
                    path.display()
                );
            }
        }
    }
    eprintln!(
        "coral circuits: {} rooms, {objects} objects, {parsed} parsed, {rejected} rejected, {clusters} clusters ({empty} without tiles), {tiles} tiles, {bits} bits ({linked} neighbour links), {connectors} connectors ({strands} strands), {crabs} crabs",
        rooms.len()
    );
    assert_eq!(rejected, 0);
    assert_eq!(objects, parsed);
}
