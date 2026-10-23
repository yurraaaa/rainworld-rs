//! A region's `properties.txt` as the game reads it twice: the `Region` constructor's
//! `RegionParams` (lines preprocessed with the timeline filter, `properties-<timeline>`
//! first), and `World.LoadMapConfig`'s `Room_Attr` lines (raw lines, `Properties-
//! <timeline>` first).
//!
//! Mirrors: Region.cs, World.cs (LoadMapConfig)

use std::fs;

use crate::creatures::{CreatureType, RoomAttraction};
use crate::world::LineFilter;
use crate::{ActiveMods, StreamingAssets};

/// `Region.RegionParams`, base-game fields (defaults from its constructor).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionParams {
    pub bat_deplete_cycles_min: i32,
    pub bat_deplete_cycles_max: i32,
    pub bat_deplete_cycles_max_if_less_than_two_left: i32,
    pub bat_deplete_cycles_max_if_less_than_five_left: i32,
    pub overseers_spawn_chance: f32,
    pub overseers_min: i32,
    pub overseers_max: i32,
    pub player_guide_overseer_spawn_chance: f32,
    pub bats_per_active_swarm_room: i32,
    pub bats_per_inactive_swarm_room: i32,
    pub scavs_min: i32,
    pub scavs_max: i32,
    pub scavs_spawn_chance: f32,
    pub scavenger_delay_initial_min: i32,
    pub scavenger_delay_initial_max: i32,
    pub scavenger_delay_repeat_min: i32,
    pub scavenger_delay_repeat_max: i32,
    pub slug_pup_spawn_chance: f32,
    pub early_cycle_chance: f32,
    pub early_cycle_flood_chance: f32,
    pub albinos: bool,
    pub black_salamander_chance: f32,
    pub locust_region: bool,
    /// `fuseSize`: the side in pixels of a `SuperStructureFuses` light.
    pub fuse_size: f32,
    /// `Subregion` lines in order, after the unnamed subregion 0.
    pub subregions: Vec<String>,
    /// `cloudsStart` and `cloudsEnd`: the altitudes of an `AboveCloudsView` in the region
    /// (none in the base game's regions).
    pub clouds_start: Option<f32>,
    pub clouds_end: Option<f32>,
    /// Keys the base fields above do not cover, with their raw values (colours, music
    /// banks, `Palette`, ...).
    pub other: Vec<(String, String)>,
}

impl Default for RegionParams {
    fn default() -> Self {
        Self {
            bat_deplete_cycles_min: 2,
            bat_deplete_cycles_max: 7,
            bat_deplete_cycles_max_if_less_than_two_left: 3,
            bat_deplete_cycles_max_if_less_than_five_left: 4,
            overseers_spawn_chance: 0.8,
            overseers_min: 1,
            overseers_max: 3,
            player_guide_overseer_spawn_chance: 1.0,
            bats_per_active_swarm_room: 10,
            bats_per_inactive_swarm_room: 4,
            scavs_min: 0,
            scavs_max: 5,
            scavs_spawn_chance: 0.3,
            scavenger_delay_initial_min: 900,
            scavenger_delay_initial_max: 1100,
            scavenger_delay_repeat_min: 4100,
            scavenger_delay_repeat_max: 8200,
            slug_pup_spawn_chance: 0.0,
            early_cycle_chance: 0.02,
            early_cycle_flood_chance: 0.33,
            albinos: false,
            black_salamander_chance: 1.0 / 3.0,
            locust_region: false,
            fuse_size: 10.0,
            subregions: Vec::new(),
            clouds_start: None,
            clouds_end: None,
            other: Vec::new(),
        }
    }
}

/// One `Room_Attr: ROOM: Type-Attraction,...` line: per creature type, or per named
/// attraction for names that are not a registered type.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomAttractions {
    pub room: String,
    pub by_type: Vec<(CreatureType, RoomAttraction)>,
    pub named: Vec<(String, RoomAttraction)>,
}

/// What a region's properties file says.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegionProperties {
    pub params: RegionParams,
    /// `Room_Attr` lines for rooms, in file order (the game applies each to the room of
    /// that exact name).
    pub room_attractions: Vec<RoomAttractions>,
    /// `Room_Attr: Default:` entries (`defaultRoomAttractions` and
    /// `defaultNamedAttractions`), values unresolved as in the game.
    pub default_attractions: Vec<(String, String)>,
}

impl RegionProperties {
    /// Reads `world/<region>/properties.txt` (or its timeline variant); defaults when it
    /// is missing.
    #[must_use]
    pub fn load(assets: &StreamingAssets, region: &str, filter: &LineFilter) -> Self {
        let dir = format!("world/{}", region.to_lowercase());
        let timeline_file = filter
            .timeline
            .map(|t| assets.file(&format!("{dir}/properties-{}.txt", t.value())))
            .filter(|p| p.is_file());
        let plain = assets.file(&format!("{dir}/properties.txt"));
        // Both readers pick the same file (names are lower case on disk).
        let text = fs::read_to_string(timeline_file.as_ref().unwrap_or(&plain)).unwrap_or_default();
        let mut props = Self {
            params: RegionParams::read(&text, filter),
            ..Self::default()
        };
        props.read_attractions(&text, filter.mods);
        props
    }

