//! Window, event loop and frame pacing.
//!
//! Each redraw hands the frame's real time to the process manager
//! ([`Frontend::update`]): the current process runs its 40 Hz ticks (gameplay input
//! per tick, sound events to the [`rw_audio::Audio`] output) and its per-frame update,
//! and the switch fade advances. Then the frame is drawn with the leftover tick
//! fraction as the interpolation factor.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rw_menu::{KeyboardKey, ProcessId};
use rw_render::{PresentRect, Renderer, Resolution, RoomLayers, WgpuRenderer};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Fullscreen, Window, WindowId};

use crate::frontend::{Frontend, FrontendRenderer};
use crate::input::{Action, Command, Script, WinitKey};
use crate::scene::{Framing, SceneRenderer, SceneView};
use crate::sprites::SpriteAssets;
use crate::timestep::{FrameClock, TICK_RATE};

/// Runs the window until it is closed or Esc is pressed in the game.
///
/// # Errors
/// When the event loop cannot be created or exits with an error, or when the window
/// or renderer could not be created.
pub fn run(
    frontend: Frontend,
    sprites: Option<Arc<SpriteAssets>>,
    options: Options,
) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(frontend, sprites, options);
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
    /// The game's screen size the frame is drawn at.
    pub resolution: Resolution,
    /// A window of `resolution` instead of the game's default borderless full screen.
    pub windowed: bool,
    pub title: String,
}

/// Window and renderer, created on `resumed`.
struct View {
    window: Arc<Window>,
    renderer: FrontendRenderer<WgpuRenderer>,
}

struct App {
    frontend: Frontend,
    clock: FrameClock,
    layers: RoomLayers,
    /// The slugcat's atlas; without it the body is drawn as circles.
    sprites: Option<Arc<SpriteAssets>>,
    /// Show the level graphic (V toggles) rather than the debug tiles.
    show_visuals: bool,
    view: Option<View>,
    /// `--screenshot`: save a frame here once a second has been simulated, then quit.
    screenshot_and_quit: Option<PathBuf>,
    resolution: Resolution,
    windowed: bool,
    title: String,
    /// The current process and the frames it has run, for `--screenshot` of a menu.
    process_frames: (Option<ProcessId>, u64),
    /// Quit after the current frame is presented.
    quit_after_frame: bool,
    /// Error that ended the loop, reported by [`run`].
    fatal: Option<Box<dyn Error>>,
}

impl App {
    fn new(frontend: Frontend, sprites: Option<Arc<SpriteAssets>>, options: Options) -> Self {
        Self {
            sprites,
            show_visuals: !options.debug_view,
            frontend,
            clock: FrameClock::default(),
            layers: RoomLayers::default(),
            view: None,
            screenshot_and_quit: options.screenshot,
            resolution: options.resolution,
            windowed: options.windowed,
            title: options.title,
            process_frames: (None, 0),
            quit_after_frame: false,
            fatal: None,
        }
    }

    /// A borderless full-screen window (the game's default, `fullscreenMode`
    /// FullScreenWindow), or with `windowed` a window of the game's resolution in
    /// physical pixels, not resizable (`resizableWindow: 0`). Either way the frame is
    /// drawn at the resolution and scaled to the window.
    fn create_view(&self, event_loop: &ActiveEventLoop) -> Result<View, Box<dyn Error>> {
        let (width, height) = self.resolution.size();
        let attrs = Window::default_attributes()
            .with_title(format!("rainworld-rs - {}", self.title))
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
        let renderer = FrontendRenderer::new(SceneRenderer::new(renderer, self.sprites.clone()));
        Ok(View { window, renderer })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Box<dyn Error>) {
        self.fatal = Some(error);
        event_loop.exit();
    }

    fn in_game(&self) -> bool {
        self.frontend.game().is_some()
    }

    fn on_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        let pressed = event.state == ElementState::Pressed;
        if let Some(action) = Action::from_key(code) {
            self.frontend.ctx.input.set(action, pressed);
        }
        if let Some(key) = KeyboardKey::from_winit(code) {
            self.frontend.ctx.menu.input.set_key(key, pressed);
        }
        if !pressed || event.repeat {
            return;
        }
        match Command::from_key(code) {
            // In the menus Esc is UI cancel.
            Some(Command::Quit) if self.in_game() => {
                log::info!("quit (Esc)");
                event_loop.exit();
            }
            Some(Command::Respawn) => {
                if let Some(game) = self.frontend.game_mut() {
                    game.scene.game_mut().respawn();
                }
            }
            Some(Command::ToggleView) => {
                self.show_visuals = !self.show_visuals;
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
                let path = PathBuf::from(format!("screenshot-{}.ppm", self.process_frames.1));
                log::info!("saving screenshot to {}", path.display());
                if let Some(view) = &mut self.view {
                    view.renderer.scene.renderer.capture_next_frame(path);
                }
            }
            _ => {}
        }
    }

    /// The cursor in stage pixels (y up), through the frame's place in the window.
    fn on_cursor(&mut self, pos: PhysicalPosition<f64>) {
        let Some(view) = &self.view else { return };
        let window = view.window.inner_size();
        let frame = self.resolution.size();
        let Some(rect) = PresentRect::fit(frame, (window.width, window.height)) else {
            return;
        };
        let [w, h] = self.frontend.ctx.menu.screen_size;
        let x = (pos.x as f32 - rect.x as f32) * w / rect.width as f32;
        let y = h - (pos.y as f32 - rect.y as f32) * h / rect.height as f32;
        self.frontend.ctx.menu.input.mouse.pos = [x, y];
    }

    /// Whether a `--screenshot` run has simulated enough: a second (and the script)
    /// of the game, or a second of a menu with its fade finished.
    fn screenshot_due(&self) -> bool {
        match self.frontend.game() {
            Some(game) => {
                let script = self.frontend.ctx.script.as_ref().map_or(0, Script::len);
                game.scene.game().ticks() >= u64::from(TICK_RATE.max(script))
            }
            None => {
                self.frontend.manager.fade_to_black() == 0.0
                    && self.process_frames.1 >= u64::from(TICK_RATE)
            }
        }
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let dt = self.clock.delta(Instant::now());
        self.frontend.update(dt);
        let id = self.frontend.current_id();
        if self.process_frames.0 == id {
            self.process_frames.1 += 1;
        } else {
            self.process_frames = (id, 0);
        }
        let screenshot = self
            .screenshot_due()
            .then(|| self.screenshot_and_quit.take())
            .flatten();
        let Some(view) = &mut self.view else { return };
        let scene_view = SceneView {
            level: self.show_visuals,
            layers: self.layers,
            framing: Framing::Auto,
        };
        self.frontend.draw(&mut view.renderer, scene_view);
        let r = &mut view.renderer.scene.renderer;
        if let Some(path) = screenshot {
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
                    view.renderer.scene.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::Focused(false) => {
                self.frontend.ctx.input.clear();
                self.frontend.ctx.menu.input.clear();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event_loop, &event),
            WindowEvent::CursorMoved { position, .. } => self.on_cursor(position),
            WindowEvent::MouseInput { state, button, .. } => {
                let index = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    _ => return,
                };
                self.frontend.ctx.menu.input.mouse.buttons[index] = state == ElementState::Pressed;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.0,
                };
                self.frontend.ctx.menu.input.mouse.scroll += lines;
            }
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
        let ticks = self.frontend.game().map_or(0, |g| g.scene.game().ticks());
        log::info!("exiting after {ticks} game ticks");
    }
}
