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

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct LoadRoomParams {
    /// Room name as the game looks it up, e.g. "SU_A22", "summit", "GATE_SU_HI". Its
    /// region's world is loaded around it (arena levels and rooms outside their
    /// world's room list get a single-room world).
    pub name: String,
    /// The slugcat to play: "White" (the Survivor, default), "Yellow" or "Red".
    pub slugcat: Option<String>,
    /// Seed of the game's random stream (`Random.InitState`, low 32 bits; default 1):
    /// spawn and every later draw. Same seed and inputs give the same run.
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
    /// Map button held (shows the HUD's meters and, after 30 ticks, the region map;
    /// the slugcat stands still). While it is held x/y pan the map, jump and throw
    /// change its layer.
    #[serde(default)]
    pub map: bool,
    /// Pick-up button held (Take, left Shift): picks up, drops, eats.
    #[serde(default)]
    pub pickup: bool,
    /// Throw button held (X).
    #[serde(default)]
    pub throw: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct HudMessageParams {
    /// The text (shown as given).
    pub text: String,
    /// Ticks to wait before showing (default 0).
    pub wait: Option<i32>,
    /// Ticks to show once faded in (default 160).
    pub time: Option<i32>,
    /// Darken the screen behind it (default true, as the tutorials do).
    pub darken: Option<bool>,
    /// Hide the meters while it shows (default true).
    pub hide_hud: Option<bool>,
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

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct NewGameParams {
    /// The campaign: "White" (Survivor), "Yellow" (Monk) or "Red" (Hunter);
    /// case-insensitive, the campaign names work too.
    pub slugcat: String,
    /// Seed of the game's random stream (default 1).
    pub seed: Option<u64>,
    /// Start as a saved story in a malnourished cycle (`saveState.malnourished`), at
    /// the start den (a dev aid; default false).
    pub malnourished: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct RespawnParams {
    /// Restart the last load_room / new_game with this seed (tick count restarts at
    /// 0); without it, respawn in the camera's room at the next seeded spawn tile as
    /// the R key does (not in a story started by new_game).
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
    /// Food pips in the stomach (`playerState.foodInStomach`, quarter pips cleared; a
    /// dev tool). Alone, it leaves the body where it is.
    pub food: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct SetDestinationParams {
    /// The realized lizard's EntityID [spawner, number], as get_state reports it.
    pub creature: [i32; 2],
    /// The destination's room name (default the lizard's room).
    pub room: Option<String>,
    /// The destination tile.
    pub x: i32,
    pub y: i32,
    /// The run speed its AI keeps (default 0.5; the behaviours that set it are not
    /// ported yet).
    pub run_speed: Option<f32>,
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
    /// With `crop_radius`: the square's centre in room pixels instead of the player's head.
    pub crop_center: Option<[f32; 2]>,
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

/// A process `load_process` can start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProcessParam {
    /// The intro roll, as the game boots (any key after a second, or 40 s, goes on to
    /// the main menu).
    #[default]
    IntroRoll,
    /// The main menu (STORY leads to the slugcat select menu).
    MainMenu,
    /// The slugcat select menu (hold START to begin a new game at the campaign's
    /// start).
    SlugcatSelect,
    /// The options menu (its import button copies the game's save of the selected
    /// slot).
    OptionsMenu,
    /// The M0 test screen: labels in both fonts over the flat main-menu scene.
    FontTest,
    /// The game, in the room's region world.
    Game,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct LoadProcessParams {
    /// Started by `Initialization` after the boot fade (default "intro_roll").
    pub process: Option<ProcessParam>,
    /// Room of the game process (default "summit").
    pub room: Option<String>,
    /// Seed of the game process (default 1).
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ImportSaveParams {
    /// The save slot: 0 (`sav`, default), 1 (`sav2`) or 2 (`sav3`).
    pub slot: Option<u8>,
    /// The game's save directory to read instead of the platform's.
    pub game_saves: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct StepProcessParams {
    /// Frames of 1/40 s to run (default 1). Each frame is one process manager update;
    /// at 40 frames per second a process ticks once per frame.
    pub frames: Option<u32>,
    /// Keys held for these frames, by Rewired name: "UpArrow", "DownArrow",
    /// "LeftArrow", "RightArrow", "Return", "Space", "Z", "X", "C", "Escape",
    /// "LeftShift", "S". They feed the menus (UI map) and, in the game, the player
    /// (gameplay map: arrows move, Z jumps); Escape skips a slideshow, S or Space held
    /// when START completes skips the intro.
    pub keys: Option<Vec<String>>,
}
