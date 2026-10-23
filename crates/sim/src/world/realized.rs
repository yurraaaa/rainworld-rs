//! A realized room (`Room`) as far as the world needs it: geometry, settings, water and
//! the loading state that decides when a player may enter (`ReadyForPlayer`).
//!
//! Mirrors: Room.cs

use std::sync::Arc;

use rw_assets::world::{AbstractRoomData, LoadedRoom};
use rw_assets::{ActiveMods, CreatureType, Room, RoomEffectType, RoomSettings, Timeline};

use super::AiMap;
use crate::creature::CreatureTemplate;
use crate::creature::fly::FliesRoomAi;
use crate::room_script::RoomScript;

use crate::{Cosmetic, ShortcutsBlinking, Water, WaterDrip, WaterGlitter};

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
    /// `abstractRoom.firstTimeRealized` while it loads.
    first_time_realized: bool,
    /// The room-specific scripts `Room.Loaded` added.
    pub scripts: Vec<RoomScript>,
    /// `aimap`, made when the room finishes loading.
    pub ai_map: Option<Arc<AiMap>>,
    /// `fliesRoomAi`, made by the first fly that enters.
    pub flies_room_ai: Option<FliesRoomAi>,
    /// `shortcutsBlinking`, made when the shortcuts are ready.
    pub shortcuts_blinking: Option<ShortcutsBlinking>,
    /// The `WaterDrip`s in the room's update list.
    pub drips: Vec<WaterDrip>,
    /// The sparks and spikes in the room's update list.
    pub cosmetics: Vec<Cosmetic>,
    pub water_glitter: WaterGlitter,
    /// The guide's `HologramLight`s in the room's update list.
    pub holo_lights: Vec<crate::creature::overseer::HologramLight>,
    /// The `LightSource`s in the room's update list.
    pub light_sources: Vec<crate::LightSource>,
    /// `Room.Loaded`'s `CosmeticSlimeMold`s (no update; drawn).
    pub slime_molds: Vec<crate::CosmeticSlimeMold>,
    /// `Room.Loaded`'s dead `TokenStalk`s in the room's update list.
    pub token_stalks: Vec<crate::TokenStalk>,
    /// The dandelions, sparks, specks and spores of the room's effects.
    pub particles: crate::room_particles::RoomParticles,
    /// `Room.gravity`: 1 until `AntiGravity` (zero-gravity rooms) is ported.
    pub gravity: f32,
    /// `insectCoordinator`, made by `Room.Loaded` for an insect effect or group.
    pub insects: Option<crate::insect::InsectCoordinator>,
    /// `ceilingTiles` (`Room.Loaded`), where `ScreenMovement` shakes drips off.
    pub ceiling_tiles: Vec<(i32, i32)>,
    /// The `DrippingSound` `Room.Loaded` added at a cycle's start, until it and its
    /// emitters are gone.
    pub dripping_sound: Option<crate::DrippingSound>,
    /// The water's calm surface loop, made with the water.
    pub water_sound: Option<crate::WaterSound>,
    /// `socialEventRecognizer`.
    pub social_events: super::social_events::SocialEventRecognizer,
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
        let insects = crate::insect::InsectCoordinator::for_room(&loaded.settings, timeline, mods);
        let ceiling_tiles = Self::find_ceiling_tiles(&loaded.room);
        Self {
            room: Arc::new(loaded.room),
            settings: loaded.settings,
            water: None,
            loading_progress: 0,
            ready_for_non_ai_creatures: false,
            wait_to_enter: wait,
            first_time_realized,
            scripts: Vec::new(),
            ai_map: None,
            flies_room_ai: None,
            shortcuts_blinking: None,
            drips: Vec::new(),
            cosmetics: Vec::new(),
            water_glitter: WaterGlitter::default(),
            holo_lights: Vec::new(),
            light_sources: Vec::new(),
            slime_molds: Vec::new(),
            token_stalks: Vec::new(),
            particles: crate::room_particles::RoomParticles::default(),
            gravity: 1.0,
            insects,
            ceiling_tiles,
            dripping_sound: None,
            water_sound: None,
            social_events: super::social_events::SocialEventRecognizer::default(),
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

    /// `ShortCutsReady` then `ReadyForAI`: fully loaded and open to the player once the
    /// delay runs out. `index` is the room's global index.
    pub(crate) fn finish_loading(&mut self, index: i32, shelter_or_gate: bool) {
        self.shortcuts_blinking = Some(ShortcutsBlinking::new(
            self.room.shortcut_map().shortcuts().len(),
        ));
        self.water = Water::new(&self.room, &self.settings);
        self.water_sound = self
            .water
            .as_ref()
            .and_then(|_| crate::WaterSound::new(&self.room));
        let lethal_water = self.settings.effect_amount(RoomEffectType::LethalWater) > 0.0;
        self.ai_map = Some(Arc::new(AiMap::new(
            &self.room,
            index,
            shelter_or_gate,
            lethal_water,
        )));
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
    /// `aimap.ExitDistanceForCreatureAndCheckNeighbours(tile, exit, Fly)` from the room
    /// file's baked Fly layer; -1 before the AI map exists.
    #[must_use]
    pub fn exit_distance(&self, tile: (i32, i32), exit: usize) -> i32 {
        let exit = i32::try_from(exit).unwrap_or(-1);
        self.ai_map.as_ref().map_or(-1, |m| {
            let fly = CreatureTemplate::of(CreatureType::Fly, ActiveMods::NONE);
            m.exit_distance_for_creature_and_check_neighbours(tile, exit, &fly)
        })
    }
}
