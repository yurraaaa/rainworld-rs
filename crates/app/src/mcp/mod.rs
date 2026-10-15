//! MCP server for `rainworld-rs --mcp`: tools that load a world, step the simulation
//! with given inputs, read the player's state and the tiles, and take screenshots, so
//! an agent can drive the app over stdio.
//!
//! Headless and deterministic: the simulation only advances inside `step` calls, with
//! no real-time clock, and screenshots render offscreen through the window's draw path
//! ([`crate::scene::Scene::draw`]). Logging goes to stderr; stdout carries only MCP.

#[cfg(test)]
mod overseer_tests;
mod params;
mod reply;
mod session;
#[cfg(test)]
mod tests;
mod tiles;
mod window;

use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};

use base64::Engine;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use rw_assets::{StreamingAssets, UnityResources};
use serde::Serialize;

use params::{
    HudMessageParams, ListRoomsParams, LoadProcessParams, LoadRoomParams, NewGameParams,
    RespawnParams, ScreenshotParams, SetPlayerParams, StepParams, StepProcessParams,
    StepSequenceParams, TilesParams,
};
pub use session::Session;

const INSTRUCTIONS: &str = "Drives rainworld-rs, a reimplementation of Rain World \
(one region at a time; of its creatures only the overseers behave, abstractly). Call load_room (a room in its region's world) or \
new_game (a campaign's start) first. The simulation runs at 40 ticks per second and only \
advances in step / step_sequence, so runs are deterministic: the same start, seed and \
inputs give the same result. Positions are room pixels, y up, 20 px per tile, in the \
room the camera shows (state.room); room-exit shortcuts carry the player to the next \
room and the camera follows. get_exits lists the room's exits and where they lead. Use \
get_tiles to read the geometry and screenshot to look at the scene. \
load_process instead starts the process manager (boot fade, then a menu screen or the \
game); step_process advances it frame by frame (1/40 s each), holding keys for the \
menus and the player, and screenshot draws it. Boot to play: load_process, step_process \
past the intro roll with a key (e.g. Z after its first second), STORY with Return, hold \
Z on START until the game runs, then step_process with arrows and Z.";

/// Serves MCP on stdin/stdout until the client disconnects; with `window`, also shows
/// the driven scene in a window.
///
/// # Errors
/// When the runtime, the transport or the window fails.
pub fn run(
    assets: StreamingAssets,
    resources: Arc<UnityResources>,
    window: bool,
) -> Result<(), Box<dyn Error>> {
    let session = Arc::new(Mutex::new(Session::new(assets, resources)));
    if window {
        window::run(session)
    } else {
        Server::new(session).serve_stdio()
    }
}

/// The MCP tool router over a shared [`Session`].
#[derive(Clone)]
pub struct Server {
    session: Arc<Mutex<Session>>,
    tool_router: ToolRouter<Self>,
}

impl Server {
    pub fn new(session: Arc<Mutex<Session>>) -> Self {
        Self {
            session,
            tool_router: Self::tool_router(),
        }
    }

