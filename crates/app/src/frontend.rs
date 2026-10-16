//! The app's processes under `rw_menu`'s [`ProcessManager`]: the game,
//! the reduced `Initialization` and the menu screens, drawn the same way in the window
//! and in MCP screenshots (each process's frame, then the switch fade over it). The
//! music player and the menu microphone run as side processes after them.

use std::sync::Arc;

use rw_assets::ActiveMods;
use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicClock, MusicOutput, SongBank, SourcePlayback, TimedSources, TrackId};
use rw_audio::{Audio, ListenerInput, MenuPlayback, MenuSoundStart};
use rw_hud::{PauseGame, PauseMenu, PauseRequest, SleepAndDeathScreen, SlugcatSelectMenu};
use rw_menu::StoryGameInitCondition;
use rw_menu::menu::{Menu, MenuCx};
use rw_menu::{
    Alignment, AtlasLoadError, Container, Font, FontTest, Initialization, IntroRoll, KeyboardKey,
    Label, MainLoopProcess, MainMenu, MenuContext, MenuShared, MenuSound, MenuSoundEvent, Node,
    NodeTransform, OptionsMenu, ProcessFactory, ProcessId, ProcessManager, ProcessRequests,
    SlideShow, SlideShowId, TextureSync, TimeStacker,
};
use rw_random::UnityRandom;
use rw_render::{
    Camera, Color, MapGlobals, MenuSceneGlobals, Renderer, Resolution, StageBatch, StageGlobals,
};
use rw_save::WinOutcome;
use rw_save::{CampaignStart, PlayerProgression, SaveSlot};
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::world::RegionState;
use rw_sim::{SessionEnd, SoundId, Vec2};

use crate::game::{Game, GameStart};
use crate::hibernation::HibernatingGame;
use crate::input::{Action, InputState, Script};
use crate::saves::{SaveConfig, SaveStateStart};
use crate::scene::{Scene, SceneRenderer, SceneView, VisualsSource};

/// What the game process is built from.
pub struct GameSetup {
    /// The start when the menus chose no campaign (the game started directly).
    pub start: GameStart,
    pub seed: u64,
    pub sound_table: Arc<SoundTable>,
    pub saves: SaveConfig,
}

/// The context every process updates in.
pub struct AppContext {
    pub menu: MenuShared,
    pub resources: Arc<UnityResources>,
    pub resolution: Resolution,
    pub game: GameSetup,
    /// The gameplay keys.
    pub input: InputState,
    /// `--script`: replaces the gameplay keys until it runs out.
    pub script: Option<Script>,
    pub audio: Option<Audio>,
    /// What the menu microphone plays.
    pub menu_audio: MenuPlayback,
    /// The music player's sources.
    pub music_sink: MusicSink,
    /// The music player's own `myTimeStacker`.
    music_stacker: TimeStacker,
    /// The process `Initialization` starts.
    pub start: ProcessId,
    /// `rainWorld.saveBackedUp`: `Win` backs the save up once per run.
    pub save_backed_up: bool,
    /// The region state the last cycle's world kept in memory, for the next cycle.
    pub carried_region_state: Option<RegionState>,
    /// The slot's progression while a dev start plays on a detached copy (D16).
    stashed_saves: Option<PlayerProgression>,
}

/// Where the music player's songs play: through the audio output, or silently in
/// ticks (no audio: `--mute`, headless and MCP runs), which keeps runs deterministic.
pub enum MusicSink {
    Timed(Box<TimedSources>),
    Output(MusicOutput),
}

/// The game process: the [`Scene`] and the pause menu over it.
pub struct GameProcess {
    pub scene: Scene,
    /// `FadeInTime` and `InitialBlackSeconds`, from the menu setup it was made with.
    fade_in_time: f32,
    initial_black_seconds: f32,
    /// `pauseMenu`: while there is one the game is paused (`GamePaused`).
    pub pause_menu: Option<PauseMenu>,
    /// `lastPauseButton`, written only on unpaused ticks.
    last_pause_button: bool,
}

/// A process the port does not have yet: its name on black; the pause button (Esc)
/// returns to the main menu (docs/DIVERGENCES.md D13).
pub struct Unported {
    id: ProcessId,
    stage: Container,
    last_pause_button: bool,
}

/// The dream's stand-in (`DreamScreen` is not ported, D13): black for a second after
/// the fade, then the next cycle, as the sleep screen's Continue starts it
/// (`startGameCondition = Load`).
pub struct SleepStandIn {
    id: ProcessId,
    stage: Container,
    frames: u32,
}

impl SleepStandIn {
    const FRAMES: u32 = 40;

    fn update(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        ctx.menu.input.take_tick_input();
        self.frames += 1;
        if self.frames == Self::FRAMES {
            ctx.menu.start_game_condition = Some(StoryGameInitCondition::Load);
            requests.switch_to(ProcessId::Game);
        }
    }
}

/// The processes the app runs.
pub enum AppProcess {
    Initialization(Initialization),
    IntroRoll(Box<IntroRoll>),
    SlideShow(Box<SlideShow>),
    Game(Box<GameProcess>),
    FontTest(FontTest),
    MainMenu(Box<MainMenu>),
    OptionsMenu(Box<OptionsMenu>),
    SlugcatSelect(Box<SlugcatSelectMenu>),
    SleepStandIn(SleepStandIn),
    /// The sleep, death and starve screens.
    SleepAndDeath(Box<SleepAndDeathScreen>),
    Unported(Unported),
}

/// The process manager and its context.
pub struct Frontend {
    pub manager: ProcessManager<AppProcess>,
    pub ctx: AppContext,
}

/// A renderer and what of a [`Frontend`] it already holds.
pub struct FrontendRenderer<R> {
    pub scene: SceneRenderer<R>,
    textures: TextureSync,
}

