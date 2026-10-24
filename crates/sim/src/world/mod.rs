//! The loaded world at run time (`World`): its abstract rooms, which of them are realized
//! (`AbstractRoom.RealizeRoom` / `Abstractize`, `World.activeRooms`), and the rooms
//! still loading.
//!
//! A realized room joins `loadingRooms` with a [`RoomPreparer`], whose stages the
//! loading step hands over (`RainWorldGame.Update` runs the preparers there). The
//! schedule is the game's minimum ([`LoadingSchedule::GAME_MINIMUM`]): stage 1 a tick
//! after the room is activated, stage 2 the tick after, where the game's thread may take
//! longer (docs/divergences/ D14). Behaviour: `rainworld-re/notes/world-loading.md`, `room-realizer.md`,
//! `rainworld-re/surveys/async-room-loading.md`.
//!
//! Mirrors: World.cs, AbstractRoom.cs

pub(crate) mod ai_map;
mod creatures;
mod dens;
mod global_rain;
mod objects;
mod quantified;
mod rain_cycle;
mod realized;
mod realizer;
mod region_state;
mod room_preparer;
mod room_rain;
mod room_rain_sound;

use std::sync::Arc;

use rustc_hash::FxHashMap;
use rw_assets::world::{AbstractRoomData, RoomSource, WorldData};
use rw_assets::{ActiveMods, AssetError, CreatureType, RoomAttraction, Timeline};

use crate::Vec2;
use crate::creature::{
    AbstractCreature, CreatureId, CreatureTemplate, FlockBehavior, WorldCoordinate,
};
use crate::item::{AbstractObject, Entity};
use rw_random::UnityRandom;

