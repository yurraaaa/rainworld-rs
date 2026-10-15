//! The loaded world at run time (`World`): its abstract rooms, which of them are realized
//! (`AbstractRoom.RealizeRoom` / `Abstractize`, `World.activeRooms`), and the rooms
//! still loading.
//!
//! Loading is synchronous: a room realized during a tick finishes loading in that
//! tick's loading step (`RainWorldGame.Update` runs the `RoomPreparer`s there), where the
//! game spreads it over frames on a thread (docs/DIVERGENCES.md D14). Behaviour:
//! `rainworld-re/notes/world-loading.md`, `room-realizer.md`.

mod realized;
mod realizer;

use std::sync::Arc;

use rw_assets::world::{AbstractRoomData, RoomSource, WorldData};
use rw_assets::{ActiveMods, AssetError, Timeline};

pub use realized::RealizedRoom;
pub use realizer::{FollowCreature, RoomRealizer};

/// A world and its realized rooms.
pub struct World {
    data: WorldData,
    /// Per abstract room (local index), its realized room.
    realized: Vec<Option<RealizedRoom>>,
    /// `activeRooms`: local indices in realization order.
    active: Vec<usize>,
    /// Realized this tick, loaded in the loading step.
    loading: Vec<usize>,
    rooms: Arc<dyn RoomSource>,
    timeline: Timeline,
    mods: ActiveMods,
    /// A story session (`game.IsStorySession`), for the room rules that ask.
    story: bool,
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
        Self {
            data,
            realized: (0..count).map(|_| None).collect(),
            active: Vec::new(),
            loading: Vec::new(),
            rooms,
            timeline,
            mods,
            story,
        }
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
    /// the active rooms, loading until this tick's loading step. The off-screen den and
    /// realized rooms are left alone.
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
        self.realized[i] = Some(RealizedRoom::new(
            loaded,
            data,
            self.story,
            self.timeline,
            self.mods,
        ));
        self.active.push(i);
        self.loading.push(i);
        Ok(())
    }

    /// `AbstractRoom.Abstractize`: drops the realized room.
    pub fn abstractize(&mut self, index: i32) {
        let Some(i) = self.local(index) else { return };
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
                room.finish_loading();
                done.push(self.data.rooms[i].index);
            }
        }
        done
    }
}