impl AppContext {
    /// Loads the fonts and strings the menus and the fade's label use; the shared
    /// random stream starts from `game.seed`.
    ///
    /// # Errors
    /// When the font atlas or a font config cannot be read.
    pub fn new(
        assets: StreamingAssets,
        resources: Arc<UnityResources>,
        resolution: Resolution,
        game: GameSetup,
        start: ProcessId,
    ) -> Result<Self, AtlasLoadError> {
        let mut menu = MenuShared::load(assets, &resources, resolution.size_f32())?;
        menu.options = game.saves.load_options();
        let saves = game.saves.load(&menu.assets, Self::slot_of(&menu.options));
        menu.set_saves(saves);
        menu.random = UnityRandom::new(game.seed as i32);
        menu.set_sound_table(Arc::clone(&game.sound_table));
        let bank = SongBank::open(&menu.assets)
            .inspect_err(|e| log::warn!("no music bank ({e}); songs are missing"))
            .ok();
        Ok(Self {
            menu,
            resources,
            resolution,
            game,
            input: InputState::default(),
            script: None,
            audio: None,
            menu_audio: MenuPlayback::default(),
            music_sink: MusicSink::Timed(Box::new(TimedSources::new(bank))),
            music_stacker: TimeStacker::default(),
            start,
            save_backed_up: false,
            carried_region_state: None,
            stashed_saves: None,
        })
    }

    /// `options.saveSlot` as a slot (Expedition's negative slots are not ported).
    fn slot_of(options: &rw_save::Options) -> SaveSlot {
        SaveSlot(u8::try_from(options.save_slot.clamp(0, 2)).unwrap_or(0))
    }

    /// `Options.OnLoadFinished` for the window: the options' screen size and mode,
    /// after `--resolution` and `--windowed` (which set the options for this run).
    pub fn apply_screen_options(&mut self, resolution: Option<Resolution>, windowed: bool) {
        if let Some(i) = resolution.and_then(|r| Resolution::CHOICES.iter().position(|&c| c == r)) {
            self.menu.options.resolution = i as i32;
        }
        if windowed {
            self.menu.options.windowed = true;
        }
        self.menu.apply_screen_options();
        self.sync_resolution();
    }

    /// The game's frame size follows Futile's screen size.
    pub fn sync_resolution(&mut self) {
        let [w, h] = self.menu.screen_size;
        self.resolution = Resolution::new(w as u32, h as u32);
    }

    /// The options' volumes, every frame (`options.soundEffectsVolume` and
    /// `musicVolume` are read live in the game).
    fn apply_volumes(&mut self) {
        if let Some(audio) = &mut self.audio {
            audio.set_sound_effects_volume(self.menu.options.sound_effects_volume);
        }
        self.menu.apply_music_volume();
    }

    /// A Rewired keyboard key goes down or up: menu input, and the player's when the
    /// gameplay map binds it.
    pub fn set_key(&mut self, key: KeyboardKey, pressed: bool) {
        self.menu.input.set_key(key, pressed);
        if let Some(action) = Action::from_rewired(key) {
            self.input.set(action, pressed);
        }
    }

    /// Plays sound and music on `audio`.
    pub fn set_audio(&mut self, audio: Audio) {
        self.audio = Some(audio);
        self.music_sink = MusicSink::Output(MusicOutput::new(&self.menu.assets));
    }

    /// The music player's frame (`MusicPlayer.RawUpdate`): its ticks on its own stacker.
    /// In a game its threat tracker reads the game and it draws from the game's random
    /// stream (which the game holds while it runs).
    fn update_music(&mut self, dt: f32, mut current: Option<&mut AppProcess>) {
        let Some(player) = &mut self.menu.music else {
            return;
        };
        for _ in 0..self.music_stacker.advance(dt, 40) {
            let game = match current.as_deref_mut() {
                Some(AppProcess::Game(g)) => Some(&mut g.scene),
                _ => None,
            };
            match &mut self.music_sink {
                MusicSink::Timed(sources) => {
                    match game {
                        Some(scene) => scene.update_music(player, sources.as_mut()),
                        None => {
                            player.set_game_view(None);
                            player.update(sources.as_mut(), &mut self.menu.random);
                        }
                    }
                    sources.advance();
                }
                MusicSink::Output(output) => {
                    if let Some(audio) = &mut self.audio {
                        let mut sources = output.sources(audio);
                        match game {
                            Some(scene) => scene.update_music(player, &mut sources),
                            None => {
                                player.set_game_view(None);
                                player.update(&mut sources, &mut self.menu.random);
                            }
                        }
                    }
                }
            }
        }
    }

    /// A fresh game drawing from the shared random stream: a story as the slugcat the
    /// menus chose (continued from its save on `Load`), or the configured start when
    /// started directly.
    fn new_game(&mut self) -> GameProcess {
        let condition = self.menu.game_start_condition();
        let story = self.menu.in_game_slugcat.and_then(|slugcat| {
            let campaign = SlugcatCampaign::of(slugcat, ActiveMods::NONE)?;
            Some((slugcat, campaign))
        });
        let start = match story {
            Some((slugcat, campaign)) if condition == StoryGameInitCondition::Load => {
                let in_memory = self.menu.saves.state_in_memory(slugcat).is_some();
                let carried = self.carried_region_state.take().filter(|_| in_memory);
                match self
                    .menu
                    .saves
                    .peek_save_state(Self::campaign_start(campaign))
                {
                    Ok(Some(state)) => {
                        let assets = &self.menu.assets;
                        let den = state.den_to_use(|room| {
                            assets
                                .find_room_file(&room.to_lowercase(), ".txt")
                                .is_some()
                        });
                        let mut story = state.story_start(den, in_memory);
                        story.live_region_state =
                            carried.filter(|r| story.den.split('_').next() == Some(&r.region_name));
                        GameStart::SavedStory {
                            slugcat,
                            story: Box::new(story),
                        }
                    }
                    other => {
                        log::error!("no {slugcat} save to continue ({other:?}); a new game");
                        GameStart::NewStory(slugcat)
                    }
                }
            }
            Some((slugcat, _)) => GameStart::NewStory(slugcat),
            None => self.game.start.clone(),
        };
        // The `RainWorldGame` constructor's first act.
        if let Some(music) = &mut self.menu.music {
            music.new_cycle_event();
        }
        let rng = std::mem::replace(&mut self.menu.random, UnityRandom::new(0));
        let table = Arc::clone(&self.game.sound_table);
        let game = Game::start(&self.menu.assets, &start, rng, table)
            .unwrap_or_else(|e| panic!("starting the game ({start:?}): {e}"));
        let source = VisualsSource {
            assets: self.menu.assets.clone(),
            resources: Arc::clone(&self.resources),
            screen: self.resolution.size_f32(),
        };
        // `StoryGameSession`: the save state, saved at once as a death and a quit.
        let mut fade_in_time = condition.game_fade_in_time();
        if story.is_none() {
            self.dev_save_state(&game);
        }
        if let Some((_, campaign)) = story {
            let load = condition == StoryGameInitCondition::Load;
            let seed = game.sim().save_state_seed();
            let world_version = self.world_version();
            match self.menu.saves.get_or_initiate_save_state(
                Self::campaign_start(campaign),
                seed,
                load,
                true,
            ) {
                Ok(state) if load => fade_in_time = state.slow_fade_in(),
                // `new SaveState`: `RainWorld.worldVersion` (`World/worldVersion.txt`).
                Ok(state) => state.world_version = world_version,
                Err(e) => log::error!("the {} save state: {e}", campaign.name),
            }
        }
        let scene = Scene::new(game, Some(source));
        if let Some(audio) = &mut self.audio {
            audio.set_ambients(&scene.game().settings().ambient_sounds);
        }
        GameProcess {
            scene,
            fade_in_time,
            initial_black_seconds: condition
                .game_initial_black_seconds(self.menu.progression.selected_slugcat),
            pause_menu: None,
            last_pause_button: false,
        }
    }

