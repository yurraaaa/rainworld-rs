//! The state the MCP tools drive: one loaded game (a world, seen from the camera's
//! room), stepped only on request, and a headless renderer for screenshots. Every tool is a method returning a typed reply or
//! an error message.
//!
//! Mirrors: - (port-only: MCP server)

#[path = "dev_tools.rs"]
mod dev_tools;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rw_assets::SlugcatName;
use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicProcess, TimedSources};
use rw_graphics::{CosmeticShape, LizardCosmetic};
use rw_menu::{KeyboardKey, MainLoopProcess, MenuShared, ProcessId, StoryGameInitCondition};
use rw_random::UnityRandom;
use rw_render::{RoomLayers, WgpuRenderer};
use rw_save::SaveSlot;
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::creature::lizard::LizardVoice;
use rw_sim::creature::{AbstractCreature, CreatureId, Lizard, RepReading, TrackedCreatureState};
use rw_sim::item::{Entity, ObjectId, SpearStickKind, Weapon};
use rw_sim::{CosmeticKind, Input, StomachObject, Vec2, WorldCoordinate};

use super::params::{
    HudMessageParams, ImportSaveParams, InputParam, ListRoomsParams, LoadProcessParams,
    LoadRoomParams, NewGameParams, ProcessParam, RespawnParams, ScreenshotParams,
    SetDestinationParams, SetPlayerParams, StepParams, StepProcessParams, StepSequenceParams,
    TilesParams, ViewParam,
};
use super::reply::{
    AbstractAiInfo, ChunkState, ControlsPageState, CreatureInfo, ExitInfo, ExitList, FlyInfo,
    FoodMeterState, HudState, ImpactState, ImportReply, KarmaMeterState, KarmaScreenState,
    KillsState, LizardChunk, LizardInfo, LobbyState, MapMarkerState, MapState, MeterState, NodeRef,
    ObjectInfo, OptionsState, OrphanedPreparerState, OverseerInfo, PauseMenuState, PlayerState,
    PreparerState, ProcessState, RainMeterState, RainState, RealizedState, RoomInfo, RoomList,
    SaveInfo, SaveStateInfo, Screenshot, SessionInfo, ShelterDoorState, SleepState, SlideShowState,
    StartUpState, State, StepTrace, StoryInfo, TextPromptState, TokenState, WeaponInfo,
};
use super::reply::{
    AgressionTrackerInfo, ConnectionInfo, CoordInfo, DynamicRelationshipInfo, GhostInfo,
    HeardNoiseInfo, LizardCosmeticInfo, LizardDecisionInfo, LizardGraphicsInfo, LizardLimbInfo,
    LizardMoveInfo, LizardSocialInfo, LizardVoiceInfo, MovementAnimationInfo, NoiseSourceInfo,
    NoiseTrackerInfo, PreyTrackerInfo, ThreatTrackerInfo, TrackedCreatureInfo, TrackedPreyInfo,
    UtilityInfo, VesselInfo,
};
use super::reply::{
    CameraShakeState, DangerInfo, HoloLightState, LightSourceState, MusicState, TriggerState,
};
use super::reply::{
    DeathFallState, FullScreenEffectState, LevelParamsState, SpearOnBackInfo, SpriteLeaserState,
    WaterLightState,
};
use super::reply::{
    FleeInfo, HeavyAiInfo, HuntInfo, LightningState, LoungeInfo, OwnedItemState, ShelterIconState,
    WaterSoundState,
};
use super::reply::{
    FliesWorldInfo, GraspInfo, HeldInfo, QuantifiedInfo, RegionStateInfo, RelationshipInfo,
    RoomDens,
};
use super::tiles::TileDump;
use crate::frontend::{AppContext, Frontend, FrontendRenderer, GameSetup};
use crate::game::{Game, GameStart};
use crate::room_catalog::RoomCatalog;
use crate::saves::SaveConfig;
use crate::scene::{Framing, Scene, SceneRenderer, SceneView, VisualsSource};
use crate::sprite_order::Drawable;
use crate::sprites::SpriteAssets;

/// Most ticks one `step` or `step_sequence` call may run (10 minutes of game time).
const MAX_TICKS: u64 = 40 * 60 * 10;
/// Largest screenshot side in pixels.
const MAX_IMAGE_SIDE: f32 = 4096.0;
/// Most tiles one `get_tiles` call returns.
const MAX_TILES: i64 = 40_000;
/// Seconds per `step_process` frame.
const FRAME_SECONDS: f32 = 1.0 / 40.0;
/// A cap on the ticks a dev start steps to load its room.
const START_READY_TICKS: u32 = 40;

pub struct Session {
    assets: StreamingAssets,
    resources: Arc<UnityResources>,
    sprites: Option<Arc<SpriteAssets>>,
    /// `sounds.txt`: the sounds' random draws are made as in the window.
    sound_table: Arc<SoundTable>,
    /// Game mode (`load_room`, `new_game`) and how it was started.
    scene: Option<Scene>,
    start: Option<GameStart>,
    /// The process manager's `blackDelay` after the switch to the game (game mode has no
    /// manager): `InitialBlackSeconds`, less a frame after each tick.
    black_delay: f32,
    /// Process mode (`load_process`) and its frame count.
    frontend: Option<(Frontend, u64)>,
    /// Created on the first screenshot.
    renderer: Option<FrontendRenderer<WgpuRenderer>>,
    /// Draws other rooms than the camera's, so `renderer`'s uploads stay the camera's.
    room_renderer: Option<FrontendRenderer<WgpuRenderer>>,
    /// The fonts, atlases and strings the game mode's HUD draws with, loaded with the
    /// first game.
    hud_shared: Option<MenuShared>,
    /// Where `load_process`'s menus keep their saves and `import_save` reads and writes.
    saves: SaveConfig,
    /// Game mode's music player's silent sources (its player is `hud_shared.music`).
    music_sources: TimedSources,
}

impl Session {
    /// A session with no room loaded; loads the slugcat atlas from `resources`.
    #[must_use]
    pub fn new(assets: StreamingAssets, resources: Arc<UnityResources>, saves: SaveConfig) -> Self {
        let sprites = SpriteAssets::load(&resources, &assets)
            .inspect_err(|e| log::warn!("no slugcat sprites ({e}); drawing the circle body"))
            .ok()
            .map(Arc::new);
        let music = TimedSources::installed(&assets);
        Self {
            sound_table: Game::load_sound_table(&assets),
            music_sources: music,
            assets,
            resources,
            sprites,
            scene: None,
            start: None,
            black_delay: 0.0,
            frontend: None,
            renderer: None,
            room_renderer: None,
            hud_shared: None,
            saves,
        }
    }

