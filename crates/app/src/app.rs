//! Window, event loop and frame pacing.
//!
//! Each redraw: advance the [`FixedTimestep`], run that many 40 Hz ticks (one
//! [`Input`](rw_sim::Input) per tick), then render with the leftover fraction as
//! the interpolation factor.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rw_graphics::Layer;
use rw_render::{Camera, Renderer, RoomCamera, RoomLayers, WaterSurface, WgpuRenderer};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::game::Game;
use crate::input::{Action, Command, InputState, Script};
use crate::sprites::SpriteAssets;
use crate::timestep::{FixedTimestep, TICK_RATE};
use crate::visuals::Visuals;

/// Screen pixels kept free around the room.
const MARGIN: f32 = 16.0;

/// Runs the window until it is closed or Esc is pressed.
///
/// # Errors
/// When the event loop cannot be created or exits with an error, or when the window
/// or renderer could not be created.
pub fn run(
    game: Game,
    visuals: Option<Visuals>,
    sprites: Option<SpriteAssets>,
    options: Options,
) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(game, visuals, sprites, options);
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
    /// The level graphic, when the room's images and palettes loaded.
    visuals: Option<Visuals>,
    /// The slugcat's atlas; without it the body is drawn as circles.
    sprites: Option<SpriteAssets>,
    /// Show the level graphic (V toggles) rather than the debug tiles.
    show_visuals: bool,
    /// Flat at the room's water row until the sim provides the wavy surface.
    water: Option<WaterSurface>,
    view: Option<View>,
    /// `--screenshot`: save a frame here once a second has been simulated, then quit.
    screenshot_and_quit: Option<PathBuf>,
    /// `--script`: replaces the keyboard until it runs out.
    script: Option<Script>,
    /// Quit after the current frame is presented.
    quit_after_frame: bool,
    /// Error that ended the loop, reported by [`run`].
    fatal: Option<Box<dyn Error>>,
}

impl App {
    fn new(
        game: Game,
        visuals: Option<Visuals>,
        sprites: Option<SpriteAssets>,
        options: Options,
    ) -> Self {
        Self {
            sprites,
            show_visuals: visuals.is_some() && !options.debug_view,
            visuals,
            water: WaterSurface::for_room(game.room()),
            game,
            input: InputState::default(),
            clock: FixedTimestep::default(),
            last_frame: None,
            layers: RoomLayers::default(),
            view: None,
            screenshot_and_quit: options.screenshot,
            script: options.script,
            quit_after_frame: false,
            fatal: None,
        }
    }

    fn create_view(&self, event_loop: &ActiveEventLoop) -> Result<View, Box<dyn Error>> {
        let room = self.game.room();
        let size = if self.visuals.is_some() {
            let [w, h] = RoomCamera::SCREEN_SIZE;
            LogicalSize::new(w, h)
        } else {
            LogicalSize::new(
                (room.pixel_width() + 2.0 * MARGIN).clamp(640.0, 1600.0),
                (room.pixel_height() + 2.0 * MARGIN).clamp(480.0, 1000.0),
            )
        };
        let attrs = Window::default_attributes()
            .with_title(format!("rainworld-rs - {}", room.name()))
            .with_inner_size(size);
        let window = Arc::new(event_loop.create_window(attrs)?);
        let physical = window.inner_size();
        let mut renderer =
            WgpuRenderer::new(Arc::clone(&window), (physical.width, physical.height))?;
        renderer.set_room(room);
        if let Some(sprites) = &self.sprites {
            sprites.upload(&mut renderer);
        }
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
            Some(Command::ToggleView) => {
                self.show_visuals = !self.show_visuals && self.visuals.is_some();
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
            let keys = self.input.take_tick_input();
            let input = self
                .script
                .as_mut()
                .and_then(Script::next_input)
                .unwrap_or(keys);
            self.game.tick(input);
            if let Some(visuals) = &mut self.visuals {
                visuals.tick(self.game.camera_follow().as_ref());
            }
        }
        // A frozen body is drawn where it stopped, not between its last two ticks.
        let alpha = if self.game.player_dead() {
            1.0
        } else {
            self.clock.alpha()
        };
        // The slugcat's sprites (also in the debug view), eyes in the palette's black.
        let face = self
            .visuals
            .as_ref()
            .map_or(rw_graphics::Color::BLACK, Visuals::black_color);
        let sprites = if self.sprites.is_some() {
            self.game.draw_list(alpha, face)
        } else {
            None
        };

        let Some(view) = &mut self.view else { return };
        let r = &mut view.renderer;
        let room = self.game.room();
        let time = (self.game.ticks() as f32 + alpha) / TICK_RATE as f32;
        let level = match &mut self.visuals {
            Some(visuals) if self.show_visuals => {
                visuals.upload(r);
                Some(visuals.view(alpha, time))
            }
            _ => None,
        };
        match &level {
            Some(level) => {
                r.begin_frame(Camera::window(level.origin, level.size, r.size()));
                r.draw_room(RoomLayers {
                    tiles: false,
                    ..self.layers
                });
            }
            None => {
                r.begin_frame(Camera::fit(
                    [room.pixel_width(), room.pixel_height()],
                    r.size(),
                    MARGIN,
                ));
                r.draw_room(self.layers);
            }
        }
        // Sprites sit at `room - camPos`; the debug view's stage is the room itself.
        let cam_pos = level.as_ref().map_or([0.0, 0.0], |l| l.origin);
        match &sprites {
            Some(list) => r.draw_sprites(list, Layer::Midground, cam_pos),
            None => {
                if let Some(body) = self.game.body() {
                    r.draw_object(body, alpha, self.game.status(), time);
                }
            }
        }
        if let Some(level) = level {
            r.draw_level(level);
        }
        if let Some(list) = &sprites {
            r.draw_sprites(list, Layer::Foreground, cam_pos);
        }
        r.draw_shortcut_vessel(self.game.vessel_tile());
        if let Some(water) = &self.water {
            r.draw_water(water);
        }
        let script_ticks = self.script.as_ref().map_or(0, Script::len);
        if self.game.ticks() >= u64::from(TICK_RATE.max(script_ticks))
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
