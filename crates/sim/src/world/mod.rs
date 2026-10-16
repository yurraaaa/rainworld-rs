//! The loaded world at run time (`World`): its abstract rooms, which of them are realized
//! (`AbstractRoom.RealizeRoom` / `Abstractize`, `World.activeRooms`), and the rooms
//! still loading.
//!
//! Loading is synchronous: a room realized during a tick finishes loading in that
//! tick's loading step (`RainWorldGame.Update` runs the `RoomPreparer`s there), where the
//! game spreads it over frames on a thread (docs/DIVERGENCES.md D14). Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`.

mod abstract_ai;
mod abstract_path;
mod ai_map;
mod ai_mapper;
mod creature_ai_map;
mod creatures;
mod flies_world_ai;
mod movement;
mod objects;
mod overseers_world_ai;
mod quantified;
mod rain_cycle;
mod realized;
mod realizer;
mod region_save;
mod region_state;
mod swarm_rooms;
mod weapons;

use std::collections::HashMap;
use std::sync::Arc;

use rw_assets::world::{AbstractRoomData, RoomSource, WorldData};
use rw_assets::{ActiveMods, AssetError, CreatureType, RoomAttraction, Timeline};

use crate::Vec2;
use crate::creature::{AbstractCreature, CreatureId, FlockBehavior, IdIssuer, WorldCoordinate};
use crate::item::AbstractObject;
use rw_random::UnityRandom;

pub use abstract_path::{FloodMethod, SearchingFor};
pub(crate) use ai_map::EIGHT_DIRECTIONS;
pub use ai_map::{AiMap, AiTile};
pub use creature_ai_map::CreatureSpecificAiMap;
pub(crate) use creatures::RoomLists;
pub use creatures::{AbstractTick, QuantifiedCount, WorldProcesses};
pub use flies_world_ai::{FliesTick, FliesWorldAi, SwarmBehavior};
pub use movement::{Accessibility, Legality, MovementConnection, MovementType, PathCost};
pub use overseers_world_ai::{DirectionFinder, OverseersWorldAi, ShelterFinder, StoryProgress};
pub use rain_cycle::{CYCLE_TIME_MAX, CYCLE_TIME_MIN, RainCycle, WaterLevelCycle, WaterStage};
pub use realized::RealizedRoom;
pub use realizer::{FollowCreature, RoomRealizer};
pub use region_state::{ConsumedItem, RegionState};
pub use swarm_rooms::{SwarmRoomMap, SwarmRoomMapper};
pub use weapons::ItemTick;
pub(crate) use weapons::UpdatePart;

/// A world and its realized rooms.
pub struct World {
    pub(crate) data: WorldData,
    /// Per abstract room (local index), its realized room.
    realized: Vec<Option<RealizedRoom>>,
    /// `activeRooms`: local indices in realization order.
    active: Vec<usize>,
    /// Realized this tick, loaded in the loading step.
    loading: Vec<usize>,
    rooms: Arc<dyn RoomSource>,
    timeline: Timeline,
    pub(crate) mods: ActiveMods,
    /// A story session (`game.IsStorySession`), for the room rules that ask.
    pub(crate) story: bool,
    /// Per abstract room, whether it was realized before this cycle (`!firstTimeRealized`).
    loaded_before: Vec<bool>,
    /// Every creature, the player's included (`AbstractCreature`s by `CreatureId`).
    creatures: Vec<AbstractCreature>,
    /// Every other physical object (`AbstractPhysicalObject`s by `ObjectId`).
    objects: Vec<AbstractObject>,
    /// Per abstract room, its creature lists.
    pub(crate) lists: Vec<RoomLists>,
    /// Per abstract room, `roomAttractions` from `Room_Attr` lines.
    attractions: Vec<HashMap<CreatureType, RoomAttraction>>,
    /// `skyAccessNodes`.
    sky_access_nodes: Vec<WorldCoordinate>,
    pub(crate) processes: WorldProcesses,
    /// `regionState`: a story world's that is not a single room (a new game's).
    pub region_state: Option<RegionState>,
    /// `rainCycle` (built with the game, see [`crate::RainWorldGame::new`]).
    pub rain_cycle: RainCycle,
    /// The last `updateList` position handed out (`Room.AddObject` appends).
    update_seq: u64,
    /// Creatures realized by `ChangeRooms` into a realized room, waiting to join the
    /// shortcut handler's lobby (`CreatureEnterFromAbstractRoom`).
    pub(crate) entering: Vec<(CreatureId, WorldCoordinate)>,
    /// The last grasp or stick serial handed out.
    pub(crate) grasp_serial: u64,
    /// The flies' `FlockBehavior`s, shared by reference in the game, by creation order.
    pub flocks: Vec<FlockBehavior>,
    /// The last `WaterDrip` id handed out.
    drip_ids: u64,
    /// The slugcat's body as the flies' graphics read their grabber, kept current by the
    /// game.
    pub(crate) player_body: Option<crate::creature::fly::PlayerBody>,
    /// The realized slugcat's `TotalMass` (`0.7 * bodyWeightFac`), kept current by the game.
    pub(crate) player_mass: f32,
    /// `RegionState.loadedCreatures`: creatures read from the save, until saved again.
    loaded_creatures: Vec<CreatureId>,
    /// The spears' sticks into creatures (both ends' `stuckObjects`).
    pub(crate) spear_sticks: Vec<crate::item::SpearStick>,
    /// What the session keeps of the creatures that die (kills, respawns).
    pub session: crate::creature::SessionDeaths,
}

