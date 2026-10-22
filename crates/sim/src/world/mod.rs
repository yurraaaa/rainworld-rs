//! The loaded world at run time (`World`): its abstract rooms, which of them are realized
//! (`AbstractRoom.RealizeRoom` / `Abstractize`, `World.activeRooms`), and the rooms
//! still loading.
//!
//! Loading is synchronous: a room realized during a tick finishes loading in that
//! tick's loading step (`RainWorldGame.Update` runs the `RoomPreparer`s there), where the
//! game spreads it over frames on a thread (docs/divergences/ D14). Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`.
//!
//! Mirrors: World.cs, AbstractRoom.cs, RoomPreparer.cs

mod abstract_ai;
mod abstract_ai_kinds;
mod abstract_path;
mod ai_map;
mod ai_mapper;
mod ai_modules;
mod creature_ai_map;
mod creatures;
mod dens;
mod flies_world_ai;
mod hearing;
mod lizards;
mod missions;
mod movement;
mod objects;
mod overseers_world_ai;
mod quantified;
mod quick_connectivity;
mod quick_path;
mod rain_cycle;
mod realized;
mod realizer;
mod region_save;
mod region_state;
mod room_water;
mod social_events;
mod swarm_rooms;
mod threat_flight;
mod tracking;
mod weapons;

use std::sync::Arc;

use rustc_hash::FxHashMap;
use rw_assets::world::{AbstractRoomData, RoomSource, WorldData};
use rw_assets::{ActiveMods, AssetError, CreatureType, RoomAttraction, Timeline};

use crate::Vec2;
use crate::creature::{AbstractCreature, CreatureId, FlockBehavior, IdIssuer, WorldCoordinate};
use crate::item::AbstractObject;
use rw_random::UnityRandom;

pub use abstract_path::{FloodMethod, SearchingFor};
pub(crate) use ai_map::EIGHT_DIRECTIONS;
pub use ai_map::{AiMap, AiTile};
pub use ai_mapper::AiMapper;
pub use creature_ai_map::CreatureSpecificAiMap;
pub(crate) use creatures::RoomLists;
pub use creatures::{AbstractTick, QuantifiedCount, WorldProcesses};
pub use flies_world_ai::{FliesTick, FliesWorldAi, SwarmBehavior};
pub(crate) use lizards::LizardRoomCx;
pub use movement::{Accessibility, Legality, MovementConnection, MovementType, PathCost};
pub use overseers_world_ai::{DirectionFinder, OverseersWorldAi, ShelterFinder, StoryProgress};
pub use quick_connectivity::QuickConnectivity;
pub use quick_path::{QuickPath, QuickPathFinder};
pub use rain_cycle::{CYCLE_TIME_MAX, CYCLE_TIME_MIN, RainCycle, WaterLevelCycle, WaterStage};
pub use realized::RealizedRoom;
pub use realizer::{FollowCreature, RoomRealizer};
pub use region_state::{ConsumedItem, RegionState};
pub use social_events::{OwnedItemOnGround, SocialEventId, SocialEventRecognizer};
pub use swarm_rooms::{SwarmRoomMap, SwarmRoomMapper};
pub use weapons::ItemTick;
pub(crate) use weapons::{OrphanLoop, UpdatePart};

