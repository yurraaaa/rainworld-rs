//! Tool results, sent as JSON text.

use std::path::PathBuf;

use serde::Serialize;

use super::params::InputParam;

#[derive(Debug, Clone, Serialize)]
pub struct RoomList {
    /// Matching rooms before `limit`.
    pub count: usize,
    pub truncated: bool,
    pub rooms: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomInfo {
    pub name: String,
    /// In tiles.
    pub width: i32,
    pub height: i32,
    pub pixel_size: [f32; 2],
    /// Bottom-left corner of each camera image (1400x800 px) in room pixels.
    pub cameras: Vec<[f32; 2]>,
    /// Tile row of the water surface.
    pub water_level: Option<i32>,
    pub water_in_front: bool,
    pub shortcuts: usize,
    /// Whether the visuals view can show the game's level graphic.
    pub level_graphic: bool,
    pub state: State,
}

#[derive(Debug, Clone, Serialize)]
pub struct State {
    pub tick: u64,
    /// The world: region acronym, or the room of a single-room world.
    pub world: String,
    /// The room the camera shows (where the tile, screenshot and set_player tools act).
    pub room: String,
    /// Shown camera (0-based), when the room has a level graphic.
    pub camera: Option<usize>,
    /// The player's room (differs from `room` while its vessel waits to enter).
    pub player_room: String,
    /// The player's `abstractNode`: the exit node last passed, -1 if none.
    pub player_node: i32,
    /// The player's vessel waiting to enter another room, if it is.
    pub lobby: Option<LobbyState>,
    /// Realized rooms, with the ticks each still makes creatures wait to enter (-1 or
    /// 0: none).
    pub realized_rooms: Vec<RealizedState>,
    /// `None` once the body fell out of the room and was destroyed.
    pub player: Option<PlayerState>,
}

/// `betweenRoomsWaitingLobby`'s vessel.
#[derive(Debug, Clone, Serialize)]
pub struct LobbyState {
    pub room: String,
    /// The destination's node it arrives at.
    pub entrance_node: i32,
    /// Shortcut updates (every 3rd tick) it has waited.
    pub updates: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RealizedState {
    pub room: String,
    pub wait_to_enter: i32,
    pub ready_for_player: bool,
}

/// The camera room's exits (`get_exits`).
#[derive(Debug, Clone, Serialize)]
pub struct ExitList {
    pub room: String,
    pub exits: Vec<ExitInfo>,
}

/// One shortcut node of the room.
#[derive(Debug, Clone, Serialize)]
pub struct ExitInfo {
    /// Node index (exits first).
    pub node: usize,
    /// Exit, Den, RegionTransportation, ...
    pub kind: String,
    /// The entrance tile in the room (where the player goes in).
    pub entrance_tile: Option<[i32; 2]>,
    /// The way into the hole from the entrance: the input that enters it.
    pub enter_input: Option<[i32; 2]>,
    /// The room behind an exit node; `None` for an unconnected exit or a non-exit.
    pub destination: Option<String>,
    /// The destination's node the player arrives at.
    pub destination_node: Option<i32>,
    /// The destination's camera shown on arrival (that node's `viewedByCamera`).
    pub destination_camera: Option<i32>,
    /// This node's `viewedByCamera` here.
    pub viewed_by_camera: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerState {
    /// Head, then hips.
    pub chunks: Vec<ChunkState>,
    pub body_mode: String,
    pub animation: String,
    pub flip_direction: i32,
    pub standing: bool,
    pub stunned: bool,
    pub stun_ticks: i32,
    pub dead: bool,
    pub in_shortcut: bool,
    pub vessel_tile: Option<[i32; 2]>,
    /// Terrain impacts of the last tick.
    pub impacts: Vec<ImpactState>,
    /// Input of the last tick.
    pub input: InputParam,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkState {
    /// Room pixels, y up.
    pub pos: [f32; 2],
    /// Pixels per tick.
    pub vel: [f32; 2],
    pub tile: [i32; 2],
    /// Terrain contact this tick, each axis -1, 0 or 1.
    pub contact: [i8; 2],
    /// Downhill x direction of a touched floor slope, or 0.
    pub on_slope: i8,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactState {
    pub chunk: usize,
    pub direction: [i8; 2],
    pub speed: f32,
    pub first_contact: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepTrace {
    pub tick: u64,
    pub head: Option<[f32; 2]>,
    pub body_mode: Option<String>,
    pub animation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Screenshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub saved: Option<PathBuf>,
}

/// The process manager's state.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessState {
    /// Frames run since `load_process`.
    pub frame: u64,
    /// The current process, `None` between the two halves of a switch.
    pub process: Option<String>,
    pub upcoming: Option<String>,
    /// `fadeToBlack`: 1 is black.
    pub fade_to_black: f32,
    /// The fade sprite's alpha, while it is shown.
    pub fade_alpha: Option<f32>,
    /// The game's tick, when the game is the current process.
    pub game_tick: Option<u64>,
    /// The main menu's Exit was pressed (`Application.Quit`); the session keeps running.
    pub quit_requested: bool,
}
