//! Tool results, sent as JSON text.

use std::path::PathBuf;

use serde::Serialize;

pub use super::music_reply::{MusicState, TriggerState};

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
    /// `manager.FadeDelayInProgress` as the room scripts read it next tick (after
    /// `new_game`, the 3 s, or the Hunter's 5.5 s, of black before the fade-in).
    pub fade_delay_in_progress: bool,
    /// The camera room's `SU_C04StartUp`, when it has one.
    pub start_up: Option<StartUpState>,
    /// The world's creatures but the player (in dens or out, abstract or realized).
    pub creatures: Vec<CreatureInfo>,
    /// The world's physical objects other than creatures (rocks, spears).
    pub objects: Vec<ObjectInfo>,
    /// `world.rainCycle`.
    pub rain: RainState,
    /// The camera room's shelter door, when it has one.
    pub shelter_door: Option<ShelterDoorState>,
    /// The music player beside the game (its song requests, threat tracker).
    pub music: Option<MusicState>,
    /// The realized rooms' trigger checkers and the objects their events added.
    pub triggers: Vec<TriggerState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShelterDoorState {
    /// `closedFac`: 0 open, 1 shut.
    pub closed_fac: f32,
    /// `closeSpeed`: negative while opening, positive while closing.
    pub close_speed: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RainState {
    pub timer: i32,
    pub cycle_length: i32,
}

/// One `AbstractCreature`.
#[derive(Debug, Clone, Serialize)]
pub struct CreatureInfo {
    /// `CreatureTemplate.Type`.
    pub kind: String,
    /// `EntityID` (spawner, number).
    pub id: [i32; 2],
    pub room: String,
    pub node: i32,
    /// `pos.Tile`; `[-1, -1]` when undefined.
    pub tile: [i32; 2],
    pub in_den: bool,
    pub realized: bool,
    pub alive: bool,
    /// The abstract AI's shared fields (every roaming type).
    pub abstract_ai: Option<AbstractAiInfo>,
    /// An overseer's abstract AI and body.
    pub overseer: Option<OverseerInfo>,
    /// A realized bat fly's body and AI.
    pub fly: Option<FlyInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObjectInfo {
    /// `AbstractPhysicalObject.AbstractObjectType`.
    pub kind: String,
    /// `EntityID` (spawner, number).
    pub id: [i32; 2],
    pub room: String,
    pub node: i32,
    /// `pos.Tile`; `[-1, -1]` when undefined.
    pub tile: [i32; 2],
    /// A spear's `stuckInWallCycles` (> 0 horizontal, < 0 vertical, 0 free).
    pub stuck_in_wall_cycles: Option<i32>,
    /// A spear's `explosive`.
    pub explosive: Option<bool>,
    /// The realized rock or spear; `None` while abstract.
    pub realized: Option<WeaponInfo>,
}

/// A held or swallowed object.
#[derive(Debug, Clone, Serialize)]
pub struct HeldInfo {
    /// `AbstractObjectType` or `CreatureTemplate.Type`.
    pub kind: String,
    /// `EntityID` (spawner, number).
    pub id: [i32; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct WeaponInfo {
    pub room: String,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    /// `Weapon.Mode`.
    pub mode: String,
    /// `rotation` (a unit vector) and `rotationSpeed` (degrees per tick).
    pub rotation: [f32; 2],
    pub rotation_speed: f32,
    pub collision_layer: i32,
    /// A spear's wall position while stuck.
    pub stuck_in_wall: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlyInfo {
    /// The room it is in; `None` inside a shortcut.
    pub room: Option<String>,
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub dead: bool,
    pub in_shortcut: bool,
    /// `FlyAI.Behavior` and `Fly.MovementMode`.
    pub behavior: Option<String>,
    pub mov_mode: Option<String>,
    /// The EntityID of what each grasp holds (a fly's one grasp holds another fly in a
    /// chain).
    pub grasps: Vec<Option<[i32; 2]>>,
    /// The EntityIDs of the creatures holding it.
    pub grabbed_by: Vec<[i32; 2]>,
}

/// A room and node (`WorldCoordinate` without its tile).
#[derive(Debug, Clone, Serialize)]
pub struct NodeRef {
    pub room: String,
    pub node: i32,
}

/// `AbstractCreatureAI`'s state.
#[derive(Debug, Clone, Serialize)]
pub struct AbstractAiInfo {
    pub destination: NodeRef,
    /// `path`, the destination first and the next step last.
    pub path: Vec<NodeRef>,
    pub migration_destination: Option<NodeRef>,
    pub den_position: Option<NodeRef>,
    pub time_buffer: i32,
    pub remain_in_den_counter: i32,
    pub time_spent_here: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverseerInfo {
    pub player_guide: bool,
    /// `destination` room.
    pub destination: String,
    pub go_to_player: bool,
    pub player_guide_counter: i32,
    /// The rooms visited, newest first.
    pub last_rooms: Vec<String>,
    /// The realized body's hover and root tiles.
    pub hover_tile: Option<[i32; 2]>,
    pub root_tile: Option<[i32; 2]>,
    /// The realized body's `mode`, main chunk position and `extended`.
    pub mode: Option<String>,
    pub pos: Option<[f32; 2]>,
    pub extended: Option<f32>,
}

/// `SU_C04StartUp`'s state.
#[derive(Debug, Clone, Serialize)]
pub struct StartUpState {
    pub counter: i32,
    /// `pushRight[0]`: the body is still held and pushed right.
    pub push_right: bool,
    pub stood_up: bool,
    pub show_controls_counter: i32,
    /// The game would show its controls page now (needs the HUD; not ported).
    pub controls_page_due: bool,
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
    /// `controller` replacing the keys ("Null"), if set.
    pub controller: Option<String>,
    /// `playerState.foodInStomach` and `quarterFoodPoints`.
    pub food: u32,
    pub quarter_food: i32,
    pub max_food: u32,
    /// `Malnourished`, `aerobicLevel` and `exhausted`.
    pub malnourished: bool,
    pub aerobic_level: f32,
    pub exhausted: bool,
    /// What each hand holds.
    pub grasps: [Option<HeldInfo>; 2],
    /// `objectInStomach` (an untyped saved object: its save string as `kind`).
    pub stomach: Option<HeldInfo>,
    /// `swallowAndRegurgitateCounter`.
    pub swallow_counter: i32,
    /// `pickUpCandidate`.
    pub pick_up_candidate: Option<HeldInfo>,
    pub eat_counter: i32,
    pub sleep: SleepState,
}

/// The player's shelter sleep and hibernation counters.
#[derive(Debug, Clone, Serialize)]
pub struct SleepState {
    pub sleep_counter: i32,
    pub sleep_curl_up: f32,
    pub sleep_when_still: bool,
    pub touched_no_input_counter: i32,
    pub force_sleep_counter: i32,
    pub ready_for_win: bool,
    pub still_in_start_shelter: bool,
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
    /// The slideshow, when it is the current process.
    pub slideshow: Option<SlideShowState>,
    /// The save slot: its saved campaigns and the current save state.
    pub save: SaveInfo,
    /// What the running game started from, when the game is the current process.
    pub story: Option<StoryInfo>,
    /// The game's pause menu, while the game has one (`GamePaused`).
    pub pause_menu: Option<PauseMenuState>,
    /// The sleep, death or starve screen, when it is the current process.
    pub karma_screen: Option<KarmaScreenState>,
    /// The options (always) and the options menu, when it is the current process.
    pub options: OptionsState,
    /// `manager.musicPlayer`, once made.
    pub music: Option<MusicState>,
    /// The running game's trigger checkers and the objects their events added.
    pub triggers: Vec<TriggerState>,
}

/// `rainWorld.options` and the options menu.
#[derive(Debug, Clone, Serialize)]
pub struct OptionsState {
    pub save_slot: i32,
    pub resolution: i32,
    pub windowed: bool,
    pub music_volume: f32,
    pub sound_effects_volume: f32,
    pub arena_music_volume: f32,
    pub vibration: bool,
    pub validation: bool,
    pub language: String,
    /// The screen's size and mode as the window should be.
    pub screen: [f32; 2],
    pub screen_full: bool,
    pub cursor_visible: bool,
    /// The menu's info label.
    pub info: Option<String>,
    /// The open dialog's text.
    pub dialog: Option<String>,
}

/// A `PauseMenu`.
#[derive(Debug, Clone, Serialize)]
pub struct PauseMenuState {
    pub counter: i32,
    pub black_fade: f32,
    pub want_to_continue: bool,
    /// `ShutDownProcess` ran (after Exit): no longer drawn, the game still paused.
    pub shut_down: bool,
    /// The buttons' signals, positions and greying; the selected one.
    pub buttons: Vec<String>,
    pub selected: Option<String>,
}

/// A `SleepAndDeathScreen` and its `KarmaLadder`.
#[derive(Debug, Clone, Serialize)]
pub struct KarmaScreenState {
    pub phase: String,
    /// Karma and cap shown, and the karma the ladder moves to.
    pub display_karma: [i32; 2],
    pub move_to_karma: i32,
    pub reinforced: bool,
    pub animation_done: bool,
    pub force_watch_animation: bool,
    /// The map is held up and shown (`RevealMap`).
    pub reveal_map: bool,
    pub info: String,
    pub buttons: Vec<String>,
    pub selected: Option<String>,
    /// The end-game meters, in ladder order.
    pub meters: Vec<MeterState>,
    /// The passage tokens: their passage, fade and position on the page.
    pub tokens: Vec<TokenState>,
}

/// An `EndgameMeter`.
#[derive(Debug, Clone, Serialize)]
pub struct MeterState {
    pub passage: String,
    /// `float` or `notch`.
    pub kind: String,
    pub powered_on: bool,
    pub energy: f32,
    pub meter_animation: f32,
    pub show_as_fulfilled: f32,
    pub fulfilled_now: bool,
}

/// An `EndgameTokens.Token`.
#[derive(Debug, Clone, Serialize)]
pub struct TokenState {
    pub passage: String,
    pub fade: f32,
    pub pos: [f32; 2],
}

/// The menus' save slot.
#[derive(Debug, Clone, Serialize)]
pub struct SaveInfo {
    /// The slot file, if saves are on disk.
    pub path: Option<String>,
    /// The base campaigns with a stored game.
    pub saved: Vec<String>,
    /// The current save state, if a story game made one.
    pub current: Option<SaveStateInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveStateInfo {
    pub slugcat: String,
    pub den: String,
    pub cycle: i32,
    pub food: i32,
    pub karma: i32,
    pub karma_cap: i32,
    pub deaths: i32,
    pub quits: i32,
    pub loaded: bool,
}

/// A story game's start values and the player's room and food now.
#[derive(Debug, Clone, Serialize)]
pub struct StoryInfo {
    pub den: String,
    pub cycle: i32,
    pub karma: i32,
    pub karma_cap: i32,
    pub player_room: Option<String>,
    pub player_tile: Option<[i32; 2]>,
    pub food_in_stomach: u32,
}

/// What `import_save` did.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReply {
    pub source: String,
    pub destination: String,
    /// Where the port's previous file of the slot went.
    pub backup: Option<String>,
    /// The campaigns stored in the imported slot, as stored (DLC ones included).
    pub save_states: Vec<String>,
}

/// A `SlideShow`'s clock and scene.
#[derive(Debug, Clone, Serialize)]
pub struct SlideShowState {
    /// Playlist index and scene.
    pub current: Option<usize>,
    pub scene: Option<String>,
    /// Seconds of slideshow time, and the fraction of the scene's time gone.
    pub time: f32,
    pub in_scene_time: f32,
    /// Waiting for its song to be the current one.
    pub stall: bool,
    pub playback_speed_mult: f32,
    /// The scene's camera (`camPos`) and focus.
    pub cam_pos: Option<[f32; 2]>,
    pub focus: Option<f32>,
    /// The scene's illustrations' alphas: its depth layers, then its flats.
    pub alphas: Vec<f32>,
}

/// The HUD's state (`get_hud`).
#[derive(Debug, Clone, Serialize)]
pub struct HudState {
    pub food_meter: FoodMeterState,
    pub karma_meter: KarmaMeterState,
    pub rain_meter: RainMeterState,
    /// `HUD.Map`'s fade.
    pub map_fade: f32,
    pub map: MapState,
    pub show_karma_food_rain: bool,
    pub text_prompt: TextPromptState,
    /// The tutorial controls page, while it shows.
    pub controls_page: Option<ControlsPageState>,
}

/// `HUD.Map`: pan and layer, the reveal and the markers.
#[derive(Debug, Clone, Serialize)]
pub struct MapState {
    pub region: String,
    pub visible: bool,
    /// Map texture pixels within the shown layer.
    pub pan: [f32; 2],
    pub layer: i32,
    /// Discovery texture pixels waiting to be revealed.
    pub reveal_pending: usize,
    /// The player marker on screen.
    pub player_marker: Option<[f32; 2]>,
    /// Each gate and shelter marker: kind, room and fade.
    pub markers: Vec<MapMarkerState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapMarkerState {
    pub kind: String,
    pub room: i32,
    pub fade: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FoodMeterState {
    pub pos: [f32; 2],
    pub fade: f32,
    pub visible_counter: i32,
    pub show_count: i32,
    /// Quarter pips shown on the next pip (`QuarterPipShower.displayQuarterFood`).
    pub quarter_pips: i32,
    pub max_food: i32,
    pub survival_limit: i32,
    pub down_in_corner: f32,
    /// Each pip's x offset from the meter's position.
    pub pip_x: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KarmaMeterState {
    pub pos: [f32; 2],
    pub fade: f32,
    pub rad: f32,
    pub force_visible_counter: i32,
    pub symbol: String,
    pub reinforced: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RainMeterState {
    pub fade: f32,
    pub circles: usize,
    pub remain_visible_counter: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextPromptState {
    pub show: f32,
    pub showing: String,
    pub text: String,
    pub queued: usize,
    /// `gameOverMode`: the player died; the map button goes to the death screen.
    pub game_over: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlsPageState {
    pub counter: i32,
    pub black_fade: f32,
    pub want_to_continue: bool,
    pub labels: Vec<String>,
}
