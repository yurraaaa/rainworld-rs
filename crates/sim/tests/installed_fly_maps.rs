//! The Fly's AI map in every SU room of the installed game (read-only): the baked
//! layer-6 exit distances as `CreatureSpecificAImap` decodes them, the Fly's accessible
//! tiles, `ExitDistanceForCreatureAndCheckNeighbours` on every tile for every node index,
//! `floorAltitude` and the Fly's outgoing paths. Set `RW_DIR` to the game install
//! directory; skipped if absent.
//!
//! The expectation is an independent Python reading of the decompile
//! (`rainworld-re/tools/overseer_model/fly_aimap.py`), its output copied here as
//! `tests/data/fly_maps_su_model.json` (`python3 fly_aimap.py out.json`): per room counts
//! and order-sensitive checksums.

use std::path::PathBuf;

use rw_assets::world::{
    GameRooms, LineFilter, Regions, RoomSource, StoryConditions, WorldData, WorldRequest,
};
use rw_assets::{ActiveMods, Room, StreamingAssets};
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::world::{AiMap, MovementType};
use serde_json::{Value, json};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const MODEL: &str = include_str!("data/fly_maps_su_model.json");
/// The checksums' modulus, 2^61 - 1.
const M: u128 = (1 << 61) - 1;

/// An order-sensitive checksum `s = s * k + v (mod M)`.
struct Sum {
    value: u128,
    factor: u128,
}

impl Sum {
    fn new(factor: u128) -> Self {
        Self { value: 0, factor }
    }

    fn add(&mut self, v: i64) {
        let v = u128::try_from(v).expect("non-negative term");
        self.value = (self.value * self.factor + v) % M;
    }

    fn json(&self) -> Value {
        json!(u64::try_from(self.value).expect("below 2^61"))
    }
}

/// One room's digest, in the model's format.
struct Digest;

impl Digest {
    fn of(room: &Room, map: &AiMap) -> Value {
        let fly = map.fly().expect("the Fly's map");
        let (w, h) = (room.width(), room.height());
        let mut grid = Sum::new(1_000_003);
        for &v in fly.grid() {
            grid.add(i64::from(v) + 7);
        }
        let nodes = usize::try_from(fly.number_of_nodes()).expect("nodes");
        let last = nodes.max(room.shortcut_map().nodes().len());
        let mut exits = Vec::new();
        for k in 0..=last {
            let node = i32::try_from(k).expect("node");
            let mut sum = Sum::new(31);
            let mut reach = 0;
            for x in 0..w {
                for y in 0..h {
                    let v = map.fly_exit_distance_and_check_neighbours((x, y), node);
                    if v > 0 {
                        reach += 1;
                    }
                    sum.add(i64::from(v) + 2);
                }
            }
            exits.push(json!([reach, sum.json()]));
        }
        let mut floor = Sum::new(31);
        let mut paths = Sum::new(31);
        let (mut standard, mut shortcut, mut npc) = (0, 0, 0);
        for x in 0..w {
            for y in 0..h {
                floor.add(i64::from(map.floor_altitude(x, y)));
                for c in map.fly_outgoing_paths(room, x, y) {
                    match c.kind {
                        MovementType::Standard => standard += 1,
                        MovementType::ShortCut => shortcut += 1,
                        MovementType::NpcTransportation => npc += 1,
                    }
                    if c.kind != MovementType::NpcTransportation {
                        paths.add(
                            i64::from(c.dest.0) * 1009
                                + i64::from(c.dest.1) * 7
                                + i64::from(c.distance),
                        );
                    }
                }
            }
        }
        let hives: Vec<usize> = room.hives().iter().map(Vec::len).collect();
        let (l, b, r, t) = fly.covered_area();
        json!({
            "size": [w, h], "nodes": fly.number_of_nodes(), "hives": hives,
            "accessible": fly.accessible_tiles().len(), "covered": [l, b, r, t],
            "layer_ints": fly.baked_ints(), "cells": fly.cells(), "grid_sum": grid.json(),
            "exit_distances": exits, "floor_sum": floor.json(),
            "links": {"Standard": standard, "ShortCut": shortcut, "NPCTransportation": npc},
            "paths_sum": paths.json(),
        })
    }
}

/// Every SU room the world file lists, the off-screen den left out.
fn su_rooms(sa: &StreamingAssets) -> (WorldData, GameRooms) {
    let campaign = &SlugcatCampaign::SURVIVOR;
    let filter = LineFilter {
        timeline: Some(campaign.timeline),
        mods: ActiveMods::NONE,
        conditions: StoryConditions::new_story(),
    };
    let regions = Regions::load(sa).expect("regions");
    let request = WorldRequest::for_first_room(sa, "SU_C04", filter);
    let data = WorldData::load(sa, &regions, &request).expect("world loads");
    let rooms = GameRooms {
        assets: sa.clone(),
        region: Some(request.name.to_lowercase()),
        timeline: campaign.timeline,
    };
    (data, rooms)
}

fn installed() -> Option<StreamingAssets> {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if sa.join("world").join("regions.txt").is_file() {
        Some(StreamingAssets::new(sa))
    } else {
        eprintln!("skipping: no world under {}", sa.display());
        None
    }
}

#[test]
fn su_fly_maps_match_reference_model() {
    let Some(sa) = installed() else { return };
    let model: Value = serde_json::from_str(MODEL).expect("model json");
    let (data, rooms) = su_rooms(&sa);
    let (mut compared, mut cells, mut accessible, mut tiles) = (0, 0, 0, 0);
    let mut mismatches = Vec::new();
    for r in data.rooms.iter().filter(|r| !r.offscreen_den) {
        let loaded = rooms.load_room(r.file_name()).expect("room loads");
        let map = AiMap::new(&loaded.room);
        let got = Digest::of(&loaded.room, &map);
        let want = &model["rooms"][&r.name];
        assert!(!want.is_null(), "{} is in the model", r.name);
        if &got != want {
            mismatches.push(format!("{}: port {got}\n model {want}", r.name));
        }
        compared += 1;
        cells += got["cells"].as_u64().expect("cells");
        accessible += got["accessible"].as_u64().expect("accessible");
        tiles += u64::try_from(loaded.room.width() * loaded.room.height()).expect("tiles");
        let fly = map.fly().expect("the Fly's map");
        assert_eq!(
            fly.cells(),
            fly.baked_ints(),
            "{}: the layer fills the grid",
            r.name
        );
    }
    println!(
        "SU fly maps: {compared} rooms, {tiles} tiles, {accessible} accessible, \
         {cells} baked cells, {} mismatching rooms",
        mismatches.len()
    );
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert_eq!(compared, model["rooms"].as_object().expect("rooms").len());
}

/// The comparison fails when one expectation is perturbed.
#[test]
fn su_fly_maps_detect_a_wrong_value() {
    let Some(sa) = installed() else { return };
    let model: Value = serde_json::from_str(MODEL).expect("model json");
    let (data, rooms) = su_rooms(&sa);
    let r = data.room_named("SU_A42").expect("SU_A42");
    let loaded = rooms.load_room(r.file_name()).expect("room loads");
    let got = Digest::of(&loaded.room, &AiMap::new(&loaded.room));
    let mut want = model["rooms"]["SU_A42"].clone();
    assert_eq!(got, want);
    want["exit_distances"][1][0] = json!(want["exit_distances"][1][0].as_u64().expect("reach") + 1);
    assert_ne!(got, want);
}
