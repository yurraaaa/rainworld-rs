//! `OverseersWorldAI`: the overseers' world process, with its shelter finder (each
//! shelter's distance from every exit of the region) and the guide's direction finder
//! (the distance to the room the guide shows the way to). Both are Dijkstra searches over
//! (room, exit node) cells, one cell per update, on the fly's baked node connectivity.
//! Neither draws a random number.
//!
//! Behaviour: `rainworld-re/notes/creature-model.md` §7.
//!
//! Mirrors: OverseersWorldAI.cs

use rw_assets::world::WorldData;
use rw_assets::{ActiveMods, CreatureType};

use super::{RegionState, World};
use crate::creature::{CreatureId, CreatureTemplate, WorldCoordinate};

impl World {
    /// The world processes' updates, in `RainWorldGame.Update`'s place for them: after
    /// the abstract room update, before `rainCycle.Update`. This is `OverseersWorldAI`'s;
    /// `FliesWorldAI.Update` follows it (`update_flies_world_ai`, called by the game),
    /// and `ScavengersWorldAI` is a marker.
    pub fn update_world_processes(&mut self, story: Option<StoryProgress<'_>>) {
        if let Some(overseers) = &mut self.processes.overseers {
            overseers.update(&self.data, self.region_state.as_ref(), story);
        }
    }

    /// `OverseerAbstractAI.SetAsPlayerGuide`'s part on the world process (made here if
    /// the guide's constructor did not).
    pub(crate) fn set_player_guide(&mut self, guide: CreatureId) {
        self.processes
            .overseers
            .get_or_insert_with(|| OverseersWorldAi::new(&self.data))
            .set_player_guide(guide);
    }

    /// `overseersWorldAI`.
    #[must_use]
    pub fn overseers_world_ai(&self) -> Option<&OverseersWorldAi> {
        self.processes.overseers.as_ref()
    }

    pub fn overseers_world_ai_mut(&mut self) -> Option<&mut OverseersWorldAi> {
        self.processes.overseers.as_mut()
    }

    #[must_use]
    pub fn direction_finder(&self) -> Option<&DirectionFinder> {
        self.overseers_world_ai()?.direction_finder.as_ref()
    }

    /// `shelterFinder.DistanceToShelter(shelter, pos)`; -1 without the process.
    #[must_use]
    pub fn distance_to_shelter(&self, shelter: usize, pos: WorldCoordinate) -> f32 {
        self.overseers_world_ai().map_or(-1.0, |o| {
            o.shelter_finder
                .distance_to_shelter(&self.data, shelter, pos)
        })
    }

    /// `directionFinder.DistanceToDestination(pos)`; -1 without a direction finder.
    #[must_use]
    pub fn distance_to_destination(&self, pos: WorldCoordinate) -> f32 {
        self.direction_finder()
            .map_or(-1.0, |d| d.distance_to_destination(&self.data, pos))
    }

    /// `directionFinder.DestinationRoomVisisted`; false without a direction finder.
    #[must_use]
    pub fn destination_room_visited(&self) -> bool {
        self.direction_finder()
            .is_some_and(|d| d.destination_room_visited(&self.data, self.region_state.as_ref()))
    }
}

/// What `OverseersWorldAI.Update` reads from the story session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryProgress<'a> {
    /// `playerSessionRecords[0].wokeUpInRegion`.
    pub woke_up_in_region: &'a str,
    /// `miscWorldSaveData.EverMetMoon` (false in a new game).
    pub ever_met_moon: bool,
    /// `deathPersistentSaveData.karma`.
    pub karma: i32,
}

/// A search cell: local room index and exit node (`IntVector2`).
type Cell = (usize, i32);

/// `OverseersWorldAI`.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseersWorldAi {
    pub shelter_finder: ShelterFinder,
    pub direction_finder: Option<DirectionFinder>,
    tried_add_progression_finder: bool,
    /// Set by `SetAsPlayerGuide`; cleared by [`Self::ditch_direction_guidance`].
    pub guide_player_to_next_region: bool,
    pub player_guide: Option<CreatureId>,
}

impl OverseersWorldAi {
    #[must_use]
    pub fn new(data: &WorldData) -> Self {
        Self {
            shelter_finder: ShelterFinder::new(data),
            direction_finder: None,
            tried_add_progression_finder: false,
            guide_player_to_next_region: false,
            player_guide: None,
        }
    }

    /// `OverseerAbstractAI.SetAsPlayerGuide`'s part on the world process.
    pub fn set_player_guide(&mut self, guide: CreatureId) {
        self.player_guide = Some(guide);
        self.guide_player_to_next_region = true;
    }

