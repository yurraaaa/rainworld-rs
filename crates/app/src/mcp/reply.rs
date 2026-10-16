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
    /// The guide's `HologramLight`s (the Monk's light in dark rooms).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub holo_lights: Vec<HoloLightState>,
    /// `HologramLight.Needed(player)` now.
    pub holo_light_needed: f32,
    /// What the session keeps of its deaths.
    pub session: SessionInfo,
    /// Each room's `entitiesInDens` (rooms with none left out).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dens: Vec<RoomDens>,
    /// Each room's `quantifiedCreatures` with a count above 0.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quantified: Vec<QuantifiedInfo>,
    /// `FliesWorldAI`'s spawn and respawn counters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flies_world: Option<FliesWorldInfo>,
    /// The world's `regionState`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_state: Option<RegionStateInfo>,
}

/// A `HologramLight`.
#[derive(Debug, Clone, Serialize)]
pub struct HoloLightState {
    pub room: String,
    /// The projecting overseer's EntityID.
    pub overseer: [i32; 2],
    pub pos: [f32; 2],
    pub proj_pos: [f32; 2],
    pub power: f32,
    pub flicker: f32,
    pub out_of_range_counter: i32,
    pub not_needed_counter: i32,
    pub active_linger: i32,
    pub respawn: bool,
    pub power_down_and_kill: bool,
    /// Its `LightSource`, once made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_source: Option<LightSourceState>,
}

/// A `LightSource` (pos, rad and alpha as last drawn).
#[derive(Debug, Clone, Serialize)]
pub struct LightSourceState {
    pub pos: [f32; 2],
    pub rad: f32,
    pub alpha: f32,
    /// After the setter's division by its largest component.
    pub color: [f32; 3],
    /// Not slated for deletion.
    pub alive: bool,
}

