//! Loads every vanilla region's world file from the installed game (read-only), as each
//! base-game timeline. The oracle is the world files themselves: room lists and
//! connections are compared with the `ROOMS` lines as written. Set `RW_DIR` to the game
//! install directory; skipped if the files are absent. Only `world/` is read, never
//! `mergedmods/` or `mods/`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rw_assets::world::{
    LineFilter, PREPROCESSING_GENERATION, Regions, Rejection, StoryConditions, WorldData,
    WorldRequest,
};
use rw_assets::{ActiveMods, NodeType, Room, StreamingAssets, Timeline};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed(StreamingAssets);

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(Self(StreamingAssets::new(sa)))
        } else {
            eprintln!("skipping: no world/regions.txt under {}", sa.display());
            None
        }
    }

    fn filter(timeline: Timeline) -> LineFilter {
        LineFilter {
            timeline: Some(timeline),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        }
    }

    fn world(&self, regions: &Regions, region: &str, timeline: Timeline) -> WorldData {
        let request = WorldRequest {
            name: region.to_owned(),
            single_room: false,
            filter: Self::filter(timeline),
        };
        WorldData::load(&self.0, regions, &request)
            .unwrap_or_else(|e| panic!("world {region} as {}: {e}", timeline.value()))
    }

    /// The names of the region's broken shelters for `timeline`.
    fn broken(&self, regions: &Regions, region: &str, timeline: Timeline) -> Vec<String> {
        let w = self.world(regions, region, timeline);
        w.shelters
            .iter()
            .zip(&w.broken_shelters)
            .filter(|(_, b)| **b)
            .map(|(&s, _)| w.room(s).expect("shelter").name.clone())
            .collect()
    }

    /// The `ROOMS` lines of a world file as written: name and connection names.
    fn rooms_as_written(&self, region: &str) -> Vec<(String, Vec<String>)> {
        let lower = region.to_lowercase();
        let path = self.0.file(&format!("world/{lower}/world_{lower}.txt"));
        let text = std::fs::read_to_string(&path).expect("world file reads");
        text.lines()
            .skip_while(|l| *l != "ROOMS")
            .skip(1)
            .take_while(|l| *l != "END ROOMS")
            .filter(|l| l.contains(" : ") && !l.starts_with("//"))
            .map(|l| {
                let mut parts = l.split(" : ");
                let name = parts.next().unwrap_or_default().to_owned();
                let conns = parts
                    .next()
                    .unwrap_or_default()
                    .split(',')
                    .map(|c| c.trim().to_owned())
                    .filter(|c| !c.is_empty())
                    .collect();
                (name, conns)
            })
            .collect()
    }
}

/// Every region of `regions.txt` loads as White, Yellow and Red: one abstract room per
/// `ROOMS` line plus the off-screen den (`NumberOfRoomsInRegion`), dropped lines only
/// blank, comments or `CREATURES` lines filtered by timeline, every connection
/// resolved, and every connection answered by the room behind it.
#[test]
fn every_vanilla_region_loads() {
    let Some(game) = Installed::open() else {
        return;
    };
    let regions = Regions::load(&game.0).expect("regions.txt");
    let names: Vec<&str> = regions.0.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "CC", "DS", "HI", "GW", "SI", "SU", "SH", "SL", "LF", "UW", "SB", "SS"
        ]
    );
    let mut total_rooms = 0;
    for timeline in [Timeline::White, Timeline::Yellow, Timeline::Red] {
        println!("as {}:", timeline.value());
        for region in &regions.0 {
            let world = game.world(&regions, &region.name, timeline);
            assert_eq!(
                i32::try_from(world.rooms.len()).expect("fits"),
                region.number_of_rooms,
                "{}: one room per ROOMS line, plus the off-screen den",
                region.name
            );
            let den = world.rooms.last().expect("the off-screen den");
            assert!(den.offscreen_den);
            assert_eq!(den.name, format!("OffScreenDen_{}", region.name));
            let first = world.first_room_index();
            assert_eq!(first, region.first_room_index);
            for (i, room) in world.rooms.iter().enumerate() {
                assert_eq!(room.index, first + i32::try_from(i).expect("fits"));
            }
            let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
            for r in &world.report.rejected {
                let kind = match &r.reason {
                    Rejection::Comment => "comment".to_owned(),
                    Rejection::Timeline(t) => format!("({t})"),
                    other => panic!("{}: line {} {:?}: {other}", region.name, r.line, r.text),
                };
                *reasons.entry(kind).or_default() += 1;
            }
            let connections: usize = world
                .rooms
                .iter()
                .map(|r| r.connections.iter().filter(|&&c| c > -1).count())
                .sum();
            let mut one_way = Vec::new();
            for room in &world.rooms {
                for &c in room.connections.iter().filter(|&&c| c > -1) {
                    let other = &world.rooms[usize::try_from(c - first).expect("in world")];
                    if other.exit_index(room.index) < 0 {
                        one_way.push(format!("{} -> {}", room.name, other.name));
                    }
                }
            }
            println!(
                "  {}: {} rooms, {connections} connections, rejected {reasons:?}, capped {:?}, one-way {one_way:?}",
                region.name,
                world.rooms.len(),
                world.report.capped_exits,
            );
            assert!(one_way.is_empty(), "{}: {one_way:?}", region.name);
            total_rooms += world.rooms.len();
        }
    }
    println!("{total_rooms} rooms over three timelines");
}

