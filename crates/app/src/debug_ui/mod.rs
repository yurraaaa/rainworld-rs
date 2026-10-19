//! The windowed app's debug UI (a port-only dev tool, D18): a Dear ImGui window with a
//! tab per tool over the game, toggled with F1. It reads the game between frames and
//! queues [`DebugAction`]s, which the app applies before the next frame's ticks
//! through the code paths of the MCP tools. It never draws from the game's random
//! streams. Drawn by the renderer's overlay ([`rw_render::Overlay`]).

mod actions;
mod tabs;
#[cfg(test)]
mod tests;
mod view;

pub use actions::DebugAction;
pub use view::DebugView;
#[cfg(test)]
pub use view::GameView;

use imgui::{Condition, ConfigFlags, DrawCmd, DrawData, FontSource, TextureId};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use rw_render::{Overlay, OverlayDraw, OverlayVertex};
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

use crate::room_catalog::RoomCatalog;

/// What a tab reads and where it queues its actions.
pub struct TabCx<'a> {
    pub view: &'a DebugView,
    /// The rooms on disk, when the world directory could be read.
    pub catalog: Option<&'a RoomCatalog>,
    pub actions: &'a mut Vec<DebugAction>,
}

/// One tool of the tab bar.
pub trait DebugTab {
    fn title(&self) -> &'static str;
    fn draw(&mut self, ui: &imgui::Ui, cx: &mut TabCx<'_>);
}

/// The font atlas to upload as the overlay's texture.
pub struct FontTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct DebugUi {
    imgui: imgui::Context,
    platform: Option<WinitPlatform>,
    open: bool,
    tabs: Vec<Box<dyn DebugTab>>,
    catalog: Option<RoomCatalog>,
    /// The last action's outcome.
    status: Option<Result<String, String>>,
    font: Option<FontTexture>,
}

impl DebugUi {
    /// A closed UI with the tools' tabs; `catalog` lists the rooms on disk.
    #[must_use]
    pub fn new(catalog: Option<RoomCatalog>) -> Self {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        imgui.set_log_filename(None);
        imgui
            .io_mut()
            .config_flags
            .insert(ConfigFlags::NO_MOUSE_CURSOR_CHANGE);
        imgui
            .fonts()
            .add_font(&[FontSource::DefaultFontData { config: None }]);
        let atlas = imgui.fonts().build_rgba32_texture();
        let font = FontTexture {
            width: atlas.width,
            height: atlas.height,
            rgba: atlas.data.to_vec(),
        };
        imgui.fonts().tex_id = TextureId::new(0);
        Self {
            imgui,
            platform: None,
            open: false,
            tabs: tabs::all(),
            catalog,
            status: None,
            font: Some(font),
        }
    }

    /// Takes winit's events and cursor for `window` from now on.
    pub fn attach(&mut self, window: &Window) {
        let mut platform = WinitPlatform::new(&mut self.imgui);
        platform.attach_window(self.imgui.io_mut(), window, HiDpiMode::Default);
        self.platform = Some(platform);
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// The font atlas, once, for the renderer.
    pub fn take_font_texture(&mut self) -> Option<FontTexture> {
        self.font.take()
    }

    /// Whether a key press goes to the UI only: it is open and takes the keyboard (a
    /// text field has focus). Releases and F1 always go on, so no key stays held.
    #[must_use]
    pub fn captures_key(&self, event: &KeyEvent) -> bool {
        self.open
            && self.imgui.io().want_capture_keyboard
            && event.state == ElementState::Pressed
            && event.physical_key != PhysicalKey::Code(KeyCode::F1)
    }

    /// Whether mouse presses and scrolling go to the UI only: it is open and the mouse
    /// is over it (releases always go on).
    #[must_use]
    pub fn captures_mouse(&self) -> bool {
        self.open && self.imgui.io().want_capture_mouse
    }

    /// Feeds a window event to the UI.
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) {
        let Some(platform) = &mut self.platform else {
            return;
        };
        let event: Event<()> = Event::WindowEvent {
            window_id: window.id(),
            event: event.clone(),
        };
        platform.handle_event(self.imgui.io_mut(), window, &event);
    }

