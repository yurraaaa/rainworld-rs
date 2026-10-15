//! A realized room (`Room`) as far as the world needs it: geometry, settings, water and
//! the loading state that decides when a player may enter (`ReadyForPlayer`).

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

use rw_assets::world::{AbstractRoomData, LoadedRoom};
use rw_assets::{ActiveMods, Room, RoomSettings, Timeline};

use super::AiMap;
use crate::room_ext::RoomExt;
use crate::room_script::RoomScript;

use crate::{ShortcutsBlinking, Water};

/// Placed objects whose `Room.Loaded` branch delays entry by 80 ticks (base game).
const WAIT_80_OBJECTS: [&str; 8] = [
    "CoralNeuron",
    "CoralCircuit",
    "CoralStem",
    "CoralStemWithNeurons",
    "WallMycelia",
    "Corruption",
    "PassiveCorruption",
    "Vine",
];

/// A realized room.
pub struct RealizedRoom {
    pub room: Arc<Room>,
    pub settings: RoomSettings,
    /// Made when the room finishes loading (`Room.Loaded`'s `AddWater`).
    pub water: Option<Water>,
    /// `loadingProgress`: 0 while loading, 3 once fully loaded.
    loading_progress: u8,
    ready_for_non_ai_creatures: bool,
    /// `waitToEnterAfterFullyLoaded`.
    wait_to_enter: i32,
    /// Per exit node, the stand-in for the fly's baked distance map, made when asked.
    exit_distances: OnceLock<Vec<Vec<i32>>>,
    /// `abstractRoom.firstTimeRealized` while it loads.
    first_time_realized: bool,
    /// The room-specific scripts `Room.Loaded` added.
    pub scripts: Vec<RoomScript>,
    /// `aimap`, made when the room finishes loading.
    pub ai_map: Option<AiMap>,
    /// `shortcutsBlinking`, made when the shortcuts are ready.
    pub shortcuts_blinking: Option<ShortcutsBlinking>,
}

impl RealizedRoom {
    /// `new Room` plus the entry delay `Room.Loaded` and `ReadyForAI` set from the
    /// room's effects, placed objects and name.
    pub(crate) fn new(
        loaded: LoadedRoom,
        data: &AbstractRoomData,
        story: bool,
        timeline: Timeline,
        mods: ActiveMods,
        first_time_realized: bool,
    ) -> Self {
        let wait = Self::entry_delay(&loaded.settings, &data.name, story, timeline, mods);
        Self {
            room: Arc::new(loaded.room),
            settings: loaded.settings,
            water: None,
            loading_progress: 0,
            ready_for_non_ai_creatures: false,
            wait_to_enter: wait,
            exit_distances: OnceLock::new(),
            first_time_realized,
            scripts: Vec::new(),
            ai_map: None,
            shortcuts_blinking: None,
        }
    }

    /// `abstractRoom.firstTimeRealized` as `Room.Loaded` sees it.
    #[must_use]
    pub fn first_time_realized(&self) -> bool {
        self.first_time_realized
    }

    /// `waitToEnterAfterFullyLoaded` after loading, base game: 40 for the `SSSwarmers`
    /// effect; 80 for active coral, wall mycelia, corruption and vine objects, and for
    /// Five Pebbles' and Moon's chambers (`SS_AI`, `SL_AI`) in a story; else -1. The
    /// More Slugcats and Watcher branches are listed in `rainworld-re/notes/world-loading.md`.
    #[must_use]
    pub fn entry_delay(
        settings: &RoomSettings,
        name: &str,
        story: bool,
        timeline: Timeline,
        mods: ActiveMods,
    ) -> i32 {
        let mut wait = -1;
        if settings.has_effect("SSSwarmers") {
            wait = wait.max(40);
        }
        let objects = settings.placed_objects_for(timeline, mods);
        if objects
            .iter()
            .any(|o| o.active && WAIT_80_OBJECTS.contains(&o.kind.as_str()))
        {
            wait = wait.max(80);
        }
        if story && (name == "SS_AI" || name == "SL_AI") {
            wait = wait.max(80);
        }
        wait
    }