impl World {
    #[must_use]
    pub fn new(
        data: WorldData,
        rooms: Arc<dyn RoomSource>,
        timeline: Timeline,
        mods: ActiveMods,
        story: bool,
    ) -> Self {
        let count = data.rooms.len();
        let region_state = (story && !data.single_room).then(|| RegionState::new_game(&data));
        let mut world = Self {
            data,
            realized: (0..count).map(|_| None).collect(),
            active: Vec::new(),
            loading: Vec::new(),
            rooms,
            timeline,
            mods,
            story,
            loaded_before: vec![false; count],
            creatures: Vec::new(),
            objects: Vec::new(),
            lists: vec![RoomLists::default(); count],
            attractions: Vec::new(),
            sky_access_nodes: Vec::new(),
            processes: WorldProcesses::default(),
            region_state,
            rain_cycle: RainCycle::default(),
            update_seq: 0,
            entering: Vec::new(),
            grasp_serial: 0,
            flocks: Vec::new(),
            drip_ids: 0,
            player_body: None,
            player_mass: crate::slugcat::CHUNK_MASS * 2.0,
            loaded_creatures: Vec::new(),
            spear_sticks: Vec::new(),
            session: crate::creature::SessionDeaths::default(),
        };
        world.index_rooms();
        world
    }

    /// A position at the end of a room's `updateList` (`Room.AddObject`).
    pub(crate) fn next_update_seq(&mut self) -> u64 {
        self.update_seq += 1;
        self.update_seq
    }

    /// `Room.AddObject` of a `WaterDrip` in realized room `room` (dropped when the room is
    /// not realized). Returns its id.
    pub(crate) fn add_drip(&mut self, room: i32, mut drip: crate::WaterDrip) -> u64 {
        self.drip_ids += 1;
        drip.id = self.drip_ids;
        drip.room = room;
        drip.update_order = self.next_update_seq();
        if let Some(r) = self.realized_mut(room) {
            r.drips.push(drip);
        }
        self.drip_ids
    }

    /// `World.RoomToWorldPos`: a room position on the region map.
    #[must_use]
    pub fn room_to_world_pos(&self, in_room: Vec2, room: i32) -> Vec2 {
        let Some(data) = self.abstract_room(room) else {
            return Vec2::ZERO;
        };
        let map = Vec2::new(data.map_pos[0], data.map_pos[1]);
        let size = Vec2::new(data.size.0 as f32 * 20.0, data.size.1 as f32 * 20.0);
        (map / 3.0 + Vec2::new(10.0, 10.0)) * 20.0 + in_room - size / 2.0
    }

    #[must_use]
    pub fn mods(&self) -> ActiveMods {
        self.mods
    }

    #[must_use]
    pub fn data(&self) -> &WorldData {
        &self.data
    }