/// Outskirts as the Survivor: the room list, in order, and the connections equal the
/// `ROOMS` lines of `world_su.txt`; `DISCONNECTED` is -1.
#[test]
fn outskirts_rooms_and_connections_match_the_file() {
    let Some(game) = Installed::open() else {
        return;
    };
    let regions = Regions::load(&game.0).expect("regions.txt");
    let world = game.world(&regions, "SU", Timeline::White);
    let written = game.rooms_as_written("SU");
    assert_eq!(world.rooms.len(), written.len() + 1);
    let mut checked = 0;
    for (room, (name, conns)) in world.rooms.iter().zip(&written) {
        assert_eq!(&room.name, name);
        let named: Vec<String> = room
            .connections
            .iter()
            .map(|&c| {
                world
                    .room(c)
                    .map_or_else(|| "DISCONNECTED".to_owned(), |r| r.name.clone())
            })
            .collect();
        // `CappingBrokenExits` only adds -1 exits past the written ones.
        assert_eq!(&named[..conns.len()], &conns[..], "{name}");
        checked += conns.len();
    }
    println!(
        "SU: {} rooms, {checked} connections as written",
        written.len()
    );
    // The path from the Survivor's start to the first shelter, both ways.
    for (room, conns) in [
        ("SU_C04", vec!["SU_A41"]),
        ("SU_A41", vec!["SU_A42", "SU_A43", "SU_C04", "SU_A44"]),
        ("SU_A43", vec!["SU_A41", "SU_A22"]),
        ("SU_A22", vec!["SU_A43", "SU_A23", "SU_S01"]),
        ("SU_S01", vec!["SU_A22"]),
    ] {
        let r = world.room_named(room).expect(room);
        let names: Vec<&str> = r
            .connections
            .iter()
            .filter_map(|&c| world.room(c))
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(names, conns, "{room}");
        for c in &r.connections {
            let back = world.room(*c).expect("connected");
            assert!(back.exit_index(r.index) >= 0, "{} -> {room}", back.name);
        }
    }
    let shelter = world.room_named("SU_S01").expect("shelter");
    assert!(shelter.shelter());
    assert_eq!(shelter.nodes[0].kind, NodeType::Exit);
}

/// Every room the vanilla worlds list was baked at `preProcessingGeneration` or later,
/// so the game uses the baked nodes (only unlisted files such as `cc_b07` are older,
/// and would be re-preprocessed).
#[test]
fn room_file_generations() {
    let Some(game) = Installed::open() else {
        return;
    };
    let regions = Regions::load(&game.0).expect("regions.txt");
    let mut old = BTreeMap::<String, usize>::new();
    let mut total = 0;
    for region in &regions.0 {
        let world = game.world(&regions, &region.name, Timeline::White);
        for room in world.rooms.iter().filter(|r| !r.offscreen_den) {
            let file = room.file_name().to_lowercase();
            let path = game.0.find_room_file(&file, ".txt").expect("room file");
            let geometry = Room::load(&path).expect("room parses");
            total += 1;
            if geometry.conn_map().expect("node map parses").generation < PREPROCESSING_GENERATION {
                *old.entry(region.name.clone()).or_default() += 1;
            }
        }
    }
    println!("{total} room files; generation < {PREPROCESSING_GENERATION}: {old:?}");
    assert!(old.is_empty());
}

/// What the world processes read: the properties files' `Broken Shelters` lines (only
/// for the named timeline), SU's lone bat migration blockage and `locks.txt`.
#[test]
fn broken_shelters_blockages_and_locks() {
    let Some(game) = Installed::open() else {
        return;
    };
    let regions = Regions::load(&game.0).expect("regions");
    let none = Vec::<String>::new();
    assert_eq!(game.broken(&regions, "SL", Timeline::White), ["SL_S11"]);
    assert_eq!(game.broken(&regions, "SL", Timeline::Red), ["SL_S11"]);
    assert_eq!(game.broken(&regions, "SL", Timeline::Yellow), none);
    assert_eq!(game.broken(&regions, "HI", Timeline::Red), ["HI_S03"]);
    assert_eq!(game.broken(&regions, "HI", Timeline::White), none);
    assert_eq!(game.broken(&regions, "LF", Timeline::Red), ["LF_S07"]);
    let su = game.world(&regions, "SU", Timeline::White);
    let a22 = su.room_named("SU_A22").expect("SU_A22").index;
    assert_eq!(su.bat_migration_blockages, [[-1, a22]]);
    let su_hi = su
        .gate_locks
        .iter()
        .find(|l| l[0] == "GATE_SU_HI")
        .expect("lock");
    assert_eq!(su_hi, &["GATE_SU_HI", "3", "2"]);
}
