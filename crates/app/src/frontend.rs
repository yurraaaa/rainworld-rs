//! The app's processes under `rw_menu`'s [`ProcessManager`]: the game,
//! the reduced `Initialization` and the menu screens, drawn the same way in the window
//! and in MCP screenshots (each process's frame, then the switch fade over it). The
//! music player and the menu microphone run as side processes after them.

use std::sync::Arc;

use rw_assets::{SoundTable, StreamingAssets, UnityResources};
use rw_audio::music::{MusicClock, MusicOutput, SongBank, SourcePlayback, TimedSources, TrackId};
use rw_audio::{Audio, ListenerInput, MenuPlayback, MenuSoundStart};
use rw_menu::menu::Menu;
use rw_menu::{
    Alignment, AtlasLoadError, Container, Font, FontTest, Initialization, IntroRoll, KeyboardKey,
    Label, MainLoopProcess, MainMenu, MenuContext, MenuShared, MenuSound, MenuSoundEvent, Node,
    NodeTransform, ProcessFactory, ProcessId, ProcessManager, ProcessRequests, SlideShow,
    SlideShowId, SlugcatSelectMenu, TextureSync, TimeStacker,
};
use rw_random::UnityRandom;
use rw_render::{Camera, Color, Renderer, Resolution, StageBatch, StageGlobals};
use rw_sim::Vec2;

use crate::game::{Game, GameStart};
use crate::input::{Action, InputState, Script};
use crate::scene::{Scene, SceneRenderer, SceneView, VisualsSource};

/// What the game process is built from.
pub struct GameSetup {
    /// The start when the menus chose no campaign (the game started directly).
    pub start: GameStart,
    pub seed: u64,
    pub sound_table: Arc<SoundTable>,
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
}

/// Where the music player's songs play: through the audio output, or silently in
/// ticks (no audio: `--mute`, headless and MCP runs), which keeps runs deterministic.
pub enum MusicSink {
    Timed(Box<TimedSources>),
    Output(MusicOutput),
}

/// The game process: the [`Scene`].
pub struct GameProcess {
    pub scene: Scene,
    /// `FadeInTime` and `InitialBlackSeconds`, from the menu setup it was made with.
    fade_in_time: f32,
    initial_black_seconds: f32,
}

/// A process the port does not have yet: its name on black; the pause button (Esc)
/// returns to the main menu (docs/DIVERGENCES.md D13).
pub struct Unported {
    id: ProcessId,
    stage: Container,
    last_pause_button: bool,
}

/// The processes the app runs.
pub enum AppProcess {
    Initialization(Initialization),
    IntroRoll(Box<IntroRoll>),
    SlideShow(Box<SlideShow>),
    Game(Box<GameProcess>),
    FontTest(FontTest),
    MainMenu(Box<MainMenu>),
    SlugcatSelect(Box<SlugcatSelectMenu>),
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
        })
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
    fn update_music(&mut self, dt: f32) {
        let Some(player) = &mut self.menu.music else {
            return;
        };
        for _ in 0..self.music_stacker.advance(dt, 40) {
            match &mut self.music_sink {
                MusicSink::Timed(sources) => {
                    player.update(sources.as_mut());
                    sources.advance();
                }
                MusicSink::Output(output) => {
                    if let Some(audio) = &mut self.audio {
                        player.update(&mut output.sources(audio));
                    }
                }
            }
        }
    }

    /// A fresh game drawing from the shared random stream: a new story as the slugcat
    /// the menus chose, or the configured start when started directly.
    fn new_game(&mut self) -> GameProcess {
        let start = match self.menu.in_game_slugcat {
            Some(slugcat) => GameStart::NewStory(slugcat),
            None => self.game.start.clone(),
        };
        let rng = std::mem::replace(&mut self.menu.random, UnityRandom::new(0));
        let table = Arc::clone(&self.game.sound_table);
        let game = Game::start(&self.menu.assets, &start, rng, table)
            .unwrap_or_else(|e| panic!("starting the game ({start:?}): {e}"));
        let source = VisualsSource {
            assets: self.menu.assets.clone(),
            resources: Arc::clone(&self.resources),
            screen: self.resolution.size_f32(),
        };
        let scene = Scene::new(game, Some(source));
        if let Some(audio) = &mut self.audio {
            audio.set_ambients(&scene.game().settings().ambient_sounds);
        }
        let condition = self.menu.game_start_condition();
        GameProcess {
            scene,
            fade_in_time: condition.game_fade_in_time(),
            initial_black_seconds: condition
                .game_initial_black_seconds(self.menu.progression.selected_slugcat),
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
}

impl ProcessFactory<AppProcess> for AppContext {
    fn create_process(&mut self, id: ProcessId, _old: Option<&AppProcess>) -> AppProcess {
        log::info!("process {id:?}");
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
            ProcessId::FontTest => AppProcess::FontTest(FontTest::new(&mut self.menu)),
            ProcessId::MainMenu => AppProcess::MainMenu(Box::new(MainMenu::new(&mut self.menu))),
            ProcessId::SlugcatSelect => {
                AppProcess::SlugcatSelect(Box::new(SlugcatSelectMenu::new(&mut self.menu)))
            }
            _ => {
                log::warn!("process {id:?} is not ported; Esc returns to the main menu");
                AppProcess::Unported(Unported::new(id, &self.menu))
            }
        }
    }

    fn update_side_processes(&mut self, current: Option<&AppProcess>, dt: f32) {
        self.update_music(dt);
        self.update_menu_mic(current.is_some_and(AppProcess::is_menu));
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
        let events = self.scene.tick(input);
        if self.scene.take_entered_room()
            && let Some(audio) = &mut ctx.audio
        {
            audio.set_ambients(&self.scene.game().settings().ambient_sounds);
        }
        self.hear_tick(ctx, events);
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut AppContext) {
        if let Some(audio) = &mut ctx.audio {
            audio.frame(time_stacker, self.scene.game().water());
        }
    }

    fn shut_down(&mut self, ctx: &mut AppContext) {
        ctx.menu.random = self.scene.game_mut().take_rng();
    }
}