    /// `Update`: a shelter finder step; on the first update, the direction finder when
    /// guiding and the player woke up in this region (`story` is `None` outside a story
    /// session or without players), else a direction finder step.
    ///
    /// Remix's branch bringing the guide to the player in the tutorial rooms
    /// (`BringToRoomAndGuidePlayer`) and More Slugcats' `DynamicGuideSymbolUpdate` are
    /// not ported.
    pub fn update(
        &mut self,
        data: &WorldData,
        region_state: Option<&RegionState>,
        story: Option<StoryProgress<'_>>,
    ) {
        if !self.shelter_finder.done {
            self.shelter_finder.update(data);
        }
        match &mut self.direction_finder {
            Some(finder) => {
                if !finder.done {
                    finder.update(data);
                }
            }
            None if !self.tried_add_progression_finder => {
                self.tried_add_progression_finder = true;
                let region = data.region.as_ref().map(|r| r.name.as_str());
                if let Some(story) = story
                    && self.guide_player_to_next_region
                    && region == Some(story.woke_up_in_region)
                {
                    let finder = DirectionFinder::new(data, region_state, story);
                    if !finder.destroy {
                        self.direction_finder = Some(finder);
                    }
                } else {
                    log::info!("guide not taking player anywhere in this region");
                }
            }
            None => {}
        }
    }

    /// `DitchDirectionGuidance`.
    pub fn ditch_direction_guidance(&mut self) {
        self.guide_player_to_next_region = false;
        self.direction_finder = None;
    }
}

/// The finders' search graph over a world's exits.
trait ExitGraph {
    /// A `[room][exit]` matrix of -1 (each room's row as long as its connections).
    fn unmapped_exits(&self) -> Vec<Vec<f32>>;

    /// The neighbour of `cell` through its room's node `j`: the room behind the exit
    /// when `j` is the cell's own connected exit (the two tubes' length), else node `j`
    /// of the same room (the fly's path length to the cell's node, 10000 without one).
    /// `None` for an undefined node.
    fn exit_step(&self, cell: Cell, j: usize) -> (Option<Cell>, f32);
}

impl ExitGraph for WorldData {
    fn unmapped_exits(&self) -> Vec<Vec<f32>> {
        self.rooms
            .iter()
            .map(|r| vec![-1.0; r.connections.len()])
            .collect()
    }

    fn exit_step(&self, cell: Cell, j: usize) -> (Option<Cell>, f32) {
        let room = &self.rooms[cell.0];
        let connection = room.connections[j];
        if i32::try_from(j).ok() == Some(cell.1) && connection > -1 {
            let back = self
                .room(connection)
                .map_or(-1, |r| r.exit_index(room.index));
            let to = usize::try_from(connection - self.first_room_index())
                .ok()
                .zip(Some(back));
            let length = self.total_shortcut_length_between(room.index, connection);
            return (to.filter(|t| t.1 >= 0), length as f32);
        }
        let Some(node) = room.nodes.get(j) else {
            return (None, 10000.0);
        };
        let fly = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
        let length = node.connection_length(cell.1, fly.pre_baked_pathing_index());
        let length = if length == -1 { 10000.0 } else { length as f32 };
        (Some((cell.0, i32::try_from(j).unwrap_or(-1))), length)
    }
}

/// The value at `cell` of a `[room][exit]` matrix, -1 out of range (`ResistanceOfCell`).
struct Matrix<'a>(&'a [Vec<f32>]);

impl Matrix<'_> {
    fn get(&self, cell: Cell) -> f32 {
        usize::try_from(cell.1)
            .ok()
            .and_then(|n| self.0.get(cell.0)?.get(n))
            .copied()
            .unwrap_or(-1.0)
    }

    /// The position in `check` of the first cell with the least value above -1.
    fn cheapest(&self, check: &[Cell]) -> Option<usize> {
        let mut best = (f32::MAX, None);
        for (i, &c) in check.iter().enumerate() {
            let v = self.get(c);
            if v > -1.0 && v < best.0 {
                best = (v, Some(i));
            }
        }
        best.1
    }
}

/// `OverseersWorldAI.ShelterFinder`: for each shelter in turn (broken ones skipped), the
/// path length from every exit of the region to the shelter's exit 0.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelterFinder {
    /// `[shelter][room][exit]`.
    matrix: Vec<Vec<Vec<f32>>>,
    currently_mapping_shelter: i32,
    check_next: Vec<Cell>,
    pub done: bool,
}

