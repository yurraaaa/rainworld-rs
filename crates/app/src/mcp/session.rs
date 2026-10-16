//! The state the MCP tools drive: one loaded game (a world, seen from the camera's
//! room), stepped only on request, and a headless renderer for screenshots. Every tool is a method returning a typed reply or
//! an error message.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rw_assets::SlugcatName;
use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicPlayer, MusicProcess, SongBank, TimedSources};
use rw_menu::{KeyboardKey, MainLoopProcess, MenuShared, ProcessId, StoryGameInitCondition};
use rw_random::UnityRandom;
use rw_render::{RoomLayers, WgpuRenderer};
use rw_save::SaveSlot;
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::creature::{AbstractCreature, CreatureId};
use rw_sim::item::{Entity, ObjectId, SpearStickKind, Weapon};
use rw_sim::{Input, StomachObject, Vec2, WorldCoordinate};

use super::params::{
    HudMessageParams, ImportSaveParams, InputParam, ListRoomsParams, LoadProcessParams,
    LoadRoomParams, NewGameParams, ProcessParam, RespawnParams, ScreenshotParams, SetPlayerParams,
    StepParams, StepProcessParams, StepSequenceParams, TilesParams, ViewParam,
};
use super::reply::{
    AbstractAiInfo, ChunkState, ControlsPageState, CreatureInfo, ExitInfo, ExitList, FlyInfo,
    FoodMeterState, HudState, ImpactState, ImportReply, KarmaMeterState, KarmaScreenState,
    KillsState, LobbyState, MapMarkerState, MapState, MeterState, NodeRef, ObjectInfo,
    OptionsState, OverseerInfo, PauseMenuState, PlayerState, ProcessState, RainMeterState,
    RainState, RealizedState, RoomInfo, RoomList, SaveInfo, SaveStateInfo, Screenshot, SessionInfo,
    ShelterDoorState, SleepState, SlideShowState, StartUpState, State, StepTrace, StoryInfo,
    TextPromptState, TokenState, WeaponInfo,
};
use super::reply::{
    FliesWorldInfo, HeldInfo, QuantifiedInfo, RegionStateInfo, RelationshipInfo, RoomDens,
};
use super::reply::{MusicState, TriggerState};
use super::tiles::TileDump;
use crate::frontend::{AppContext, Frontend, FrontendRenderer, GameSetup};
use crate::game::{Game, GameStart};
use crate::saves::SaveConfig;
use crate::scene::{Framing, Scene, SceneRenderer, SceneView, VisualsSource};
use crate::sprites::SpriteAssets;