pub use crate::creature::abstract_ai::path_finder::{FloodMethod, SearchingFor};
pub use crate::creature::fly::swarm_rooms::{SwarmRoomMap, SwarmRoomMapper};
pub use crate::creature::fly::world_ai::{FliesTick, FliesWorldAi, SwarmBehavior};
pub(crate) use crate::creature::lizard::world::LizardRoomCx;
pub use crate::creature::overseer::world_ai::{
    DirectionFinder, OverseersWorldAi, ShelterFinder, StoryProgress,
};
pub use crate::creature::social::social_events::{
    OwnedItemOnGround, SocialEventId, SocialEventRecognizer,
};
pub use crate::item::weapon_world::ItemTick;
pub(crate) use crate::item::weapon_world::{LayerZero, OrphanLoop, UpdatePart};
pub use crate::world::ai_map::ai_mapper::AiMapper;
pub use crate::world::ai_map::creature_ai_map::CreatureSpecificAiMap;
pub use crate::world::ai_map::movement::{
    Accessibility, Legality, MovementConnection, MovementType, PathCost,
};
pub use crate::world::ai_map::quick_connectivity::QuickConnectivity;
pub use crate::world::ai_map::quick_path::{QuickPath, QuickPathFinder};
pub(crate) use ai_map::EIGHT_DIRECTIONS;
pub use ai_map::{AiMap, AiTile};
pub(crate) use creatures::RoomLists;
pub use creatures::{AbstractTick, QuantifiedCount, WorldProcesses};
pub use global_rain::{DeathRain, DeathRainMode, GlobalRain, PassiveRain, RainTick};
pub use rain_cycle::{CYCLE_TIME_MAX, CYCLE_TIME_MIN, RainCycle, WaterLevelCycle, WaterStage};
pub use realized::RealizedRoom;
pub use realizer::{FollowCreature, RoomRealizer};
pub use region_state::{ConsumedItem, RegionState};
pub use room_preparer::{LoadingSchedule, PreparerStage, PreparerStatus, RoomPreparer};
pub use room_rain::{ChunkPush, PushAround, RoomRain, SmashView};
pub use room_rain_sound::{RainLoops, RainSoundInput};

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
    pub(crate) realized: Vec<Option<RealizedRoom>>,
    /// `activeRooms`: local indices in realization order.
    active: Vec<usize>,
    /// `loadingRooms`, oldest first.
    loading: Vec<RoomPreparer>,
    /// How many loading-step calls each stage takes.
    pub loading_schedule: LoadingSchedule,
    /// Rooms realized since the game last ran their `Room.Loaded` tail (global indices).
    pub(crate) pending_loaded: Vec<i32>,
    rooms: Arc<dyn RoomSource>,
    timeline: Timeline,
    pub(crate) mods: ActiveMods,
    /// `rainWorld.screenSize`, set by the app from the chosen resolution.
    screen_size: Vec2,
    /// A story session (`game.IsStorySession`), for the room rules that ask.
    pub(crate) story: bool,
    /// Per abstract room, whether it was realized before this cycle (`!firstTimeRealized`).
    loaded_before: Vec<bool>,
    /// Every creature, the player's included (`AbstractCreature`s by `CreatureId`).
    creatures: Vec<AbstractCreature>,
    /// Every other physical object (`AbstractPhysicalObject`s by `ObjectId`).
    pub(crate) objects: Vec<AbstractObject>,
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
    pub(crate) update_seq: u64,
    /// Loop emitters whose item left the room (swallowed), until their next update.
    pub(crate) orphan_loops: Vec<OrphanLoop>,
    /// Creatures realized by `ChangeRooms` into a realized room, waiting to join the
    /// shortcut handler's lobby (`CreatureEnterFromAbstractRoom`).
    pub(crate) entering: Vec<(CreatureId, WorldCoordinate)>,
    /// The last grasp or stick serial handed out.
    pub(crate) grasp_serial: u64,
    /// The last `OwnedItemOnGround` serial handed out.
    pub(crate) owned_item_serial: u64,
    /// The last room realization serial handed out ([`RealizedRoom::serial`]).
    room_serial: u64,
    /// The graphics modules' internal-container moves since the last take, in order.
    pub(crate) sprite_moves: Vec<SpriteMove>,
    /// `CanBePulledIntoShortcut(player)` after the last grasp change on a slugcat, until
    /// the game copies it into the player's chunks (`sync_player_pulled`).
    pub(crate) player_pulled: Option<bool>,
    /// The realized player's `dead` flag (its body lives outside the world).
    pub(crate) player_dead: bool,
    /// The flies' `FlockBehavior`s, shared by reference in the game, by creation order.
    pub flocks: Vec<FlockBehavior>,
    /// The last `WaterDrip` id handed out.
    pub(crate) drip_ids: u64,
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
    /// Every `Mycelium` of the world's rooms (wall mycelia, overseers' lenses).
    pub(crate) mycelia: crate::MyceliumPool,
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
            loading_schedule: LoadingSchedule::default(),
            pending_loaded: Vec::new(),
            rooms,
            timeline,
            mods,
            screen_size: crate::effects::sky::background_scene::SCREEN_SIZE,
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
            room_serial: 0,
            sprite_moves: Vec::new(),
            player_pulled: None,
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
            mycelia: crate::MyceliumPool::default(),
        };
        world.index_rooms();
        world
    }

    /// Every `Mycelium` of the world's rooms.
    #[must_use]
    pub fn mycelia(&self) -> &crate::MyceliumPool {
        &self.mycelia
    }

    #[must_use]
    pub fn screen_size(&self) -> Vec2 {
        self.screen_size
    }

    /// Sets `options.ScreenSize` before the rooms that read it load.
    pub fn set_screen_size(&mut self, size: Vec2) {
        self.screen_size = size;
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
    pub(crate) fn light_source(
        &self,
        id: u64,
    ) -> Option<&crate::effects::lights::light_source::LightSource> {
        self.realized
            .iter()
            .flatten()
            .find_map(|r| r.light_sources.iter().find(|l| l.id == id))
    }

    pub(crate) fn light_source_mut(
        &mut self,
        id: u64,
    ) -> Option<&mut crate::effects::lights::light_source::LightSource> {
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
    pub(crate) fn local(&self, index: i32) -> Option<usize> {
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

    /// `World.ActivateRoom` / `AbstractRoom.RealizeRoom`: reads the room, adds it to
    /// the active rooms and gives it a preparer. The rest of `LoadFromDataString` and
    /// `Room.Loaded` (with the items) is the game's (`RainWorldGame::rooms_loaded`), run
    /// before anything else draws. The off-screen
    /// den and realized rooms are left alone.
    ///
    /// # Errors
    /// When the room's files cannot be read.
    pub fn activate_room(&mut self, index: i32) -> Result<(), AssetError> {
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
        self.loaded_before[i] = true;
        let data = &self.data.rooms[i];
        let shelter_or_gate = data.shelter() || data.gate();
        let mut realized = RealizedRoom::new(
            loaded,
            shelter_or_gate,
            self.timeline,
            self.mods,
            first_time,
        );
        self.room_serial += 1;
        realized.serial = self.room_serial;
        self.realized[i] = Some(realized);
        self.active.push(i);
        self.loading
            .push(RoomPreparer::new(index, self.room_serial));
        self.pending_loaded.push(index);
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
        if let Some(room) = self.realized[i].take() {
            if let Some(system) = room.coral {
                for wall in &system.wall_mycelia {
                    self.mycelia.release_owner(wall.owner);
                }
                for neuron in &system.neurons {
                    self.mycelia.release_owner(neuron.owner);
                }
                for segment in system.stems.iter().flat_map(|stem| &stem.segments) {
                    self.mycelia.release_owner(segment.owner);
                }
                for connector in system.circuits.iter().flat_map(|c| &c.connectors) {
                    self.mycelia.release_owner(connector.owner);
                }
                self.mycelia.drop_registry(system.id);
            }
            log::info!("abstracted room {}", self.data.rooms[i].name);
        }
        self.active.retain(|&a| a != i);
        // The preparer stays in `loadingRooms` (`Abstractize` leaves it, D14), handing
        // its stages to the dropped room.
        self.pending_loaded.retain(|&r| r != index);
    }

    /// The loading step's `loadingRooms` loop, newest first: a done preparer is removed,
    /// every other one is updated once. Returns the stages handed over, in order (the
    /// game runs each in its preparer's call; the calls do nothing else).
    pub fn update_loading_rooms(&mut self) -> Vec<(i32, u64, PreparerStage)> {
        let schedule = self.loading_schedule;
        let mut stages = Vec::new();
        for k in (0..self.loading.len()).rev() {
            if self.loading[k].done() {
                self.loading.remove(k);
                continue;
            }
            let (room, serial) = (self.loading[k].room, self.loading[k].serial);
            stages.extend(
                self.loading[k]
                    .update(schedule)
                    .into_iter()
                    .map(|s| (room, serial, s)),
            );
        }
        if schedule == LoadingSchedule::Immediate {
            self.loading.retain(|p| !p.done());
        }
        stages
    }

    /// `loadingRooms`, oldest first.
    #[must_use]
    pub fn loading_rooms(&self) -> &[RoomPreparer] {
        &self.loading
    }

    /// The preparer of global room `index`'s realization while it loads.
    #[must_use]
    pub fn room_preparer(&self, index: i32) -> Option<&RoomPreparer> {
        let serial = self.realized(index)?.serial;
        self.loading.iter().find(|p| p.serial == serial)
    }

    /// Whether `serial` is global room `index`'s live realization.
    #[must_use]
    pub fn is_realization(&self, index: i32, serial: u64) -> bool {
        self.realized(index).is_some_and(|r| r.serial == serial)
    }

    /// The preparers whose room was abstracted while it loaded, oldest first.
    pub fn orphaned_preparers(&self) -> impl Iterator<Item = &RoomPreparer> {
        self.loading
            .iter()
            .filter(|p| !self.is_realization(p.room, p.serial))
    }

    /// Loading steps until no preparer is left ([`Self::loading_step`], world-only tests).
    #[cfg(test)]
    pub(crate) fn finish_loading(&mut self) {
        while !self.loading.is_empty() {
            self.loading_step();
        }
    }

    /// One loading step with the stages' room-side parts, without the game's creatures,
    /// objects and listeners (world-only tests).
    #[cfg(test)]
    pub(crate) fn loading_step(&mut self) {
        for (index, serial, stage) in self.update_loading_rooms() {
            if !self.is_realization(index, serial) {
                continue;
            }
            match stage {
                PreparerStage::ShortcutsReady => {
                    if let Some(r) = self.realized_mut(index) {
                        r.make_shortcuts_blinking();
                        r.set_shortcuts_ready(false);
                    }
                }
                PreparerStage::ReadyForAi => {
                    self.build_ai_map(index);
                    if let Some(r) = self.realized_mut(index) {
                        r.begin_ready_for_ai();
                        r.end_ready_for_ai();
                    }
                }
            }
        }
    }

    /// The thread's `AImapper` result for global room `index`, as `ReadyForAI` finds it.
    pub(crate) fn build_ai_map(&mut self, index: i32) {
        let Some(i) = self.local(index) else { return };
        let data = &self.data.rooms[i];
        let shelter_or_gate = data.shelter() || data.gate();
        if let Some(r) = &mut self.realized[i] {
            r.build_ai_map(index, shelter_or_gate);
        }
    }

    /// `AllowedToExistInRoom` of creature `id` in global room `index`: `readyForAI`, or
    /// `shortCutsReady` for a template without `AI`.
    #[must_use]
    pub fn allowed_to_exist_in_room(&self, id: CreatureId, index: i32) -> bool {
        self.realized(index).is_some_and(|r| {
            r.ready_for_ai() || (r.shortcuts_ready() && !self.creature(id).template.ai)
        })
    }

    /// `ShortcutHandler.VesselAllowedInRoom` for a vessel carrying creature `id` into
    /// global room `index`: the room is realized and `ReadyForPlayer`, and every
    /// creature connected to `id` is `CreatureAllowedInRoom` (just `id` when nothing is
    /// stuck to it).
    #[must_use]
    pub fn vessel_allowed_in_room(&self, id: CreatureId, index: i32) -> bool {
        if !self
            .realized(index)
            .is_some_and(RealizedRoom::ready_for_player)
        {
            return false;
        }
        self.connected_objects(Entity::Creature(id))
            .iter()
            .filter_map(|e| e.creature())
            .all(|c| self.creature_allowed_in_room(c, index))
    }

    /// `ShortcutHandler.CreatureAllowedInRoom`: the slugcat once the room is
    /// `ReadyForPlayer`; another creature once it is `readyForAI`, or before that once it
    /// is open to creatures without `AI` and it does not `RequiresAIMapToEnterRoom`.
    #[must_use]
    pub fn creature_allowed_in_room(&self, id: CreatureId, index: i32) -> bool {
        let Some(r) = self.realized(index) else {
            return false;
        };
        if self.creature(id).kind() == CreatureType::Slugcat {
            return r.ready_for_player();
        }
        if !r.ready_for_ai() {
            return r.ready_for_non_ai_creatures() && !self.requires_ai_map_to_enter_room(id);
        }
        true
    }

    /// `AbstractCreature.RequiresAIMapToEnterRoom`: its template or a creature connected
    /// to it `requireAImap`.
    #[must_use]
    pub fn requires_ai_map_to_enter_room(&self, id: CreatureId) -> bool {
        self.creature(id).template.require_ai_map
            || self
                .connected_objects(Entity::Creature(id))
                .iter()
                .filter_map(|e| e.creature())
                .any(|c| self.creature(c).template.require_ai_map)
    }

    /// `ShortCutsReady`'s `flag2`: a creature of room `index` not in a den whose template
    /// `requireAImap`, or a counted quantified type that does.
    #[must_use]
    pub fn room_creatures_require_ai_map(&self, index: i32) -> bool {
        let creature = self.room_creatures(index).iter().any(|&id| {
            let c = self.creature(id);
            !c.in_den && c.template.require_ai_map
        });
        creature
            || CreatureTemplate::quantified_templates(self.mods)
                .iter()
                .any(|t| {
                    t.require_ai_map
                        && self.number_of_quantified_creature_in_room(index, t.kind) > 0
                })
    }
}
