//! `OverseersWorldAI`'s shelter and direction finders and the flies' swarm room map on a
//! new Survivor game in SU, and the baked node connectivity they read, in the installed
//! game (read-only). Set `RW_DIR` to the game install directory; skipped if absent.
//!
//! The expectations are an independent Python model of the decompiled game
//! (`rainworld-re/tools/overseer_model/world_ai_model.py`, `world_ai.py`), its output
//! copied here as `tests/data/world_ai_su_model.json` (`python3 world_ai_model.py
//! out.json`), made with the fly's connectivity layer at 6 (`FLY_PBI`; the room files
//! give the bat hives paths only there), and connectivity cells read by hand from
//! `su_a42.txt`.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::world::{GameRooms, LineFilter, Regions, StoryConditions, WorldData, WorldRequest};
use rw_assets::{ActiveMods, CreatureType, StreamingAssets};
use rw_random::UnityRandom;
use rw_sim::RainWorldGame;
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::creature::CreatureTemplate;
use rw_sim::world::{DirectionFinder, ShelterFinder, StoryProgress, World};
use serde_json::Value;

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const MODEL: &str = include_str!("data/world_ai_su_model.json");

struct Installed(StreamingAssets);

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(Self(StreamingAssets::new(sa)))
        } else {
            eprintln!("skipping: no world under {}", sa.display());
            None
        }
    }

    /// SU as a new Survivor game loads it, population and `FliesWorldAI` included.
    fn su(&self) -> World {
        let campaign = &SlugcatCampaign::SURVIVOR;
        let filter = LineFilter {
            timeline: Some(campaign.timeline),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        };
        let regions = Regions::load(&self.0).expect("regions");
        let request = WorldRequest::for_first_room(&self.0, "SU_C04", filter);
        let data = WorldData::load(&self.0, &regions, &request).expect("world loads");
        let rooms = Arc::new(GameRooms {
            assets: self.0.clone(),
            region: Some(request.name.to_lowercase()),
            timeline: campaign.timeline,
        });
        let mut world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
        RainWorldGame::new_game_world(&mut world, campaign, &mut UnityRandom::new(42));
        world
    }
}

/// Cells compared and those that differ, with the first few differences.
#[derive(Default)]
struct Tally {
    cells: usize,
    mismatches: usize,
    first: Vec<String>,
}

impl Tally {
    fn check(&mut self, what: impl FnOnce() -> String, port: f32, model: f32) {
        self.cells += 1;
        if port != model {
            self.mismatches += 1;
            if self.first.len() < 20 {
                self.first
                    .push(format!("{}: port {port}, model {model}", what()));
            }
        }
    }

    fn flag(&mut self, what: &str, same: bool) {
        self.check(|| what.to_owned(), f32::from(u8::from(same)), 1.0);
    }

    /// A `{room: [value per exit]}` model matrix (missing rooms all -1) against the
    /// port's `[local room][exit]`.
    fn matrix(&mut self, label: &str, data: &WorldData, port: &[Vec<f32>], model: &Value) {
        for (i, row) in port.iter().enumerate() {
            let name = &data.rooms[i].name;
            let expected = model.get(name.as_str()).and_then(Value::as_array);
            for (j, &v) in row.iter().enumerate() {
                let m = expected.map_or(-1.0, |e| e[j].as_f64().expect("number") as f32);
                self.check(|| format!("{label} {name}[{j}]"), v, m);
            }
        }
    }

    fn check_next(&mut self, label: &str, data: &WorldData, port: &[(usize, i32)], model: &Value) {
        let port: Vec<(String, i64)> = port
            .iter()
            .map(|&(r, n)| (data.rooms[r].name.clone(), i64::from(n)))
            .collect();
        let model: Vec<(String, i64)> = model
            .as_array()
            .expect("checkNext")
            .iter()
            .map(|c| {
                (
                    c[0].as_str().expect("room").to_owned(),
                    c[1].as_i64().expect("node"),
                )
            })
            .collect();
        self.flag(
            &format!("{label} checkNext {port:?} vs {model:?}"),
            port == model,
        );
    }