impl ShelterFinder {
    #[must_use]
    pub fn new(data: &WorldData) -> Self {
        Self {
            matrix: data
                .shelters
                .iter()
                .map(|_| data.unmapped_exits())
                .collect(),
            currently_mapping_shelter: -1,
            check_next: Vec::new(),
            done: false,
        }
    }

    fn current(&self) -> usize {
        usize::try_from(self.currently_mapping_shelter).unwrap_or(0)
    }

    /// `[shelter][local room][exit]`, -1 where not reached.
    #[must_use]
    pub fn matrix(&self) -> &[Vec<Vec<f32>>] {
        &self.matrix
    }

    /// `checkNext`: (local room, exit) cells waiting.
    #[must_use]
    pub fn check_next(&self) -> &[(usize, i32)] {
        &self.check_next
    }

    /// The shelter list position being mapped, -1 before the first update.
    #[must_use]
    pub fn currently_mapping_shelter(&self) -> i32 {
        self.currently_mapping_shelter
    }

    /// One cell: the cheapest queued one spreads to its room's other exits and, through
    /// its own exit, into the room behind it.
    pub fn update(&mut self, data: &WorldData) {
        if self.done {
            return;
        }
        let shelter = self.current();
        let Some(i) = Matrix(&self.matrix[shelter]).cheapest(&self.check_next) else {
            self.next_shelter(data);
            return;
        };
        let cell = self.check_next.remove(i);
        let here = Matrix(&self.matrix[shelter]).get(cell);
        for j in 0..data.rooms[cell.0].connections.len() {
            let (to, length) = data.exit_step(cell, j);
            let Some(to) = to else { continue };
            let Ok(node) = usize::try_from(to.1) else {
                continue;
            };
            let slot = &mut self.matrix[shelter][to.0][node];
            if length > -1.0 && *slot == -1.0 {
                *slot = here + length;
                self.check_next.push(to);
            }
        }
    }

    /// `NextShelter`: seeds the next unbroken shelter's exit 0.
    fn next_shelter(&mut self, data: &WorldData) {
        loop {
            self.currently_mapping_shelter += 1;
            self.check_next.clear();
            let shelter = self.current();
            if shelter >= self.matrix.len() {
                self.done = true;
                return;
            }
            if data.broken_shelters.get(shelter).copied().unwrap_or(false) {
                log::info!("shelter {shelter} broken, not mapping");
                continue;
            }
            let Some(room) = data.room(data.shelters[shelter]) else {
                continue;
            };
            let local = usize::try_from(room.index - data.first_room_index()).unwrap_or(0);
            if let Some(slot) = self.matrix[shelter][local].first_mut() {
                *slot = 0.0;
                self.check_next.push((local, 0));
            }
            return;
        }
    }

    /// `DistanceToShelter`: -1 for an unmapped cell or a position outside the matrix
    /// (where the game throws).
    #[must_use]
    pub fn distance_to_shelter(
        &self,
        data: &WorldData,
        shelter: usize,
        pos: WorldCoordinate,
    ) -> f32 {
        let Some(matrix) = self.matrix.get(shelter) else {
            return -1.0;
        };
        let Ok(room) = usize::try_from(pos.room - data.first_room_index()) else {
            return -1.0;
        };
        Matrix(matrix).get((room, pos.node))
    }
}

/// `OverseersWorldAI.DirectionFinder`: the path length from every exit of the region to
/// the room the guide shows the way to (`showToRoom`).
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionFinder {
    /// `[room][exit]`.
    matrix: Vec<Vec<f32>>,
    check_next: Vec<Cell>,
    pub done: bool,
    /// Global index of the destination room, -1 if the world has none of that name.
    pub show_to_room: i32,
    /// The gate's karma requirement minus one (`locks.txt`), 0 without a gate symbol.
    pub min_karma: i32,
    destroy: bool,
    show_gate_symbol: bool,
}

