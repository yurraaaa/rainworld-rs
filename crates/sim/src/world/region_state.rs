//! `RegionState`: what a story save keeps of one region between cycles (swarm room
//! counters, lineages, the saved population and objects, consumed items, visited rooms),
//! read from and written to its `REGIONSTATE` string (`<rgA>` entries, `<rgB>` key and
//! value, `<rgC>` list items). A story world that is not a single room has one
//! (`World.regionState`).
//!
//! Saved creatures and objects the port does not simulate are kept as their strings and
//! written back unchanged (docs/divergences/ D16).
//!
//! The world's side of it: `RegionState.AdaptWorldToRegionState` (the saved population
//! back into its dens when a world loads), `AdaptRegionStateToWorld` (the population
//! written back at hibernation), the creature strings of
//! `SaveState.AbstractCreatureFromString` / `AbstractCreatureToStringStoryWorld`, and
//! `SaveState.DepleteOneSwarmRoom`.
//!
//! Mirrors: RegionState.cs, World.cs, SaveState.cs

use rw_assets::CreatureType;
use rw_assets::world::WorldData;
use rw_random::UnityRandom;

use super::World;
use crate::creature::population::PopulationContext;
use crate::creature::{AbstractCreature, CreatureId, CreatureTemplate, EntityId, WorldCoordinate};
use crate::shelter::ShelterDoor;

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
                        if !state
                            .swarm_room_counters
                            .iter()
                            .any(|(n, _)| *n == room.name)
                        {
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

/// A saved creature string's parts (`Type<cA>ID<cA>ROOM.node<cA>state`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavedCreature<'a> {
    pub kind: &'a str,
    pub id: EntityId,
    pub room: &'a str,
    pub node: i32,
    /// The `<cB>`-separated state entries, as read.
    pub state: &'a str,
}

impl<'a> SavedCreature<'a> {
    /// `AbstractCreatureFromString`'s split; `None` when malformed (the game throws).
    pub fn parse(s: &'a str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(4, "<cA>").collect();
        let id_part = parts.get(1)?.split("<cB>").next()?;
        let mut id = id_part.split('.');
        let (_, spawner, number) = (id.next()?, id.next()?, id.next()?);
        let pos = parts.get(2)?;
        let (room, node) = pos.rsplit_once('.')?;
        Some(Self {
            kind: parts[0],
            id: EntityId::new(spawner.trim().parse().ok()?, number.trim().parse().ok()?),
            room,
            node: node.trim().parse().ok()?,
            state: parts.get(3).copied().unwrap_or(""),
        })
    }
}

impl World {
    /// `WorldCoordinate.ResolveRoomName` through the room index map: the off-screen den is
    /// `<region>OffscreenDen` there.
    #[must_use]
    pub fn save_room_name(&self, room: i32) -> Option<String> {
        let data = self.abstract_room(room)?;
        Some(if data.offscreen_den {
            format!("{}OffscreenDen", self.data.name)
        } else {
            data.name.clone()
        })
    }

    /// The room of a saved room name (`BackwardsCompatibilityRemix.ParseRoomIndex`), in
    /// this world.
    #[must_use]
    pub fn room_from_save_name(&self, name: &str) -> Option<i32> {
        if name == format!("{}OffscreenDen", self.data.name) {
            return Some(self.offscreen_den_index());
        }
        self.data
            .rooms
            .iter()
            .find(|r| r.name == name && !r.offscreen_den)
            .map(|r| r.index)
    }

    /// `WorldCoordinate.SaveToString` of a den (`Lineage.denString`'s base).
    #[must_use]
    pub fn coordinate_save_string(&self, c: WorldCoordinate) -> String {
        let room = self
            .save_room_name(c.room)
            .unwrap_or_else(|| c.room.to_string());
        format!("{room}.{}.{}.{}", c.tile.0, c.tile.1, c.node)
    }

    /// `SaveState.AbstractCreatureToStringStoryWorld(critter, pos)`.
    #[must_use]
    pub fn creature_save_string(&self, c: &AbstractCreature, pos: WorldCoordinate) -> String {
        let room = self
            .save_room_name(pos.room)
            .unwrap_or_else(|| pos.room.to_string());
        format!(
            "{}<cA>ID.{}.{}<cB>0<cA>{room}.{}<cA>{}",
            c.kind().value(),
            c.id.spawner,
            c.id.number,
            pos.node,
            c.state_save_string()
        )
    }

