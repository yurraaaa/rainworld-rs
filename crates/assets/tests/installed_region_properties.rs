//! The overseer rules of every vanilla region's `properties.txt` (read-only), as
//! `Region.RegionParams` reads them: the defaults (decompile `RegionParams()`:
//! `overseersSpawnChance = 0.8f; overseersMin = 1; overseersMax = 3;
//! playerGuideOverseerSpawnChance = 1f; batsPerActiveSwarmRoom = 10`) unless a line sets
//! them. Set `RW_DIR` to the game install directory; skipped if absent.

use std::path::PathBuf;

use rw_assets::world::{LineFilter, StoryConditions};
use rw_assets::{ActiveMods, RegionProperties, StreamingAssets, Timeline};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

/// Region, `overseersSpawnChance`, `overseersMin`, `overseersMax`,
/// `playerGuideOverseerSpawnChance`, `batsPerActiveSwarmRoom`, as the files say (GW has
/// none of these lines).
const EXPECTED: [(&str, f32, i32, i32, f32, i32); 12] = [
    ("CC", 0.5, 0, 4, 0.5, 10),
    ("DS", 0.2, 0, 2, 0.0, 10),
    ("HI", 0.5, 0, 4, 0.7, 10),
    ("GW", 0.8, 1, 3, 1.0, 10),
    ("SI", 0.4, 0, 4, 0.2, 10),
    ("SU", 0.8, 0, 4, 1.0, 7),
    ("SH", 0.2, 0, 3, 1.0, 10),
    ("SL", 0.2, 0, 4, 0.7, 10),
    ("LF", 0.8, 0, 6, 0.4, 10),
    ("UW", 1.0, 4, 8, 0.0, 10),
    ("SB", 0.01, 1, 1, 0.0, 10),
    ("SS", 0.0, 0, 4, 0.0, 10),
];

#[test]
fn overseer_rules_of_each_vanilla_region() {
    let root =
        std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
    let sa = root.join("RainWorld_Data").join("StreamingAssets");
    if !sa.join("world").join("regions.txt").is_file() {
        eprintln!("skipping: no world under {}", sa.display());
        return;
    }
    let assets = StreamingAssets::new(sa);
    let filter = LineFilter {
        timeline: Some(Timeline::White),
        mods: ActiveMods::NONE,
        conditions: StoryConditions::new_story(),
    };
    for (region, chance, min, max, guide, bats) in EXPECTED {
        let p = RegionProperties::load(&assets, region, &filter).params;
        let got = (
            p.overseers_spawn_chance,
            p.overseers_min,
            p.overseers_max,
            p.player_guide_overseer_spawn_chance,
            p.bats_per_active_swarm_room,
        );
        assert_eq!(got, (chance, min, max, guide, bats), "{region}");
    }
}