impl DirectionFinder {
    /// The constructor: the destination by region (`EverMetMoon` picks the later route),
    /// its gate's karma requirement, and the destination room's exits seeded at 0.
    /// More Slugcats picks the destination with `getOverseerStoryGoalRoom` (its
    /// `StoryRegionPrioritys`), not ported.
    #[must_use]
    pub fn new(
        data: &WorldData,
        region_state: Option<&RegionState>,
        story: StoryProgress<'_>,
    ) -> Self {
        let mut finder = Self {
            matrix: data.unmapped_exits(),
            check_next: Vec::new(),
            done: false,
            show_to_room: -1,
            min_karma: 0,
            destroy: false,
            show_gate_symbol: true,
        };
        let region = data.region.as_ref().map_or("", |r| r.name.as_str());
        let met = story.ever_met_moon;
        let destination = match region {
            "SU" if !met => Some("GATE_SU_HI"),
            "HI" => Some(if met { "GATE_HI_SH" } else { "GATE_HI_GW" }),
            "GW" if !met => Some("GATE_GW_SL"),
            "SH" => Some(if met { "GATE_SH_UW" } else { "GATE_SH_SL" }),
            "SL" if met => Some("GATE_SH_SL"),
            "SL" => {
                finder.show_gate_symbol = false;
                Some("SL_AI")
            }
            _ => None,
        };
        let Some(destination) = destination else {
            finder.destroy = true;
            return finder;
        };
        if let Some(room) = data.room_by_name(destination) {
            finder.show_to_room = room.index;
        }
        if finder.show_gate_symbol {
            finder.min_karma = Self::gate_karma(data, destination, region).unwrap_or(0);
        }
        log::info!("guide to: {destination} karma req: {}", finder.min_karma);
        if story.karma < finder.min_karma && finder.destination_room_visited(data, region_state) {
            log::info!("progression direction finder killed: low karma, destination visited");
            finder.destroy = true;
        }
        // The game throws here when the destination room is missing.
        let Some(room) = data.room(finder.show_to_room) else {
            finder.destroy = true;
            return finder;
        };
        let local = usize::try_from(room.index - data.first_room_index()).unwrap_or(0);
        for m in 0..room.connections.len() {
            finder
                .check_next
                .push((local, i32::try_from(m).unwrap_or(-1)));
            finder.matrix[local][m] = 0.0;
        }
        finder
    }

    /// The first `locks.txt` line for `gate` that names `region` in the gate's name:
    /// that side's karma number minus one.
    fn gate_karma(data: &WorldData, gate: &str, region: &str) -> Option<i32> {
        for line in &data.gate_locks {
            if line.first().map(String::as_str) != Some(gate) {
                continue;
            }
            let side = gate.split('_').skip(1).position(|part| part == region);
            if let Some(side) = side {
                return Some(line.get(side + 1)?.trim().parse::<i32>().ok()? - 1);
            }
        }
        None
    }

    /// `[local room][exit]`, -1 where not reached.
    #[must_use]
    pub fn matrix(&self) -> &[Vec<f32>] {
        &self.matrix
    }

    /// `checkNext`: (local room, exit) cells waiting.
    #[must_use]
    pub fn check_next(&self) -> &[(usize, i32)] {
        &self.check_next
    }

    /// One cell, as [`ShelterFinder::update`] (only the exits that are also nodes).
    pub fn update(&mut self, data: &WorldData) {
        if self.done || self.destroy {
            return;
        }
        let Some(i) = Matrix(&self.matrix).cheapest(&self.check_next) else {
            self.done = true;
            return;
        };
        let cell = self.check_next.remove(i);
        let here = Matrix(&self.matrix).get(cell);
        let room = &data.rooms[cell.0];
        for j in 0..room.connections.len().min(room.nodes.len()) {
            let (to, length) = data.exit_step(cell, j);
            // An exit not answered by the room behind it: the game indexes node -1 and throws.
            let Some(to) = to else { continue };
            let Ok(node) = usize::try_from(to.1) else {
                continue;
            };
            let slot = &mut self.matrix[to.0][node];
            if length > -1.0 && *slot == -1.0 {
                *slot = here + length;
                self.check_next.push(to);
            }
        }
    }

    /// `DistanceToDestination`: -1 for an unmapped cell or a position outside the matrix
    /// (where the game throws).
    #[must_use]
    pub fn distance_to_destination(&self, data: &WorldData, pos: WorldCoordinate) -> f32 {
        let Ok(room) = usize::try_from(pos.room - data.first_room_index()) else {
            return -1.0;
        };
        Matrix(&self.matrix).get((room, pos.node))
    }

    /// `DestinationRoomVisisted`: the region state's `roomsVisited` holds the destination
    /// room's name (false without a region state, where the game throws).
    #[must_use]
    pub fn destination_room_visited(
        &self,
        data: &WorldData,
        region_state: Option<&RegionState>,
    ) -> bool {
        let Some(room) = data.room(self.show_to_room) else {
            return false;
        };
        region_state.is_some_and(|s| s.rooms_visited.contains(&room.name))
    }
}