    /// `AdaptWorldToRegionState` with the port's objects: saved rocks and spears join
    /// their rooms, other objects and the sticks stay as text, the saved creatures go
    /// back into their dens (a shelter's into the room), then the region's
    /// `RainCycleTick` for the cycles since it was last updated (stuck spears count down,
    /// the creatures' states tick once).
    pub(crate) fn adapt_world_to_region_state(
        &mut self,
        cx: &mut PopulationContext<'_>,
        food_rep_bonus: i32,
    ) {
        let Some(mut state) = self.region_state.take() else {
            return;
        };
        let objects = std::mem::take(&mut state.saved_objects);
        let (loaded, unrecognized) = self.load_saved_objects(objects);
        state.saved_objects = loaded;
        state.unrecognized_saved_objects = unrecognized;
        let population = std::mem::take(&mut state.saved_population);
        for entry in population {
            match self.load_saved_creature(&entry, cx) {
                Some(id) => {
                    self.loaded_creatures.push(id);
                    state.saved_population.push(entry);
                }
                // The game drops a known creature it cannot place; the port keeps it.
                None => state.unrecognized_population.push(entry),
            }
        }
        let cycle = cx.session.cycle_number;
        self.stuck_in_wall_tick(cycle - state.last_cycle_updated);
        if cycle - state.last_cycle_updated > 0 {
            self.creatures_cycle_tick(cx.rng);
        }
        state.rain_cycle_tick(cycle - state.last_cycle_updated, food_rep_bonus, cycle);
        self.region_state = Some(state);
    }

    /// `RegionState.RainCycleTick`'s creatures: `CreatureState.CycleTick` of each
    /// creature among each room's entities, then in its dens, rooms in order.
    fn creatures_cycle_tick(&mut self, rng: &mut UnityRandom) {
        let ids: Vec<CreatureId> = self
            .lists
            .iter()
            .flat_map(|l| {
                l.entities
                    .iter()
                    .filter_map(|e| e.creature())
                    .chain(l.in_dens.iter().copied())
            })
            .collect();
        for id in ids {
            self.creature_mut(id).state.cycle_tick(rng);
        }
    }

    /// `AbstractCreatureFromString(world, s, onlyInCurrentRegion: true)` and its placing.
    fn load_saved_creature(
        &mut self,
        s: &str,
        cx: &mut PopulationContext<'_>,
    ) -> Option<CreatureId> {
        let saved = SavedCreature::parse(s)?;
        let kind = CreatureType::from_value(saved.kind, self.mods)?;
        let room = match self.room_from_save_name(saved.room) {
            Some(room) => room,
            // Out of the region: back to its spawner's den, if the spawner is here.
            None => self.spawner_den(saved.id)?,
        };
        let pos = WorldCoordinate::new(room, -1, -1, saved.node);
        let template = CreatureTemplate::of(kind, self.mods);
        let mut c = self.new_creature(template, pos, saved.id, cx.session, cx.ids, cx.rng);
        c.load_state(saved.state);
        let id = self.insert_creature(c);
        if self.abstract_room(room).is_some_and(|r| r.shelter()) {
            self.add_entity(id);
        } else {
            self.move_entity_to_den(id, cx.rng);
        }
        Some(id)
    }

    /// `World.GetSpawner(id).den`, for a spawner of this region.
    fn spawner_den(&self, id: EntityId) -> Option<i32> {
        let region = self.data.region.as_ref()?.region_number;
        let index = usize::try_from(id.spawner - region * 1000).ok()?;
        let den = self.data.spawners.get(index)?.den();
        Some(den.room)
    }

