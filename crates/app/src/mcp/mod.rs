//! MCP server for `rainworld-rs --mcp`: tools that load a room, step the simulation
//! with given inputs, read the player's state and the tiles, and take screenshots, so
//! an agent can drive the app over stdio.
//!
//! Headless and deterministic: the simulation only advances inside `step` calls, with
//! no real-time clock, and screenshots render offscreen through the window's draw path
//! ([`crate::scene::Scene::draw`]). Logging goes to stderr; stdout carries only MCP.

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
use rw_assets::{AssetCache, StreamingAssets};
use serde::Serialize;

use params::{
    ListRoomsParams, LoadRoomParams, RespawnParams, ScreenshotParams, SetPlayerParams, StepParams,
    StepSequenceParams, TilesParams,
};
pub use session::Session;

const INSTRUCTIONS: &str = "Drives rainworld-rs, a reimplementation of Rain World's \
slugcat controller in single rooms. Call load_room first. The simulation runs at 40 ticks \
per second and only advances in step / step_sequence, so runs are deterministic: the \
same room, seed and inputs give the same result. Positions are room pixels, y up, 20 px \
per tile. Use get_tiles to read the geometry and screenshot to look at the scene.";

/// Serves MCP on stdin/stdout until the client disconnects; with `window`, also shows
/// the driven scene in a window.
///
/// # Errors
/// When the runtime, the transport or the window fails.
pub fn run(assets: StreamingAssets, cache: AssetCache, window: bool) -> Result<(), Box<dyn Error>> {
    let session = Arc::new(Mutex::new(Session::new(assets, cache)));
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
        description = "Load a room and spawn the player (tick 0). Returns size in tiles, \
        cameras, water, shortcut count and the initial state."
    )]
    fn load_room(
        &self,
        Parameters(p): Parameters<LoadRoomParams>,
    ) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.load_room(&p)?)
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

    #[tool(description = "Respawn the player: next seeded spawn tile, or restart with a seed.")]
    fn respawn(&self, Parameters(p): Parameters<RespawnParams>) -> Result<CallToolResult, String> {
        Self::json(&self.session()?.respawn(&p)?)
    }

    #[tool(
        description = "Tick, camera and player state: chunk positions, velocities, tiles and \
        contacts, body mode, animation, facing, standing, stun, death, shortcut travel, \
        this tick's terrain impacts and input."
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
        tiles, with the slugcat) and return it as a PNG image."
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
