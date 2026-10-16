//! The world's side of the region state: `RegionState.AdaptWorldToRegionState` (the
//! saved population back into its dens when a world loads), `AdaptRegionStateToWorld`
//! (the population written back at hibernation), the creature strings of
//! `SaveState.AbstractCreatureFromString` / `AbstractCreatureToStringStoryWorld`, and
//! `SaveState.DepleteOneSwarmRoom`.

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use super::World;
use crate::creature::population::PopulationContext;
use crate::creature::{AbstractCreature, CreatureId, CreatureTemplate, EntityId, WorldCoordinate};
use crate::shelter::ShelterDoor;

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

impl AbstractCreature {
    /// `CreatureState.BaseSaveString` of a creature the port made this cycle: its
    /// health, social memory and meat are untouched, so only death and `spawnData` show.
    fn base_save_string(&self) -> String {
        let mut text = String::new();
        if !self.state.alive {
            text.push_str("Dead<cB>");
        }
        if let Some(data) = &self.spawn_data {
            text.push_str(&format!("SpawnData<cC>{data}<cB>"));
        }
        text
    }

    /// `CreatureState.ToString()`: a loaded creature's state as read, else the base
    /// string.
    fn state_save_string(&self) -> String {
        self.saved_state
            .clone()
            .unwrap_or_else(|| self.base_save_string())
    }

    /// `CreatureState.LoadFromString`'s `Dead` and `SpawnData`.
    fn load_state(&mut self, state: &str) {
        for entry in state.split("<cB>") {
            let mut kv = entry.split("<cC>");
            match kv.next() {
                Some("Dead") => self.state.alive = false,
                Some("SpawnData") => self.spawn_data = kv.next().map(str::to_owned),
                _ => {}
            }
        }
        self.saved_state = Some(state.to_owned());
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
    /// `RainCycleTick` for the cycles since it was last updated (stuck spears count down).
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
        state.rain_cycle_tick(cycle - state.last_cycle_updated, food_rep_bonus, cycle);
        self.region_state = Some(state);
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
            self.move_entity_to_den(id);
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
    /// respawn. The saved sticks are kept as read. Returns `SaveToString()`.
    pub fn adapt_region_state_to_world(
        &mut self,
        player_shelter: i32,
        respawn: &mut Vec<i32>,
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
            if !respawn.contains(&c.id.spawner) {
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