    /// Blocks serving stdio on a single-threaded runtime.
    ///
    /// # Errors
    /// When the runtime cannot start or the MCP handshake fails.
    pub fn serve_stdio(self) -> Result<(), Box<dyn Error>> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            log::info!("MCP server on stdio");
            let service = self.serve(rmcp::transport::stdio()).await?;
            let reason = service.waiting().await?;
            log::info!("MCP session ended: {reason:?}");
            Ok(())
        })
    }

    fn session(&self) -> Result<MutexGuard<'_, Session>, String> {
        self.session
            .lock()
            .map_err(|_| "session state was poisoned by an earlier failure".to_owned())
    }

    fn json(value: &impl Serialize) -> Result<CallToolResult, String> {
        let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

#[tool_router]
impl Server {
    #[tool(description = "List room names (lower case), optionally by name prefix or region.")]
    fn list_rooms(
        &self,
        Parameters(p): Parameters<ListRoomsParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.list_rooms(&p)?)
    }

    #[tool(
        description = "Load a room's region world and spawn the player in that room (tick \
        0). Returns the room's size in tiles, cameras, water, shortcut count and the \
        initial state."
    )]
    fn load_room(
        &self,
        Parameters(p): Parameters<LoadRoomParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.load_room(&p)?)
    }

    #[tool(
        description = "Start a new story game as a slugcat (White, Yellow, Red): its start \
        room and position in its region's world. Returns the room info and state."
    )]
    fn new_game(&self, Parameters(p): Parameters<NewGameParams>) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.new_game(&p)?)
    }

    #[tool(
        description = "The camera room's shortcut nodes: kind, entrance tile, the input \
        that enters it, and for exits the destination room, arrival node and camera."
    )]
    fn get_exits(&self) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.exits()?)
    }

    #[tool(description = "Run ticks (40 per second) holding one input; returns the state after.")]
    fn step(&self, Parameters(p): Parameters<StepParams>) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.step(&p)?)
    }

    #[tool(
        description = "Run several {ticks, input} steps in order; returns the state after, \
        and with trace the head position, body mode and animation after each step."
    )]
    fn step_sequence(
        &self,
        Parameters(p): Parameters<StepSequenceParams>,
    ) -> Result<CallToolResult, String> {
        let (state, trace) = self.session()?.step_sequence(&p)?;
        if trace.is_empty() {
            Self::json(&state)
        } else {
            Self::json(&serde_json::json!({ "state": state, "trace": trace }))
        }
    }

    #[tool(
        description = "Respawn the player in the camera's room at the next seeded spawn \
        tile, or restart the last load_room / new_game with a seed."
    )]
    fn respawn(&self, Parameters(p): Parameters<RespawnParams>) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.respawn(&p)?)
    }

    #[tool(
        description = "Tick, world, camera room and camera, the player's room and node, \
        the vessel waiting to enter a room, realized rooms and their entry waits, and \
        player state: chunk positions, velocities, tiles and \
        contacts, body mode, animation, facing, standing, stun, death, shortcut travel, \
        this tick's terrain impacts, input and controller; the new game's fade delay and \
        SU_C04 start script; the world's creatures (kind, ID, room, node, tile, den, \
        realized; overseers' destination, guide state, last rooms and tiles)."
    )]
    fn get_state(&self) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.state()?)
    }

    #[tool(
        description = "Move the player to a tile centre or a pixel position, optionally with \
        a velocity, to set up a scenario. Movement state is kept."
    )]
    fn set_player(
        &self,
        Parameters(p): Parameters<SetPlayerParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.set_player(&p)?)
    }

    #[tool(
        description = "Render the current frame as the window shows it (level graphic or debug \
        tiles, with the slugcat; or the loaded process with its fade) and return it as a \
        PNG image."
    )]
    fn screenshot(
        &self,
        Parameters(p): Parameters<ScreenshotParams>,
    ) -> Result<CallToolResult, String> {
        let shot = self.session()?.screenshot(&p)?;
        let data = base64::engine::general_purpose::STANDARD.encode(&shot.png);
        let info = serde_json::json!({
            "width": shot.width,
            "height": shot.height,
            "saved": shot.saved,
        });
        Ok(CallToolResult::success(vec![
            ContentBlock::image(data, "image/png"),
            ContentBlock::text(info.to_string()),
        ]))
    }

    #[tool(
        description = "Start the process manager as the game boots (black, Initialization, \
        fade) into a process: intro_roll (default, as the game boots; any key goes on to \
        the main menu), main_menu (STORY leads to slugcat select, whose held START begins \
        a new game at the campaign's start), slugcat_select, font_test (labels in the \
        game's fonts over the flat main-menu scene; menu submit goes on to the game) or \
        game (in the room's region world). Replaces any loaded game."
    )]
    fn load_process(
        &self,
        Parameters(p): Parameters<LoadProcessParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.load_process(&p)?)
    }

    #[tool(
        description = "Run process manager frames of 1/40 s (one process tick each once \
        running), holding keys (held through all the frames, released after); the keys \
        drive the menus (Rewired UI map) and the player in the game (gameplay map: arrows \
        move, Z jumps; S held with START skips the intro slideshow, Escape skips it while it \
        plays); returns the current process, the fade state, the slideshow's clock and \
        scene, and whether Exit asked to quit."
    )]
    fn step_process(
        &self,
        Parameters(p): Parameters<StepProcessParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.step_process(&p)?)
    }

    #[tool(
        description = "The HUD (after the first tick of a load_room / new_game in a region \
        world): food meter position, fade and pips, karma meter position, fade, radius and \
        symbol, rain meter fade and circle count, the map's fade, the text prompt's \
        message and fade, and the tutorial controls page while it shows."
    )]
    fn get_hud(&self) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.hud_state()?)
    }

    #[tool(
        description = "Queue a message on the HUD's text prompt (TextPrompt.AddMessage, as \
        tutorials do: wait, time in ticks, darken, hide_hud); returns the HUD state."
    )]
    fn hud_message(
        &self,
        Parameters(p): Parameters<HudMessageParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.hud_message(&p)?)
    }

    #[tool(
        description = "Text map of the room's tiles in a rectangle (default the whole room), \
        one character per tile, top row first, with a legend."
    )]
    fn get_tiles(&self, Parameters(p): Parameters<TilesParams>) -> Result<CallToolResult, String> {
        let text = self.session()?.tiles(&p)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}