    /// A dev start's save state: a new game's at the start room, in memory only. It
    /// lives in a detached copy of the slot's progression (made afresh for each dev
    /// game), so `Win`, starving and the screens after them have it while nothing
    /// reaches the disk (D16).
    fn dev_save_state(&mut self, game: &Game) {
        let real = match self.stashed_saves.take() {
            Some(real) => real,
            None => std::mem::replace(&mut self.menu.saves, PlayerProgression::in_memory(None)),
        };
        self.menu.saves = real.detached();
        self.stashed_saves = Some(real);
        let sim = game.sim();
        let den = sim
            .world()
            .abstract_room(sim.player_pos().room)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        let start = CampaignStart {
            slugcat: sim.campaign().name,
            den: &den,
            vanilla: true,
        };
        let seed = sim.save_state_seed();
        let world_version = self.world_version();
        match self
            .menu
            .saves
            .get_or_initiate_save_state(start, seed, false, false)
        {
            Ok(state) => state.world_version = world_version,
            Err(e) => log::error!("the dev start's save state: {e}"),
        }
    }

    /// The slot's progression back after a dev start, before any process but the game
    /// and its screens.
    fn end_dev_saves(&mut self, next: ProcessId) {
        let game_or_screen = matches!(
            next,
            ProcessId::Game
                | ProcessId::SleepScreen
                | ProcessId::GhostScreen
                | ProcessId::DeathScreen
                | ProcessId::KarmaToMaxScreen
                | ProcessId::Dream
                | ProcessId::StarveScreen
        );
        if game_or_screen {
            return;
        }
        if let Some(real) = self.stashed_saves.take() {
            log::info!("dev start over: the slot's progression again");
            self.carried_region_state = None;
            self.menu.set_saves(real);
        }
    }

    /// `RainWorld.worldVersion` (`World/worldVersion.txt`).
    fn world_version(&self) -> i32 {
        std::fs::read_to_string(self.menu.assets.resolve_file_path("World/worldVersion.txt"))
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    fn campaign_start(campaign: &SlugcatCampaign) -> CampaignStart<'static> {
        CampaignStart {
            slugcat: campaign.name,
            den: campaign.start_den,
            vanilla: campaign.start_den_is_vanilla,
        }
    }

    /// `RainWorldGame.ExitGame(asDeath, asQuit)` of a story game after `clock` ticks:
    /// the progression and the death-persistent data, as a death after 30 seconds and a
    /// quit after 5. `AppendTimeOnCycleEnd` is not ported (D16).
    pub fn exit_game(&mut self, as_death: bool, as_quit: bool, clock: u64) {
        if self.menu.saves.current_save_state.is_none() {
            return;
        }
        let result = self
            .menu
            .saves
            .save_progression_and_death_persistent_data_of_current_state(
                as_death && clock > 1200,
                as_quit && clock > 200,
            );
        if let Err(e) = result {
            log::error!("saving on exit: {e}");
        }
    }

    /// The menu microphone's frame: its loops fade with whether a menu is the main
    /// process, and what it started or changed is played.
    fn update_menu_mic(&mut self, menu_is_main: bool) {
        // A loop's `Update` sets its volume from the fade before stepping the fade.
        let mixes: Vec<(u32, f32, f32)> = self
            .menu
            .mic
            .loops()
            .iter()
            .map(|l| (l.handle.0, l.volume_value(), l.loop_pitch))
            .collect();
        self.menu.mic.graf_update(menu_is_main);
        let events = self.menu.mic.take_events();
        let Some(audio) = &mut self.audio else {
            return;
        };
        for event in events {
            match event {
                MenuSoundEvent::Play(s) => {
                    self.menu_audio
                        .play(audio, Self::sound_start(&s, false), None);
                }
                MenuSoundEvent::StartLoop(handle, s) => {
                    // A loop starts at `fade * loopVolume` = 0.
                    let mut start = Self::sound_start(&s, true);
                    start.volume = 0.0;
                    self.menu_audio.play(audio, start, Some(handle.0));
                }
                MenuSoundEvent::StopLoop(handle) => self.menu_audio.stop(handle.0),
            }
        }
        for (handle, volume, pitch) in mixes {
            self.menu_audio.set_loop(handle, volume, pitch);
        }
        self.menu_audio.frame(audio);
    }

    fn sound_start(s: &MenuSound, looped: bool) -> MenuSoundStart {
        MenuSoundStart {
            clip: s.clip,
            variation: s.variation,
            sound_volume: s.volume,
            sound_pitch: s.pitch,
            pan: s.pan,
            volume: s.call_volume,
            pitch: s.call_pitch,
            looped,
        }
    }
}