/// Most ticks one `step` or `step_sequence` call may run (10 minutes of game time).
const MAX_TICKS: u64 = 40 * 60 * 10;
/// Largest screenshot side in pixels.
const MAX_IMAGE_SIDE: f32 = 4096.0;
/// Most tiles one `get_tiles` call returns.
const MAX_TILES: i64 = 40_000;
/// Seconds per `step_process` frame.
const FRAME_SECONDS: f32 = 1.0 / 40.0;

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
        let music = TimedSources::new(SongBank::open(&assets).ok());
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
        let world = self.assets.file("world");
        let mut dirs: Vec<(String, PathBuf)> = std::fs::read_dir(&world)
            .map_err(|e| format!("cannot read {}: {e}", world.display()))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let region = name.strip_suffix("-rooms")?.to_owned();
                Some((region, entry.path()))
            })
            .collect();
        dirs.push(("gates".to_owned(), self.assets.file("world/gates")));
        dirs.push(("arena".to_owned(), self.assets.file("levels")));
        let region = params.region.as_deref().map(str::to_lowercase);
        let prefix = params.prefix.as_deref().unwrap_or("").to_lowercase();
        let mut rooms: Vec<String> = dirs
            .iter()
            .filter(|(r, _)| region.as_ref().is_none_or(|want| want == r))
            .filter_map(|(_, dir)| std::fs::read_dir(dir).ok())
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let file = entry.file_name().into_string().ok()?;
                let name = file.strip_suffix(".txt")?;
                let image = entry.path().with_file_name(format!("{name}_1.png"));
                (name.starts_with(&prefix) && image.is_file()).then(|| name.to_owned())
            })
            .collect();
        rooms.sort();
        rooms.dedup();
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
        // would: made once with the default options, its `UpdateMusicContext` and the
        // new game's `NewCycleEvent`.
        if let Some(shared) = &mut self.hud_shared {
            if shared.music.is_none() {
                shared.music = MusicPlayer::new(shared.music_settings, None);
            }
            if let Some(music) = &mut shared.music {
                music.update_music_context(Some(MusicProcess::StoryGame));
                music.new_cycle_event();
            }
        }
        let rng = UnityRandom::new(seed as i32);
        let game = Game::start(&self.assets, &start, rng, Arc::clone(&self.sound_table))?;
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
        Ok(())
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
                if !self.scene_or_err()?.game_mut().respawn() {
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
        Ok(State {
            tick: game.ticks(),
            world: world.name().to_owned(),
            room: game.room().name().to_owned(),
            camera: scene.visuals().map(|_| scene.camera().current()),
            player_room: Self::room_name(scene, pos.room),
            player_node: pos.node,
            lobby: sim.lobby().map(|l| LobbyState {
                room: Self::room_name(scene, l.room),
                entrance_node: l.entrance_node,
                updates: l.updates,
            }),
            realized_rooms: world
                .active_rooms()
                .into_iter()
                .filter_map(|i| {
                    let r = world.realized(i)?;
                    Some(RealizedState {
                        room: Self::room_name(scene, i),
                        wait_to_enter: r.wait_to_enter(),
                        ready_for_player: r.ready_for_player(),
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
            rain: RainState {
                timer: world.rain_cycle.timer,
                cycle_length: world.rain_cycle.cycle_length,
            },
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
            dens: Self::dens(scene),
            quantified: Self::quantified(scene),
            flies_world: world.flies_world_ai().map(|f| FliesWorldInfo {
                to_respawn: f.flies_to_respawn,
                to_spawn: f.flies_to_spawn.clone(),
                all_initial_spawned: f.all_initial_flies_spawned,
            }),
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
            guide_likes_player: sim.guide_state().likes_player,
            guide_increase_like_on_save: sim.guide_state().increase_like_on_save,
            influences: deaths
                .influences
                .iter()
                .map(|i| (i.community.value().to_owned(), i.influence))
                .collect(),
        }
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
                    .or_else(|| c.overseer_body().map(|b| b.stun)),
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
                }),
                fly: Self::fly_info(scene, c),
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
            return self.process_screenshot(params, level, scale);
        }
        let scene = self.scene()?;
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
        scene.draw(&mut target.scene, 1.0, view);
        // The HUD is in screen pixels: drawn over the game's own camera window only.
        if let (Some(shared), true) = (&mut self.hud_shared, level && framing == Framing::Auto) {
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
        let scene = self.scene()?;
        let room = scene.game().room();
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
        let player = scene
            .game()
            .live_player()
            .filter(|p| !p.in_shortcut())
            .map(|p| p.chunks().iter().map(|c| c.pos.tile()).collect())
            .unwrap_or_default();
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
                    }
                })
                .collect(),
            body_mode: format!("{:?}", player.body_mode()),
            animation: player.animation().value().to_owned(),
            flip_direction: player.flip_direction(),
            standing: player.is_standing(),
            stunned: player.is_stunned(),
            stun_ticks: player.stun_ticks(),
            dead: player.is_dead(),
            in_shortcut: player.in_shortcut(),
            vessel_tile: player.shortcut_vessel_tile().map(Into::into),
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
            stomach: player.object_in_stomach().map(|o| match o {
                StomachObject::Object(id) => HeldInfo::of(sim.world(), Entity::Object(*id)),
                StomachObject::Saved(s) => HeldInfo {
                    kind: s.clone(),
                    id: [0, 0],
                },
            }),
            swallow_counter: player.grab_state().swallow_and_regurgitate_counter,
            pick_up_candidate: player
                .grab_state()
                .pick_up_candidate
                .map(|e| HeldInfo::of(sim.world(), e)),
            eat_counter: player.grab_state().eat_counter,
            sleep: SleepState {
                sleep_counter: player.sleep_counter(),
                sleep_curl_up: player.sleep_curl_up(),
                sleep_when_still: player.sleep_when_still(),
                touched_no_input_counter: player.touched_no_input_counter(),
                force_sleep_counter: player.force_sleep_counter(),
                ready_for_win: player.ready_for_win(),
                still_in_start_shelter: player.still_in_start_shelter(),
            },
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

impl HeldInfo {
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