    /// `LoadMapConfig`'s `Room_Attr` lines (not preprocessed).
    fn read_attractions(&mut self, text: &str, mods: ActiveMods) {
        for line in text.lines() {
            let line = Self::spaced(line, ":");
            let parts: Vec<&str> = line.split(": ").collect();
            if parts.len() != 3 || parts[0] != "Room_Attr" {
                continue;
            }
            let entries = parts[2].split(',').filter(|e| !e.is_empty());
            if parts[1] == "Default" {
                for entry in entries {
                    let mut f = entry.split('-');
                    let (Some(name), Some(value)) = (f.next(), f.next()) else {
                        continue;
                    };
                    self.default_attractions
                        .push((name.to_owned(), value.to_owned()));
                }
                continue;
            }
            let mut attractions = RoomAttractions {
                room: parts[1].to_owned(),
                ..RoomAttractions::default()
            };
            for entry in entries {
                let mut f = entry.split('-');
                let (Some(name), Some(value)) = (f.next(), f.next()) else {
                    continue;
                };
                let attraction = value.trim().parse::<i32>().map_or_else(
                    |_| RoomAttraction::from_value(value),
                    RoomAttraction::from_number,
                );
                match CreatureType::from_value(name, mods) {
                    Some(t) => attractions.by_type.push((t, attraction)),
                    None => attractions.named.push((name.to_owned(), attraction)),
                }
            }
            self.room_attractions.push(attractions);
        }
    }

    /// The attraction lines of room `name` (exact spelling), in file order.
    pub fn attractions_of<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a RoomAttractions> {
        self.room_attractions.iter().filter(move |a| a.room == name)
    }

    /// `Custom.ValidateSpacedDelimiter`: a line ending in the delimiter gets a space.
    fn spaced(line: &str, delimiter: &str) -> String {
        if line.ends_with(delimiter) {
            format!("{line} ")
        } else {
            line.to_owned()
        }
    }
}

impl RegionParams {
    /// `proceduralMusicBank` (the last such line; `""` without one).
    #[must_use]
    pub fn procedural_music_bank(&self) -> &str {
        self.other
            .iter()
            .rev()
            .find(|(k, _)| k == "proceduralMusicBank")
            .map_or("", |(_, v)| v.as_str())
    }

    /// The `Region` constructor's loop over the preprocessed lines.
    fn read(text: &str, filter: &LineFilter) -> Self {
        let mut params = Self::default();
        for raw in text.lines() {
            let Ok(line) = filter.preprocess(raw) else {
                continue;
            };
            let line = RegionProperties::spaced(line, ":");
            let parts: Vec<&str> = line.split(": ").collect();
            if parts.len() < 2 {
                continue;
            }
            params.set(parts[0], parts[1]);
        }
        params
    }

    fn set(&mut self, key: &str, value: &str) {
        let int = value.trim().parse::<i32>().ok();
        let float = value.trim().parse::<f32>().ok();
        let bool_true = value.trim().to_lowercase() == "true";
        match key {
            "batDepleteCyclesMin" => Self::put(&mut self.bat_deplete_cycles_min, int),
            "batDepleteCyclesMax" => Self::put(&mut self.bat_deplete_cycles_max, int),
            "batDepleteCyclesMaxIfLessThanTwoLeft" => {
                Self::put(&mut self.bat_deplete_cycles_max_if_less_than_two_left, int);
            }
            "batDepleteCyclesMaxIfLessThanFiveLeft" => {
                Self::put(&mut self.bat_deplete_cycles_max_if_less_than_five_left, int);
            }
            "overseersSpawnChance" => Self::put(&mut self.overseers_spawn_chance, float),
            "overseersMin" => Self::put(&mut self.overseers_min, int),
            "overseersMax" => Self::put(&mut self.overseers_max, int),
            "playerGuideOverseerSpawnChance" => {
                Self::put(&mut self.player_guide_overseer_spawn_chance, float);
            }
            "scavsMin" => Self::put(&mut self.scavs_min, int),
            "scavsMax" => Self::put(&mut self.scavs_max, int),
            // The game parses this one as an int.
            "scavsSpawnChance" => Self::put(&mut self.scavs_spawn_chance, int.map(|i| i as f32)),
            "Subregion" => self.subregions.push(value.to_owned()),
            "batsPerActiveSwarmRoom" => Self::put(&mut self.bats_per_active_swarm_room, int),
            "batsPerInactiveSwarmRoom" => Self::put(&mut self.bats_per_inactive_swarm_room, int),
            "scavsDelayInitialMin" => Self::put(&mut self.scavenger_delay_initial_min, int),
            "scavsDelayInitialMax" => Self::put(&mut self.scavenger_delay_initial_max, int),
            "scavsDelayRepeatMin" => Self::put(&mut self.scavenger_delay_repeat_min, int),
            "scavsDelayRepeatMax" => Self::put(&mut self.scavenger_delay_repeat_max, int),
            "pupSpawnChance" => Self::put(&mut self.slug_pup_spawn_chance, float),
            "earlyCycleChance" => Self::put(&mut self.early_cycle_chance, float),
            "earlyCycleFloodChance" => Self::put(&mut self.early_cycle_flood_chance, float),
            "albinos" => self.albinos = bool_true,
            "locustRegion" => self.locust_region = bool_true,
            "blackSalamanderChance" => Self::put(&mut self.black_salamander_chance, float),
            "fuseSize" => Self::put(&mut self.fuse_size, float),
            "cloudsStart" => Self::put(&mut self.clouds_start, float.map(Some)),
            "cloudsEnd" => Self::put(&mut self.clouds_end, float.map(Some)),
            _ => self.other.push((key.to_owned(), value.trim().to_owned())),
        }
    }

    fn put<T>(field: &mut T, value: Option<T>) {
        if let Some(v) = value {
            *field = v;
        }
    }
}