    /// Shows the outcome of an applied action.
    pub fn set_status(&mut self, status: Result<String, String>) {
        self.status = Some(status);
    }

    /// One UI frame of `dt` seconds over a window of `physical` pixels at `scale`
    /// physical pixels per logical one: the overlay to draw and the actions asked for.
    pub fn frame(
        &mut self,
        view: &DebugView,
        window: Option<&Window>,
        physical: (u32, u32),
        scale: f32,
        dt: f32,
    ) -> (Overlay, Vec<DebugAction>) {
        let io = self.imgui.io_mut();
        io.update_delta_time(std::time::Duration::from_secs_f32(dt.max(1e-4)));
        io.display_size = [physical.0 as f32 / scale, physical.1 as f32 / scale];
        io.display_framebuffer_scale = [scale, scale];
        if let (Some(platform), Some(window)) = (&self.platform, window) {
            let _ = platform.prepare_frame(self.imgui.io_mut(), window);
        }
        let mut actions = Vec::new();
        let ui = self.imgui.new_frame();
        let mut cx = TabCx {
            view,
            catalog: self.catalog.as_ref(),
            actions: &mut actions,
        };
        ui.window("Debug (F1)")
            .position([10.0, 10.0], Condition::FirstUseEver)
            .size([520.0, 440.0], Condition::FirstUseEver)
            .build(|| {
                if let Some(status) = &self.status {
                    match status {
                        Ok(text) => ui.text(text),
                        Err(text) => ui.text_colored([1.0, 0.45, 0.4, 1.0], text),
                    }
                    ui.separator();
                }
                if let Some(_bar) = ui.tab_bar("tools") {
                    for tab in &mut self.tabs {
                        if let Some(_item) = ui.tab_item(tab.title()) {
                            tab.draw(ui, &mut cx);
                        }
                    }
                }
            });
        if let (Some(platform), Some(window)) = (&mut self.platform, window) {
            platform.prepare_render(ui, window);
        }
        let overlay = Overlay::from_draw_data(self.imgui.render());
        (overlay, actions)
    }
}

/// An overlay from Dear ImGui's draw data.
pub trait FromDrawData {
    fn from_draw_data(data: &DrawData) -> Self;
}

impl FromDrawData for Overlay {
    /// The draw lists in physical pixels, one index buffer with each list's indices
    /// offset to its vertices; every draw samples the font atlas.
    fn from_draw_data(data: &DrawData) -> Self {
        let origin = data.display_pos;
        let [sx, sy] = data.framebuffer_scale;
        let mut overlay = Overlay::default();
        for list in data.draw_lists() {
            let base = u32::try_from(overlay.vertices.len()).unwrap_or(u32::MAX);
            let first = u32::try_from(overlay.indices.len()).unwrap_or(u32::MAX);
            overlay
                .vertices
                .extend(list.vtx_buffer().iter().map(|v| OverlayVertex {
                    pos: [(v.pos[0] - origin[0]) * sx, (v.pos[1] - origin[1]) * sy],
                    uv: v.uv,
                    color: v.col,
                }));
            overlay
                .indices
                .extend(list.idx_buffer().iter().map(|&i| base + u32::from(i)));
            for cmd in list.commands() {
                let DrawCmd::Elements { count, cmd_params } = cmd else {
                    continue;
                };
                let [x0, y0, x1, y1] = cmd_params.clip_rect;
                let offset = u32::try_from(cmd_params.idx_offset).unwrap_or(u32::MAX);
                overlay.draws.push(OverlayDraw {
                    first: first + offset,
                    count: u32::try_from(count).unwrap_or(0),
                    clip: [
                        (x0 - origin[0]) * sx,
                        (y0 - origin[1]) * sy,
                        (x1 - origin[0]) * sx,
                        (y1 - origin[1]) * sy,
                    ],
                });
            }
        }
        overlay
    }
}
