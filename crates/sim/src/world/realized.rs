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
    /// `Room.Loaded`'s placed `LightBeam`s in the room's update list.
    pub light_beams: Vec<crate::LightBeam>,
    /// `Room.Loaded`'s placed `SpotLight`s (no update; drawn).
    pub spot_lights: Vec<crate::SpotLight>,
    /// `Room.Update` has run once (`darkenLightsFactor` is set at its end).
    pub updated: bool,
    /// `Room.Loaded`'s dead `TokenStalk`s in the room's update list.
    pub token_stalks: Vec<crate::TokenStalk>,
    /// The dandelions, sparks, specks and spores of the room's effects.
    pub particles: crate::room_particles::RoomParticles,
    /// `Room.Loaded`'s light fixtures and the tile parse's waterfalls.
    pub fixtures: crate::RoomFixtures,
    /// The superstructure projectors, scan lines and star matrices.
    pub projections: crate::RoomProjections,
    /// `Room.gravity`: set every tick by the room's `AntiGravity`.
    pub gravity: f32,
    /// `Room.Loaded`'s `AntiGravity` (a `ZeroG` or `BrokenZeroG` effect).
    pub anti_gravity: Option<crate::AntiGravity>,
    /// `Room.Loaded`'s `CustomDecal`s (no per-tick state; drawn in `Foreground`).
    pub decals: Vec<crate::CustomDecal>,
    /// `Room.Loaded`'s `Rainbow`s in the room's update list.
    pub rainbows: Vec<crate::Rainbow>,
    /// `insectCoordinator`, made by `Room.Loaded` for an insect effect or group.
    pub insects: Option<crate::insect::InsectCoordinator>,
    /// `ceilingTiles` (`Room.Loaded`), where `ScreenMovement` shakes drips off.
    pub ceiling_tiles: Vec<(i32, i32)>,
    /// The `DrippingSound` `Room.Loaded` added at a cycle's start, until it and its
    /// emitters are gone.
    pub dripping_sound: Option<crate::DrippingSound>,
    /// The water's calm surface loop, made with the water.
    pub water_sound: Option<crate::WaterSound>,
    /// `Room.Loaded`'s `AboveCloudsView`s and `RoofTopView`s.
    pub background_scenes: Vec<crate::BackgroundScene>,
    /// The `DustpuffSpawner`s (a `RoofTopView`'s constructor makes one) and their
    /// `DustPuff`s in the room's update list.
    pub dust_spawners: Vec<crate::DustpuffSpawner>,
    pub dust_puffs: Vec<crate::DustPuff>,
    /// `lightning`, made by `Room.Loaded` for a `Lightning` or `BkgOnlyLightning` effect.
    pub lightning: Option<crate::Lightning>,
    /// `Room.Loaded`'s `ElectricDeath`s in the room's update list.
    pub electric_deaths: Vec<crate::ElectricDeath>,
    /// `Room.Loaded`'s `RoomBorderPushBack`s in the room's update list.
    pub border_push_backs: Vec<crate::BorderPushBack>,
    /// `backgroundNoise`: what this tick's `MakeBackgroundNoise` calls raised it to.
    background_noise: f32,
    /// `lastBackgroundNoise` (`BackgroundNoise`): last tick's, what hearing reads.
    last_background_noise: f32,
    /// `socialEventRecognizer`.
    pub social_events: super::social_events::SocialEventRecognizer,
    /// The room's `CoralNeuronSystem`, made by `Room.Loaded` for an `SSSwarmers` effect or
    /// a coral or wall mycelia object.
    pub coral: Option<crate::CoralNeuronSystem>,
    /// `climbableVines`, made by the first vine to register (lent to the player for its
    /// update).
    pub climbable_vines: Option<crate::ClimbableVinesSystem>,
    /// `camerasChangedTick`: counts the camera's position changes in the room.
    pub cameras_changed_tick: i32,
}

impl RealizedRoom {
    /// What a vine's constructor does with `room.climbableVines`: make it when the room has
    /// none, then `vines.Add(this)`.
    pub fn register_climbable_vine(&mut self, vine: Box<dyn crate::ClimbableVine>) {
        self.climbable_vines
            .get_or_insert_with(crate::ClimbableVinesSystem::new)
            .register(vine);
    }

    /// `darkenLightsFactor`: 0 until the room's first `Update`, then the `DarkenLights`
    /// amount (the DLC dust-storm blend is not ported, D24).
    #[must_use]
    pub fn darken_lights_factor(&self) -> f32 {
        if self.updated {
            self.settings.effect_amount(RoomEffectType::DarkenLights)
        } else {
            0.0
        }
    }

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
            light_beams: Vec::new(),
            spot_lights: Vec::new(),
            updated: false,
            token_stalks: Vec::new(),
            particles: crate::room_particles::RoomParticles::default(),
            fixtures: crate::RoomFixtures::default(),
            projections: crate::RoomProjections::default(),
            gravity: 1.0,
            anti_gravity: None,
            decals: Vec::new(),
            rainbows: Vec::new(),
            insects,
            background_scenes: Vec::new(),
            dust_spawners: Vec::new(),
            dust_puffs: Vec::new(),
            ceiling_tiles,
            dripping_sound: None,
            water_sound: None,
            lightning: None,
            electric_deaths: Vec::new(),
            border_push_backs: Vec::new(),
            background_noise: 0.0,
            last_background_noise: 0.0,
            social_events: super::social_events::SocialEventRecognizer::default(),
            coral: None,
            climbable_vines: None,
            cameras_changed_tick: 0,
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

    /// The start of `Room.Update`: the entry delay counts down once fully loaded, and
    /// the background noise of last tick is kept and drops by 0.05.
    pub(crate) fn update_wait(&mut self) {
        if self.wait_to_enter > 0 && self.fully_loaded() {
            self.wait_to_enter -= 1;
        }
        self.last_background_noise = self.background_noise;
        self.background_noise = (self.background_noise - 0.05).max(0.0);
    }

    /// `MakeBackgroundNoise`: raises this tick's noise to `noise` (up to 1).
    pub(crate) fn make_background_noise(&mut self, noise: f32) {
        self.background_noise = if noise < self.background_noise {
            self.background_noise
        } else {
            noise.min(1.0)
        };
    }

    /// `BackgroundNoise`: how much of the last tick's noise dulls what creatures hear.
    #[must_use]
    pub fn last_background_noise(&self) -> f32 {
        self.last_background_noise
    }
}

impl RealizedRoom {
    /// What a `Mycelium` of an owner in this room reads of it.
    pub(crate) fn mycelium_env(
        &self,
        index: i32,
        camera: crate::CameraView,
    ) -> crate::MyceliumEnv<'_> {
        crate::MyceliumEnv {
            room: &self.room,
            room_index: index,
            ai_map: self.ai_map.as_deref(),
            camera,
            cameras_changed_tick: self.cameras_changed_tick,
            system: self.coral.as_ref().map(crate::CoralNeuronSystem::state),
        }
    }

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