impl MenuContext for AppContext {
    fn menu(&mut self) -> &mut MenuShared {
        &mut self.menu
    }

    fn music_playback(&self, track: TrackId) -> Option<SourcePlayback> {
        match &self.music_sink {
            MusicSink::Timed(sources) => sources.playback(track),
            MusicSink::Output(output) => output.playback(track),
        }
    }

    fn load_save_slot(&mut self) {
        let slot = Self::slot_of(&self.menu.options);
        let saves = self.game.saves.load(&self.menu.assets, slot);
        self.menu.set_saves(saves);
    }

    fn save_options(&mut self) {
        self.game.saves.save_options(&self.menu.options);
    }

    fn import_save_slot(&mut self, slot: SaveSlot) -> Result<(), String> {
        let report = self.game.saves.import(&self.menu.assets, slot)?;
        log::info!(
            "imported {} into {} (backup {:?})",
            report.source.display(),
            report.destination.display(),
            report.backup
        );
        if slot == Self::slot_of(&self.menu.options) {
            self.load_save_slot();
        }
        Ok(())
    }
}

impl ProcessFactory<AppProcess> for AppContext {
    fn create_process(&mut self, id: ProcessId, old: Option<&AppProcess>) -> AppProcess {
        log::info!("process {id:?}");
        self.end_dev_saves(id);
        // `PostSwitchMainProcess`: `progression.Revert()` before every process but these
        // screens and dreams, so a game loads the stored state (or a starved one).
        if !matches!(
            id,
            ProcessId::Initialization
                | ProcessId::SleepScreen
                | ProcessId::GhostScreen
                | ProcessId::DeathScreen
                | ProcessId::KarmaToMaxScreen
                | ProcessId::Dream
                | ProcessId::StarveScreen
        ) {
            self.menu.saves.revert();
        }
        match id {
            ProcessId::Initialization => {
                AppProcess::Initialization(Initialization::new(self.start))
            }
            ProcessId::IntroRoll => AppProcess::IntroRoll(Box::new(IntroRoll::new(&mut self.menu))),
            ProcessId::SlideShow => {
                let id = self.menu.next_slideshow.unwrap_or_else(|| {
                    log::warn!("no slideshow chosen; showing the Survivor's intro");
                    SlideShowId::WhiteIntro
                });
                AppProcess::SlideShow(Box::new(SlideShow::new(&mut self.menu, id)))
            }
            ProcessId::Game => AppProcess::Game(Box::new(self.new_game())),
            ProcessId::Dream => AppProcess::SleepStandIn(SleepStandIn {
                id,
                stage: Container::default(),
                frames: 0,
            }),
            ProcessId::SleepScreen | ProcessId::DeathScreen | ProcessId::StarveScreen => {
                // `CommunicateWithUpcomingProcess` hands the screen the game's package
                // before its first update.
                match old.and_then(|o| o.screen_package(id, &self.menu.saves, &self.menu.assets)) {
                    Some(package) => AppProcess::SleepAndDeath(Box::new(SleepAndDeathScreen::new(
                        &mut self.menu,
                        id,
                        package,
                    ))),
                    None => {
                        log::warn!("{id:?} without a game to hand it its data");
                        AppProcess::Unported(Unported::new(id, &self.menu))
                    }
                }
            }
            ProcessId::FontTest => AppProcess::FontTest(FontTest::new(&mut self.menu)),
            ProcessId::MainMenu => AppProcess::MainMenu(Box::new(MainMenu::new(&mut self.menu))),
            ProcessId::OptionsMenu => {
                AppProcess::OptionsMenu(Box::new(OptionsMenu::new(&mut self.menu)))
            }
            ProcessId::SlugcatSelect => {
                AppProcess::SlugcatSelect(Box::new(SlugcatSelectMenu::new(&mut self.menu)))
            }
            _ => {
                log::warn!("process {id:?} is not ported; Esc returns to the main menu");
                AppProcess::Unported(Unported::new(id, &self.menu))
            }
        }
    }

    fn update_side_processes(&mut self, mut current: Option<&mut AppProcess>, dt: f32) {
        self.apply_volumes();
        self.update_music(dt, current.as_deref_mut());
        self.update_menu_mic(current.is_some_and(|p| p.is_menu()));
    }

    fn pre_switch(&mut self, next: ProcessId) {
        self.menu.pre_switch(next);
    }

    fn post_switch(&mut self, current: &AppProcess) {
        self.menu.update_music_context(current.id());
    }

    fn frame_start(&mut self, current: Option<&AppProcess>) {
        self.menu
            .create_music_player(current.map(MainLoopProcess::id));
    }
}

impl MainLoopProcess<AppContext> for GameProcess {
    fn id(&self) -> ProcessId {
        ProcessId::Game
    }

    fn fade_in_time(&self) -> f32 {
        self.fade_in_time
    }

    fn initial_black_seconds(&self) -> f32 {
        self.initial_black_seconds
    }

    /// `TimeSpeedFac`: 0 while paused.
    fn time_speed_fac(&self) -> f32 {
        if self.pause_menu.is_some() { 0.0 } else { 1.0 }
    }