/// One room's den contents, by EntityID.
#[derive(Debug, Clone, Serialize)]
pub struct RoomDens {
    pub room: String,
    pub creatures: Vec<[i32; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuantifiedInfo {
    pub room: String,
    pub node: i32,
    /// `CreatureTemplate.Type`.
    pub kind: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FliesWorldInfo {
    /// `fliesToRespawn`.
    pub to_respawn: i32,
    /// `fliesToSpawn`, per swarm room.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub to_spawn: Vec<i32>,
    pub all_initial_spawned: bool,
}

/// `RegionState`'s counters (empty lists left out).
#[derive(Debug, Clone, Serialize)]
pub struct RegionStateInfo {
    pub last_cycle_updated: i32,
    pub new_rooms_this_cycle: i32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub swarm_room_counters: Vec<(String, i32)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lineage_counters: Vec<(String, i32)>,
    /// Each consumed item: origin room, placed object index, wait cycles.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub consumed_items: Vec<(String, i32, i32)>,
    pub rooms_visited: usize,
    /// Saved creature, object and stick strings.
    pub saved_population: usize,
    pub saved_objects: usize,
    pub saved_sticks: usize,
}

/// The first player's session kills and peace, and the cycle's respawn additions.
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    /// Each kill: `CreatureTemplate.Type` and `EntityID`.
    pub kills: Vec<(String, [i32; 2])>,
    pub peaceful: bool,
    /// Spawners added to `respawnCreatures` and `waitRespawnCreatures` this cycle.
    pub respawn: Vec<i32>,
    pub wait_respawn: Vec<i32>,
    /// The guide state's `likesPlayer` and `increaseLikeOnSave`.
    pub guide_likes_player: f32,
    pub guide_increase_like_on_save: bool,
    /// Community influences to apply at `Win`: community, influence.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub influences: Vec<(String, f32)>,
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
    /// `killTag`'s EntityID and `killTagCounter`.
    pub kill_tag: Option<[i32; 2]>,
    pub kill_tag_counter: i32,
    /// The realized creature's `stun`.
    pub stun: Option<i32>,
    /// The spear it is impaled on (`ImpaledOnSpearStick`), by EntityID.
    pub impaled_on: Option<[i32; 2]>,
    /// The abstract AI's shared fields (every roaming type).
    pub abstract_ai: Option<AbstractAiInfo>,
    /// An overseer's abstract AI and body.
    pub overseer: Option<OverseerInfo>,
    /// A realized bat fly's body and AI.
    pub fly: Option<FlyInfo>,
    /// A realized lizard's body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lizard: Option<LizardInfo>,
    /// The realized creature's danger to the player.
    pub danger: Option<DangerInfo>,
    /// `state.socialMemory`'s relationships.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub social: Vec<RelationshipInfo>,
    /// What its grip sticks carry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub carries: Vec<HeldInfo>,
}

/// `SocialMemory.Relationship`.
#[derive(Debug, Clone, Serialize)]
pub struct RelationshipInfo {
    /// The subject's EntityID.
    pub subject: [i32; 2],
    pub like: f32,
    pub temp_like: f32,
    pub fear: f32,
    pub temp_fear: f32,
    pub know: f32,
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
    /// The creatures impaled on a spear, by EntityID, with their place on it.
    pub impaled: Vec<([i32; 2], i32)>,
    /// The creature a spear is lodged in, by EntityID, with the chunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lodged_in: Option<([i32; 2], usize)>,
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

/// A realized lizard (`kill_tag` and `stun` are the creature entry's).
#[derive(Debug, Clone, Serialize)]
pub struct LizardInfo {
    /// The room it is in; `None` inside a shortcut.
    pub room: Option<String>,
    /// Head, middle and hips.
    pub chunks: Vec<LizardChunk>,
    /// `LizardState.health`, `throatHealth` and `limbHealth`.
    pub health: f32,
    pub throat_health: f32,
    pub limb_health: [f32; 4],
    pub dead: bool,
    pub in_shortcut: bool,
    /// `jawOpen`, `lastJawOpen` and `jawForcedShut`.
    pub jaw_open: f32,
    pub last_jaw_open: f32,
    pub jaw_forced_shut: f32,
    pub bite_delay: i32,
    /// `applyGravity` and `inAllowedTerrainCounter` (footing).
    pub apply_gravity: bool,
    pub in_allowed_terrain_counter: i32,
    pub turned_by_rock_counter: i32,
    pub lungs: f32,
    /// `effectColor` (RGB).
    pub effect_color: [f32; 3],
    /// The EntityID of what each grasp holds.
    pub grasps: Vec<Option<[i32; 2]>>,
    /// The EntityIDs of the creatures holding it.
    pub grabbed_by: Vec<[i32; 2]>,
    /// The spears lodged in it, by EntityID, with their chunk.
    pub spears: Vec<([i32; 2], usize)>,
    /// Its AI and movement.
    pub movement: LizardMoveInfo,
    /// Its graphics module, while its room is viewed (or was, and it has not left it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<LizardGraphicsInfo>,
}

/// A lizard's `LizardGraphics`.
#[derive(Debug, Clone, Serialize)]
pub struct LizardGraphicsInfo {
    pub culled: bool,
    /// `legsGrabbing` (the body's `LegsGripping`) and `noGripCounter`.
    pub legs_grabbing: i32,
    pub no_grip_counter: i32,
    pub limbs: Vec<LizardLimbInfo>,
    /// `head.pos` and `HeadRotation(1)` (degrees), `headDepthRotation`, `depthRotation`.
    pub head: [f32; 2],
    pub head_rotation: f32,
    pub head_depth_rotation: f32,
    pub depth_rotation: f32,
    /// `JawOpen` as the head draws it.
    pub jaw_open: f32,
    pub blink: f32,
    pub flicker: i32,
    pub white_flicker: i32,
    pub look_pos: [f32; 2],
    /// The cosmetics' kinds, in sprite order.
    pub cosmetics: Vec<String>,
    /// Its head light's id while it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<u64>,
}

/// One `LizardLimb`.
#[derive(Debug, Clone, Serialize)]
pub struct LizardLimbInfo {
    pub mode: String,
    pub pos: [f32; 2],
    /// `absoluteHuntPos` while it holds terrain or reaches for it.
    pub hunt_pos: [f32; 2],
    pub grab_pos: [f32; 2],
    pub grip_counter: i32,
    pub reaching_for_terrain: bool,
    pub currently_disabled: bool,
    pub flip: f32,
}

/// A realized lizard's path finder, stuck tracker and movement.
#[derive(Debug, Clone, Serialize)]
pub struct LizardMoveInfo {
    /// The path finder's destination and effectual destination; `None` without one.
    pub destination: Option<CoordInfo>,
    pub effectual_destination: Option<CoordInfo>,
    /// Its accessibility mapper is running.
    pub mapping: bool,
    pub forbidden_entrance_counter: i32,
    pub stranded: bool,
    /// `AI.runSpeed`, and the MCP `set_destination` override it starts from.
    pub run_speed: f32,
    pub dev_run_speed: Option<f32>,
    pub animation: String,
    /// `followingConnection`, `upcomingConnections` and `movementAnimation`.
    pub following: Option<ConnectionInfo>,
    pub upcoming: Vec<ConnectionInfo>,
    pub movement_animation: Option<MovementAnimationInfo>,
    pub grip_point: Option<[f32; 2]>,
    /// `LegsGripping` and `NoGripCounter` (fixed until the limbs exist).
    pub legs_gripping: i32,
    pub no_grip_counter: i32,
    pub time_spent_trying_this_move: i32,
    pub desperation_smoother: f32,
    pub body_wiggle_counter: i32,
    /// The stuck tracker's counter and `Utility()`.
    pub stuck_counter: i32,
    pub stuck_utility: f32,
    /// `enteringShortCut`.
    pub entering_shortcut: Option<[i32; 2]>,
    /// Its shortcut vessel while in the tubes.
    pub vessel: Option<VesselInfo>,
}

/// A `WorldCoordinate`.
#[derive(Debug, Clone, Serialize)]
pub struct CoordInfo {
    pub room: String,
    pub tile: [i32; 2],
    pub node: i32,
}

/// A `MovementConnection`.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub kind: String,
    pub start: [i32; 2],
    pub dest: [i32; 2],
    pub distance: i32,
}

/// A lizard's `MovementAnimation`.
#[derive(Debug, Clone, Serialize)]
pub struct MovementAnimationInfo {
    pub connection: ConnectionInfo,
    pub step: i32,
    pub total_steps: i32,
}

/// A creature's `ShortCutVessel`.
#[derive(Debug, Clone, Serialize)]
pub struct VesselInfo {
    pub room: String,
    pub pos: [i32; 2],
    /// `lastPositions`, newest first.
    pub last_positions: Vec<[i32; 2]>,
    pub wait: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LizardChunk {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
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
    /// `followCreature`'s EntityID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_creature: Option<[i32; 2]>,
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
    /// The realized body's `dead` and `dying` (0 to 1 as it dies away).
    pub dead: Option<bool>,
    pub dying: Option<f32>,
    /// The communication module's `mostDangerousCreatureInRoom` (EntityID) and its
    /// `CreatureDangerScore`.
    pub most_dangerous_creature: Option<[i32; 2]>,
    pub most_dangerous_creature_danger: Option<f32>,
}

/// What the threat music and the guide read of a realized creature.
#[derive(Debug, Clone, Serialize)]
pub struct DangerInfo {
    /// The template's `dangerousToPlayer`.
    pub dangerous_to_player: f32,
    pub danger_pos: [f32; 2],
    /// `CurrentPlayerAggression` (none without a realized AI).
    pub player_aggression: Option<f32>,
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
    /// The kill trophies (`SleepScreenKills`), when the cycle had kills.
    pub kills: Option<KillsState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KillsState {
    /// Each trophy's creature type, position on the page and whether it shows yet.
    pub trophies: Vec<(String, [f32; 2], bool)>,
    pub counted_and_done: bool,
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
