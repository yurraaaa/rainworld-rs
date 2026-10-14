//! Window, event loop and frame pacing.
//!
//! Each redraw: advance the [`FixedTimestep`], run that many 40 Hz ticks (one
//! [`Input`](rw_sim::Input) per tick), then render with the leftover fraction as
//! the interpolation factor. Each tick's sound events go to the [`Audio`] output, and
//! each frame mixes it.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rw_audio::{Audio, ListenerInput};
use rw_render::{Renderer, Resolution, RoomLayers, WgpuRenderer};
use rw_sim::{SoundEvent, Vec2};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Fullscreen, Window, WindowId};

use crate::input::{Action, Command, InputState, Script};
use crate::scene::{Framing, Scene, SceneRenderer, SceneView};
use crate::sprites::SpriteAssets;
use crate::timestep::{FixedTimestep, TICK_RATE};

/// Runs the window until it is closed or Esc is pressed.
///
/// # Errors
/// When the event loop cannot be created or exits with an error, or when the window
/// or renderer could not be created.
pub fn run(
    scene: Scene,
    sprites: Option<Arc<SpriteAssets>>,
    options: Options,
) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(scene, sprites, options);
    event_loop.run_app(&mut app)?;
    match app.fatal.take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Command-line options of the window.
pub struct Options {
    /// Save a frame here once a second has been simulated, then quit.
    pub screenshot: Option<PathBuf>,
    /// Start in the debug view.
    pub debug_view: bool,
    /// Input steps played instead of the keyboard; the screenshot waits for them.
    pub script: Option<Script>,
    /// The game's screen size the frame is drawn at.
    pub resolution: Resolution,
    /// A window of `resolution` instead of the game's default borderless full screen.
    pub windowed: bool,
    /// The sound output; `None` plays silently.
    pub audio: Option<Audio>,
}

/// Window and renderer, created on `resumed`.
struct View {
    window: Arc<Window>,
    renderer: SceneRenderer<WgpuRenderer>,
}

struct App {
    scene: Scene,
    input: InputState,
    clock: FixedTimestep,
    last_frame: Option<Instant>,
    layers: RoomLayers,
    /// The slugcat's atlas; without it the body is drawn as circles.
    sprites: Option<Arc<SpriteAssets>>,
    /// Show the level graphic (V toggles) rather than the debug tiles.
    show_visuals: bool,
    view: Option<View>,
    /// `--screenshot`: save a frame here once a second has been simulated, then quit.
    screenshot_and_quit: Option<PathBuf>,
    /// `--script`: replaces the keyboard until it runs out.
    script: Option<Script>,
    resolution: Resolution,
    windowed: bool,
    audio: Option<Audio>,
    /// Quit after the current frame is presented.
    quit_after_frame: bool,
    /// Error that ended the loop, reported by [`run`].
    fatal: Option<Box<dyn Error>>,
}

impl App {
    fn new(scene: Scene, sprites: Option<Arc<SpriteAssets>>, options: Options) -> Self {
        Self {
            sprites,
            show_visuals: scene.visuals().is_some() && !options.debug_view,
            scene,
            input: InputState::default(),
            clock: FixedTimestep::default(),
            last_frame: None,
            layers: RoomLayers::default(),
            view: None,
            screenshot_and_quit: options.screenshot,
            script: options.script,
            resolution: options.resolution,
            windowed: options.windowed,
            audio: options.audio,
            quit_after_frame: false,
            fatal: None,
        }
    }