    /// `RainWorldGame.Update`: the pause menu, then a paused tick (the HUD) or an
    /// unpaused one, which may open the pause menu between the cameras and the rooms.
    fn update(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        self.scene
            .game_mut()
            .set_fade_delay_in_progress(requests.fade_delay_in_progress());
        let keys = ctx.input.take_tick_input();
        let input = ctx
            .script
            .as_mut()
            .and_then(Script::next_input)
            .unwrap_or(keys);
        if self.pause_menu.is_some() {
            self.update_pause_menu(ctx, requests);
        }
        if self.pause_menu.is_some() {
            self.scene.paused_tick(input, &mut ctx.menu);
            self.prompt_death_screen(ctx, requests);
            let events = self.scene.game_mut().take_sound_events();
            if let Some(audio) = &mut ctx.audio {
                audio.play_events(events, self.scene.game().body().map(|b| &b.chunks[..]));
            }
            return;
        }
        self.scene.paused = false;
        self.scene.update_cameras(input, &mut ctx.menu);
        self.prompt_death_screen(ctx, requests);
        // `RWInput.CheckPauseButton(0, inMenu: false)`; the menus' latched keys are
        // spent with the tick.
        let pause = ctx.menu.input.pause_action();
        ctx.menu.input.take_tick_input();
        let opened = pause && !self.last_pause_button && self.pause_allowed(requests);
        if opened {
            self.open_pause_menu(ctx);
        }
        self.last_pause_button = pause;
        let events = self.scene.update_rooms(input, &mut ctx.menu, opened);
        self.report_progression(ctx);
        if let Some(end) = self.scene.game_mut().sim_mut().take_session_end() {
            self.end_session(end, ctx, requests);
        }
        if self.scene.take_entered_room()
            && let Some(audio) = &mut ctx.audio
        {
            audio.new_room(&self.scene.game().settings().ambient_sounds);
        }
        self.hear_tick(ctx, events);
        self.scene.paused = self.pause_menu.is_some();
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut AppContext) {
        let mouse = Vec2::from(ctx.menu.input.mouse.pos);
        let (volume, time_speed) = match &mut self.pause_menu {
            Some(menu) => {
                menu.graf_update(time_stacker, mouse);
                (menu.mic_volume(), menu.mic_time_speed(time_stacker))
            }
            None => (1.0, 1.0),
        };
        if let Some(audio) = &mut ctx.audio {
            audio.set_pause_mix(volume, time_speed);
            audio.frame(time_stacker, self.scene.game().water());
        }
    }

    /// `RainWorldGame.ShutDownProcess`: the camera's microphone goes `AllQuiet`.
    fn shut_down(&mut self, ctx: &mut AppContext) {
        if let Some(audio) = &mut ctx.audio {
            audio.all_quiet();
        }
        if let Some(menu) = &mut self.pause_menu {
            menu.shut_down();
        }
        ctx.menu.random = self.scene.game_mut().take_rng();
    }
}

impl GameProcess {
    /// The conditions on opening the pause menu: the HUD's map closed (fade below 0.1),
    /// no game-over prompt, no fade, the camera's room fully loaded.
    fn pause_allowed(&self, requests: &ProcessRequests) -> bool {
        let hud_allows = self
            .scene
            .hud()
            .is_none_or(|h| h.map.fade < 0.1 && !h.text_prompt.game_over_mode);
        hud_allows
            && requests.fade_to_black() == 0.0
            && self.scene.game().sim().room_safe_for_pause()
    }

    /// `pauseMenu = new PauseMenu(manager, this)`.
    fn open_pause_menu(&mut self, ctx: &mut AppContext) {
        let sim = self.scene.game().sim();
        let session = sim.story_session();
        let game = PauseGame {
            story: sim.is_story_session(),
            cycle_number: session.cycle_number,
            clock: sim.clock(),
            red_is_out_of_cycles: sim.red_is_out_of_cycles(),
        };
        let menu = PauseMenu::new(&mut ctx.menu, &game, self.scene.hud_mut());
        self.pause_menu = Some(menu);
        log::info!("paused");
        self.play_menu_sounds(ctx);
    }

    /// `pauseMenu.Update()` and what it asks of the game.
    fn update_pause_menu(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        let Some(menu) = &mut self.pause_menu else {
            return;
        };
        let mut cx = MenuCx {
            shared: &mut ctx.menu,
            requests,
        };
        let asked = menu.update(&mut cx);
        self.play_menu_sounds(ctx);
        for request in asked {
            match request {
                PauseRequest::ContinuePaused => {
                    if let Some(mut menu) = self.pause_menu.take() {
                        menu.shut_down();
                    }
                    log::info!("unpaused");
                }
                PauseRequest::ExitToMenu => {
                    self.exit_to_menu(ctx, requests);
                    if let Some(menu) = &mut self.pause_menu {
                        menu.shut_down();
                    }
                }
            }
        }
    }

    /// `RainWorldGame.ExitToMenu`: the music's death event, `ExitGame(true, true)` and a
    /// switch to the main menu.
    fn exit_to_menu(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        if requests.upcoming().is_some() {
            return;
        }
        if let Some(music) = &mut ctx.menu.music {
            music.death_event();
        }
        let sim = self.scene.game().sim();
        log::info!("Exit game");
        if sim.is_story_session() {
            self.scene.game().store_map_discovery(&mut ctx.menu.saves);
            ctx.exit_game(true, true, sim.clock());
        }
        requests.switch_to(ProcessId::MainMenu);
    }

    /// What the tick tells the progression: shelters entered (`TempDiscoverShelter`)
    /// and the regions whose shelter door closed (`regionsVisited`).
    fn report_progression(&mut self, ctx: &mut AppContext) {
        let sim = self.scene.game_mut().sim_mut();
        if !sim.is_story_session() {
            return;
        }
        let slugcat = sim.campaign().name;
        for shelter in sim.take_entered_shelters() {
            ctx.menu.saves.temp_discover_shelter(&shelter);
        }
        for region in sim.take_closed_shelter_regions() {
            ctx.menu.saves.misc.add_region_visited(&region, slugcat);
        }
    }

    /// `GoToDeathScreen` from the game-over prompt (a map press 40 ticks after the death),
    /// during camera 0's HUD update, before the rooms update.
    fn prompt_death_screen(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        let asked = self
            .scene
            .hud_mut()
            .is_some_and(|h| std::mem::take(&mut h.text_prompt.go_to_death_screen));
        if asked {
            self.end_session(SessionEnd::Death, ctx, requests);
        }
    }