    fn shelter_finder(&mut self, k: i64, data: &WorldData, port: &ShelterFinder, model: &Value) {
        self.flag(
            &format!("k={k} shelter done"),
            port.done == model["done"].as_bool().unwrap(),
        );
        self.flag(
            &format!("k={k} currentlyMappingShelter"),
            i64::from(port.currently_mapping_shelter())
                == model["currentlyMappingShelter"].as_i64().unwrap(),
        );
        self.check_next(
            &format!("k={k} shelter"),
            data,
            port.check_next(),
            &model["checkNext"],
        );
        for (s, matrix) in port.matrix().iter().enumerate() {
            let shelter = &data.room(data.shelters[s]).expect("shelter").name;
            let label = format!("k={k} shelter {shelter}");
            self.matrix(&label, data, matrix, &model["matrix"][shelter.as_str()]);
        }
    }

    fn direction_finder(
        &mut self,
        k: i64,
        data: &WorldData,
        port: Option<&DirectionFinder>,
        model: &Value,
    ) {
        let (Some(port), false) = (port, model.is_null()) else {
            self.flag(
                &format!("k={k} direction finder exists"),
                port.is_none() && model.is_null(),
            );
            return;
        };
        self.flag(
            &format!("k={k} direction done"),
            port.done == model["done"].as_bool().unwrap(),
        );
        self.flag(
            &format!("k={k} showToRoom"),
            i64::from(port.show_to_room) == model["showToRoom"].as_i64().unwrap(),
        );
        self.flag(
            &format!("k={k} minKarma"),
            i64::from(port.min_karma) == model["minKarma"].as_i64().unwrap(),
        );
        self.check_next(
            &format!("k={k} direction"),
            data,
            port.check_next(),
            &model["checkNext"],
        );
        self.matrix(
            &format!("k={k} direction"),
            data,
            port.matrix(),
            &model["matrix"],
        );
    }

    fn report(&self, what: &str) {
        eprintln!(
            "{what}: {} cells compared, {} mismatches",
            self.cells, self.mismatches
        );
        assert!(
            self.mismatches == 0,
            "{what} differs from the model:\n{}",
            self.first.join("\n")
        );
    }
}

/// Survivor's start: woke up in SU, never met Moon, karma 0.
const SURVIVOR_START: StoryProgress<'static> = StoryProgress {
    woke_up_in_region: "SU",
    ever_met_moon: false,
    karma: 0,
};

#[test]
fn su_finders_match_the_model() {
    let Some(game) = Installed::open() else {
        return;
    };
    let model: Value = serde_json::from_str(MODEL).expect("model json");
    let mut world = game.su();
    let data = world.data().clone();
    for (list, key) in [
        (&data.shelters, "shelters"),
        (&data.swarm_rooms, "swarmRooms"),
    ] {
        let names: Vec<&str> = list
            .iter()
            .map(|&r| data.room(r).expect("room").name.as_str())
            .collect();
        assert_eq!(serde_json::to_value(names).unwrap(), model[key]);
    }
    let blockages: Vec<Vec<i32>> = data
        .bat_migration_blockages
        .iter()
        .map(|b| b.to_vec())
        .collect();
    assert_eq!(
        serde_json::to_value(blockages).unwrap(),
        model["migrationBlockages"]
    );
    let ai = world
        .overseers_world_ai()
        .expect("the guide made the process");
    assert!(ai.guide_player_to_next_region);

    let snapshots = model["snapshots"].as_array().expect("snapshots");
    let done_at = model["finders_done_after_updates"]
        .as_i64()
        .expect("done at");
    let mut tally = Tally::default();
    let mut updates = 0;
    let mut port_done_at = None;
    for snapshot in snapshots {
        let k = snapshot["updates"].as_i64().expect("updates");
        while updates < k {
            world.update_world_processes(Some(SURVIVOR_START));
            updates += 1;
            let ai = world.overseers_world_ai().unwrap();
            if port_done_at.is_none()
                && ai.shelter_finder.done
                && ai.direction_finder.as_ref().is_some_and(|d| d.done)
            {
                port_done_at = Some(updates);
            }
        }
        let ai = world.overseers_world_ai().unwrap();
        tally.shelter_finder(k, &data, &ai.shelter_finder, &snapshot["shelter_finder"]);
        tally.direction_finder(
            k,
            &data,
            ai.direction_finder.as_ref(),
            &snapshot["direction_finder"],
        );
    }
    assert_eq!(port_done_at, Some(done_at));
    eprintln!(
        "finders done after {done_at} updates (k: {:?})",
        snapshots.iter().map(|s| &s["updates"]).collect::<Vec<_>>()
    );
    tally.report("shelter and direction finders");
}