    /// `AdaptRegionStateToWorld(playerShelter, -1)`: the unrecognized objects and
    /// creatures return to the saved lists, then the rooms' stuck spears and shelter
    /// objects, and every saved creature of the world's rooms
    /// (`CreatureToStringInDenPos`); loaded creatures not saved and dead are put up for
    /// respawn unless their spawner is on `respawn` or `wait` (`waitRespawnCreatures`)
    /// already. The saved sticks are kept as read. Returns `SaveToString()`.
    pub fn adapt_region_state_to_world(
        &mut self,
        player_shelter: i32,
        respawn: &mut Vec<i32>,
        wait: &[i32],
    ) -> Option<String> {
        let mut state = self.region_state.take()?;
        state.saved_objects = std::mem::take(&mut state.unrecognized_saved_objects);
        state.saved_objects.extend(self.objects_to_save());
        state.saved_population = std::mem::take(&mut state.unrecognized_population);
        for l in 0..self.lists.len() {
            let lists = &self.lists[l];
            let ids: Vec<CreatureId> = lists
                .creatures
                .iter()
                .chain(&lists.in_dens)
                .copied()
                .collect();
            for id in ids {
                let c = self.creature(id);
                if c.template.quantified || !c.template.save_creature {
                    continue;
                }
                let text = self.creature_to_string_in_den_pos(c, player_shelter);
                if text.is_empty() {
                    continue;
                }
                state.saved_population.push(text);
                self.loaded_creatures.retain(|&x| x != id);
            }
        }
        for &id in &self.loaded_creatures {
            let c = self.creature(id);
            if c.state.alive {
                continue;
            }
            if !respawn.contains(&c.id.spawner) && !wait.contains(&c.id.spawner) {
                respawn.push(c.id.spawner);
            }
        }
        let text = state.save_to_string();
        self.region_state = Some(state);
        Some(text)
    }

    /// `RegionState.CreatureToStringInDenPos`: in a shelter's range (the player's
    /// shelter, dead, or a creature that does not move off-screen) at the room's node 0,
    /// else at its spawn den or its AI's den; nothing for the slugcat, a position out of
    /// the region or a dead creature outside a shelter.
    fn creature_to_string_in_den_pos(&self, c: &AbstractCreature, player_shelter: i32) -> String {
        if c.kind() == CreatureType::Slugcat {
            return String::new();
        }
        let room = self.abstract_room(c.pos.room);
        let in_shelter = room.is_some_and(|r| {
            r.shelter() && ShelterDoor::coord_inside_shelter_range(c.pos.tile, r.ancient_shelter)
        });
        let mut pos = c.spawn_den;
        if in_shelter
            && (c.pos.room == player_shelter
                || !c.state.alive
                || c.template.off_screen_speed == 0.0)
        {
            pos = Some(WorldCoordinate::new(c.pos.room, -1, -1, 0));
        } else if let Some(den) =
            c.ai.as_ref()
                .and_then(|a| a.den_position(c, self.offscreen_den_index()))
                .filter(|d| self.abstract_room(d.room).is_some())
        {
            pos = Some(den);
        }
        let Some(pos) = pos.filter(|p| self.abstract_room(p.room).is_some()) else {
            return String::new();
        };
        let shelter = self.abstract_room(pos.room).is_some_and(|r| r.shelter());
        if !shelter && !c.state.alive {
            return String::new();
        }
        self.creature_save_string(c, pos)
    }

    /// `SaveState.DepleteOneSwarmRoom` with only this world's region state loaded: one
    /// `Range` over the regions (just this one), then, with candidates, one to deplete
    /// for `Region.CyclesToDepleteASwarmRoom(active)` cycles.
    pub fn deplete_one_swarm_room(&mut self, rng: &mut UnityRandom) {
        rng.range_i32(0, 1);
        let params = self.data.properties.params.clone();
        let Some(state) = self.region_state.as_mut() else {
            return;
        };
        if state.candidates_for_deplete_swarm_rooms.is_empty() {
            return;
        }
        let active = state
            .swarm_room_counters
            .iter()
            .filter(|(_, c)| *c == 0)
            .count();
        let n = i32::try_from(state.candidates_for_deplete_swarm_rooms.len()).unwrap_or(1);
        let pick = usize::try_from(rng.range_i32(0, n)).unwrap_or(0);
        let room = state.candidates_for_deplete_swarm_rooms[pick].clone();
        let cycles = if active < 1 {
            1
        } else {
            let max = if active < 2 {
                params.bat_deplete_cycles_max_if_less_than_two_left
            } else if active < 5 {
                params.bat_deplete_cycles_max_if_less_than_five_left
            } else {
                params.bat_deplete_cycles_max
            };
            rng.range_i32(params.bat_deplete_cycles_min, max)
        };
        state.set_swarm_room_counter(&room, cycles);
    }
}
