//! The app's processes under `rw_menu`'s [`ProcessManager`]: the single-room game,
//! the reduced `Initialization` and the menu screens, drawn the same way in the window
//! and in MCP screenshots (each process's frame, then the switch fade over it). The
//! menu microphone runs as the side process after them.

use std::sync::Arc;

use rw_assets::{Room, RoomSettings, SoundTable, StreamingAssets, UnityResources};
use rw_audio::{Audio, ListenerInput, MenuPlayback, MenuSoundStart};
use rw_graphics::SlugcatName;
use rw_menu::menu::Menu;
use rw_menu::{
    Alignment, AtlasLoadError, Container, Font, FontTest, Initialization, Label, MainLoopProcess,
    MainMenu, MenuContext, MenuMic, MenuShared, MenuSound, MenuSoundEvent, Node, NodeTransform,
    ProcessFactory, ProcessId, ProcessManager, ProcessRequests, Slugcat, SlugcatSelectMenu,
    TextureSync,
};
use rw_random::UnityRandom;
use rw_render::{Camera, Color, Renderer, Resolution, StageBatch, StageGlobals};
use rw_sim::Vec2;

use crate::game::Game;
use crate::input::{InputState, Script};
use crate::scene::{Scene, SceneRenderer, SceneView};
use crate::visuals::Visuals;

/// What the game process is built from.
pub struct GameSetup {
    pub room: Room,
    pub settings: RoomSettings,
    pub seed: u64,
    pub spawn: Option<(i32, i32)>,
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
    /// The process `Initialization` starts.
    pub start: ProcessId,
}

/// The game process: the single-room [`Scene`].
pub struct GameProcess {
    pub scene: Scene,
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
        menu.mic = MenuMic::new(Arc::clone(&game.sound_table));
        Ok(Self {
            menu,
            resources,
            resolution,
            game,
            input: InputState::default(),
            script: None,
            audio: None,
            menu_audio: MenuPlayback::default(),
            start,
        })
    }

    /// A fresh game in the configured room, drawing from the shared random stream, as
    /// the slugcat the menus chose (the Survivor when started directly).
    fn new_game(&mut self) -> GameProcess {
        let setup = &self.game;
        let visuals = Visuals::load(
            &self.menu.assets,
            &self.resources,
            &setup.room,
            self.resolution.size_f32(),
        )
        .inspect_err(|e| log::warn!("no level graphic ({e}); showing the debug view"))
        .ok();
        let slugcat = match self.menu.in_game_slugcat {
            Some(Slugcat::Yellow) => SlugcatName::Yellow,
            Some(Slugcat::Red) => SlugcatName::Red,
            Some(Slugcat::White) | None => SlugcatName::White,
        };
        let rng = std::mem::replace(&mut self.menu.random, UnityRandom::new(0));
        let mut game = Game::with_rng(
            setup.room.clone(),
            &setup.settings,
            rng,
            setup.spawn,
            slugcat,
        );
        game.set_sound_table(Arc::clone(&setup.sound_table));
        if let Some(audio) = &mut self.audio {
            audio.set_ambients(&setup.settings.ambient_sounds);
        }
        GameProcess {
            scene: Scene::new(game, visuals),
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
}

impl ProcessFactory<AppProcess> for AppContext {
    fn create_process(&mut self, id: ProcessId, _old: Option<&AppProcess>) -> AppProcess {
        log::info!("process {id:?}");
        match id {
            ProcessId::Initialization => {
                AppProcess::Initialization(Initialization::new(self.start))
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

    fn update_side_processes(&mut self, current: Option<&AppProcess>, _dt: f32) {
        self.update_menu_mic(current.is_some_and(AppProcess::is_menu));
    }

    fn pre_switch(&mut self, next: ProcessId) {
        self.menu.pre_switch(next);
    }
}

impl MainLoopProcess<AppContext> for GameProcess {
    fn id(&self) -> ProcessId {
        ProcessId::Game
    }

    fn update(&mut self, ctx: &mut AppContext, _requests: &mut ProcessRequests) {
        let keys = ctx.input.take_tick_input();
        let input = ctx
            .script
            .as_mut()
            .and_then(Script::next_input)
            .unwrap_or(keys);
        let events = self.scene.tick(input);
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
            Self::Game(_) | Self::MainMenu(_) | Self::SlugcatSelect(_) => None,
        }
    }

    /// The `Menu` of a menu process.
    #[must_use]
    pub fn menu(&self) -> Option<&Menu> {
        match self {
            Self::MainMenu(p) => Some(p.menu()),
            Self::SlugcatSelect(p) => Some(p.menu()),
            _ => None,
        }
    }

    fn menu_mut(&mut self) -> Option<&mut Menu> {
        match self {
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
            Self::Game(p) => p.shut_down(ctx),
            Self::FontTest(p) => p.shut_down(ctx),
            Self::MainMenu(p) => p.shut_down(ctx),
            Self::SlugcatSelect(p) => p.shut_down(ctx),
            Self::Unported(_) => {}
        }
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

    /// Queues the frame: the game's scene or the menu stage, then the fade.
    pub fn draw<R: Renderer>(&mut self, target: &mut FrontendRenderer<R>, view: SceneView) {
        target
            .textures
            .sync(&self.ctx.menu.atlases, &mut target.scene.renderer);
        target.scene.renderer.set_stage_globals(StageGlobals {
            rain: self.manager.shaders_time() / 5.0,
            text_gradient: self.ctx.menu.text_gradient,
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
