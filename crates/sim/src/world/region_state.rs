//! `RegionState`: what a story save keeps of one region between cycles (swarm room
//! counters, lineages, the saved population and objects, consumed items, visited rooms),
//! read from and written to its `REGIONSTATE` string (`<rgA>` entries, `<rgB>` key and
//! value, `<rgC>` list items). A story world that is not a single room has one
//! (`World.regionState`).
//!
//! Saved creatures and objects the port does not simulate are kept as their strings and
//! written back unchanged (docs/DIVERGENCES.md D16).

use rw_assets::world::WorldData;
use rw_random::UnityRandom;

/// `RegionState.ConsumedItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumedItem {
    /// The origin room's name (`ResolveOriginRoom`, or the unknown name).
    pub origin_room: String,
    /// Whether the room was unknown when read (`INV.` form).
    pub invalid: bool,
    pub placed_object_index: i32,
    pub wait_cycles: i32,
}

impl ConsumedItem {
    /// `FromString`; `None` on a malformed entry (where the game's `int.Parse` throws).
    #[must_use]
    pub fn from_string(s: &str, data: &WorldData) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        let skip = usize::from(parts.len() == 4 && parts[0] == "INV");
        let room = (*parts.get(skip)?).to_owned();
        Some(Self {
            invalid: data.room_named(&room).is_none(),
            origin_room: room,
            placed_object_index: parts.get(1 + skip)?.trim().parse().ok()?,
            wait_cycles: parts.get(2 + skip)?.trim().parse().ok()?,
        })
    }

    /// `ToString`.
    #[must_use]
    pub fn save_string(&self) -> String {
        if self.invalid {
            format!(
                "INV.{}.{}.{}",
                self.origin_room, self.placed_object_index, self.wait_cycles
            )
        } else {
            format!(
                "{}.{}.{}",
                self.origin_room, self.placed_object_index, self.wait_cycles
            )
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionState {
    /// `regionName` (`World.name`).
    pub region_name: String,
    pub last_cycle_updated: i32,
    /// `swarmRoomCounters` in dictionary order: cycles until the swarm room's flies
    /// return (0: active).
    pub swarm_room_counters: Vec<(String, i32)>,
    /// `lineageCounters` by `Lineage.denString`, in dictionary order.
    pub lineage_counters: Vec<(String, i32)>,
    pub saved_objects: Vec<String>,
    pub unrecognized_saved_objects: Vec<String>,
    pub saved_population: Vec<String>,
    pub unrecognized_population: Vec<String>,
    pub saved_sticks: Vec<String>,
    pub consumed_items: Vec<ConsumedItem>,
    /// `roomsVisited`: room names in visit order (`ReportRoomVisited`).
    pub rooms_visited: Vec<String>,
    /// `candidatesForDepleteSwarmRooms`: swarm rooms a caught fly came from.
    pub candidates_for_deplete_swarm_rooms: Vec<String>,
    /// The Watcher's entries (`RIPPLEREGIONS`, `SENTIENTROT`, `COLLECTEDRIPPLEEGGS`,
    /// `SHOWRIPPLETHRESHOLD`, `RESPAWNRIPPLESPIDER`), read and written as text.
    pub watcher: Vec<(String, String)>,
    /// `unrecognizedSaveStrings` (`key<rgB>value`), `REGIONNAME` among them: the
    /// constructor does not read it, so each save writes one more copy, as the game's does.
    pub unrecognized: Vec<String>,
    pub new_rooms_this_cycle: i32,
}

impl RegionState {
    /// The Watcher keys, in `SaveToString`'s order.
    const WATCHER_KEYS: [&'static str; 5] = [
        "RIPPLEREGIONS",
        "SENTIENTROT",
        "COLLECTEDRIPPLEEGGS",
        "SHOWRIPPLETHRESHOLD",
        "RESPAWNRIPPLESPIDER",
    ];

    /// `new RegionState(saveState, world)` without a load string.
    #[must_use]
    pub fn new_game(data: &WorldData) -> Self {
        Self {
            region_name: data.name.clone(),
            swarm_room_counters: data
                .swarm_rooms
                .iter()
                .filter_map(|&r| data.room(r))
                .map(|r| (r.name.clone(), 0))
                .collect(),
            ..Self::default()
        }
    }

    /// `new RegionState(saveState, world)` with `regionLoadStrings[region]`: the
    /// entries read, missing swarm rooms added at 0, and when none is active one drawn
    /// to be (`Random.Range`). Malformed numbers are skipped where the game throws.
    #[must_use]
    pub fn from_string(
        data: &WorldData,
        s: &str,
        mods: rw_assets::ActiveMods,
        rng: &mut UnityRandom,
    ) -> Self {
        let mut state = Self::new_game(data);
        for entry in s.split("<rgA>") {
            let parts: Vec<&str> = entry.split("<rgB>").collect();
            if parts.len() < 2 {
                continue;
            }
            let (key, value) = (parts[0], parts[1]);
            match key {
                "LASTCYCLEUPDATED" => {
                    state.last_cycle_updated = value.trim().parse().unwrap_or(0);
                }
                "SWARMROOMS" => {
                    state.swarm_room_counters = Self::string_int_dict(value);
                    for room in data.swarm_rooms.iter().filter_map(|&r| data.room(r)) {
                        if !state.swarm_room_counters.iter().any(|(n, _)| *n == room.name) {
                            state.swarm_room_counters.push((room.name.clone(), 0));
                        }
                    }
                }
                "LINEAGES" => state.lineage_counters = Self::string_int_dict(value),
                "OBJECTS" => state.saved_objects = Self::items(value),
                "POPULATION" => {
                    for item in Self::items(value) {
                        let kind = item.split("<cA>").next().unwrap_or("");
                        if rw_assets::CreatureType::from_value(kind, mods).is_some() {
                            state.saved_population.push(item);
                        } else {
                            state.unrecognized_population.push(item);
                        }
                    }
                }
                "STICKS" => state.saved_sticks = Self::items(value),
                "CONSUMEDITEMS" => {
                    state.consumed_items = value
                        .split("<rgC>")
                        .filter(|i| !i.is_empty())
                        .filter_map(|i| ConsumedItem::from_string(i, data))
                        .collect();
                }
                "ROOMSVISITED" => {
                    state.rooms_visited = value
                        .split(',')
                        .filter(|r| !r.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                _ if Self::WATCHER_KEYS.contains(&key) => {
                    state.watcher.push((key.to_owned(), value.to_owned()));
                }
                _ if !key.trim().is_empty() => {
                    state.unrecognized.push(format!("{key}<rgB>{value}"));
                }
                _ => {}
            }
        }
        if !state.swarm_room_counters.is_empty()
            && !state.swarm_room_counters.iter().any(|(_, c)| *c == 0)
        {
            let n = i32::try_from(state.swarm_room_counters.len()).unwrap_or(i32::MAX);
            let pick = usize::try_from(rng.range_i32(0, n)).unwrap_or(0);
            state.swarm_room_counters[pick].1 = 0;
        }
        state
    }

    /// `SaveUtils.LoadStringIntDict`: `key:value` pairs by `,` (later keys replace).
    fn string_int_dict(s: &str) -> Vec<(String, i32)> {
        let mut dict: Vec<(String, i32)> = Vec::new();
        for pair in s.split(',').filter(|p| p.contains(':')) {
            let mut it = pair.split(':');
            let key = it.next().unwrap_or("").to_owned();
            let Some(value) = it.next().and_then(|v| v.trim().parse().ok()) else {
                continue;
            };
            match dict.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => dict.push((key, value)),
            }
        }
        dict
    }

    fn items(s: &str) -> Vec<String> {
        s.split("<rgC>")
            .filter(|i| !i.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// `SaveToString`.
    #[must_use]
    pub fn save_to_string(&self) -> String {
        let mut text = format!(
            "REGIONNAME<rgB>{}<rgA>LASTCYCLEUPDATED<rgB>{}<rgA>SWARMROOMS<rgB>{}<rgA>",
            self.region_name,
            self.last_cycle_updated,
            Self::dict_string(&self.swarm_room_counters)
        );
        if !self.lineage_counters.is_empty() {
            text.push_str(&format!(
                "LINEAGES<rgB>{}<rgA>",
                Self::dict_string(&self.lineage_counters)
            ));
        }
        if !self.saved_objects.is_empty() {
            text.push_str("OBJECTS<rgB>");
            for o in self
                .saved_objects
                .iter()
                .chain(&self.unrecognized_saved_objects)
            {
                text.push_str(o);
                text.push_str("<rgC>");
            }
            text.push_str("<rgA>");
        }
        if !self.saved_population.is_empty() || !self.unrecognized_population.is_empty() {
            text.push_str("POPULATION<rgB>");
            for c in self
                .saved_population
                .iter()
                .chain(&self.unrecognized_population)
            {
                text.push_str(c);
                text.push_str("<rgC>");
            }
            text.push_str("<rgA>");
        }
        if !self.saved_sticks.is_empty() {
            text.push_str("STICKS<rgB>");
            for s in &self.saved_sticks {
                text.push_str(s);
                text.push_str("<rgC>");
            }
            text.push_str("<rgA>");
        }
        if !self.consumed_items.is_empty() {
            let items: Vec<String> = self
                .consumed_items
                .iter()
                .map(ConsumedItem::save_string)
                .collect();
            text.push_str(&format!("CONSUMEDITEMS<rgB>{}<rgA>", items.join("<rgC>")));
        }
        text.push_str(&format!(
            "ROOMSVISITED<rgB>{}<rgA>",
            self.rooms_visited.join(",")
        ));
        for key in Self::WATCHER_KEYS {
            for (k, v) in self.watcher.iter().filter(|(k, _)| k == key) {
                text.push_str(&format!("{k}<rgB>{v}<rgA>"));
            }
        }
        for u in &self.unrecognized {
            text.push_str(u);
            text.push_str("<rgA>");
        }
        text
    }

    fn dict_string(dict: &[(String, i32)]) -> String {
        dict.iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// `ReportRoomVisited`.
    pub fn report_room_visited(&mut self, room: &str) {
        if !self.rooms_visited.iter().any(|r| r == room) {
            self.rooms_visited.push(room.to_owned());
            self.new_rooms_this_cycle += 1;
        }
    }

    /// `SwarmRoomActive(swarmRoomName)`.
    #[must_use]
    pub fn swarm_room_active_named(&self, room: &str) -> bool {
        self.swarm_room_counters
            .iter()
            .any(|(n, c)| n == room && *c == 0)
    }

    /// `SwarmRoomActive(swarmRoomIndex)`.
    #[must_use]
    pub fn swarm_room_active(&self, data: &WorldData, swarm_room: usize) -> bool {
        data.swarm_rooms
            .get(swarm_room)
            .and_then(|&r| data.room(r))
            .is_some_and(|r| self.swarm_room_active_named(&r.name))
    }

    /// `swarmRoomCounters[room] = value`.
    pub fn set_swarm_room_counter(&mut self, room: &str, value: i32) {
        match self.swarm_room_counters.iter_mut().find(|(n, _)| n == room) {
            Some(entry) => entry.1 = value,
            None => self.swarm_room_counters.push((room.to_owned(), value)),
        }
    }

    /// `lineageCounters[den]`, if set.
    #[must_use]
    pub fn lineage_counter(&self, den: &str) -> Option<i32> {
        self.lineage_counters
            .iter()
            .find(|(d, _)| d == den)
            .map(|(_, c)| *c)
    }

    pub fn set_lineage_counter(&mut self, den: &str, value: i32) {
        match self.lineage_counters.iter_mut().find(|(d, _)| d == den) {
            Some(entry) => entry.1 = value,
            None => self.lineage_counters.push((den.to_owned(), value)),
        }
    }

    /// The region-state part of `RainCycleTick(ticks, foodRepBonus)`: depleted swarm
    /// rooms and consumed items count down (creature states' `CycleTick` is the
    /// world's), then `lastCycleUpdated` becomes the save's cycle.
    pub fn rain_cycle_tick(&mut self, ticks: i32, food_rep_bonus: i32, cycle_number: i32) {
        if ticks > 0 {
            for (_, c) in &mut self.swarm_room_counters {
                if *c > 0 {
                    *c = (*c - ticks - food_rep_bonus).max(0);
                }
            }
            for i in (0..self.consumed_items.len()).rev() {
                let item = &mut self.consumed_items[i];
                if item.wait_cycles > -1 {
                    item.wait_cycles -= ticks + food_rep_bonus;
                    if item.wait_cycles < 1 {
                        self.consumed_items.remove(i);
                    }
                }
            }
        }
        self.last_cycle_updated = cycle_number;
    }
}