    /// The loaded scene, for the mirror window.
    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        self.scene.as_mut()
    }

    /// The loaded process manager, for the mirror window.
    pub fn frontend_mut(&mut self) -> Option<&mut Frontend> {
        self.frontend.as_mut().map(|(f, _)| f)
    }

    pub fn sprites(&self) -> Option<Arc<SpriteAssets>> {
        self.sprites.clone()
    }

    fn scene(&self) -> Result<&Scene, String> {
        self.scene
            .as_ref()
            .ok_or_else(|| "no game loaded; call load_room or new_game first".to_owned())
    }

    fn scene_or_err(&mut self) -> Result<&mut Scene, String> {
        self.scene
            .as_mut()
            .ok_or_else(|| "no game loaded; call load_room or new_game first".to_owned())
    }

    /// Room names (lower case, as on disk) under `world/<region>-rooms`, `world/gates`
    /// and `levels` that have a first camera image.
    ///
    /// # Errors
    /// When no room directory can be read.
    pub fn list_rooms(&self, params: &ListRoomsParams) -> Result<RoomList, String> {
        let region = params.region.as_deref().map(str::to_lowercase);
        let prefix = params.prefix.as_deref().unwrap_or("").to_lowercase();
        let mut rooms = RoomCatalog::read(&self.assets)?.rooms(region.as_deref(), &prefix);
        let count = rooms.len();
        let limit = params.limit.unwrap_or(500) as usize;
        rooms.truncate(limit);
        Ok(RoomList {
            count,
            truncated: count > rooms.len(),
            rooms,
        })
    }

    /// Loads a room's region world (as the game looks the room up) and spawns the
    /// player in that room with `seed`.
    ///
    /// # Errors
    /// When the room is not found or its world does not load.
    pub fn load_room(&mut self, params: &LoadRoomParams) -> Result<RoomInfo, String> {
        let name = params.name.to_lowercase();
        let path = self
            .assets
            .find_room_file(&name, ".txt")
            .ok_or_else(|| format!("room {:?} not found (see list_rooms)", params.name))?;
        log::debug!("room file {}", path.display());
        let slugcat = match &params.slugcat {
            Some(name) => Self::slugcat_named(name)?,
            None => SlugcatName::White,
        };
        let start = GameStart::Room {
            name: params.name.clone(),
            tile: params.spawn_tile.map(|[x, y]| (x, y)),
            slugcat,
        };
        self.start_game(start, params.seed.unwrap_or(1))?;
        self.room_info()
    }

    /// A new story game (`StoryGameInitCondition.New`) as `slugcat`: its start den and
    /// position, in its region's world.
    ///
    /// # Errors
    /// On an unknown slugcat, or when the world does not load.
    pub fn new_game(&mut self, params: &NewGameParams) -> Result<RoomInfo, String> {
        let name = Self::slugcat_named(&params.slugcat)?;
        let start = if params.malnourished == Some(true) {
            let campaign = SlugcatCampaign::of(name, rw_assets::ActiveMods::NONE)
                .ok_or_else(|| format!("no campaign for {name}"))?;
            let story = rw_sim::campaign::StoryStart {
                malnourished: true,
                ..rw_sim::campaign::StoryStart::new_game(campaign)
            };
            GameStart::SavedStory {
                slugcat: name,
                story: Box::new(story),
            }
        } else {
            GameStart::NewStory(name)
        };
        self.start_game(start, params.seed.unwrap_or(1))?;
        self.room_info()
    }

    /// The base-game campaign called `name` (its value or its English name).
    fn slugcat_named(name: &str) -> Result<SlugcatName, String> {
        let want = match name.to_lowercase().as_str() {
            "survivor" => "white".to_owned(),
            "monk" => "yellow".to_owned(),
            "hunter" => "red".to_owned(),
            other => other.to_owned(),
        };
        SlugcatCampaign::all(rw_assets::ActiveMods::NONE)
            .into_iter()
            .find(|c| c.name.value().eq_ignore_ascii_case(&want))
            .map(|c| c.name)
            .ok_or_else(|| format!("unknown slugcat {name:?} (White, Yellow, Red)"))
    }

    fn start_game(&mut self, start: GameStart, seed: u64) -> Result<(), String> {
        if self.hud_shared.is_none() {
            let screen = rw_render::Resolution::DEFAULT.size_f32();
            self.hud_shared = MenuShared::load(self.assets.clone(), &self.resources, screen)
                .inspect_err(|e| log::warn!("no fonts ({e}); the game has no HUD"))
                .ok();
        }
        // The music player runs beside the game as the process manager's side process
        // would: made with the default options, its `UpdateMusicContext` and the new
        // game's `NewCycleEvent`. Each dev start (not the game, where the player outlives
        // sessions) gets a fresh player and sources: the player draws from the game's
        // random stream, so runs then repeat within one server process.
        self.music_sources.destroy_all();
        if let Some(shared) = &mut self.hud_shared {
            shared.music = shared.new_music_player(None);
            if let Some(music) = &mut shared.music {
                music.update_music_context(Some(MusicProcess::StoryGame));
                music.new_cycle_event();
            }
        }
        let rng = UnityRandom::new(seed as i32);
        let game = Game::start(
            &self.assets,
            &start,
            rng,
            Arc::clone(&self.sound_table),
            rw_render::Resolution::DEFAULT.size_f32(),
        )?;
        let source = VisualsSource {
            assets: self.assets.clone(),
            resources: Arc::clone(&self.resources),
            screen: rw_render::Resolution::DEFAULT.size_f32(),
        };
        log::info!("started {start:?}, seed {seed}");
        // `RainWorldGame.InitialBlackSeconds`: a new story waits in black, else (`Dev`)
        // none.
        self.black_delay = match &start {
            GameStart::NewStory(name) => {
                StoryGameInitCondition::New.game_initial_black_seconds(*name)
            }
            GameStart::SavedStory { slugcat, .. } => {
                StoryGameInitCondition::Load.game_initial_black_seconds(*slugcat)
            }
            GameStart::Room { slugcat, .. } => {
                StoryGameInitCondition::Dev.game_initial_black_seconds(*slugcat)
            }
        };
        self.scene = Some(Scene::new(game, Some(source)));
        self.start = Some(start);
        self.frontend = None;
        self.step_until_start_room_ready();
        Ok(())
    }

    /// Ticks the new game without input until its start room is `readyForAI` (the
    /// game's minimum loading schedule takes three ticks), so the reply and the first
    /// `step` see a loaded room (D14).
    fn step_until_start_room_ready(&mut self) {
        for _ in 0..START_READY_TICKS {
            if self
                .scene
                .as_ref()
                .is_none_or(|s| s.game().sim().camera_room_ready())
            {
                return;
            }
            self.tick(Input::NONE);
        }
        log::warn!("the start room is not ready after {START_READY_TICKS} ticks");
    }

    fn room_info(&self) -> Result<RoomInfo, String> {
        let scene = self.scene()?;
        let room = scene.game().room();
        Ok(RoomInfo {
            name: room.name().to_owned(),
            width: room.width(),
            height: room.height(),
            pixel_size: [room.pixel_width(), room.pixel_height()],
            cameras: room.cameras().to_vec(),
            water_level: room.water_level(),
            water_in_front: room.water_in_front(),
            shortcuts: room.shortcut_map().shortcuts().len(),
            level_graphic: scene.visuals().is_some(),
            state: self.state()?,
        })
    }

    /// Runs `ticks` ticks holding `input`.
    ///
    /// # Errors
    /// Without a room, or on a bad input or tick count.
    pub fn step(&mut self, params: &StepParams) -> Result<State, String> {
        let input = params.input.unwrap_or_default().to_input()?;
        let ticks = u64::from(params.ticks.unwrap_or(1));
        if ticks > MAX_TICKS {
            return Err(format!("at most {MAX_TICKS} ticks per call"));
        }
        self.scene_or_err()?;
        for _ in 0..ticks {
            self.tick(input);
        }
        self.state()
    }

    /// One game tick as the process manager runs it: the room scripts see the fade
    /// delay as it stood before the frame; the delay then counts down by the frame.
    fn tick(&mut self, input: Input) {
        let delay = self.black_delay;
        let Some(scene) = self.scene.as_mut() else {
            return;
        };
        scene.game_mut().set_fade_delay_in_progress(delay > 0.0);
        scene.tick(input, self.hud_shared.as_mut());
        if let Some(music) = self.hud_shared.as_mut().and_then(|s| s.music.as_mut()) {
            scene.update_music(music, &mut self.music_sources);
            self.music_sources.advance();
        }
        if self.black_delay > 0.0 {
            self.black_delay -= FRAME_SECONDS;
        }
    }

    /// Runs the steps in order; with `trace`, also reports where each step ended.
    ///
    /// # Errors
    /// Without a room, or on a bad input or too many ticks in total.
    pub fn step_sequence(
        &mut self,
        params: &StepSequenceParams,
    ) -> Result<(State, Vec<StepTrace>), String> {
        let steps = params
            .steps
            .iter()
            .map(|s| Ok((s.input.unwrap_or_default().to_input()?, s.ticks)))
            .collect::<Result<Vec<_>, String>>()?;
        let total: u64 = steps.iter().map(|(_, t)| u64::from(*t)).sum();
        if total > MAX_TICKS {
            return Err(format!("at most {MAX_TICKS} ticks per call (got {total})"));
        }
        let mut trace = Vec::new();
        for (input, ticks) in steps {
            self.scene_or_err()?;
            for _ in 0..ticks {
                self.tick(input);
            }
            if params.trace.unwrap_or(false) {
                trace.push(StepTrace::of(self.scene()?));
            }
        }
        Ok((self.state()?, trace))
    }

    /// A fresh player: with `seed`, as if the room was loaded with it; without,
    /// continuing the seeded spawn sequence (the window's R).
    ///
    /// # Errors
    /// Without a room.
    pub fn respawn(&mut self, params: &RespawnParams) -> Result<State, String> {
        match (params.seed, self.start.clone()) {
            (Some(seed), Some(start)) => self.start_game(start, seed)?,
            _ => {
                if !self.scene_or_err()?.respawn() {
                    return Err("no respawn in a story session".to_owned());
                }
            }
        }
        self.state()
    }

    /// Moves the player to a tile centre or a room-pixel position.
    ///
    /// # Errors
    /// Without a room, or unless exactly one of `tile` and `pos` is given.
    pub fn set_player(&mut self, params: &SetPlayerParams) -> Result<State, String> {
        if let Some(food) = params.food {
            self.scene_or_err()?.game_mut().set_food_in_stomach(food);
        }
        let pos = match (params.tile, params.pos) {
            (Some([x, y]), None) => Vec2::from(rw_assets::tile_centre(x, y)),
            (None, Some(pos)) => Vec2::from(pos),
            (None, None) if params.food.is_some() => return self.state(),
            _ => return Err("give exactly one of tile or pos".to_owned()),
        };
        let vel = Vec2::from(params.vel.unwrap_or([0.0, 0.0]));
        self.scene_or_err()?.game_mut().set_player(pos, vel);
        self.state()
    }

    /// A dev override: sets a realized lizard's destination (`AbstractCreatureAI.
    /// SetDestination`) and the run speed its AI keeps.
    ///
    /// # Errors
    /// Without a room, for an unknown room or creature, or one that is not a realized
    /// lizard.
    pub fn set_destination(&mut self, params: &SetDestinationParams) -> Result<State, String> {
        let sim = self.scene_or_err()?.game_mut().sim_mut();
        let [spawner, number] = params.creature;
        let id = sim
            .world()
            .creatures()
            .find(|(_, c)| c.id.spawner == spawner && c.id.number == number)
            .map(|(id, _)| id)
            .ok_or_else(|| format!("no creature {spawner}.{number}"))?;
        if params.release {
            if !sim.dev_release_lizard(id) {
                return Err(format!("{spawner}.{number} is not a realized lizard"));
            }
            return self.state();
        }
        let (Some(x), Some(y)) = (params.x, params.y) else {
            return Err("x and y are required unless release".to_owned());
        };
        let room = match &params.room {
            Some(name) => sim
                .world()
                .room_index(name)
                .ok_or_else(|| format!("no room {name}"))?,
            None => sim.world().creature(id).pos.room,
        };
        let dest = WorldCoordinate::new(room, x, y, -1);
        let speed = Some(params.run_speed.unwrap_or(0.5));
        if !sim.dev_set_lizard_destination(id, dest, speed) {
            return Err(format!("{spawner}.{number} is not a realized lizard"));
        }
        self.state()
    }

    /// The current tick and player state.
    ///
    /// # Errors
    /// Without a room.
    pub fn state(&self) -> Result<State, String> {
        let scene = self.scene()?;
        let game = scene.game();
        let sim = game.sim();
        let world = sim.world();
        let pos = sim.player_pos();
        let projections = super::projection_reply::ProjectionStates::all(world);
        Ok(State {
            tick: game.ticks(),
            world: world.name().to_owned(),
            room: game.room().name().to_owned(),
            camera: scene.visuals().map(|_| scene.camera().current()),
            screen_size: world.screen_size().to_array(),
            camera_shake: CameraShakeState::of(sim.camera_shake()),
            player_room: Self::room_name(scene, pos.room),
            player_node: pos.node,
            lobby: sim.lobby().map(|l| LobbyState {
                room: Self::room_name(scene, l.room),
                entrance_node: l.entrance_node,
                updates: l.updates,
            }),
            orphaned_preparers: world
                .orphaned_preparers()
                .map(|p| OrphanedPreparerState {
                    room: Self::room_name(scene, p.room),
                    preparer: PreparerState::of(p),
                })
                .collect(),
            realized_rooms: world
                .active_rooms()
                .into_iter()
                .filter_map(|i| {
                    let r = world.realized(i)?;
                    Some(RealizedState {
                        room: Self::room_name(scene, i),
                        loading_progress: r.loading_progress(),
                        preparer: world.room_preparer(i).map(PreparerState::of),
                        wait_to_enter: r.wait_to_enter(),
                        ready_for_player: r.ready_for_player(),
                        ceiling_drips: r.settings.ceiling_drips,
                        rumble_intensity: r.settings.rumble_intensity,
                        drips: r
                            .drips
                            .iter()
                            .filter(|d| !d.slated)
                            .map(|d| d.pos.to_array())
                            .collect(),
                        bubbles: r
                            .cosmetics
                            .iter()
                            .filter(|c| !c.slated)
                            .filter_map(|c| match &c.kind {
                                CosmeticKind::Bubble(b) => Some(b.pos.to_array()),
                                _ => None,
                            })
                            .collect(),
                        splashes: r
                            .cosmetics
                            .iter()
                            .filter(|c| !c.slated)
                            .filter_map(|c| match &c.kind {
                                CosmeticKind::Splash(s) => Some(s.pos.to_array()),
                                _ => None,
                            })
                            .collect(),
                        bubble_emitters: r.water.as_ref().map_or_else(Vec::new, |w| {
                            w.bubble_emitters().iter().map(|e| e.amount).collect()
                        }),
                        dripping_sound: r
                            .dripping_sound
                            .as_ref()
                            .filter(|d| !d.slated)
                            .map(|d| d.sound_loop.volume()),
                        lightning: r.lightning.as_ref().map(LightningState::from),
                        background_noise: Some(r.last_background_noise()).filter(|n| *n > 0.0),
                        water_sound: r.water_sound.as_ref().map(|w| WaterSoundState {
                            rect: [w.rect.left, w.rect.bottom, w.rect.right, w.rect.top],
                            volume: w.sound_loop.volume(),
                            playing: w.playing(),
                        }),
                        owned_items: r
                            .social_events
                            .owned_items_on_ground
                            .iter()
                            .map(|o| OwnedItemState::of(world, o))
                            .collect(),
                    })
                })
                .collect(),
            player: game.live_player().map(|p| PlayerState::of(p, game.sim())),
            fade_delay_in_progress: self.black_delay > 0.0,
            start_up: game.start_up_script().map(|s| StartUpState {
                counter: s.counter(),
                push_right: s.push_right(),
                stood_up: s.stood_up(),
                show_controls_counter: s.show_controls_counter(),
                controls_page_due: s.controls_page_due(),
            }),
            creatures: Self::creatures(scene),
            objects: Self::objects(scene),
            rain: RainState::of(sim),
            full_screen_effect: scene
                .level_state()
                .and_then(|s| s.full_screen)
                .map(FullScreenEffectState::from),
            camera_move: scene.pending_move().map(|m| super::reply::CameraMoveState {
                room: m.room,
                camera: m.camera,
            }),
            shortcut_graphics: scene
                .game()
                .shortcut_wait()
                .map(|(room, waiting)| super::reply::ShortcutGraphicsState { room, waiting }),
            death_fall: DeathFallState::all(sim.world()),
            water_light: scene.water_light().map(WaterLightState::from),
            level_params: scene.level_state().map(LevelParamsState::from),
            shelter_door: sim
                .shelter_door(sim.camera_room())
                .map(|d| ShelterDoorState {
                    closed_fac: d.closed_fac,
                    close_speed: d.close_speed,
                }),
            session: Self::session_info(scene),
            music: self.hud_shared.as_ref().and_then(|s| {
                let records = s
                    .saves
                    .current_save_state
                    .as_ref()
                    .map(|st| &st.death_persistent.songs_play_records[..]);
                s.music.as_ref().map(|m| MusicState::of(m, records))
            }),
            triggers: TriggerState::all(sim),
            sprite_order: Self::sprite_order(scene),
            holo_lights: Self::holo_lights(scene),
            lights: super::reply::RoomLightState::all(sim),
            insects: super::reply::RoomInsects::all(sim.world()),
            background_scene: super::reply::BackgroundSceneState::all(
                sim.world(),
                Some(super::reply::CameraOnScene {
                    room: sim.camera_room(),
                    camera_position: rw_sim::Vec2::from(
                        scene.camera().position(scene.camera().current()),
                    ),
                    window: rw_sim::Vec2::from(scene.camera().view_origin(1.0)),
                }),
            ),
            slime_molds: super::reply::SlimeMoldState::all(sim.world()),
            token_stalks: super::reply::TokenStalkState::all(sim.world()),
            coral_systems: super::reply::CoralSystemState::all(sim.world()),
            climbable_vines: super::reply::ClimbableVinesState::all(sim.world()),
            wall_mycelia: super::reply::WallMyceliaState::all(sim.world()),
            coral_circuits: super::reply::CoralCircuitState::all(sim.world()),
            coral_neurons: super::reply::CoralNeuronState::all(
                sim.world(),
                sim.player().and_then(|p| p.vine_pos()).map(|v| v.vine),
            ),
            coral_stems: super::reply::CoralStemState::all(
                sim.world(),
                sim.player().and_then(|p| p.vine_pos()).map(|v| v.vine),
            ),
            projected_circles: super::reply::ProjectedCircleState::all(sim.world()),
            neuron_sparks: super::reply::NeuronSparkState::all(sim.world()),
            particles: super::reply::RoomParticlesState::all(sim.world()),
            decals: super::reply::DecalState::all(sim.world()),
            rainbows: super::reply::RainbowState::all(sim.world()),
            light_beams: super::reply::LightBeamState::all(sim.world()),
            spot_lights: super::reply::SpotLightState::all(sim.world()),
            light_fixtures: super::reply::LightFixtureState::all(sim.world()),
            light_rods: super::reply::LightRodState::all(sim.world()),
            zap_coils: super::reply::ZapCoilState::all(sim.world()),
            zap_coil_lights: super::reply::ZapCoilLightState::all(sim.world()),
            zap_flashes: super::reply::ZapFlashState::all(sim.world()),
            fuses: super::reply::FuseState::all(sim.world()),
            deep_processing: super::reply::DeepProcessingState::all(sim.world()),
            deep_processing_lights: super::reply::DeepProcessingLightState::all(sim.world()),
            gravity_disruptors: super::reply::GravityDisruptorState::all(sim.world()),
            room_gravity: super::reply::RoomGravityState::all(sim.world()),
            broken_anti_gravity: super::reply::BrokenAntiGravityState::of(sim.world()),
            waterfalls: super::reply::WaterFallState::all(sim.world()),
            electric_death: super::reply::ElectricDeathState::all(sim.world()),
            border_push_back: super::reply::BorderPushBackState::all(sim.world()),
            shortcut_helper: super::reply::ShortcutHelperState::all(sim.world()),
            projectors: projections.projectors,
            scan_lines: projections.scan_lines,
            star_matrices: projections.star_matrices,
            holo_light_needed: sim.holo_light_needed(),
            dens: Self::dens(scene),
            quantified: Self::quantified(scene),
            heavy_ai: HeavyAiInfo {
                concurrent: sim.heavy_ai().concurrent,
                delayed_except_last_run: sim.heavy_ai().delayed_except_last_run,
                last_run: sim
                    .heavy_ai()
                    .last_run
                    .iter()
                    .map(|e| [e.spawner, e.number])
                    .collect(),
            },
            flies_world: world.flies_world_ai().map(|f| FliesWorldInfo {
                to_respawn: f.flies_to_respawn,
                to_spawn: f.flies_to_spawn.clone(),
                all_initial_spawned: f.all_initial_flies_spawned,
            }),
            orphan_emitters: world.orphan_loop_count(),
            region_state: world.region_state.as_ref().map(|r| RegionStateInfo {
                last_cycle_updated: r.last_cycle_updated,
                new_rooms_this_cycle: r.new_rooms_this_cycle,
                swarm_room_counters: r.swarm_room_counters.clone(),
                lineage_counters: r.lineage_counters.clone(),
                consumed_items: r
                    .consumed_items
                    .iter()
                    .map(|c| (c.origin_room.clone(), c.placed_object_index, c.wait_cycles))
                    .collect(),
                rooms_visited: r.rooms_visited.len(),
                saved_population: r.saved_population.len(),
                saved_objects: r.saved_objects.len(),
                saved_sticks: r.saved_sticks.len(),
            }),
        })
    }

    /// The rooms with creatures in their dens.
    fn dens(scene: &Scene) -> Vec<RoomDens> {
        let world = scene.game().sim().world();
        world
            .data()
            .rooms
            .iter()
            .filter(|r| !world.room_dens(r.index).is_empty())
            .map(|r| RoomDens {
                room: r.name.clone(),
                creatures: world
                    .room_dens(r.index)
                    .iter()
                    .map(|&c| Self::entity_id(scene, c))
                    .collect(),
            })
            .collect()
    }

    fn quantified(scene: &Scene) -> Vec<QuantifiedInfo> {
        let world = scene.game().sim().world();
        world
            .data()
            .rooms
            .iter()
            .flat_map(|r| {
                world
                    .room_quantified(r.index)
                    .iter()
                    .filter(|q| q.count > 0)
                    .map(|q| QuantifiedInfo {
                        room: r.name.clone(),
                        node: q.node,
                        kind: q.kind.value().to_owned(),
                        count: q.count,
                    })
            })
            .collect()
    }

    fn session_info(scene: &Scene) -> SessionInfo {
        let sim = scene.game().sim();
        let deaths = sim.session_deaths();
        SessionInfo {
            kills: deaths
                .kills
                .iter()
                .map(|k| (k.kind.value().to_owned(), [k.id.spawner, k.id.number]))
                .collect(),
            peaceful: deaths.peaceful,
            respawn: deaths.respawn_creatures.clone(),
            wait_respawn: deaths.wait_respawn_creatures.clone(),
            saved_respawn: sim.story().respawn_creatures.clone(),
            like_of_player: rw_save::CommunityId::ALL
                .into_iter()
                .map(|c| {
                    let world = sim.world();
                    let like = world.communities.like_of_player(
                        c,
                        world.region_name(),
                        world.session_difficulty,
                    );
                    (c.value().to_owned(), like)
                })
                .collect(),
            guide_likes_player: sim.guide_state().likes_player,
            guide_increase_like_on_save: sim.guide_state().increase_like_on_save,
            influences: deaths
                .influences
                .iter()
                .map(|i| (i.community.value().to_owned(), i.influence))
                .collect(),
        }
    }

    fn sprite_order(scene: &Scene) -> Vec<SpriteLeaserState> {
        let order = scene.game().sprite_order();
        let (shown, behind, holders) = (order.shown_set(), order.behind_set(), order.holders());
        order
            .order()
            .map(|d| {
                let (kind, id, index) = Self::drawable(scene, d);
                SpriteLeaserState {
                    kind,
                    id,
                    index,
                    behind: behind.contains(&d),
                    held_by: holders.get(&d).and_then(|&h| Self::drawable(scene, h).1),
                    shown: shown.contains(&d),
                }
            })
            .collect()
    }

    /// A drawable's kind, EntityID and port id.
    fn drawable(scene: &Scene, d: Drawable) -> (&'static str, Option<[i32; 2]>, Option<u64>) {
        let sim = scene.game().sim();
        match d {
            Drawable::Entity(Entity::Creature(c)) if c == sim.player_creature() => {
                ("player", Some(Self::entity_id(scene, c)), None)
            }
            Drawable::Entity(Entity::Creature(c)) => {
                ("creature", Some(Self::entity_id(scene, c)), None)
            }
            Drawable::Entity(Entity::Object(o)) => {
                let e = sim.world().object(o).id;
                ("object", Some([e.spawner, e.number]), None)
            }
            Drawable::ShelterDoor => ("shelter_door", None, None),
            Drawable::Drip(i) => ("drip", None, Some(i)),
            Drawable::Cosmetic(i) => ("cosmetic", None, Some(i)),
            Drawable::HoloLight(i) => ("holo_light", None, Some(i)),
            Drawable::LightSource(i) => ("light_source", None, Some(i)),
            Drawable::Insect(i) => ("insect", None, Some(i)),
            Drawable::SlimeMold(i) => ("slime_mold", None, Some(i)),
            Drawable::TokenStalk(i) => ("token_stalk", None, Some(i)),
            Drawable::Particle(i) => ("particle", None, Some(i)),
            Drawable::Decal(i) => ("decal", None, Some(i)),
            Drawable::Rainbow(i) => ("rainbow", None, Some(i)),
            Drawable::LightBeam(i) => ("light_beam", None, Some(i)),
            Drawable::SpotLight(i) => ("spot_light", None, Some(i)),
            Drawable::SceneElement(i) => ("scene_element", None, Some(i)),
            Drawable::DustPuff(i) => ("dust_puff", None, Some(i)),
            Drawable::Fixture(i) => ("fixture", None, Some(i)),
            Drawable::ElectricDeath(i) => ("electric_death", None, Some(i)),
            Drawable::Projection(i) => ("projection", None, Some(i)),
            Drawable::Coral(i) => ("coral", None, Some(i)),
            Drawable::Shortcuts => ("shortcuts", None, None),
            Drawable::FullScreenEffect => ("full_screen_effect", None, None),
            Drawable::DeathFall => ("death_fall", None, None),
            Drawable::WaterLight => ("water_light", None, None),
        }
    }

    fn holo_lights(scene: &Scene) -> Vec<HoloLightState> {
        let world = scene.game().sim().world();
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| world.realized(i))
            .flat_map(|r| r.holo_lights.iter().map(move |l| (r, l)))
            .map(|(r, l)| {
                let o = world.creature(l.overseer).id;
                let light_source = l
                    .light_source
                    .and_then(|id| r.light_sources.iter().find(|s| s.id == id))
                    .map(|s| LightSourceState {
                        pos: s.pos.to_array(),
                        rad: s.rad,
                        alpha: s.alpha,
                        color: s.color,
                        alive: !s.slated,
                    });
                HoloLightState {
                    room: Self::room_name(scene, l.room),
                    overseer: [o.spawner, o.number],
                    pos: l.pos.to_array(),
                    proj_pos: l.proj_pos.to_array(),
                    power: l.power,
                    flicker: l.flicker,
                    out_of_range_counter: l.out_of_range_counter,
                    not_needed_counter: l.not_needed_counter,
                    active_linger: l.active_linger,
                    respawn: l.respawn,
                    power_down_and_kill: l.power_down_and_kill,
                    light_source,
                }
            })
            .collect()
    }

    /// The world's creatures but the player, in creation order.
    fn creatures(scene: &Scene) -> Vec<CreatureInfo> {
        let sim = scene.game().sim();
        let world = sim.world();
        world
            .creatures()
            .filter(|(id, c)| *id != sim.player_creature() && !c.slated_for_deletion)
            .map(|(id, c)| CreatureInfo {
                kind: c.kind().value().to_owned(),
                id: [c.id.spawner, c.id.number],
                room: Self::room_name(scene, c.pos.room),
                node: c.pos.node,
                tile: [c.pos.tile.0, c.pos.tile.1],
                in_den: c.in_den,
                realized: c.is_realized(),
                alive: c.state.alive,
                kill_tag: c.kill_tag.map(|k| {
                    let k = world.creature(k).id;
                    [k.spawner, k.number]
                }),
                kill_tag_counter: c.kill_tag_counter,
                stun: c
                    .fly()
                    .map(|f| f.stun)
                    .or_else(|| c.overseer_body().map(|b| b.stun))
                    .or_else(|| c.lizard().map(|l| l.stun)),
                impaled_on: world
                    .spear_sticks_of(Entity::Creature(id))
                    .iter()
                    .find(|s| matches!(s.kind, SpearStickKind::Impaled { .. }))
                    .map(|s| {
                        let o = world.object(s.spear).id;
                        [o.spawner, o.number]
                    }),
                abstract_ai: Self::abstract_ai_info(scene, c),
                overseer: c.overseer_ai().map(|o| OverseerInfo {
                    player_guide: o.is_player_guide,
                    destination: c
                        .ai
                        .as_ref()
                        .map_or_else(String::new, |a| Self::room_name(scene, a.destination.room)),
                    go_to_player: o.go_to_player,
                    player_guide_counter: o.player_guide_counter,
                    last_rooms: o
                        .last_rooms
                        .iter()
                        .map(|&r| Self::room_name(scene, r))
                        .collect(),
                    hover_tile: c.overseer_body().map(|b| [b.hover_tile.0, b.hover_tile.1]),
                    root_tile: c.overseer_body().map(|b| [b.root_tile.0, b.root_tile.1]),
                    mode: c.overseer_body().map(|b| format!("{:?}", b.mode)),
                    pos: c.overseer_body().map(|b| b.chunk().pos.to_array()),
                    extended: c.overseer_body().map(|b| b.extended),
                    dead: c.overseer_body().map(|b| b.dead),
                    dying: c.overseer_body().map(|b| b.dying),
                    most_dangerous_creature: o
                        .real_ai
                        .as_ref()
                        .and_then(|a| a.communication.as_ref())
                        .and_then(|m| m.most_dangerous_creature_in_room)
                        .map(|d| {
                            let d = world.creature(d).id;
                            [d.spawner, d.number]
                        }),
                    most_dangerous_creature_danger: o
                        .real_ai
                        .as_ref()
                        .and_then(|a| a.communication.as_ref())
                        .map(|m| m.most_dangerous_creature_danger),
                }),
                fly: Self::fly_info(scene, c),
                lizard: Self::lizard_info(scene, id, c),
                danger: world
                    .creature_danger(id, scene.game().sim().player_creature())
                    .map(|d| DangerInfo {
                        dangerous_to_player: d.dangerous_to_player,
                        danger_pos: d.danger_pos.to_array(),
                        player_aggression: d.player_aggression,
                    }),
                social: c
                    .state
                    .social_memory
                    .iter()
                    .flat_map(|m| &m.relationships)
                    .map(|r| RelationshipInfo {
                        subject: [r.subject.spawner, r.subject.number],
                        like: r.like,
                        temp_like: r.temp_like,
                        fear: r.fear,
                        temp_fear: r.temp_fear,
                        know: r.know,
                    })
                    .collect(),
                carries: c
                    .sticks
                    .iter()
                    .filter(|s| s.a == id)
                    .map(|s| HeldInfo::of(world, s.b))
                    .collect(),
            })
            .collect()
    }

    fn node_ref(scene: &Scene, c: WorldCoordinate) -> NodeRef {
        NodeRef {
            room: Self::room_name(scene, c.room),
            node: c.node,
        }
    }

    /// The plain abstract AI of a creature that runs it.
    fn abstract_ai_info(scene: &Scene, c: &AbstractCreature) -> Option<AbstractAiInfo> {
        if !c.template.abstract_ai_ported() {
            return None;
        }
        let ai = c.ai.as_ref()?;
        let den = scene.game().sim().world().offscreen_den_index();
        Some(AbstractAiInfo {
            destination: Self::node_ref(scene, ai.destination),
            path: ai.path.iter().map(|&p| Self::node_ref(scene, p)).collect(),
            migration_destination: ai.migration_destination.map(|m| Self::node_ref(scene, m)),
            den_position: ai.den_position(c, den).map(|d| Self::node_ref(scene, d)),
            time_buffer: ai.time_buffer,
            remain_in_den_counter: c.remain_in_den_counter,
            time_spent_here: c.time_spent_here,
            follow_creature: ai.follow_creature.map(|f| Self::entity_id(scene, f)),
        })
    }

    /// The world's objects, in creation order.
    fn objects(scene: &Scene) -> Vec<ObjectInfo> {
        scene
            .game()
            .sim()
            .world()
            .objects()
            .map(|(oid, o)| ObjectInfo {
                kind: o.kind.value().to_owned(),
                id: [o.id.spawner, o.id.number],
                room: Self::room_name(scene, o.pos.room),
                node: o.pos.node,
                tile: [o.pos.tile.0, o.pos.tile.1],
                stuck_in_wall_cycles: o.spear().map(|s| s.stuck_in_wall_cycles),
                explosive: o.spear().map(|s| s.explosive),
                realized: o
                    .realized
                    .as_deref()
                    .map(|w| Self::weapon_info(scene, oid, w)),
            })
            .collect()
    }

    fn weapon_info(scene: &Scene, id: ObjectId, w: &Weapon) -> WeaponInfo {
        let world = scene.game().sim().world();
        let c = w.chunk();
        WeaponInfo {
            room: Self::room_name(scene, w.room),
            pos: c.pos.to_array(),
            vel: c.vel.to_array(),
            mode: w.mode.value().to_owned(),
            rotation: w.rotation.to_array(),
            rotation_speed: w.rotation_speed,
            collision_layer: w.collision_layer,
            stuck_in_wall: w
                .spear()
                .and_then(|s| s.stuck_in_wall)
                .map(|p| p.to_array()),
            impaled: world
                .spear_sticks_of(Entity::Object(id))
                .iter()
                .filter_map(|s| match s.kind {
                    SpearStickKind::Impaled {
                        on_spear_position, ..
                    } => {
                        let c = world.creature(s.creature).id;
                        Some(([c.spawner, c.number], on_spear_position))
                    }
                    SpearStickKind::Lodged { .. } => None,
                })
                .collect(),
            lodged_in: world
                .spear_sticks_of(Entity::Object(id))
                .iter()
                .find_map(|s| match s.kind {
                    SpearStickKind::Lodged { chunk, .. } => {
                        let c = world.creature(s.creature).id;
                        Some(([c.spawner, c.number], chunk))
                    }
                    SpearStickKind::Impaled { .. } => None,
                }),
            released_emitters: w.sound_loop.released.iter().filter(|e| !e.slated).count(),
        }
    }

    fn fly_info(scene: &Scene, c: &AbstractCreature) -> Option<FlyInfo> {
        let f = c.fly()?;
        let chunk = f.chunk();
        Some(FlyInfo {
            room: f.room.map(|r| Self::room_name(scene, r)),
            pos: chunk.pos.to_array(),
            vel: chunk.vel.to_array(),
            dead: f.dead,
            in_shortcut: f.in_shortcut,
            behavior: f.ai.behavior.map(|b| b.value().to_owned()),
            mov_mode: f.mov_mode.map(|m| m.value().to_owned()),
            grasps: c
                .grasps
                .iter()
                .map(|g| g.map(|g| HeldInfo::of(scene.game().sim().world(), g.grabbed).id))
                .collect(),
            grabbed_by: c
                .grabbed_by
                .iter()
                .map(|g| Self::entity_id(scene, g.grabber))
                .collect(),
        })
    }

    fn lizard_info(scene: &Scene, id: CreatureId, c: &AbstractCreature) -> Option<LizardInfo> {
        let l = c.lizard()?;
        let world = scene.game().sim().world();
        let holding = c.grasps.first().is_some_and(Option::is_some);
        let (throat_health, limb_health) = c.state.lizard_health_parts().unwrap_or((1.0, [1.0; 4]));
        Some(LizardInfo {
            room: l.room.map(|r| Self::room_name(scene, r)),
            chunks: l
                .chunks()
                .iter()
                .map(|ch| LizardChunk {
                    pos: ch.pos.to_array(),
                    vel: ch.vel.to_array(),
                })
                .collect(),
            health: c.state.health,
            throat_health,
            limb_health,
            meat_left: c.state.meat_left,
            dead: l.dead,
            in_shortcut: l.in_shortcut,
            jaw_open: l.jaw_open(holding),
            last_jaw_open: l.last_jaw_open,
            jaw_forced_shut: l.jaw_forced_shut,
            bite_delay: l.bite_delay,
            apply_gravity: l.apply_gravity,
            in_allowed_terrain_counter: l.in_allowed_terrain_counter,
            turned_by_rock_counter: l.turned_by_rock_counter,
            lungs: l.lungs,
            effect_color: l.effect_color,
            grasps: c
                .grasps
                .iter()
                .map(|g| g.map(|g| HeldInfo::of(world, g.grabbed).id))
                .collect(),
            grabbed_by: c
                .grabbed_by
                .iter()
                .map(|g| Self::entity_id(scene, g.grabber))
                .collect(),
            spears: world
                .spear_sticks_of(Entity::Creature(id))
                .iter()
                .filter_map(|s| match s.kind {
                    SpearStickKind::Lodged { chunk, .. } => {
                        let o = world.object(s.spear).id;
                        Some(([o.spawner, o.number], chunk))
                    }
                    SpearStickKind::Impaled { .. } => None,
                })
                .collect(),
            movement: Self::lizard_move_info(scene, id, l),
            decision: Self::lizard_decision_info(scene, id, l),
            tracker: scene
                .game()
                .sim()
                .tracker_readings(id)
                .iter()
                .map(|r| Self::tracked_creature_info(scene, r))
                .collect(),
            relationships: l
                .ai
                .relationships
                .relationships
                .iter()
                .filter_map(|&k| l.ai.tracker.rep(k))
                .filter_map(|r| {
                    let d = r.dynamic_relationship.as_ref()?;
                    Some(DynamicRelationshipInfo {
                        id: Self::entity_id(scene, r.creature),
                        kind: d.current.kind.value().to_owned(),
                        intensity: d.current.intensity,
                        module: d.tracked_by_module.map(|m| m.value().to_owned()),
                        spear: d.state.map(|TrackedCreatureState::Lizard(s)| s.spear),
                    })
                })
                .collect(),
            casual_aggression_target: l
                .ai
                .casual_aggression_target
                .and_then(|k| l.ai.tracker.rep(k))
                .map(|r| Self::entity_id(scene, r.creature)),
            noise: Self::noise_tracker_info(scene, l),
            graphics: Self::lizard_graphics_info(scene, c, l, holding),
        })
    }

    fn noise_tracker_info(scene: &Scene, l: &rw_sim::creature::Lizard) -> NoiseTrackerInfo {
        let world = scene.game().sim().world();
        let n = &l.ai.noise;
        NoiseTrackerInfo {
            hearing_skill: n.hearing_skill,
            mysterious_noises: n.mysterious_noises,
            mysterious_noise_counter: n.mysterious_noise_counter,
            utility: n.utility(),
            sound_to_examine: n.sound_to_examine().map(|s| s.pos.to_array()),
            sources: n
                .sources
                .iter()
                .map(|s| NoiseSourceInfo {
                    pos: s.pos.to_array(),
                    age: s.age,
                    creature: s
                        .creature_rep
                        .and_then(|k| l.ai.tracker.rep(k))
                        .map(|r| Self::entity_id(scene, r.creature)),
                    strength: s.last_sound_strength,
                    slated: s.slated_for_deletion,
                })
                .collect(),
            heard: n
                .heard
                .iter()
                .map(|h| HeardNoiseInfo {
                    ticks_ago: h.ticks_ago,
                    mysterious: h.mysterious,
                    pos: h.noise.pos.to_array(),
                    strength: h.noise.strength,
                    interesting: h.noise.interesting,
                    source: h.noise.source.map(|e| HeldInfo::of_noise(world, e)),
                })
                .collect(),
        }
    }

    fn tracked_creature_info(scene: &Scene, r: &RepReading) -> TrackedCreatureInfo {
        let world = scene.game().sim().world();
        TrackedCreatureInfo {
            id: Self::entity_id(scene, r.creature),
            kind: world.creature(r.creature).kind().value().to_owned(),
            representation: if r.elaborate { "Elaborate" } else { "Simple" }.to_owned(),
            age: r.age,
            ticks_since_seen: r.ticks_since_seen,
            forget_counter: r.forget_counter,
            visual_contact: r.visual_contact,
            estimated_chance_of_finding: r.estimated_chance_of_finding,
            best_guess: Self::coord_info(scene, r.best_guess),
            last_seen: Self::coord_info(scene, r.last_seen),
            priority: r.priority,
            delete_me_next_frame: r.delete_me_next_frame,
            ghosts: r
                .ghosts
                .iter()
                .map(|g| GhostInfo {
                    coord: Self::coord_info(scene, g.coord),
                    pos: g.pos.to_array(),
                    generation: g.generation,
                    stopped: g.stopped,
                })
                .collect(),
        }
    }

    fn lizard_graphics_info(
        scene: &Scene,
        c: &AbstractCreature,
        l: &Lizard,
        holding: bool,
    ) -> Option<LizardGraphicsInfo> {
        let link = l.graphics.as_ref()?;
        let g = scene
            .game()
            .lizard_graphics()
            .iter()
            .find(|g| g.id == c.id)?;
        let jaw = l.jaw_open(holding);
        Some(LizardGraphicsInfo {
            culled: g.culled,
            legs_grabbing: link.legs_grabbing,
            no_grip_counter: link.no_grip_counter,
            limbs: g
                .limbs
                .iter()
                .map(|limb| LizardLimbInfo {
                    mode: format!("{:?}", limb.limb.mode),
                    pos: limb.limb.part.pos.to_array(),
                    hunt_pos: limb.limb.absolute_hunt_pos.to_array(),
                    grab_pos: limb.grab_pos.to_array(),
                    grip_counter: limb.grip_counter,
                    reaching_for_terrain: limb.reaching_for_terrain,
                    currently_disabled: limb.currently_disabled,
                    flip: limb.flip,
                })
                .collect(),
            head: link.head.pos.to_array(),
            head_rotation: link.head.rotation(l.params, jaw),
            head_depth_rotation: g.head_depth_rotation,
            depth_rotation: g.depth_rotation,
            jaw_open: jaw,
            blink: g.blink,
            flicker: g.flicker,
            white_flicker: link.white_flicker,
            look_pos: g.look_pos.to_array(),
            look_creature: link
                .looker
                .and_then(|lk| lk.look_creature)
                .and_then(|k| l.ai.tracker.rep(k))
                .map(|r| Self::entity_id(scene, r.creature)),
            look_focus_delay: link.looker.map_or(0, |lk| lk.look_focus_delay),
            eye_beams_active: g.eye_beams_active,
            cosmetics: g.cosmetics.iter().map(Self::lizard_cosmetic_info).collect(),
            light: link.light,
            voice_visualization_intensity: g.voice_visualization_intensity,
            voice_visualization: g.voice_visualization,
            released_emitters: link
                .sound_loop
                .released
                .iter()
                .chain(&l.dropped_loop_emitters)
                .filter(|e| !e.slated)
                .count(),
            voice: l.voice.as_ref().map(Self::lizard_voice_info),
        })
    }

    fn lizard_cosmetic_info(c: &LizardCosmetic) -> LizardCosmeticInfo {
        let (graphic, colored, count, tips) = match &c.shape {
            CosmeticShape::SpineSpikes(b) | CosmeticShape::BumpHawk(b) => (
                b.graphic,
                b.colored,
                usize::try_from(b.bumps).unwrap_or(0),
                Vec::new(),
            ),
            CosmeticShape::LongShoulderScales(s)
            | CosmeticShape::ShortBodyScales(s)
            | CosmeticShape::TailTuft(s)
            | CosmeticShape::LongHeadScales(s) => (
                s.graphic,
                i32::from(s.colored),
                s.positions.len(),
                s.scale_objects
                    .iter()
                    .map(|o| o.part.pos.to_array())
                    .collect(),
            ),
        };
        LizardCosmeticInfo {
            kind: c.shape.name().to_owned(),
            start_sprite: c.start_sprite,
            sprites: c.number_of_sprites,
            overlap: c.sprites_overlap.value().to_owned(),
            graphic,
            colored,
            count,
            tips,
        }
    }

    fn lizard_voice_info(v: &LizardVoice) -> LizardVoiceInfo {
        LizardVoiceInfo {
            trigger: v.my_voice_trigger.map(|s| s.name().to_owned()),
            sound: v.sound_id.map(|s| s.name().to_owned()),
            emotion: v.articulation.map(|e| e.value().to_owned()),
            progression: v.current_articulation_progression,
            emotion_intensity: v.current_emotion_intensity,
            volume: v.volume,
            pitch: v.pitch,
            making_a_sound: v.making_a_sound(),
            saved_intensity: v.saved_intensity,
            visualized_intensity: v.visualized_intensity(),
            emitter: v.sound_a.is_some(),
            playing: v.sound_a.as_ref().is_some_and(|s| s.emitter.playing),
            released: v.released.len(),
        }
    }

    fn lizard_move_info(scene: &Scene, id: CreatureId, l: &Lizard) -> LizardMoveInfo {
        let sim = scene.game().sim();
        let world = sim.world();
        let pf = world.path_finder(id);
        let vessel = sim.creature_vessel(id).map(|(v, in_lobby)| {
            let (pos, last) = v.positions();
            VesselInfo {
                room: Self::room_name(scene, v.room),
                pos: [pos.0, pos.1],
                last_positions: last.iter().map(|t| [t.0, t.1]).collect(),
                wait: v.wait(),
                in_lobby,
                connected: world
                    .connected_objects(Entity::Creature(id))
                    .into_iter()
                    .skip(1)
                    .map(|e| HeldInfo::of(world, e))
                    .collect(),
            }
        });
        LizardMoveInfo {
            destination: pf.map(|f| Self::coord_info(scene, f.get_destination())),
            effectual_destination: pf
                .map(|f| Self::coord_info(scene, f.get_effectual_destination())),
            mapping: world.path_finder_mapping(id),
            forbidden_entrance_counter: pf.map_or(0, |f| f.forbidden_entrance_counter),
            stranded: l.ai.stranded,
            run_speed: l.ai.run_speed,
            dev_run_speed: l.ai.dev_run_speed,
            animation: l.animation.value().to_owned(),
            following: l.following_connection.map(ConnectionInfo::of),
            upcoming: l
                .upcoming_connections
                .iter()
                .copied()
                .map(ConnectionInfo::of)
                .collect(),
            movement_animation: l
                .movement_animation
                .as_ref()
                .map(|a| MovementAnimationInfo {
                    connection: ConnectionInfo::of(a.connection),
                    step: a.step,
                    total_steps: a.total_steps,
                }),
            grip_point: l.grip_point.map(Vec2::to_array),
            legs_gripping: l.legs_gripping(),
            no_grip_counter: l.no_grip_counter(),
            time_spent_trying_this_move: l.time_spent_trying_this_move,
            desperation_smoother: l.desperation_smoother,
            body_wiggle_counter: l.body_wiggle_counter,
            stuck_counter: l.ai.stuck.stuck_counter,
            stuck_utility: l.ai.stuck.utility(),
            entering_shortcut: l.entering_shortcut.map(|t| [t.0, t.1]),
            vessel,
        }
    }

    /// The EntityID of the creature lizard `l`'s representation `k` stands for.
    fn rep_entity(
        scene: &Scene,
        l: &Lizard,
        k: Option<rw_sim::creature::RepKey>,
    ) -> Option<[i32; 2]> {
        k.and_then(|k| l.ai.tracker.rep(k))
            .map(|r| Self::entity_id(scene, r.creature))
    }

    fn lizard_decision_info(scene: &Scene, id: CreatureId, l: &Lizard) -> LizardDecisionInfo {
        let sim = scene.game().sim();
        let world = sim.world();
        let c = world.creature(id);
        let ai = &l.ai;
        let readings = sim.lizard_utility_readings(id);
        let abs = c.ai.as_ref();
        LizardDecisionInfo {
            behavior: ai.behavior.value().to_owned(),
            current_utility: ai.current_utility,
            highest: readings
                .as_ref()
                .and_then(|r| r.highest)
                .map(|m| m.value().to_owned()),
            highest_utility: readings.as_ref().map_or(0.0, |r| r.highest_utility),
            utilities: readings
                .as_ref()
                .map(|r| {
                    r.modules
                        .iter()
                        .map(|&(m, utility, weight)| UtilityInfo {
                            module: m.value().to_owned(),
                            utility,
                            weight,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            travel_utility: readings.as_ref().map_or(0.0, |r| r.travel),
            fear: ai.fear,
            hunger: ai.hunger,
            rain_fear: ai.rain_fear,
            excitement: ai.excitement,
            panic: ai.panic,
            last_distress_length: ai.last_distress_length,
            destination: Self::coord_info(scene, abs.map_or(c.pos, |a| a.destination)),
            idle_counter: ai.idle_counter,
            idle_restlessness: ai.idle_restlessness,
            idle_spot_win_streak: ai.idle_spot_win_streak,
            unable_to_find_comfortable_position: ai.unable_to_find_comfortable_position,
            forbidden_idle_spot: Self::coord_info(scene, ai.forbidden_idle_spot),
            den: abs
                .and_then(|a| a.den_position)
                .map(|d| Self::coord_info(scene, d)),
            den_finder: ai.den_finder.status.value().to_owned(),
            den_search_updates_left: ai.den_finder.search.as_ref().map(|s| s.updates_left),
            focus_creature: Self::rep_entity(scene, l, ai.focus_creature),
            prey: PreyTrackerInfo {
                current: Self::rep_entity(scene, l, ai.prey.current),
                frustration: ai.prey.frustration,
                prey: ai
                    .prey
                    .prey
                    .iter()
                    .map(|p| TrackedPreyInfo {
                        id: Self::rep_entity(scene, l, Some(p.rep)),
                        unreachable_counter: p.unreachable_counter,
                        at_position_but_cant_see_counter: p.at_position_but_cant_see_counter,
                    })
                    .collect(),
            },
            threat: ThreatTrackerInfo {
                current_threat: ai.threat.current_threat,
                utility: ai.threat.utility(),
                most_threatening: Self::rep_entity(scene, l, ai.threat.most_threatening),
                reset_counter: ai.threat.reset_counter,
                creatures: ai
                    .threat
                    .creatures
                    .iter()
                    .map(|t| (Self::rep_entity(scene, l, Some(t.rep)), t.severity))
                    .collect(),
                points: ai
                    .threat
                    .points
                    .iter()
                    .map(|p| ([p.pos.tile.0, p.pos.tile.1], p.severity))
                    .collect(),
            },
            agression: AgressionTrackerInfo {
                highest_anger: ai.agression.highest_anger,
                target: Self::rep_entity(scene, l, ai.agression.highest_target),
                creatures: ai
                    .agression
                    .creatures
                    .iter()
                    .map(|a| {
                        (
                            Self::rep_entity(scene, l, Some(a.rep)),
                            a.anger,
                            a.base_anger,
                        )
                    })
                    .collect(),
            },
            injury: rw_sim::creature::lizard::LizardAi::injury_utility(c.state.health),
            hunt: Self::hunt_info(scene, id, l),
            lounge: LoungeInfo {
                lounge_delay: l.lounge_delay,
                post_lounge_stun: l.post_lounge_stun,
                attempt_bite_frames: ai.attempt_bite_frames,
                bite_delay: l.bite_delay,
                jaw_open: l.jaw_open(c.grasps.first().is_some_and(Option::is_some)),
                lounge_dir: [l.lounge_dir.x, l.lounge_dir.y],
            },
            flee: FleeInfo {
                flee_to: ai.flee_to.map(|d| Self::coord_info(scene, d)),
                saved: Self::coord_info(scene, ai.threat.saved_flee_dest),
                test: Self::coord_info(scene, ai.threat.test_flee_dest),
                test_threat: ai.threat.flee_threat,
                anti_flicker_counter: ai.threat.anti_flicker_counter,
            },
            heavy_ai_granted: ai.heavy_ai_granted,
            social: LizardSocialInfo::of(world, l),
        }
    }

    /// The most attractive prey while hunting, as the tracker reads it now.
    fn hunt_info(scene: &Scene, id: CreatureId, l: &Lizard) -> Option<HuntInfo> {
        if l.ai.behavior != rw_sim::creature::lizard::LizardBehavior::Hunt {
            return None;
        }
        let prey =
            l.ai.prey
                .most_attractive()
                .and_then(|k| l.ai.tracker.rep(k))?
                .creature;
        let r = scene
            .game()
            .sim()
            .tracker_readings(id)
            .into_iter()
            .find(|r| r.creature == prey)?;
        Some(HuntInfo {
            target: Some(Self::entity_id(scene, r.creature)),
            visual_contact: r.visual_contact,
            age: r.age,
            guess: Self::coord_info(scene, r.best_guess),
        })
    }

    fn coord_info(scene: &Scene, c: WorldCoordinate) -> CoordInfo {
        CoordInfo {
            room: Self::room_name(scene, c.room),
            tile: [c.tile.0, c.tile.1],
            node: c.node,
        }
    }

    /// Creature `id`'s EntityID as `[spawner, number]`.
    fn entity_id(scene: &Scene, id: CreatureId) -> [i32; 2] {
        let e = scene.game().sim().world().creature(id).id;
        [e.spawner, e.number]
    }

    fn room_name(scene: &Scene, index: i32) -> String {
        scene
            .game()
            .sim()
            .world()
            .abstract_room(index)
            .map_or_else(|| format!("#{index}"), |r| r.name.clone())
    }

    /// The camera room's shortcut nodes: entrance, kind, where each exit leads and the
    /// camera it arrives in.
    ///
    /// # Errors
    /// Without a game.
    pub fn exits(&self) -> Result<ExitList, String> {
        ExitList::of(self.scene()?)
    }

    /// Renders the scene offscreen as the window would and encodes it as PNG.
    ///
    /// # Errors
    /// Without a room or a GPU, or when saving fails.
    pub fn screenshot(&mut self, params: &ScreenshotParams) -> Result<Screenshot, String> {
        let level = params.view.unwrap_or_default() == ViewParam::Visuals;
        let scale = params.scale.unwrap_or(1.0);
        if !(scale > 0.0 && scale <= 4.0) {
            return Err("scale must be in (0, 4]".to_owned());
        }
        if self.frontend.is_some() {
            if params.room.is_some() {
                return Err("room needs a game (load_room or new_game)".to_owned());
            }
            return self.process_screenshot(params, level, scale);
        }
        let room = self.room_named(params.room.as_deref())?;
        let scene = self.scene()?;
        if room != scene.game().sim().camera_room() {
            return self.room_screenshot(room, params, level, scale);
        }
        let (framing, size) = match params.crop_radius {
            Some(r) if r > 0.0 => {
                let centre = params.crop_center.unwrap_or_else(|| scene.focus());
                let origin = [centre[0] - r, centre[1] - r];
                let size = [2.0 * r, 2.0 * r];
                (Framing::Rect { origin, size }, size)
            }
            Some(_) => return Err("crop_radius must be positive".to_owned()),
            None => (Framing::Auto, scene.auto_size(level)),
        };
        let pixels = size.map(|s| (s * scale).round().clamp(1.0, MAX_IMAGE_SIDE) as u32);
        let view = SceneView {
            level,
            layers: RoomLayers {
                tiles: true,
                grid: params.grid.unwrap_or(false),
            },
            framing,
        };
        self.ensure_renderer(pixels)?;
        let (Some(scene), Some(target)) = (self.scene.as_mut(), self.renderer.as_mut()) else {
            return Err("no game loaded; call load_room or new_game first".to_owned());
        };
        target.scene.renderer.set_frame_size((pixels[0], pixels[1]));
        let hud = level && framing == Framing::Auto;
        if let (Some(shared), true) = (&self.hud_shared, hud) {
            scene.prepare_hud(&shared.atlases, 1.0);
        }
        scene.draw(&mut target.scene, 1.0, view);
        // The HUD is in screen pixels: drawn over the game's own camera window only.
        if let (Some(shared), true) = (&mut self.hud_shared, hud) {
            target.draw_scene_hud(shared, scene, 1.0);
        }
        Self::encode(target, params)
    }

    /// Queues a message on the HUD's text prompt (`TextPrompt.AddMessage`).
    ///
    /// # Errors
    /// Without a game or before its HUD fired up (the first tick).
    pub fn hud_message(&mut self, params: &HudMessageParams) -> Result<HudState, String> {
        let hud = self
            .scene_or_err()?
            .hud_mut()
            .ok_or("no HUD yet; step once after load_room / new_game")?;
        hud.add_message(
            &params.text,
            params.wait.unwrap_or(0),
            params.time.unwrap_or(160),
            params.darken.unwrap_or(true),
            params.hide_hud.unwrap_or(true),
        );
        self.hud_state()
    }

    /// The HUD's meters, prompt and controls page.
    ///
    /// # Errors
    /// Without a game or before its HUD fired up.
    pub fn hud_state(&self) -> Result<HudState, String> {
        let process_game = self.frontend.as_ref().and_then(|(f, _)| f.game());
        let scene = match process_game {
            Some(game) => &game.scene,
            None => self.scene()?,
        };
        let hud = scene
            .hud()
            .ok_or("no HUD yet; step once after load_room / new_game")?;
        Ok(HudState::of(hud))
    }

    fn ensure_renderer(&mut self, pixels: [u32; 2]) -> Result<(), String> {
        if self.renderer.is_none() {
            let renderer = WgpuRenderer::headless((pixels[0], pixels[1]))
                .map_err(|e| format!("no GPU renderer: {e}"))?;
            self.renderer = Some(FrontendRenderer::new(SceneRenderer::new(
                renderer,
                self.sprites.clone(),
            )));
        }
        Ok(())
    }

    /// The process manager's frame (menu stage or game, and the fade) at the game's
    /// screen size times `scale`.
    fn process_screenshot(
        &mut self,
        params: &ScreenshotParams,
        level: bool,
        scale: f32,
    ) -> Result<Screenshot, String> {
        if params.crop_radius.is_some() {
            return Err("crop_radius needs a room (load_room)".to_owned());
        }
        let size = self
            .frontend
            .as_ref()
            .map_or(rw_render::Resolution::DEFAULT.size_f32(), |(f, _)| {
                f.ctx.menu.screen_size
            });
        let pixels = size.map(|s| (s * scale).round().clamp(1.0, MAX_IMAGE_SIDE) as u32);
        self.ensure_renderer(pixels)?;
        let (Some((frontend, _)), Some(target)) = (self.frontend.as_mut(), self.renderer.as_mut())
        else {
            return Err("no process loaded".to_owned());
        };
        target.scene.renderer.set_frame_size((pixels[0], pixels[1]));
        let view = SceneView {
            level,
            layers: RoomLayers {
                tiles: true,
                grid: params.grid.unwrap_or(false),
            },
            framing: Framing::Auto,
        };
        frontend.draw(target, view);
        Self::encode(target, params)
    }

    /// Renders the queued frame and encodes it, saving it when asked.
    fn encode(
        target: &mut FrontendRenderer<WgpuRenderer>,
        params: &ScreenshotParams,
    ) -> Result<Screenshot, String> {
        let image = target
            .scene
            .renderer
            .render_image()
            .map_err(|e| format!("rendering failed: {e}"))?;
        let png = image
            .encode_png()
            .map_err(|e| format!("PNG encoding failed: {e}"))?;
        let saved = match &params.save_path {
            Some(path) => {
                let path = Path::new(path);
                std::fs::write(path, &png)
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                Some(path.to_owned())
            }
            None => None,
        };
        Ok(Screenshot {
            png,
            width: image.width(),
            height: image.height(),
            saved,
        })
    }

    /// Starts a process manager (boot fade, `Initialization`, then `params.process`),
    /// replacing any loaded room.
    ///
    /// # Errors
    /// When the room is not found or the fonts cannot be loaded.
    pub fn load_process(&mut self, params: &LoadProcessParams) -> Result<ProcessState, String> {
        let name = params.room.as_deref().unwrap_or("summit").to_lowercase();
        let path = self
            .assets
            .find_room_file(&name, ".txt")
            .ok_or_else(|| format!("room {name:?} not found (see list_rooms)"))?;
        log::debug!("room file {}", path.display());
        let setup = GameSetup {
            start: GameStart::Room {
                name,
                tile: None,
                slugcat: SlugcatName::White,
            },
            seed: params.seed.unwrap_or(1),
            sound_table: Arc::clone(&self.sound_table),
            saves: self.saves.clone(),
        };
        let start = match params.process.unwrap_or_default() {
            ProcessParam::IntroRoll => ProcessId::IntroRoll,
            ProcessParam::MainMenu => ProcessId::MainMenu,
            ProcessParam::SlugcatSelect => ProcessId::SlugcatSelect,
            ProcessParam::OptionsMenu => ProcessId::OptionsMenu,
            ProcessParam::FontTest => ProcessId::FontTest,
            ProcessParam::Game => ProcessId::Game,
        };
        let mut ctx = AppContext::new(
            self.assets.clone(),
            Arc::clone(&self.resources),
            rw_render::Resolution::DEFAULT,
            setup,
            start,
        )
        .map_err(|e| format!("loading the fonts: {e}"))?;
        ctx.apply_screen_options(None, false);
        self.frontend = Some((Frontend::new(ctx), 0));
        self.scene = None;
        self.start = None;
        self.process_state()
    }

    /// Runs process manager frames of 1/40 s holding `keys`, for the menus and the
    /// player.
    ///
    /// # Errors
    /// Without a process, or on an unknown key or too many frames.
    pub fn step_process(&mut self, params: &StepProcessParams) -> Result<ProcessState, String> {
        let frames = u64::from(params.frames.unwrap_or(1));
        if frames > MAX_TICKS {
            return Err(format!("at most {MAX_TICKS} frames per call"));
        }
        let keys = params
            .keys
            .iter()
            .flatten()
            .map(|k| KeyParse::key(k))
            .collect::<Result<Vec<_>, _>>()?;
        let (frontend, count) = self
            .frontend
            .as_mut()
            .ok_or("no process loaded; call load_process first")?;
        for &key in &keys {
            frontend.ctx.set_key(key, true);
        }
        for _ in 0..frames {
            frontend.update(FRAME_SECONDS);
            *count += 1;
        }
        for &key in &keys {
            frontend.ctx.set_key(key, false);
        }
        self.process_state()
    }

    /// The process manager's state.
    ///
    /// # Errors
    /// Without a process.
    pub fn process_state(&self) -> Result<ProcessState, String> {
        let (frontend, frame) = self
            .frontend
            .as_ref()
            .ok_or("no process loaded; call load_process first")?;
        let m = &frontend.manager;
        let saves = &frontend.ctx.menu.saves;
        let game = frontend.game().map(|g| g.scene.game());
        Ok(ProcessState {
            save: SaveInfo {
                path: saves.path().map(|p| p.display().to_string()),
                saved: saves
                    .stored_save_numbers()
                    .into_iter()
                    .map(|n| n.value().to_owned())
                    .collect(),
                current: saves.current_save_state.as_ref().map(|s| SaveStateInfo {
                    slugcat: s.save_state_number.value().to_owned(),
                    den: s.den_position.clone(),
                    cycle: s.cycle_number,
                    food: s.food,
                    karma: s.death_persistent.karma,
                    karma_cap: s.death_persistent.karma_cap,
                    deaths: s.death_persistent.deaths,
                    quits: s.death_persistent.quits,
                    loaded: s.loaded,
                    respawn_creatures: s.respawn_creatures.clone(),
                    wait_respawn_creatures: s.wait_respawn_creatures.clone(),
                    dragon_slayer: s.death_persistent.win_state.dragon_slayer(),
                }),
            },
            story: game.map(|g| {
                let sim = g.sim();
                let story = sim.story();
                let pos = sim.player_pos();
                StoryInfo {
                    den: story.den.clone(),
                    cycle: story.cycle_number,
                    karma: story.karma,
                    karma_cap: story.karma_cap,
                    player_room: sim.world().abstract_room(pos.room).map(|r| r.name.clone()),
                    player_tile: Some([pos.tile.0, pos.tile.1]),
                    food_in_stomach: sim.food_in_stomach(),
                }
            }),
            frame: *frame,
            process: m.current().map(|p| format!("{:?}", p.id())),
            upcoming: m.upcoming().map(|id| format!("{id:?}")),
            fade_to_black: m.fade_to_black(),
            fade_alpha: m.fade_sprite().map(|s| s.alpha),
            game_tick: frontend.game().map(|g| g.scene.game().ticks()),
            quit_requested: frontend.quit_requested(),
            slideshow: frontend.slideshow().map(|s| SlideShowState {
                current: s.current(),
                scene: s.current_scene().map(|id| format!("{id:?}")),
                time: s.time(),
                in_scene_time: s.in_scene_time(),
                stall: s.stall(),
                playback_speed_mult: s.playback_speed_mult(),
                cam_pos: s.scene_camera().map(|c| c.0.to_array()),
                focus: s.scene_camera().map(|c| c.1),
                alphas: s.illustration_alphas(),
            }),
            pause_menu: frontend
                .game()
                .and_then(|g| g.pause_menu.as_ref())
                .map(Self::pause_menu_state),
            karma_screen: frontend.sleep_screen().map(Self::karma_screen_state),
            options: Self::options_state(frontend),
            music: frontend.ctx.menu.music.as_ref().map(|m| {
                let records = saves
                    .current_save_state
                    .as_ref()
                    .map(|s| &s.death_persistent.songs_play_records[..]);
                MusicState::of(m, records)
            }),
            triggers: game.map_or_else(Vec::new, |g| TriggerState::all(g.sim())),
        })
    }

    /// Imports one of the game's save slots into the port's save directory; a loaded
    /// process's menus then use it (slot 0).
    ///
    /// # Errors
    /// On a bad slot, without save directories, or when the import fails.
    pub fn import_save(&mut self, params: &ImportSaveParams) -> Result<ImportReply, String> {
        let slot = params.slot.unwrap_or(0);
        let slot = SaveSlot::ALL
            .get(usize::from(slot))
            .copied()
            .ok_or_else(|| format!("slot {slot}: 0, 1 or 2"))?;
        let mut config = self.saves.clone();
        if let Some(dir) = &params.game_saves {
            config.game_saves = Some(PathBuf::from(dir));
        }
        let report = config.import(&self.assets, slot)?;
        if slot == SaveSlot::default()
            && let Some((frontend, _)) = &mut self.frontend
        {
            let saves = config.load(&self.assets, slot);
            frontend.ctx.menu.set_saves(saves);
        }
        Ok(ImportReply {
            source: report.source.display().to_string(),
            destination: report.destination.display().to_string(),
            backup: report.backup.map(|b| b.display().to_string()),
            save_states: report.save_states,
        })
    }

    fn options_state(frontend: &Frontend) -> OptionsState {
        let shared = &frontend.ctx.menu;
        let o = &shared.options;
        let menu = frontend.options_menu();
        OptionsState {
            save_slot: o.save_slot,
            resolution: o.resolution,
            windowed: o.windowed,
            music_volume: o.music_volume,
            sound_effects_volume: o.sound_effects_volume,
            arena_music_volume: o.arena_music_volume,
            vibration: o.vibration,
            validation: o.validation,
            language: o.language.value().to_owned(),
            screen: shared.screen.size,
            screen_full: shared.screen.full_screen,
            cursor_visible: shared.screen.cursor_visible,
            info: menu.map(|m| m.menu().info_text().to_owned()),
            dialog: menu.and_then(|m| m.dialog()).map(|d| d.text().to_owned()),
        }
    }

    fn pause_menu_state(p: &rw_hud::PauseMenu) -> PauseMenuState {
        let menu = p.menu();
        PauseMenuState {
            counter: p.counter,
            black_fade: p.black_fade,
            want_to_continue: p.want_to_continue(),
            shut_down: p.shut_down,
            buttons: p
                .buttons()
                .into_iter()
                .map(|b| Self::describe_button(menu, b))
                .collect(),
            selected: menu
                .selected
                .and_then(|s| menu.button(s))
                .map(|b| b.signal_text.clone()),
        }
    }

    fn karma_screen_state(p: &rw_hud::SleepAndDeathScreen) -> KarmaScreenState {
        let menu = p.menu();
        let l = p.ladder();
        KarmaScreenState {
            phase: l.phase.value().to_owned(),
            display_karma: [l.display_karma.0, l.display_karma.1],
            move_to_karma: l.move_to_karma,
            reinforced: l.reinforced,
            animation_done: l.all_animation_done(),
            force_watch_animation: p.force_watch_animation(),
            reveal_map: p.reveal_map(),
            info: menu.info_text().to_owned(),
            buttons: p
                .buttons()
                .into_iter()
                .map(|b| Self::describe_button(menu, b))
                .collect(),
            selected: menu
                .selected
                .and_then(|s| menu.button(s))
                .map(|b| b.signal_text.clone()),
            meters: l.meters.iter().map(Self::meter_state).collect(),
            tokens: p.tokens().map_or_else(Vec::new, |t| {
                t.tokens
                    .iter()
                    .map(|token| TokenState {
                        passage: l.meters[token.meter].tracker.id.value().to_owned(),
                        fade: token.fade,
                        pos: (t.pos + token.pos).to_array(),
                    })
                    .collect()
            }),
            kills: p.kills().map(|k| KillsState {
                trophies: k
                    .trophies
                    .iter()
                    .map(|t| {
                        (
                            t.symbol.kind.value().to_owned(),
                            (k.pos + t.pos).to_array(),
                            t.sprites.is_some(),
                        )
                    })
                    .collect(),
                counted_and_done: k.counted_and_done,
            }),
        }
    }

    fn meter_state(m: &rw_hud::EndgameMeter) -> MeterState {
        MeterState {
            passage: m.tracker.id.value().to_owned(),
            kind: match m.meter.kind {
                rw_hud::MeterKind::Float(_) => "float",
                rw_hud::MeterKind::Notch(_) => "notch",
            }
            .to_owned(),
            powered_on: m.powered_on,
            energy: m.energy,
            meter_animation: m.meter_animation,
            show_as_fulfilled: m.show_as_fulfilled,
            fulfilled_now: m.fulfilled_now,
        }
    }

    /// A button's signal, screen position and greying.
    fn describe_button(menu: &rw_menu::menu::Menu, b: rw_menu::menu::ObjectId) -> String {
        let Some(data) = menu.button(b) else {
            return String::new();
        };
        let pos = menu.screen_pos(b);
        let greyed = if data.behav.greyed_out { " greyed" } else { "" };
        format!(
            "{} at ({:.2}, {:.2}){greyed}",
            data.signal_text, pos.x, pos.y
        )
    }

    /// Text map of the tiles in a rectangle (default: the whole room).
    ///
    /// # Errors
    /// Without a room, or when the rectangle is empty or too large.
    pub fn tiles(&self, params: &TilesParams) -> Result<String, String> {
        let (room, player) = self.room_tiles(self.room_named(params.room.as_deref())?)?;
        let room = room.as_ref();
        let [x0, y0, x1, y1] = params
            .rect
            .unwrap_or([0, 0, room.width() - 1, room.height() - 1]);
        if x1 < x0 || y1 < y0 {
            return Err("rect is [x0, y0, x1, y1] with x0 <= x1 and y0 <= y1".to_owned());
        }
        let count = (i64::from(x1 - x0) + 1) * (i64::from(y1 - y0) + 1);
        if count > MAX_TILES {
            return Err(format!("{count} tiles requested; at most {MAX_TILES}"));
        }
        Ok(TileDump {
            room,
            min: (x0, y0),
            max: (x1, y1),
            player,
        }
        .to_string())
    }
}

/// Menu key names of `step_process`.
struct KeyParse;

impl KeyParse {
    fn key(name: &str) -> Result<KeyboardKey, String> {
        Ok(match name {
            "UpArrow" => KeyboardKey::UpArrow,
            "DownArrow" => KeyboardKey::DownArrow,
            "LeftArrow" => KeyboardKey::LeftArrow,
            "RightArrow" => KeyboardKey::RightArrow,
            "Return" => KeyboardKey::Return,
            "Space" => KeyboardKey::Space,
            "Z" => KeyboardKey::Z,
            "X" => KeyboardKey::X,
            "C" => KeyboardKey::C,
            "S" => KeyboardKey::S,
            "Escape" => KeyboardKey::Escape,
            "LeftShift" => KeyboardKey::LeftShift,
            _ => return Err(format!("unknown key {name:?}")),
        })
    }
}

impl InputParam {
    fn to_input(self) -> Result<Input, String> {
        if !(-1..=1).contains(&self.x) || !(-1..=1).contains(&self.y) {
            return Err(format!("input x and y must be -1, 0 or 1 (got {self:?})"));
        }
        Ok(Input::new(self.x, self.y, self.jump)
            .with_map(self.map)
            .with_hands(self.pickup, self.throw))
    }
}

impl Scene {
    /// Where a cropped screenshot centres: the head, the shortcut tile being travelled,
    /// or the room's middle.
    fn focus(&self) -> [f32; 2] {
        let game = self.game();
        match (game.body(), game.vessel_tile()) {
            (Some(body), _) => body.chunks[0].pos.to_array(),
            (None, Some((x, y))) => rw_assets::tile_centre(x, y),
            (None, None) => {
                let room = game.room();
                [room.pixel_width() / 2.0, room.pixel_height() / 2.0]
            }
        }
    }
}

impl PlayerState {
    pub(super) fn of(player: &rw_sim::Player, sim: &rw_sim::RainWorldGame) -> Self {
        let input = player.input_history()[0];
        let danger = player.danger_grasp();
        Self {
            chunks: player
                .chunks()
                .iter()
                .map(|c| {
                    let contact = c.contact();
                    ChunkState {
                        pos: c.pos.to_array(),
                        vel: c.vel.to_array(),
                        tile: c.pos.tile().into(),
                        contact: [contact.x, contact.y],
                        on_slope: c.on_slope(),
                        loudness: c.loudness,
                    }
                })
                .collect(),
            body_mode: format!("{:?}", player.body_mode()),
            animation: player.animation().value().to_owned(),
            flip_direction: player.flip_direction(),
            standing: player.is_standing(),
            stunned: player.is_stunned(),
            stun_ticks: player.stun_ticks(),
            can_jump: player.can_jump(),
            jump_chunk_counter: player.jump_chunk_counter(),
            dead: player.is_dead(),
            in_shortcut: player.in_shortcut(),
            vessel_tile: player.shortcut_vessel_tile().map(Into::into),
            carried_by: player.carried_by().map(|c| {
                let id = sim.world().creature(c).id;
                [id.spawner, id.number]
            }),
            impacts: player
                .body
                .impacts()
                .iter()
                .map(|i| ImpactState {
                    chunk: i.chunk,
                    direction: [i.direction.x, i.direction.y],
                    speed: i.speed,
                    first_contact: i.first_contact,
                })
                .collect(),
            input: InputParam {
                x: input.x,
                y: input.y,
                jump: input.jump,
                map: input.map,
                pickup: input.pickup,
                throw: input.throw,
            },
            controller: player.controller().map(|c| format!("{c:?}")),
            food: sim.food_in_stomach(),
            quarter_food: sim.quarter_food_points(),
            max_food: sim.max_food(),
            malnourished: player.malnourished(),
            aerobic_level: player.aerobic_level(),
            exhausted: player.exhausted(),
            grasps: sim
                .player_grasps()
                .map(|g| g.map(|e| HeldInfo::of(sim.world(), e))),
            grasp_chunks: [0, 1].map(|i| {
                sim.world()
                    .creature(sim.player_creature())
                    .grasps
                    .get(i)
                    .copied()
                    .flatten()
                    .map(|g| g.chunk_grabbed)
            }),
            stomach: player.object_in_stomach().map(|o| match o {
                StomachObject::Object(id) => HeldInfo::of(sim.world(), Entity::Object(*id)),
                StomachObject::Saved(s) => HeldInfo {
                    kind: s.clone(),
                    id: [0, 0],
                },
            }),
            swallow_counter: player.grab_state().swallow_and_regurgitate_counter,
            released_slide_loops: player.released_slide_loop_count(),
            pick_up_candidate: player
                .grab_state()
                .pick_up_candidate
                .map(|e| HeldInfo::of(sim.world(), e)),
            eat_counter: player.grab_state().eat_counter,
            eat_meat: player.grab_state().eat_meat,
            priv_sneak: player.priv_sneak(),
            sneak: player.sneak(
                sim.world()
                    .realized(sim.player_pos().room)
                    .and_then(|r| r.water.as_ref()),
            ),
            sleep: SleepState {
                sleep_counter: player.sleep_counter(),
                sleep_curl_up: player.sleep_curl_up(),
                sleep_when_still: player.sleep_when_still(),
                touched_no_input_counter: player.touched_no_input_counter(),
                force_sleep_counter: player.force_sleep_counter(),
                ready_for_win: player.ready_for_win(),
                still_in_start_shelter: player.still_in_start_shelter(),
            },
            held_by: sim
                .player_grabbed_by()
                .iter()
                .map(|g| GraspInfo::of(sim.world(), g))
                .collect(),
            danger_grasp: danger.grasp.map(|g| GraspInfo::of(sim.world(), &g)),
            danger_grasp_time: danger.time,
            danger_grasp_throw_button: danger.last_throw_button,
            danger_grasp_pickup_button: danger.pickup_button,
            can_struggle: danger.grasp.is_some()
                && danger.time < 30
                && !player.is_dead()
                && player.stun_ticks() < 30,
            cant_be_grabbed_counter: danger.cant_be_grabbed_counter,
            vine_pos: player.vine_pos().map(super::reply::VinePosState::of),
            vine_grab_delay: player.vine_grab_delay(),
            vine_climb_cursor: player.vine_climb_cursor().to_array(),
            want_to_grab: player.want_to_grab(),
            spear_on_back: player.spear_on_back().map(|s| SpearOnBackInfo {
                spear: s
                    .spear
                    .map(|id| HeldInfo::of(sim.world(), Entity::Object(id))),
                counter: s.counter,
                increment: s.increment,
                flip: s.flip,
                interaction_locked: s.interaction_locked,
                stick: s.stick.is_some(),
            }),
            circuit_swim_resistance: player.circuit_swim_resistance(),
            circuit_jump_meter: player.circuit_jump_meter(),
            swim_bits: player
                .swim_bits()
                .into_iter()
                .flatten()
                .map(super::reply::SwimBitState::of)
                .collect(),
        }
    }
}

impl GraspInfo {
    fn of(world: &rw_sim::world::World, g: &rw_sim::creature::Grasp) -> Self {
        Self {
            grabber: HeldInfo::of(world, Entity::Creature(g.grabber)),
            grasp_used: g.grasp_used,
            chunk: g.chunk_grabbed,
            pacifying: g.pacifying,
            dominance: g.dominance,
            serial: g.serial,
        }
    }
}

impl StepTrace {
    fn of(scene: &Scene) -> Self {
        let game = scene.game();
        let player = game.live_player();
        Self {
            tick: game.ticks(),
            head: player.map(|p| p.head().pos.to_array()),
            body_mode: player.map(|p| format!("{:?}", p.body_mode())),
            animation: player.map(|p| p.animation().value().to_owned()),
        }
    }
}

impl ExitList {
    pub(super) fn of(scene: &Scene) -> Result<Self, String> {
        let game = scene.game();
        let world = game.sim().world();
        let here = game.sim().camera_room();
        let abstract_room = world.abstract_room(here).ok_or("no camera room")?;
        let room = game.room();
        let exits = abstract_room
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let entrance = room.shortcut_map().leading_to_node(i).map(|d| d.start);
                let dest = abstract_room
                    .connections
                    .get(i)
                    .copied()
                    .filter(|&c| c > -1);
                let dest_room = dest.and_then(|d| world.abstract_room(d));
                let dest_node = dest_room.map(|d| d.exit_index(here));
                let dest_camera = dest_room
                    .zip(dest_node)
                    .and_then(|(d, n)| d.nodes.get(usize::try_from(n).ok()?))
                    .map(|n| n.viewed_by_camera);
                ExitInfo {
                    node: i,
                    kind: format!("{:?}", node.kind),
                    entrance_tile: entrance.map(Into::into),
                    enter_input: entrance.map(|(x, y)| {
                        let (hx, hy) = room.shortcut_hole_direction(x, y);
                        [-hx, -hy]
                    }),
                    destination: dest_room.map(|d| d.name.clone()),
                    destination_node: dest_node,
                    destination_camera: dest_camera,
                    viewed_by_camera: node.viewed_by_camera,
                }
            })
            .collect();
        Ok(Self {
            room: room.name().to_owned(),
            exits,
        })
    }
}

impl MapState {
    fn of(map: &rw_hud::Map) -> Self {
        Self {
            region: map.data.region.clone(),
            visible: map.visible,
            pan: map.pan_pos.to_array(),
            layer: map.layer,
            reveal_pending: map.reveal_pixels.len(),
            player_marker: map.player_marker().map(|m| m.pos.to_array()),
            markers: map
                .markers
                .iter()
                .map(|m| MapMarkerState {
                    kind: match m.kind {
                        rw_hud::MarkerKind::Gate { .. } => "gate",
                        rw_hud::MarkerKind::Shelter => "shelter",
                        rw_hud::MarkerKind::Death { .. } => "death",
                        rw_hud::MarkerKind::Flower => "flower",
                    }
                    .to_owned(),
                    room: m.room,
                    fade: m.fade,
                    items: m
                        .shelter
                        .iter()
                        .flat_map(|s| &s.items)
                        .map(|i| ShelterIconState {
                            sprite: i.symbol.sprite_name.clone(),
                            pos: i.pos.to_array(),
                            status: i.data.status,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl HudState {
    pub(super) fn of(hud: &rw_hud::Hud) -> Self {
        let food = &hud.food_meter;
        let karma = &hud.karma_meter;
        Self {
            food_meter: FoodMeterState {
                pos: food.pos.to_array(),
                fade: food.fade,
                visible_counter: food.visible_counter,
                show_count: food.show_count,
                quarter_pips: food
                    .quarter_pip_shower
                    .as_ref()
                    .map_or(0, |q| q.display_quarter_food),
                max_food: food.max_food,
                survival_limit: food.survival_limit,
                down_in_corner: food.down_in_corner,
                pip_x: food.circles.iter().map(|c| c.slow_x_add).collect(),
            },
            karma_meter: KarmaMeterState {
                pos: karma.pos.to_array(),
                fade: karma.fade,
                rad: karma.rad,
                force_visible_counter: karma.force_visible_counter,
                symbol: karma.symbol.clone(),
                reinforced: karma.show_as_reinforced,
            },
            rain_meter: RainMeterState {
                fade: hud.rain_meter.fade,
                circles: hud.rain_meter.circles.len(),
                remain_visible_counter: hud.rain_meter.remain_visible_counter,
            },
            map_fade: hud.map.fade,
            map: MapState::of(&hud.map),
            show_karma_food_rain: hud.show_karma_food_rain,
            text_prompt: TextPromptState {
                show: hud.text_prompt.show,
                showing: format!("{:?}", hud.text_prompt.currently_showing),
                text: hud.prompt_text().to_owned(),
                queued: hud.text_prompt.messages.len(),
                game_over: hud.text_prompt.game_over_mode,
                dependent_on_grasp: hud.text_prompt.dependent_on_grasp,
            },
            controls_page: hud.controls_page.as_ref().map(|p| ControlsPageState {
                counter: p.counter,
                black_fade: p.black_fade,
                want_to_continue: p.want_to_continue,
                labels: hud.controls_page_texts().unwrap_or_default(),
            }),
        }
    }
}

impl ConnectionInfo {
    fn of(c: rw_sim::world::MovementConnection) -> Self {
        Self {
            kind: c.kind.value().to_owned(),
            start: [c.start.tile.0, c.start.tile.1],
            dest: [c.dest.tile.0, c.dest.tile.1],
            distance: c.distance,
        }
    }
}

impl HeldInfo {
    /// A noise's source: a creature or item, or a coral stem's segment.
    fn of_noise(world: &rw_sim::world::World, source: rw_sim::NoiseSource) -> Self {
        match source {
            rw_sim::NoiseSource::Entity(e) => Self::of(world, e),
            rw_sim::NoiseSource::Segment(s) => {
                let id = world
                    .realized(s.room)
                    .and_then(|r| r.coral.as_ref())
                    .and_then(|c| c.stems.get(s.stem as usize))
                    .and_then(|stem| stem.segments.get(s.segment as usize))
                    .map(|g| g.id);
                Self {
                    kind: "StemSegment".to_owned(),
                    id: id.map_or([0, 0], |id| [id.spawner, id.number]),
                }
            }
        }
    }

    /// What a hand holds: its kind and EntityID.
    fn of(world: &rw_sim::world::World, e: Entity) -> Self {
        let (kind, id) = match e {
            Entity::Creature(id) => {
                let c = world.creature(id);
                (c.kind().value().to_owned(), c.id)
            }
            Entity::Object(id) => {
                let o = world.object(id);
                (o.kind.value().to_owned(), o.id)
            }
        };
        Self {
            kind,
            id: [id.spawner, id.number],
        }
    }
}

impl PreparerState {
    fn of(p: &rw_sim::world::RoomPreparer) -> Self {
        Self {
            status: p.status().value(),
            calls: p.calls(),
            done: p.done(),
        }
    }
}