    /// The end of a story cycle: `Win` saves and goes to the sleep screen (or a dream);
    /// starving and dying save the cycle as a death and go to the starve and death
    /// screens (D13, D16).
    fn end_session(
        &mut self,
        end: SessionEnd,
        ctx: &mut AppContext,
        requests: &mut ProcessRequests,
    ) {
        if requests.upcoming().is_some() {
            return;
        }
        match end {
            SessionEnd::Win { malnourished } => {
                log::info!("Win (malnourished: {malnourished})");
                self.scene.game().store_map_discovery(&mut ctx.menu.saves);
                // A dev start's copy has no file to back up, and its `Win` is not the
                // run's backup.
                let mut dev_backed_up = true;
                let backed_up = if self.scene.game().story_session() {
                    &mut ctx.save_backed_up
                } else {
                    &mut dev_backed_up
                };
                let assets = ctx.menu.assets.clone();
                let mut game = HibernatingGame {
                    sim: self.scene.game_mut().sim_mut(),
                    assets: &assets,
                };
                let next = match game.win(&mut ctx.menu.saves, malnourished, backed_up) {
                    Ok((outcome, region)) => {
                        ctx.carried_region_state = region;
                        match outcome {
                            WinOutcome::Dream(_) => ProcessId::Dream,
                            WinOutcome::SleepScreen => ProcessId::SleepScreen,
                        }
                    }
                    Err(e) => {
                        log::error!("saving the cycle: {e}");
                        ProcessId::SleepScreen
                    }
                };
                requests.switch_to(next);
            }
            SessionEnd::Starve => {
                log::info!("GO TO STARVE");
                self.place_karma_flower_on_death_spot(ctx);
                self.session_ended_died(ctx);
                if let Some(music) = &mut ctx.menu.music {
                    music.death_event();
                }
                requests.switch_to(ProcessId::StarveScreen);
            }
            SessionEnd::Death => {
                self.session_ended_died(ctx);
                requests.switch_to(ProcessId::DeathScreen);
            }
        }
    }

    /// `PlaceKarmaFlowerOnDeathSpot` outside `GameOver` (the starve path).
    fn place_karma_flower_on_death_spot(&mut self, ctx: &mut AppContext) {
        let sim = self.scene.game_mut().sim_mut();
        if !sim.place_karma_flower() {
            return;
        }
        let grow_pos = sim.karma_flower_grow_pos();
        let game = HibernatingGame {
            sim,
            assets: &ctx.menu.assets,
        };
        game.place_karma_flower_on_death_spot(&mut ctx.menu.saves, grow_pos);
    }

    /// `SessionEnded(survived: false)` and its save, with the HUD prompt's death room
    /// and position (`cameras[0].hud`).
    fn session_ended_died(&mut self, ctx: &mut AppContext) {
        let death = self
            .scene
            .hud()
            .map(|h| (h.text_prompt.death_room, h.text_prompt.death_pos));
        let assets = ctx.menu.assets.clone();
        let mut game = HibernatingGame {
            sim: self.scene.game_mut().sim_mut(),
            assets: &assets,
        };
        if let Err(e) = game.session_ended_died(&mut ctx.menu.saves, death) {
            log::error!("saving the cycle as a death: {e}");
        }
    }

    /// The pause menu's `PlaySound`s, which go to the game's microphone in volume
    /// group 1 (`menuMic` is null in the game) and draw from the game's stream.
    fn play_menu_sounds(&mut self, ctx: &mut AppContext) {
        let (sounds, rng, _) = self.scene.game_mut().sim_mut().camera_io();
        for (id, [pan, volume, pitch]) in std::mem::take(&mut ctx.menu.game_mic_sounds) {
            match SoundId::from_name(id.name()) {
                Some(id) => sounds.play_disembodied_in_group(id, pan, volume, pitch, 1, rng),
                None => log::warn!("{} is not a game sound", id.name()),
            }
        }
    }

    /// Feeds a tick's sound events and the listener to the audio output.
    fn hear_tick(&self, ctx: &mut AppContext, events: Vec<rw_sim::SoundEvent>) {
        let Some(audio) = &mut ctx.audio else { return };
        let game = self.scene.game();
        let camera_pos = if self.scene.visuals().is_some() {
            Vec2::from(self.scene.camera().view_origin(1.0))
        } else {
            Vec2::ZERO
        };
        let listener = ListenerInput {
            camera_pos,
            screen_width: ctx.resolution.size_f32()[0],
            follow: Some(game.sound_follow()),
        };
        let chunks = game.body().map(|b| &b.chunks[..]);
        audio.tick(events, &listener, chunks, game.water());
    }
}

impl Unported {
    fn new(id: ProcessId, shared: &MenuShared) -> Self {
        let mut stage = Container::default();
        if let Some(font) = shared.atlases.font(Font::FONT) {
            let text = format!("{id:?} is not ported yet. Esc returns to the main menu.");
            let mut label = Label::new(font.clone(), &text);
            label.set_alignment(Alignment::Center);
            let [w, h] = shared.screen_size;
            label.transform = NodeTransform::at(w / 2.0 + 0.01, h / 2.0 + 0.01);
            stage.add(Node::Label(label));
        }
        Self {
            id,
            stage,
            last_pause_button: false,
        }
    }

    fn update(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        ctx.menu.input.take_tick_input();
        let pause = ctx.menu.input.pause_button();
        if pause && !self.last_pause_button {
            requests.switch_to(ProcessId::MainMenu);
        }
        self.last_pause_button = pause;
    }
}

impl AppProcess {
    /// The menu stage this process shows, if it is not the game or a menu.
    #[must_use]
    pub fn stage(&self) -> Option<&Container> {
        match self {
            Self::Initialization(p) => Some(p.stage()),
            Self::FontTest(p) => Some(p.stage()),
            Self::Unported(p) => Some(&p.stage),
            Self::SleepStandIn(p) => Some(&p.stage),
            Self::SleepAndDeath(_)
            | Self::Game(_)
            | Self::IntroRoll(_)
            | Self::SlideShow(_)
            | Self::MainMenu(_)
            | Self::OptionsMenu(_)
            | Self::SlugcatSelect(_) => None,
        }
    }

    /// The `Menu` of a menu process.
    #[must_use]
    pub fn menu(&self) -> Option<&Menu> {
        match self {
            Self::IntroRoll(p) => Some(p.menu()),
            Self::SlideShow(p) => Some(p.menu()),
            Self::MainMenu(p) => Some(p.menu()),
            Self::OptionsMenu(p) => Some(p.menu()),
            Self::SlugcatSelect(p) => Some(p.menu()),
            Self::SleepAndDeath(p) => Some(p.menu()),
            _ => None,
        }
    }