    /// `ShortCutsReady` then `ReadyForAI` (no creature needs the AI map): fully loaded
    /// and open to the player once the delay runs out.
    pub(crate) fn finish_loading(&mut self) {
        self.shortcuts_blinking = Some(ShortcutsBlinking::new(
            self.room.shortcut_map().shortcuts().len(),
        ));
        self.water = Water::new(&self.room, &self.settings);
        self.ai_map = Some(AiMap::new(&self.room));
        self.loading_progress = 3;
        self.ready_for_non_ai_creatures = true;
    }

    #[must_use]
    pub fn fully_loaded(&self) -> bool {
        self.loading_progress >= 3
    }

    /// `shortCutsReady`.
    #[must_use]
    pub fn shortcuts_ready(&self) -> bool {
        self.loading_progress >= 1
    }

    /// `readyForAI`.
    #[must_use]
    pub fn ready_for_ai(&self) -> bool {
        self.loading_progress >= 2
    }

    /// `ReadyForPlayer`.
    #[must_use]
    pub fn ready_for_player(&self) -> bool {
        self.ready_for_non_ai_creatures && self.wait_to_enter < 1
    }

    #[must_use]
    pub fn wait_to_enter(&self) -> i32 {
        self.wait_to_enter
    }

    /// The start of `Room.Update`: the entry delay counts down once fully loaded.
    pub(crate) fn update_wait(&mut self) {
        if self.wait_to_enter > 0 && self.fully_loaded() {
            self.wait_to_enter -= 1;
        }
    }
}

impl RealizedRoom {
    /// Stands in for `aimap.ExitDistanceForCreatureAndCheckNeighbours(tile, exit, Fly)`:
    /// the first of the tile, then its left, down, right and up neighbours with a
    /// distance, else -1. The distance is the 4-connected step count through non-Solid
    /// tiles from the entrance of the shortcut leading to that exit, not the game's
    /// baked fly pathing (docs/DIVERGENCES.md D14).
    #[must_use]
    pub fn exit_distance(&self, tile: (i32, i32), exit: usize) -> i32 {
        let maps = self.exit_distances.get_or_init(|| self.distance_maps());
        let Some(map) = maps.get(exit) else {
            return -1;
        };
        [(0, 0), (-1, 0), (0, -1), (1, 0), (0, 1)]
            .into_iter()
            .map(|(dx, dy)| self.distance_at(map, (tile.0 + dx, tile.1 + dy)))
            .find(|&d| d > -1)
            .unwrap_or(-1)
    }

    fn distance_at(&self, map: &[i32], (x, y): (i32, i32)) -> i32 {
        if !self.room.contains(x, y) {
            return -1;
        }
        map[(x * self.room.height() + y) as usize]
    }

    /// One breadth-first distance grid (x-major) per exit node.
    fn distance_maps(&self) -> Vec<Vec<i32>> {
        let room = &self.room;
        let map = room.shortcut_map();
        let exits = map
            .nodes()
            .iter()
            .filter(|&&(x, y)| room.tile(x, y).shortcut == rw_assets::Shortcut::RoomExit)
            .count();
        (0..exits)
            .map(|exit| {
                let mut grid = vec![-1; (room.width() * room.height()) as usize];
                if let Some(start) = map.leading_to_node(exit).map(|s| s.start) {
                    self.flood(&mut grid, start);
                }
                grid
            })
            .collect()
    }

    fn flood(&self, grid: &mut [i32], start: (i32, i32)) {
        let room = &self.room;
        let h = room.height();
        let mut queue = VecDeque::from([start]);
        grid[(start.0 * h + start.1) as usize] = 0;
        while let Some((x, y)) = queue.pop_front() {
            let d = grid[(x * h + y) as usize];
            for (dx, dy) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if room.contains(nx, ny)
                    && grid[(nx * h + ny) as usize] == -1
                    && !room.is_solid_tile(nx, ny)
                {
                    grid[(nx * h + ny) as usize] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
}
