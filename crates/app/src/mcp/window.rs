//! `--mcp --window`: a window that shows the scene the MCP tools drive. It never steps
//! the simulation; V toggles the level graphic / debug view, G the grid, Esc quits.

use std::error::Error;
use std::sync::{Arc, Mutex};

use rw_render::{Camera, Renderer, Resolution, RoomLayers, WgpuRenderer};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use super::{Server, Session};
use crate::input::Command;
use crate::scene::{Framing, SceneRenderer, SceneView};

/// Serves MCP on a background thread and shows the session's scene until the window
/// closes or the client disconnects.
///
/// # Errors
/// When the event loop, window or renderer cannot be created.
pub fn run(session: Arc<Mutex<Session>>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<McpEnded>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let proxy = event_loop.create_proxy();
    let server = Server::new(Arc::clone(&session));
    std::thread::spawn(move || {
        if let Err(e) = server.serve_stdio() {
            log::error!("MCP server failed: {e}");
        }
        // The loop may already be gone if the window was closed first.
        let _ = proxy.send_event(McpEnded);
    });
    let mut mirror = Mirror {
        session,
        view: None,
        level: true,
        layers: RoomLayers::default(),
        fatal: None,
    };
    event_loop.run_app(&mut mirror)?;
    mirror.fatal.map_or(Ok(()), Err)
}

/// The MCP client disconnected.
#[derive(Debug)]
struct McpEnded;

struct Mirror {
    session: Arc<Mutex<Session>>,
    view: Option<(Arc<Window>, SceneRenderer<WgpuRenderer>)>,
    level: bool,
    layers: RoomLayers,
    fatal: Option<Box<dyn Error>>,
}

impl Mirror {
    fn create_view(
        &self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(Arc<Window>, SceneRenderer<WgpuRenderer>), Box<dyn Error>> {
        // Windowed, so a driven session never covers the screen.
        let resolution = Resolution::DEFAULT;
        let attrs = Window::default_attributes()
            .with_title("rainworld-rs - MCP")
            .with_inner_size(PhysicalSize::new(resolution.width, resolution.height))
            .with_resizable(false);
        let window = Arc::new(event_loop.create_window(attrs)?);
        let size = window.inner_size();
        let renderer =
            WgpuRenderer::new(Arc::clone(&window), (size.width, size.height), resolution)?;
        let sprites = self.session.lock().map_err(|e| e.to_string())?.sprites();
        Ok((window, SceneRenderer::new(renderer, sprites)))
    }

    fn redraw(&mut self) -> Result<(), Box<dyn Error>> {
        let Some((_, target)) = &mut self.view else {
            return Ok(());
        };
        let mut session = self.session.lock().map_err(|e| e.to_string())?;
        match session.scene_mut() {
            Some(scene) => {
                let view = SceneView {
                    level: self.level,
                    layers: self.layers,
                    framing: Framing::Auto,
                };
                scene.draw(target, 1.0, view);
            }
            None => {
                target
                    .renderer
                    .begin_frame(Camera::fit([1.0, 1.0], target.renderer.size(), 0.0))
            }
        }
        drop(session);
        target.renderer.present()?;
        Ok(())
    }
}

impl ApplicationHandler<McpEnded> for Mirror {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.view.is_some() {
            return;
        }
        match self.create_view(event_loop) {
            Ok(view) => self.view = Some(view),
            Err(e) => {
                self.fatal = Some(e);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: McpEnded) {
        log::info!("MCP client disconnected; closing the window");
        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some((_, target)) = &mut self.view {
                    target.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                match Command::from_key(code) {
                    Some(Command::Quit) => event_loop.exit(),
                    Some(Command::ToggleView) => self.level = !self.level,
                    Some(Command::ToggleGrid) => self.layers.grid = !self.layers.grid,
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.redraw() {
                    self.fatal = Some(e);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some((window, _)) = &self.view {
            window.request_redraw();
        }
    }
}