    fn menu_mut(&mut self) -> Option<&mut Menu> {
        match self {
            Self::IntroRoll(p) => Some(p.menu_mut()),
            Self::SlideShow(p) => Some(p.menu_mut()),
            Self::MainMenu(p) => Some(p.menu_mut()),
            Self::OptionsMenu(p) => Some(p.menu_mut()),
            Self::SlugcatSelect(p) => Some(p.menu_mut()),
            Self::SleepAndDeath(p) => Some(p.menu_mut()),
            _ => None,
        }
    }

    /// `currentMainLoop is Menu`.
    #[must_use]
    pub fn is_menu(&self) -> bool {
        self.menu().is_some()
    }

    #[must_use]
    pub fn game(&self) -> Option<&GameProcess> {
        match self {
            Self::Game(g) => Some(g),
            _ => None,
        }
    }

    pub fn game_mut(&mut self) -> Option<&mut GameProcess> {
        match self {
            Self::Game(g) => Some(g),
            _ => None,
        }
    }
}

impl MainLoopProcess<AppContext> for AppProcess {
    fn id(&self) -> ProcessId {
        match self {
            Self::Initialization(_) => ProcessId::Initialization,
            Self::IntroRoll(_) => ProcessId::IntroRoll,
            Self::SlideShow(_) => ProcessId::SlideShow,
            Self::Game(_) => ProcessId::Game,
            Self::FontTest(_) => ProcessId::FontTest,
            Self::MainMenu(_) => ProcessId::MainMenu,
            Self::OptionsMenu(_) => ProcessId::OptionsMenu,
            Self::SlugcatSelect(_) => ProcessId::SlugcatSelect,
            Self::Unported(p) => p.id,
            Self::SleepStandIn(p) => p.id,
            Self::SleepAndDeath(p) => p.id(),
        }
    }

    fn frames_per_second(&self) -> i32 {
        match self {
            Self::SleepAndDeath(p) => p.frames_per_second(),
            _ => 40,
        }
    }

    fn update(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        match self {
            Self::Initialization(p) => p.update(ctx, requests),
            Self::IntroRoll(p) => p.update(ctx, requests),
            Self::SlideShow(p) => p.update(ctx, requests),
            Self::Game(p) => p.update(ctx, requests),
            Self::FontTest(p) => p.update(ctx, requests),
            Self::MainMenu(p) => p.update(ctx, requests),
            Self::OptionsMenu(p) => p.update(ctx, requests),
            Self::SlugcatSelect(p) => p.update(ctx, requests),
            Self::Unported(p) => p.update(ctx, requests),
            Self::SleepStandIn(p) => p.update(ctx, requests),
            Self::SleepAndDeath(p) => {
                let input = ctx.input.take_tick_input();
                let mut cx = MenuCx {
                    shared: &mut ctx.menu,
                    requests,
                };
                p.update(&mut cx, input);
            }
        }
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut AppContext) {
        match self {
            Self::Initialization(p) => p.graf_update(time_stacker, ctx),
            Self::IntroRoll(p) => p.graf_update(time_stacker, ctx),
            Self::SlideShow(p) => p.graf_update(time_stacker, ctx),
            Self::Game(p) => p.graf_update(time_stacker, ctx),
            Self::FontTest(p) => p.graf_update(time_stacker, ctx),
            Self::MainMenu(p) => p.graf_update(time_stacker, ctx),
            Self::OptionsMenu(p) => p.graf_update(time_stacker, ctx),
            Self::SlugcatSelect(p) => p.graf_update(time_stacker, ctx),
            Self::SleepAndDeath(p) => {
                let mouse = Vec2::from(ctx.menu.input.mouse.pos);
                p.graf_update(time_stacker, mouse, ctx.menu.screen_size[1]);
            }
            Self::Unported(_) | Self::SleepStandIn(_) => {}
        }
    }

    fn shut_down(&mut self, ctx: &mut AppContext) {
        match self {
            Self::Initialization(p) => p.shut_down(ctx),
            Self::IntroRoll(p) => p.shut_down(ctx),
            Self::SlideShow(p) => p.shut_down(ctx),
            Self::Game(p) => p.shut_down(ctx),
            Self::FontTest(p) => p.shut_down(ctx),
            Self::MainMenu(p) => p.shut_down(ctx),
            Self::OptionsMenu(p) => p.shut_down(ctx),
            Self::SlugcatSelect(p) => p.shut_down(ctx),
            Self::SleepAndDeath(p) => p.shut_down(&mut ctx.menu),
            Self::Unported(_) | Self::SleepStandIn(_) => {}
        }
    }

    fn fade_in_time(&self) -> f32 {
        match self {
            Self::Game(p) => p.fade_in_time(),
            _ => 0.45,
        }
    }

    fn initial_black_seconds(&self) -> f32 {
        match self {
            Self::Game(p) => p.initial_black_seconds(),
            _ => 0.0,
        }
    }

    fn removes_loading_label(&self) -> bool {
        matches!(self, Self::SlideShow(_))
    }

    fn time_speed_fac(&self) -> f32 {
        match self {
            Self::Game(p) => p.time_speed_fac(),
            Self::SleepAndDeath(p) => p.frames_per_second() as f32 / 40.0,
            _ => 1.0,
        }
    }

    /// The intro roll and the slideshow keep their own clocks after the frame
    /// (`RawUpdate`).
    fn raw_update(
        &mut self,
        dt: f32,
        stacker: &mut TimeStacker,
        ctx: &mut AppContext,
        requests: &mut ProcessRequests,
    ) {
        match self {
            Self::IntroRoll(p) => return p.raw_update(dt, stacker, ctx, requests),
            Self::SlideShow(p) => return p.raw_update(dt, stacker, ctx, requests),
            _ => {}
        }
        for _ in 0..stacker.advance(dt, self.frames_per_second()) {
            self.update(ctx, requests);
        }
        self.graf_update(stacker.0, ctx);
    }

