//! Window, event loop and frame pacing.
//!
//! Each redraw: advance the [`FixedTimestep`], run that many 40 Hz ticks (one
//! [`Input`](rw_sim::Input) per tick), then render with the leftover fraction as
//! the interpolation factor.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rw_render::{Camera, Renderer, RoomLayers, WgpuRenderer};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::game::Game;
use crate::input::{Action, Command, InputState};
use crate::timestep::{FixedTimestep, TICK_RATE};

/// Screen pixels kept free around the room.
const MARGIN: f32 = 16.0;

/// Runs the window until it is closed or Esc is pressed.
///
/// # Errors
/// When the event loop cannot be created or exits with an error, or when the window
/// or renderer could not be created.
pub fn run(game: Game, screenshot: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(game, screenshot);
    event_loop.run_app(&mut app)?;
    match app.fatal.take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Window and renderer, created on `resumed`.
struct View {
    window: Arc<Window>,
    renderer: WgpuRenderer,
}

struct App {
    game: Game,
    input: InputState,
    clock: FixedTimestep,
    last_frame: Option<Instant>,
    layers: RoomLayers,
    view: Option<View>,
    /// `--screenshot`: save a frame here once a second has been simulated, then quit.
    screenshot_and_quit: Option<PathBuf>,
    /// Quit after the current frame is presented.
    quit_after_frame: bool,
    /// Error that ended the loop, reported by [`run`].
    fatal: Option<Box<dyn Error>>,
}

impl App {
    fn new(game: Game, screenshot_and_quit: Option<PathBuf>) -> Self {
        Self {
            game,
            input: InputState::default(),
            clock: FixedTimestep::default(),
            last_frame: None,
            layers: RoomLayers::default(),
            view: None,
            screenshot_and_quit,
            quit_after_frame: false,
            fatal: None,
        }
    }

    fn create_view(&self, event_loop: &ActiveEventLoop) -> Result<View, Box<dyn Error>> {
        let room = self.game.room();
        let size = LogicalSize::new(
            (room.pixel_width() + 2.0 * MARGIN).clamp(640.0, 1600.0),
            (room.pixel_height() + 2.0 * MARGIN).clamp(480.0, 1000.0),
        );
        let attrs = Window::default_attributes()
            .with_title(format!("rainworld-rs - {}", room.name()))
            .with_inner_size(size);
        let window = Arc::new(event_loop.create_window(attrs)?);
        let physical = window.inner_size();
        let mut renderer =
            WgpuRenderer::new(Arc::clone(&window), (physical.width, physical.height))?;
        renderer.set_room(room);
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
                self.game.respawn();
                self.clock.reset();
            }
            Some(Command::ToggleGrid) => {
                self.layers.grid = !self.layers.grid;
                log::info!("grid {}", if self.layers.grid { "on" } else { "off" });
            }
            Some(Command::Screenshot) => {
                let path = PathBuf::from(format!("screenshot-{}.ppm", self.game.ticks()));
                log::info!("saving screenshot to {}", path.display());
                if let Some(view) = &mut self.view {
                    view.renderer.capture_next_frame(path);
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
            let input = self.input.take_tick_input();
            self.game.tick(input);
        }
        let alpha = self.clock.alpha();

        let Some(view) = &mut self.view else { return };
        let r = &mut view.renderer;
        let room = self.game.room();
        r.begin_frame(Camera::fit(
            [room.pixel_width(), room.pixel_height()],
            r.size(),
            MARGIN,
        ));
        r.draw_room(self.layers);
        r.draw_object(self.game.body(), alpha);
        if self.game.ticks() >= u64::from(TICK_RATE)
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
                    view.renderer.resize(size.width, size.height);
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
        log::info!("exiting after {} ticks", self.game.ticks());
    }
}