    /// A borderless full-screen window (the game's default, `fullscreenMode`
    /// FullScreenWindow), or with `windowed` a window of the game's resolution in
    /// physical pixels, not resizable (`resizableWindow: 0`). Either way the frame is
    /// drawn at the resolution and scaled to the window.
    fn create_view(&self, event_loop: &ActiveEventLoop) -> Result<View, Box<dyn Error>> {
        let room = self.scene.game().room();
        let (width, height) = self.resolution.size();
        let attrs = Window::default_attributes()
            .with_title(format!("rainworld-rs - {}", room.name()))
            .with_inner_size(PhysicalSize::new(width, height))
            .with_resizable(false)
            .with_fullscreen((!self.windowed).then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attrs)?);
        // Hidden over the game window during play; the pause and main menus show it again.
        window.set_cursor_visible(false);
        let physical = window.inner_size();
        log::info!(
            "resolution {}, window {}x{}",
            self.resolution,
            physical.width,
            physical.height
        );
        let renderer = WgpuRenderer::new(
            Arc::clone(&window),
            (physical.width, physical.height),
            self.resolution,
        )?;
        let renderer = SceneRenderer::new(renderer, self.sprites.clone());
        Ok(View { window, renderer })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Box<dyn Error>) {
        self.fatal = Some(error);
        event_loop.exit();
    }

    fn on_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        let pressed = event.state == ElementState::Pressed;
        if let Some(action) = Action::from_key(code) {
            self.input.set(action, pressed);
        }
        if !pressed || event.repeat {
            return;
        }
        match Command::from_key(code) {
            Some(Command::Quit) => {
                log::info!("quit (Esc)");
                event_loop.exit();
            }
            Some(Command::Respawn) => {
                self.scene.game_mut().respawn();
                self.clock.reset();
            }
            Some(Command::ToggleView) => {
                self.show_visuals = !self.show_visuals && self.scene.visuals().is_some();
                log::info!(
                    "{} view",
                    if self.show_visuals { "visual" } else { "debug" }
                );
            }
            Some(Command::ToggleGrid) => {
                self.layers.grid = !self.layers.grid;
                log::info!("grid {}", if self.layers.grid { "on" } else { "off" });
            }
            Some(Command::Screenshot) => {
                let path = PathBuf::from(format!("screenshot-{}.ppm", self.scene.game().ticks()));
                log::info!("saving screenshot to {}", path.display());
                if let Some(view) = &mut self.view {
                    view.renderer.renderer.capture_next_frame(path);
                }
            }
            None => {}
        }
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed = self.last_frame.map_or_else(Default::default, |t| now - t);
        self.last_frame = Some(now);
        for _ in 0..self.clock.advance(elapsed) {
            let keys = self.input.take_tick_input();
            let input = self
                .script
                .as_mut()
                .and_then(Script::next_input)
                .unwrap_or(keys);
            let events = self.scene.tick(input);
            self.hear_tick(events);
        }
        if let Some(audio) = &mut self.audio {
            audio.frame(self.clock.alpha(), self.scene.game().water());
        }
        let Some(view) = &mut self.view else { return };
        let scene_view = SceneView {
            level: self.show_visuals,
            layers: self.layers,
            framing: Framing::Auto,
        };
        self.scene
            .draw(&mut view.renderer, self.clock.alpha(), scene_view);
        let r = &mut view.renderer.renderer;
        let script_ticks = self.script.as_ref().map_or(0, Script::len);
        if self.scene.game().ticks() >= u64::from(TICK_RATE.max(script_ticks))
            && let Some(path) = self.screenshot_and_quit.take()
        {
            log::info!("saving screenshot to {}", path.display());
            r.capture_next_frame(path);
            self.quit_after_frame = true;
        }
        if let Err(e) = r.present() {
            self.fail(event_loop, Box::new(e));
        } else if self.quit_after_frame {
            event_loop.exit();
        }
    }

    /// Feeds a tick's sound events and the listener to the audio output.
    fn hear_tick(&mut self, events: Vec<SoundEvent>) {
        let Some(audio) = &mut self.audio else { return };
        let game = self.scene.game();
        let camera_pos = self
            .scene
            .visuals()
            .map_or(Vec2::ZERO, |v| Vec2::from(v.camera_pos()));
        let listener = ListenerInput {
            camera_pos,
            screen_width: self.resolution.size_f32()[0],
            follow: Some(game.sound_follow()),
        };
        let chunks = game.body().map(|b| &b.chunks[..]);
        audio.tick(events, &listener, chunks, game.water());
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.view.is_some() {
            return;
        }
        match self.create_view(event_loop) {
            Ok(view) => {
                view.window.request_redraw();
                self.view = Some(view);
            }
            Err(e) => self.fail(event_loop, e),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("window close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(view) = &mut self.view {
                    view.renderer.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::Focused(false) => self.input.clear(),
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event_loop, &event),
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(view) = &self.view {
            view.window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        log::info!("exiting after {} ticks", self.scene.game().ticks());
    }
}