    fn communicate_with_upcoming(&mut self, next: &mut Self, ctx: &mut AppContext) {
        if let Some(menu) = self.menu_mut() {
            menu.communicate_with_upcoming(next.menu_mut(), &mut ctx.menu);
        }
        // `SleepAndDeathScreen.CommunicateWithUpcomingProcess`: after the death screen
        // the cycle starts 340 ticks in (no `PrecycleModule` without the DLCs).
        if self.id() == ProcessId::DeathScreen
            && let Self::Game(game) = next
        {
            game.scene.game_mut().sim_mut().world_mut().rain_cycle.timer = 340;
        }
    }
}

impl Frontend {
    /// Boots the process manager (black, then `Initialization`).
    pub fn new(mut ctx: AppContext) -> Self {
        let manager = ProcessManager::new(ctx.menu.screen_size, &mut ctx);
        Self { manager, ctx }
    }

    /// One frame of `dt` seconds (`ProcessManager.Update`).
    pub fn update(&mut self, dt: f32) {
        self.manager.update(dt, &mut self.ctx);
        self.ctx.sync_resolution();
    }

    /// The options menu, when it is the current process.
    #[must_use]
    pub fn options_menu(&self) -> Option<&OptionsMenu> {
        match self.manager.current() {
            Some(AppProcess::OptionsMenu(o)) => Some(o),
            _ => None,
        }
    }

    /// `Application.Quit()` was called (the main menu's Exit).
    #[must_use]
    pub fn quit_requested(&self) -> bool {
        self.ctx.menu.quit
    }

    #[must_use]
    pub fn current_id(&self) -> Option<ProcessId> {
        self.manager.current().map(MainLoopProcess::id)
    }

    #[must_use]
    pub fn game(&self) -> Option<&GameProcess> {
        self.manager.current().and_then(AppProcess::game)
    }

    pub fn game_mut(&mut self) -> Option<&mut GameProcess> {
        self.manager.current_mut().and_then(AppProcess::game_mut)
    }

    /// The sleep, death or starve screen, when it is the current process.
    #[must_use]
    pub fn sleep_screen(&self) -> Option<&SleepAndDeathScreen> {
        match self.manager.current() {
            Some(AppProcess::SleepAndDeath(s)) => Some(s),
            _ => None,
        }
    }

    /// The slideshow, when it is the current process.
    #[must_use]
    pub fn slideshow(&self) -> Option<&SlideShow> {
        match self.manager.current() {
            Some(AppProcess::SlideShow(s)) => Some(s),
            _ => None,
        }
    }

    /// Queues the frame: the game's scene or the menu stage, then the fade.
    pub fn draw<R: Renderer>(&mut self, target: &mut FrontendRenderer<R>, view: SceneView) {
        let alpha = self.manager.time_stacker();
        let mut map = MapGlobals::default();
        match self.manager.current_mut() {
            Some(AppProcess::Game(game)) => {
                game.scene.sync_hud_textures(&mut self.ctx.menu.atlases);
                map = game.scene.hud_map_globals(alpha);
            }
            Some(AppProcess::SleepAndDeath(screen)) => {
                let hud = screen.hud_mut();
                hud.sync_textures(&mut self.ctx.menu.atlases);
                map = hud.map_globals(alpha).unwrap_or_default();
            }
            _ => {}
        }
        target
            .textures
            .sync(&self.ctx.menu.atlases, &mut target.scene.renderer);
        let menu_scene = self
            .manager
            .current()
            .and_then(|p| p.menu())
            .and_then(|m| m.scene_globals)
            .unwrap_or_default();
        target.scene.renderer.set_stage_globals(StageGlobals {
            rain: self.manager.shaders_time() / 5.0,
            text_gradient: self.ctx.menu.text_gradient,
            noise2: self.ctx.menu.noise2,
            sprite_rect: [0.0; 4],
            map,
            menu_scene,
        });
        let screen = self.ctx.menu.screen_size;
        let mut batch = StageBatch::default();
        let atlases = &self.ctx.menu.atlases;
        match self.manager.current_mut() {
            Some(AppProcess::Game(game)) => {
                game.scene.draw(&mut target.scene, alpha, view);
                game.scene.draw_hud(atlases, &mut batch, alpha);
                if let Some(menu) = game.pause_menu.as_ref().filter(|m| !m.shut_down) {
                    menu.menu().stage.push_quads(atlases, &mut batch);
                }
            }
            current => {
                let r = &mut target.scene.renderer;
                r.begin_frame(Camera::window([0.0, 0.0], screen, r.size()));
                r.set_background(Color::BLACK);
                if let Some(menu) = current.as_ref().and_then(|p| p.menu()) {
                    menu.stage.push_quads(atlases, &mut batch);
                    if let Some(AppProcess::OptionsMenu(options)) = current.as_ref()
                        && let Some(dialog) = options.dialog()
                    {
                        dialog.menu().stage.push_quads(atlases, &mut batch);
                    }
                } else if let Some(stage) = current.as_ref().and_then(|p| p.stage()) {
                    stage.push_quads(atlases, &mut batch);
                }
            }
        }
        self.manager
            .fade_overlay(atlases, &self.ctx.menu.translator)
            .push_quads(atlases, &mut batch);
        target.scene.renderer.draw_stage(&batch, screen);
    }
}

impl<R: Renderer> FrontendRenderer<R> {
    /// The HUD of `scene` over the frame its `draw` queued (a game run without the
    /// process manager).
    pub fn draw_scene_hud(&mut self, shared: &mut MenuShared, scene: &mut Scene, alpha: f32) {
        scene.sync_hud_textures(&mut shared.atlases);
        self.textures
            .sync(&shared.atlases, &mut self.scene.renderer);
        self.scene.renderer.set_stage_globals(StageGlobals {
            rain: 0.0,
            text_gradient: shared.text_gradient,
            noise2: shared.noise2,
            sprite_rect: [0.0; 4],
            map: scene.hud_map_globals(alpha),
            menu_scene: MenuSceneGlobals::default(),
        });
        let mut batch = StageBatch::default();
        scene.draw_hud(&shared.atlases, &mut batch, alpha);
        self.scene.renderer.draw_stage(&batch, shared.screen_size);
    }

    pub fn new(scene: SceneRenderer<R>) -> Self {
        Self {
            scene,
            textures: TextureSync::default(),
        }
    }
}
