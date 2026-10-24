//! A realized room (`Room`) as far as the world needs it: geometry, settings, water and
//! the loading state that decides when a player may enter (`ReadyForPlayer`).
//!
//! Mirrors: Room.cs

use std::sync::Arc;

use rw_assets::world::LoadedRoom;
use rw_assets::{
    ActiveMods, CreatureType, Room, RoomBorderExit, RoomEffectType, RoomSettings, Timeline,
};

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
    /// `Room.Loaded`'s `AddWater`.
    pub water: Option<Water>,
    /// `loadingProgress`: 0 while loading, 1 once `ShortCutsReady`, 2 during
    /// `ReadyForAI`, 3 once fully loaded.
    loading_progress: u8,
    ready_for_non_ai_creatures: bool,
    /// Which realization of its room this is (port-only: the `Room` object's identity,
    /// matching it to its preparer).
    pub(crate) serial: u64,
    /// `waitToEnterAfterFullyLoaded`.
    wait_to_enter: i32,
    /// `abstractRoom.firstTimeRealized` while it loads.
    first_time_realized: bool,
    /// The room-specific scripts `Room.Loaded` added.
    pub scripts: Vec<RoomScript>,
    /// The scripts' place in the update list.
    pub script_order: u64,
    /// `borderExits`, made by the `ShortcutMapper` constructor at realization.
    pub border_exits: Vec<RoomBorderExit>,
    /// `aimap`, made by the preparer's thread before `ReadyForAI`.
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
    pub particles: crate::effects::particles::room_particles::RoomParticles,
    /// `Room.Loaded`'s light fixtures and the tile parse's waterfalls.
    pub fixtures: crate::RoomFixtures,
    /// The superstructure projectors, scan lines and star matrices.
    pub projections: crate::RoomProjections,
    /// `Room.gravity`: set every tick by the room's `AntiGravity` or its gradient-gravity
    /// script.
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
    /// The water was made by `Room.AddWater` during play (the rain's flood), not loaded.
    pub added_water: bool,
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
    /// `roomRain`, made by `Room.Loaded` (not in an intact shelter).
    pub room_rain: Option<crate::world::RoomRain>,
    /// `deathFallGraphic`, made by `Room.Loaded` for a room with an open bottom.
    pub death_fall: Option<crate::DeathFall>,
    /// `backgroundNoise`: what this tick's `MakeBackgroundNoise` calls raised it to.
    background_noise: f32,
    /// `lastBackgroundNoise` (`BackgroundNoise`): last tick's, what hearing reads.
    last_background_noise: f32,
    /// `socialEventRecognizer`.
    pub social_events: crate::creature::social::social_events::SocialEventRecognizer,
    /// The room's `CoralNeuronSystem`, made by `Room.Loaded` for an `SSSwarmers` effect or
    /// a coral or wall mycelia object.
    pub coral: Option<crate::CoralNeuronSystem>,
    /// `climbableVines`, made by the first vine to register.
    pub climbable_vines: Option<crate::ClimbableVinesSystem>,
    /// The vines registered at run time, whose owners these are.
    pub placed_vines: crate::PlacedVines,
    /// The `ShortcutHelper` `ShortCutsReady` added.
    pub shortcut_helper: Option<crate::game::shortcut_helper::ShortcutHelper>,
    /// `camerasChangedTick`: counts the camera's position changes in the room.
    pub cameras_changed_tick: i32,
}

impl RealizedRoom {
    /// What a vine's constructor does with `room.climbableVines` (room `index`): make it
    /// when the room has none, then `vines.Add(this)`.
    pub fn register_climbable_vine(&mut self, index: i32, vine: Box<dyn crate::ClimbableVine>) {
        let vine_ref = crate::VineRef::Placed {
            room: index,
            id: vine.id(),
        };
        self.placed_vines.add(vine);
        self.climbable_vines
            .get_or_insert_with(crate::ClimbableVinesSystem::new)
            .register(vine_ref);
    }