    /// `World.name`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.data.name
    }

    #[must_use]
    pub fn single_room(&self) -> bool {
        self.data.single_room
    }

    /// `World.firstRoomIndex`.
    #[must_use]
    pub fn first_room_index(&self) -> i32 {
        self.data.first_room_index()
    }

    /// `World.NumberOfRooms`, the off-screen den included.
    #[must_use]
    pub fn number_of_rooms(&self) -> usize {
        self.data.rooms.len()
    }

    /// Local index of global room `index`.
    fn local(&self, index: i32) -> Option<usize> {
        let i = usize::try_from(index.checked_sub(self.first_room_index())?).ok()?;
        (i < self.data.rooms.len()).then_some(i)
    }

    /// `GetAbstractRoom(index)`.
    #[must_use]
    pub fn abstract_room(&self, index: i32) -> Option<&AbstractRoomData> {
        self.data.room(index)
    }

    /// Global index of the room named `name`.
    #[must_use]
    pub fn room_index(&self, name: &str) -> Option<i32> {
        self.data.room_named(name).map(|r| r.index)
    }

    /// The realized room of global room `index`.
    #[must_use]
    pub fn realized(&self, index: i32) -> Option<&RealizedRoom> {
        self.realized.get(self.local(index)?)?.as_ref()
    }

    pub fn realized_mut(&mut self, index: i32) -> Option<&mut RealizedRoom> {
        let i = self.local(index)?;
        self.realized.get_mut(i)?.as_mut()
    }

    /// Global indices of the active rooms, in `activeRooms` order.
    #[must_use]
    pub fn active_rooms(&self) -> Vec<i32> {
        self.active
            .iter()
            .map(|&i| self.data.rooms[i].index)
            .collect()
    }

    /// `World.ActivateRoom` / `AbstractRoom.RealizeRoom`: reads the room and adds it to
    /// the active rooms, loading until this tick's loading step. On its first
    /// realization in the cycle (`firstTimeRealized`, cleared at the end of
    /// `Room.Loaded`) the room makes its items, with their IDs and draws (a story's
    /// `MultiplayerItem`s on every realization). The off-screen
    /// den and realized rooms are left alone.
    ///
    /// # Errors
    /// When the room's files cannot be read.
    pub fn activate_room(
        &mut self,
        index: i32,
        ids: &mut IdIssuer,
        rng: &mut UnityRandom,
    ) -> Result<(), AssetError> {
        let Some(i) = self.local(index) else {
            return Ok(());
        };
        let data = &self.data.rooms[i];
        if self.realized[i].is_some() || data.offscreen_den {
            return Ok(());
        }
        let loaded = self.rooms.load_room(data.file_name())?;
        log::info!("realized room {}", data.name);
        let first_time = !self.loaded_before[i];
        self.place_room_items(index, &loaded.room, &loaded.settings, first_time, ids, rng);
        self.loaded_before[i] = true;
        let data = &self.data.rooms[i];
        self.realized[i] = Some(RealizedRoom::new(
            loaded,
            data,
            self.story,
            self.timeline,
            self.mods,
            first_time,
        ));
        self.active.push(i);
        self.loading.push(i);
        Ok(())
    }

    /// `AbstractRoom.Abstractize`: its realized creatures and objects become abstract,
    /// then the realized room is dropped.
    pub fn abstractize(&mut self, index: i32) {
        let Some(i) = self.local(index) else { return };
        if self.realized[i].is_some() {
            self.abstractize_objects(index);
            self.abstractize_creatures(index);
            self.count_quantified_creatures(index);
        }
        if self.realized[i].take().is_some() {
            log::info!("abstracted room {}", self.data.rooms[i].name);
        }
        self.active.retain(|&a| a != i);
        self.loading.retain(|&a| a != i);
    }

    /// The loading step: rooms realized this tick finish loading (`ShortCutsReady`,
    /// `ReadyForAI`). Returns their global indices.
    pub fn finish_loading(&mut self) -> Vec<i32> {
        let mut done = Vec::new();
        for i in std::mem::take(&mut self.loading) {
            if let Some(room) = &mut self.realized[i] {
                let data = &self.data.rooms[i];
                room.finish_loading(data.index, data.shelter() || data.gate());
                done.push(self.data.rooms[i].index);
            }
        }
        done
    }
}
