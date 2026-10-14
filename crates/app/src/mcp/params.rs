//! Tool parameters; their doc comments become the JSON schema descriptions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ListRoomsParams {
    /// Only rooms whose name starts with this (case-insensitive), e.g. "su_a".
    pub prefix: Option<String>,
    /// Only rooms of this region: a world region prefix such as "su" or "hi",
    /// "gates", or "arena" (the `levels` directory).
    pub region: Option<String>,
    /// Most names returned (default 500).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LoadRoomParams {
    /// Room name as the game looks it up, e.g. "SU_A22", "summit", "GATE_SU_HI".
    pub name: String,
    /// Spawn RNG seed (default 1). Same seed and inputs give the same run.
    pub seed: Option<u64>,
    /// Spawn on this tile [x, y] (y up) instead of a seeded spawn tile.
    pub spawn_tile: Option<[i32; 2]>,
}

/// One tick of controller input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct InputParam {
    /// -1 left, 0, 1 right.
    #[serde(default)]
    pub x: i8,
    /// -1 down, 0, 1 up.
    #[serde(default)]
    pub y: i8,
    /// Jump button held.
    #[serde(default)]
    pub jump: bool,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct StepParams {
    /// Ticks to run at 40 Hz (default 1).
    pub ticks: Option<u32>,
    /// Input held for every tick (default: nothing pressed).
    pub input: Option<InputParam>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Step {
    /// Ticks to hold the input.
    pub ticks: u32,
    /// Default: nothing pressed.
    pub input: Option<InputParam>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct StepSequenceParams {
    /// Steps run in order.
    pub steps: Vec<Step>,
    /// Also return tick, head position, body mode and animation after each step.
    pub trace: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct RespawnParams {
    /// Restart the room's spawn sequence with this seed (tick count restarts at 0);
    /// without it, respawn at the next seeded spawn tile as the R key does.
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct SetPlayerParams {
    /// Tile [x, y] (y up) whose centre the body is put at.
    pub tile: Option<[i32; 2]>,
    /// Room-pixel position [x, y] (y up, 20 px per tile) of the body's middle.
    pub pos: Option<[f32; 2]>,
    /// Velocity [x, y] in px per tick for both chunks (default 0).
    pub vel: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ViewParam {
    /// The game's level graphic, framed by the game's room camera.
    #[default]
    Visuals,
    /// Debug tiles (solids, slopes, poles, shortcuts) of the whole room.
    Debug,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ScreenshotParams {
    /// "visuals" (default) or "debug".
    pub view: Option<ViewParam>,
    /// Draw tile boundary lines.
    pub grid: Option<bool>,
    /// Show only a square of this half-size in room pixels around the player's head.
    pub crop_radius: Option<f32>,
    /// Output pixels per room pixel (default 1, at most 4).
    pub scale: Option<f32>,
    /// Also write the PNG to this path.
    pub save_path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct TilesParams {
    /// Inclusive tile rectangle [x0, y0, x1, y1] (y up); default the whole room.
    pub rect: Option<[i32; 4]>,
}