    /// The room's `climbableVines` with its vines' owners, for the player's update.
    pub fn vine_access(&mut self) -> Option<crate::VineAccess<'_>> {
        let (neurons, stems) = match self.coral.as_mut() {
            Some(c) => (c.neurons.as_mut_slice(), c.stems.as_mut_slice()),
            None => (&mut [][..], &mut [][..]),
        };
        Some(crate::VineAccess {
            system: self.climbable_vines.as_ref()?,
            host: crate::RoomVines {
                placed: &mut self.placed_vines,
                neurons,
                stems,
            },
        })
    }

    /// The same, read-only (the graphics).
    #[must_use]
    pub fn vine_view(&self) -> Option<(&crate::ClimbableVinesSystem, crate::RoomVinesView<'_>)> {
        Some((
            self.climbable_vines.as_ref()?,
            crate::RoomVinesView {
                placed: &self.placed_vines,
                neurons: self.coral.as_ref().map_or(&[], |c| c.neurons.as_slice()),
                stems: self.coral.as_ref().map_or(&[], |c| c.stems.as_slice()),
            },
        ))
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

    /// `new Room`, the `ShortcutMapper` constructor's border exits, `Room.Loaded`'s water and entry delay (from the room's effects
    /// and placed objects).
    pub(crate) fn new(
        loaded: LoadedRoom,
        shelter_or_gate: bool,
        timeline: Timeline,
        mods: ActiveMods,
        first_time_realized: bool,
    ) -> Self {
        let wait = Self::entry_delay(&loaded.settings, timeline, mods);
        let insects = crate::insect::InsectCoordinator::for_room(&loaded.settings, timeline, mods);
        let ceiling_tiles = Self::find_ceiling_tiles(&loaded.room);
        let border_exits = loaded.room.border_exits(shelter_or_gate);
        let (water, water_sound) = Self::make_water(&loaded.room, &loaded.settings, mods);
        Self {
            room: Arc::new(loaded.room),
            settings: loaded.settings,
            water,
            loading_progress: 0,
            ready_for_non_ai_creatures: false,
            serial: 0,
            wait_to_enter: wait,
            first_time_realized,
            scripts: Vec::new(),
            script_order: 0,
            border_exits,
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
            particles: crate::effects::particles::room_particles::RoomParticles::default(),
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
            water_sound,
            added_water: false,
            lightning: None,
            electric_deaths: Vec::new(),
            room_rain: None,
            border_push_backs: Vec::new(),
            death_fall: None,
            background_noise: 0.0,
            last_background_noise: 0.0,
            social_events: crate::creature::social::social_events::SocialEventRecognizer::default(),
            coral: None,
            climbable_vines: None,
            placed_vines: crate::PlacedVines::default(),
            shortcut_helper: None,
            cameras_changed_tick: 0,
        }
    }

    /// `new Water(room, defaultWaterLevel)`: the water with its surface rumble flag, and
    /// its `waterSounds` loop.
    fn make_water(
        room: &Room,
        settings: &RoomSettings,
        mods: ActiveMods,
    ) -> (Option<Water>, Option<crate::WaterSound>) {
        let mut water = Water::new(room, settings);
        if let Some(w) = water.as_mut() {
            w.rumble.active = settings.rain_danger(mods);
        }
        let sound = water.as_ref().and_then(|_| crate::WaterSound::new(room));
        (water, sound)
    }

    /// `Room.AddWater` in a room without a water object: `defaultWaterLevel` -10 when
    /// it had none (so `floatWaterLevel`, read only without a water object, no longer
    /// matters), then the `Water` and its loop. The camera's `NewObjectInRoom` is the
    /// app's (it makes the surface sprites once the room has water); the waterfalls
    /// read the room's water each tick, so `ConnectToWaterObject` has nothing to do.
    /// Returns whether it made the water.
    pub(crate) fn add_water(&mut self, mods: ActiveMods) -> bool {
        if self.water.is_some() {
            return false;
        }
        Arc::make_mut(&mut self.room).add_water();
        let (water, sound) = Self::make_water(&self.room, &self.settings, mods);
        self.water = water;
        self.water_sound = sound;
        self.added_water = self.water.is_some();
        self.added_water
    }

    /// `abstractRoom.firstTimeRealized` as `Room.Loaded` sees it.
    #[must_use]
    pub fn first_time_realized(&self) -> bool {
        self.first_time_realized
    }

    /// `waitToEnterAfterFullyLoaded` after `Room.Loaded`, base game: 40 for the
    /// `SSSwarmers` effect; 80 for active coral, wall mycelia, corruption and vine
    /// objects; else -1 (`ReadyForAI` raises it for the oracle rooms,
    /// [`Self::raise_oracle_wait`]). The More Slugcats and Watcher branches are listed in
    /// `rainworld-re/notes/world-loading.md`.
    #[must_use]
    pub fn entry_delay(settings: &RoomSettings, timeline: Timeline, mods: ActiveMods) -> i32 {
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
        wait
    }

    /// `ReadyForAI`'s oracle rooms: Five Pebbles' and Moon's chambers (`SS_AI`, `SL_AI`)
    /// in a story wait at least 80 ticks (the `Oracle` it makes there is not ported).
    pub(crate) fn raise_oracle_wait(&mut self, name: &str, story: bool) {
        if story && (name == "SS_AI" || name == "SL_AI") {
            self.wait_to_enter = self.wait_to_enter.max(80);
        }
    }

    /// `ShortCutsReady`'s first step: `shortcutsBlinking`.
    pub(crate) fn make_shortcuts_blinking(&mut self) {
        self.shortcuts_blinking = Some(ShortcutsBlinking::new(
            self.room.shortcut_map().shortcuts().len(),
        ));
    }

    /// `ShortCutsReady`'s `loadingProgress = 1`, open to creatures without `AI` when no
    /// creature of the room needs the AI map.
    pub(crate) fn set_shortcuts_ready(&mut self, ready_for_non_ai_creatures: bool) {
        self.loading_progress = 1;
        self.ready_for_non_ai_creatures |= ready_for_non_ai_creatures;
    }

    /// The thread's `room.aimap = aiMapper.ReturnAIMap()` (with the heat maps), before
    /// `ReadyForAI`. `index` is the room's global index.
    pub(crate) fn build_ai_map(&mut self, index: i32, shelter_or_gate: bool) {
        let lethal_water = self.settings.effect_amount(RoomEffectType::LethalWater) > 0.0;
        self.ai_map = Some(Arc::new(AiMap::new(
            &self.room,
            index,
            shelter_or_gate,
            lethal_water,
        )));
    }

    /// `ReadyForAI`'s `loadingProgress = 2`.
    pub(crate) fn begin_ready_for_ai(&mut self) {
        self.loading_progress = 2;
    }

    /// `ReadyForAI`'s `loadingProgress = 3` and `readyForNonAICreaturesToEnter`.
    pub(crate) fn end_ready_for_ai(&mut self) {
        self.loading_progress = 3;
        self.ready_for_non_ai_creatures = true;
    }

    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// `loadingProgress`.
    #[must_use]
    pub fn loading_progress(&self) -> u8 {
        self.loading_progress
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

    /// `readyForNonAICreaturesToEnter`.
    #[must_use]
    pub fn ready_for_non_ai_creatures(&self) -> bool {
        self.ready_for_non_ai_creatures
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