/// One camera call a graphics module's internal container makes for `obj`'s sprites:
/// `MoveObjectToInternalContainer` into lizard `into`'s jaws, or
/// `MoveObjectToContainer(obj, null)` back on top of its default containers (`None`).
/// `seq` is the update-list counter at the call, placing it among the room's
/// `AddObject`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteMove {
    pub seq: u64,
    pub obj: crate::item::Entity,
    pub into: Option<crate::creature::CreatureId>,
}

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
    attractions: Vec<FxHashMap<CreatureType, RoomAttraction>>,
    /// `skyAccessNodes`.
    sky_access_nodes: Vec<WorldCoordinate>,
    pub(crate) processes: WorldProcesses,
    /// `regionState`: a story world's that is not a single room (a new game's).
    pub region_state: Option<RegionState>,
    /// `rainCycle` (built with the game, see [`crate::RainWorldGame::new`]).
    pub rain_cycle: RainCycle,
    /// The last `updateList` position handed out (`Room.AddObject` appends).
    update_seq: u64,
    /// Loop emitters whose item left the room (swallowed), until their next update.
    pub(crate) orphan_loops: Vec<OrphanLoop>,
    /// Creatures realized by `ChangeRooms` into a realized room, waiting to join the
    /// shortcut handler's lobby (`CreatureEnterFromAbstractRoom`).
    pub(crate) entering: Vec<(CreatureId, WorldCoordinate)>,
    /// The last grasp or stick serial handed out.
    pub(crate) grasp_serial: u64,
    /// The last `OwnedItemOnGround` serial handed out.
    pub(crate) owned_item_serial: u64,
    /// The graphics modules' internal-container moves since the last take, in order.
    pub(crate) sprite_moves: Vec<SpriteMove>,
    /// The realized player's `dead` flag (its body lives outside the world).
    pub(crate) player_dead: bool,
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
    /// The Hunter's `AbstractOnBackStick`s.
    pub(crate) on_back_sticks: Vec<crate::item::OnBackStick>,
    /// Spears pulled out of creatures since the game last played their
    /// `Spear_Dislodged_From_Creature` (room, position).
    pub(crate) dislodged: Vec<(i32, Vec2)>,
    /// What the session keeps of the creatures that die (kills, respawns).
    pub session: crate::creature::SessionDeaths,
    /// `session.creatureCommunities`: the player's standing, changed at once by the
    /// session's influences (the save applies them again at `Win`).
    pub communities: rw_save::CreatureCommunities,
    /// `session.difficulty`.
    pub session_difficulty: f32,
    /// The realized AIs' path finders, their accessibility mappers and the frame's
    /// `PathfinderResourceDivider` (the game's).
    pub(crate) path_finders: crate::creature::PathFinders,
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
            orphan_loops: Vec::new(),
            entering: Vec::new(),
            grasp_serial: 0,
            owned_item_serial: 0,
            sprite_moves: Vec::new(),
            player_dead: false,
            flocks: Vec::new(),
            drip_ids: 0,
            player_body: None,
            player_mass: crate::slugcat::CHUNK_MASS * 2.0,
            loaded_creatures: Vec::new(),
            spear_sticks: Vec::new(),
            on_back_sticks: Vec::new(),
            dislodged: Vec::new(),
            session: crate::creature::SessionDeaths::default(),
            communities: rw_save::CreatureCommunities::new(&[]),
            session_difficulty: 0.0,
            path_finders: crate::creature::PathFinders::default(),
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

    /// `Room.AddObject` of a drip that took update-list place `seq`.
    pub(crate) fn add_drip_at(&mut self, room: i32, mut drip: crate::WaterDrip, seq: u64) {
        self.drip_ids += 1;
        drip.id = self.drip_ids;
        drip.room = room;
        drip.update_order = seq;
        if let Some(r) = self.realized_mut(room) {
            r.drips.push(drip);
        }
    }

    /// The update-list counter (`next_update_seq`'s last value).
    #[must_use]
    pub(crate) fn update_seq(&self) -> u64 {
        self.update_seq
    }

    pub(crate) fn set_update_seq(&mut self, seq: u64) {
        self.update_seq = seq;
    }

    /// Light source `id` in whichever realized room holds it.
    #[must_use]
    pub(crate) fn light_source(&self, id: u64) -> Option<&crate::light_source::LightSource> {
        self.realized
            .iter()
            .flatten()
            .find_map(|r| r.light_sources.iter().find(|l| l.id == id))
    }

    pub(crate) fn light_source_mut(
        &mut self,
        id: u64,
    ) -> Option<&mut crate::light_source::LightSource> {
        self.realized
            .iter_mut()
            .flatten()
            .find_map(|r| r.light_sources.iter_mut().find(|l| l.id == id))
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
    pub fn abstractize(&mut self, index: i32, rng: &mut UnityRandom) {
        let Some(i) = self.local(index) else { return };
        if self.realized[i].is_some() {
            self.abstractize_objects(index);
            self.abstractize_creatures(index, rng);
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