impl GameProcess {
    /// Feeds a tick's sound events and the listener to the audio output.
    fn hear_tick(&self, ctx: &mut AppContext, events: Vec<rw_sim::SoundEvent>) {
        let Some(audio) = &mut ctx.audio else { return };
        let game = self.scene.game();
        let camera_pos = self
            .scene
            .visuals()
            .map_or(Vec2::ZERO, |v| Vec2::from(v.camera_pos()));
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
            Self::Game(_)
            | Self::IntroRoll(_)
            | Self::SlideShow(_)
            | Self::MainMenu(_)
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
            Self::SlugcatSelect(p) => Some(p.menu()),
            _ => None,
        }
    }

    fn menu_mut(&mut self) -> Option<&mut Menu> {
        match self {
            Self::IntroRoll(p) => Some(p.menu_mut()),
            Self::SlideShow(p) => Some(p.menu_mut()),
            Self::MainMenu(p) => Some(p.menu_mut()),
            Self::SlugcatSelect(p) => Some(p.menu_mut()),
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
            Self::SlugcatSelect(_) => ProcessId::SlugcatSelect,
            Self::Unported(p) => p.id,
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
            Self::SlugcatSelect(p) => p.update(ctx, requests),
            Self::Unported(p) => p.update(ctx, requests),
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
            Self::SlugcatSelect(p) => p.graf_update(time_stacker, ctx),
            Self::Unported(_) => {}
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
            Self::SlugcatSelect(p) => p.shut_down(ctx),
            Self::Unported(_) => {}
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
        target
            .textures
            .sync(&self.ctx.menu.atlases, &mut target.scene.renderer);
        target.scene.renderer.set_stage_globals(StageGlobals {
            rain: self.manager.shaders_time() / 5.0,
            text_gradient: self.ctx.menu.text_gradient,
            noise2: self.ctx.menu.noise2,
            sprite_rect: [0.0; 4],
        });
        let screen = self.ctx.menu.screen_size;
        let mut batch = StageBatch::default();
        let alpha = self.manager.time_stacker();
        let atlases = &self.ctx.menu.atlases;
        match self.manager.current_mut() {
            Some(AppProcess::Game(game)) => game.scene.draw(&mut target.scene, alpha, view),
            current => {
                let r = &mut target.scene.renderer;
                r.begin_frame(Camera::window([0.0, 0.0], screen, r.size()));
                r.set_background(Color::BLACK);
                if let Some(menu) = current.as_ref().and_then(|p| p.menu()) {
                    menu.stage.push_quads(atlases, &mut batch);
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
    pub fn new(scene: SceneRenderer<R>) -> Self {
        Self {
            scene,
            textures: TextureSync::default(),
        }
    }
}
