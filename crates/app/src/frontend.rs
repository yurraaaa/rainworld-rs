//! The app's processes under `rw_menu`'s [`ProcessManager`]: the single-room game,
//! the reduced `Initialization` and the menu screens, drawn the same way in the window
//! and in MCP screenshots (each process's frame, then the switch fade over it).

use std::sync::Arc;

use rw_assets::{Room, RoomSettings, SoundTable, StreamingAssets, UnityResources};
use rw_audio::{Audio, ListenerInput};
use rw_menu::{
    AtlasLoadError, Container, FontTest, Initialization, MainLoopProcess, MenuContext, MenuShared,
    ProcessFactory, ProcessId, ProcessManager, ProcessRequests, TextureSync,
};
use rw_render::{Camera, Color, Renderer, Resolution, StageBatch};
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
    /// The process `Initialization` starts.
    pub start: ProcessId,
}

/// The game process: the single-room [`Scene`].
pub struct GameProcess {
    pub scene: Scene,
}

/// A process the port does not have yet: a black screen.
pub struct Unported {
    id: ProcessId,
    stage: Container,
}

/// The processes the app runs.
pub enum AppProcess {
    Initialization(Initialization),
    Game(Box<GameProcess>),
    FontTest(FontTest),
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
    /// Loads the fonts and strings the menus and the fade's label use.
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
        let menu = MenuShared::load(assets, &resources, resolution.size_f32())?;
        Ok(Self {
            menu,
            resources,
            resolution,
            game,
            input: InputState::default(),
            script: None,
            audio: None,
            start,
        })
    }

    /// A fresh game in the configured room.
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
        let mut game = Game::new(setup.room.clone(), &setup.settings, setup.seed, setup.spawn);
        game.set_sound_table(Arc::clone(&setup.sound_table));
        if let Some(audio) = &mut self.audio {
            audio.set_ambients(&setup.settings.ambient_sounds);
        }
        GameProcess {
            scene: Scene::new(game, visuals),
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
            _ => {
                log::warn!("process {id:?} is not ported; showing black");
                AppProcess::Unported(Unported {
                    id,
                    stage: Container::default(),
                })
            }
        }
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

impl AppProcess {
    /// The menu stage this process shows, if it is not the game.
    #[must_use]
    pub fn stage(&self) -> Option<&Container> {
        match self {
            Self::Initialization(p) => Some(p.stage()),
            Self::FontTest(p) => Some(p.stage()),
            Self::Unported(p) => Some(&p.stage),
            Self::Game(_) => None,
        }
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
            Self::Unported(p) => p.id,
        }
    }

    fn update(&mut self, ctx: &mut AppContext, requests: &mut ProcessRequests) {
        match self {
            Self::Initialization(p) => p.update(ctx, requests),
            Self::Game(p) => p.update(ctx, requests),
            Self::FontTest(p) => p.update(ctx, requests),
            Self::Unported(_) => {}
        }
    }

    fn graf_update(&mut self, time_stacker: f32, ctx: &mut AppContext) {
        match self {
            Self::Initialization(p) => p.graf_update(time_stacker, ctx),
            Self::Game(p) => p.graf_update(time_stacker, ctx),
            Self::FontTest(p) => p.graf_update(time_stacker, ctx),
            Self::Unported(_) => {}
        }
    }

    fn shut_down(&mut self, ctx: &mut AppContext) {
        match self {
            Self::Initialization(p) => p.shut_down(ctx),
            Self::Game(p) => p.shut_down(ctx),
            Self::FontTest(p) => p.shut_down(ctx),
            Self::Unported(_) => {}
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
        let screen = self.ctx.menu.screen_size;
        let mut batch = StageBatch::default();
        let alpha = self.manager.time_stacker();
        match self.manager.current_mut() {
            Some(AppProcess::Game(game)) => game.scene.draw(&mut target.scene, alpha, view),
            current => {
                let r = &mut target.scene.renderer;
                r.begin_frame(Camera::window([0.0, 0.0], screen, r.size()));
                r.set_background(Color::BLACK);
                if let Some(stage) = current.as_ref().and_then(|p| p.stage()) {
                    stage.push_quads(&self.ctx.menu.atlases, &mut batch);
                }
            }
        }
        let atlases = &self.ctx.menu.atlases;
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