#[test]
fn su_swarm_map_matches_the_model() {
    let Some(game) = Installed::open() else {
        return;
    };
    let model: Value = serde_json::from_str(MODEL).expect("model json");
    let world = game.su();
    let data = world.data();
    let map = world
        .processes()
        .swarm_room_map
        .as_ref()
        .expect("swarm map");
    let expected = &model["swarm_room_map"]["map"];
    let mut tally = Tally::default();
    for (i, room) in data.rooms.iter().enumerate() {
        for node in 0..room.nodes.len() {
            for swarm in 0..data.swarm_rooms.len() {
                let port = map.get(i, node, swarm).expect("cell");
                let m = expected[room.name.as_str()][node.to_string()][swarm]
                    .as_f64()
                    .map_or(-1.0, |v| v as f32);
                tally.check(|| format!("{}[{node}][{swarm}]", room.name), port, m);
            }
        }
    }
    tally.report("swarm room map");
    // A new story game's region state has every swarm room active.
    for &r in &data.swarm_rooms {
        assert!(world.active_swarm_room(r));
    }
    assert!(!world.active_swarm_room(world.room_index("SU_C04").unwrap()));
}

#[test]
fn su_node_connectivity_matches_the_room_file() {
    let Some(game) = Installed::open() else {
        return;
    };
    let world = game.su();
    let fly = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE).pre_baked_pathing_index();
    assert_eq!(fly, 6);
    // su_a42.txt line 10: exit 0 and bat hives 1..3; only the fly's layer (6) has paths.
    let a42 = world.data().room_named("SU_A42").expect("SU_A42");
    let n = &a42.nodes;
    assert_eq!(
        (n[0].connection_cost(1, fly), n[0].connection_length(1, fly)),
        (615, 16)
    );
    assert_eq!(
        (n[1].connection_cost(0, fly), n[1].connection_length(0, fly)),
        (628, 14)
    );
    assert_eq!(
        (n[2].connection_cost(3, fly), n[2].connection_length(3, fly)),
        (375, 10)
    );
    assert_eq!(
        (n[3].connection_cost(0, fly), n[3].connection_length(0, fly)),
        (1359, 44)
    );
    assert_eq!(n[1].connection_cost(1, fly), -1);
    for layer in (0..15).filter(|&l| l != fly) {
        assert!((0..4).all(|o| n[0].connection_cost(o, layer) == -1));
    }
    assert_eq!(n[0].connectivity.layers(), 15);
    // Every room the world lists is generation 20, with 15 layers.
    for room in &world.data().rooms {
        for node in &room.nodes {
            assert_eq!(node.connectivity.layers(), 15, "{}", room.name);
        }
    }
    // The off-screen den's nodes connect everywhere at 1.
    let den = world.data().rooms.last().expect("den");
    assert!(den.nodes.iter().all(|d| {
        (0..5).all(|o| d.connection_cost(o, fly) == 1 && d.connection_length(o, fly) == 1)
    }));
}
