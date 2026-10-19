//! The player: base slugcat movement on the two-chunk body (`Player`).
//!
//! Behaviour: `rainworld-re/notes/slugcat-movement.md` (tick order, input, body modes,
//! jumps), `horizontal-beams.md`, `vertical-poles.md`, `ledge-grab.md` and
//! `slope-collision.md` section 5. Stats are the session's `SlugcatStats` (the
//! Survivor's until [`Player::set_stats`]; no adrenaline); exhaustion and
//! `aerobicLevel` are in `exhaustion.rs`. Fall
//! damage, stun and death: `falls.md`. Skid, flip, roll, belly slide and rocket jump:
//! `advanced-moves.md`. Corridors: `corridors.md`. Shortcuts: `shortcuts.md`.
//! Swimming, breath and drowning: `water.md`.
//! What is not ported is listed in `docs/divergences/`.
//!
//! Mirrors: Player.cs

mod advanced;
mod animation;
mod body_mode;
mod corridor;
mod exhaustion;
mod falls;
mod grab;
mod input;
mod jump;
mod ledge;
mod movement;
mod poles;
mod shortcut;
mod sleep;
mod sounds;
mod swim;
#[cfg(test)]
mod tests;

use rw_assets::{Room, Terrain};
use rw_random::UnityRandom;

pub use grab::{GrabState, StomachObject};
pub use input::{INPUT_HISTORY, Input, PlayerController};
pub use shortcut::VesselStep;
pub use sleep::ShelterCheck;

use crate::campaign::{SlugcatCampaign, SlugcatStats};
use crate::room_ext::RoomExt;
use crate::shortcut::ShortcutVessel;
use crate::{
    BodyChunk, EmitterPlace, LoopEmitter, NoiseSink, PhysicalObject, SoundId, Sounds, Vec2, Water,
    WaterDrip, place_in_room,
};
use sounds::{QueuedNoise, QueuedSound};

/// Index of the head chunk (`bodyChunks[0]`, `mainBodyChunk`).
pub const HEAD: usize = 0;
/// Index of the hips chunk (`bodyChunks[1]`).
pub const HIPS: usize = 1;

/// Conscious player surface friction, re-applied every tick.
const SURFACE_FRICTION: f32 = 0.5;
/// Surface friction while stunned or dead.
const UNCONSCIOUS_SURFACE_FRICTION: f32 = 0.3;
/// `EffectiveRoomGravity`: room gravity 1 (no gravity effects are ported).
const ROOM_GRAVITY: f32 = 1.0;

/// Coarse movement state (`Player.BodyModeIndex`), recomputed every tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BodyMode {
    /// Airborne (or anything not otherwise classified).
    #[default]
    Default,
    /// On the ground, upright.
    Stand,
    /// On the ground, on all fours.
    Crawl,
    /// Head pressed against a wall the player pushes into.
    WallClimb,
    /// On a beam or pole (hanging, climbing, pulling up or standing on it).
    ClimbingOnBeam,
    /// Squeezed into a one-tile tunnel or shaft.
    CorridorClimb,
    /// In water (`DeepSwim` or `SurfaceSwim`).
    Swimming,
    /// Stun counter above 0.
    Stunned,
    Dead,
}

/// Fine movement state (`Player.AnimationIndex`); only the ported ones exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Animation {
    #[default]
    None,
    /// Turning around while crawling.
    CrawlTurn,
    /// Getting up from all fours.
    StandUp,
    /// Lowering to all fours.
    DownOnFours,
    /// Crawling up a one-tile step.
    LedgeCrawl,
    /// Hanging from a horizontal beam by the hands.
    HangFromBeam,
    /// Pulling up onto a horizontal beam.
    GetUpOnBeam,
    /// Standing on top of a horizontal beam.
    StandOnBeam,
    /// Hanging from a wall top by the head, hauling up.
    LedgeGrab,
    /// Climbing a vertical pole.
    ClimbOnBeam,
    /// Climbing over the top end of a vertical pole.
    GetUpToBeamTip,
    /// Standing on the top end of a vertical pole.
    BeamTip,
    /// Hanging from the bottom end of a vertical pole.
    HangUnderVerticalBeam,
    /// Somersault of the skid flip jump or the belly-slide whiplash jump.
    Flip,
    /// Rolling along the ground after a down-diagonal landing.
    Roll,
    /// Launched body-first out of a roll or belly slide.
    RocketJump,
    /// Sliding on the belly.
    BellySlide,
    /// Turning around inside a one-tile tunnel or shaft.
    CorridorTurn,
    /// Swimming under water with strokes and boosts.
    DeepSwim,
    /// Paddling at the surface, head held above it.
    SurfaceSwim,
    Dead,
}

impl Animation {
    /// The game's `AnimationIndex` spelling.
    pub fn value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::CrawlTurn => "CrawlTurn",
            Self::StandUp => "StandUp",
            Self::DownOnFours => "DownOnFours",
            Self::LedgeCrawl => "LedgeCrawl",
            Self::HangFromBeam => "HangFromBeam",
            Self::GetUpOnBeam => "GetUpOnBeam",
            Self::StandOnBeam => "StandOnBeam",
            Self::LedgeGrab => "LedgeGrab",
            Self::ClimbOnBeam => "ClimbOnBeam",
            Self::GetUpToBeamTip => "GetUpToBeamTip",
            Self::BeamTip => "BeamTip",
            Self::HangUnderVerticalBeam => "HangUnderVerticalBeam",
            Self::Flip => "Flip",
            Self::Roll => "Roll",
            Self::RocketJump => "RocketJump",
            Self::BellySlide => "BellySlide",
            Self::CorridorTurn => "CorridorTurn",
            Self::DeepSwim => "DeepSwim",
            Self::SurfaceSwim => "SurfaceSwim",
            Self::Dead => "Dead",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Some(match value {
            "None" => Self::None,
            "CrawlTurn" => Self::CrawlTurn,
            "StandUp" => Self::StandUp,
            "DownOnFours" => Self::DownOnFours,
            "LedgeCrawl" => Self::LedgeCrawl,
            "HangFromBeam" => Self::HangFromBeam,
            "GetUpOnBeam" => Self::GetUpOnBeam,
            "StandOnBeam" => Self::StandOnBeam,
            "LedgeGrab" => Self::LedgeGrab,
            "ClimbOnBeam" => Self::ClimbOnBeam,
            "GetUpToBeamTip" => Self::GetUpToBeamTip,
            "BeamTip" => Self::BeamTip,
            "HangUnderVerticalBeam" => Self::HangUnderVerticalBeam,
            "Flip" => Self::Flip,
            "Roll" => Self::Roll,
            "RocketJump" => Self::RocketJump,
            "BellySlide" => Self::BellySlide,
            "CorridorTurn" => Self::CorridorTurn,
            "DeepSwim" => Self::DeepSwim,
            "SurfaceSwim" => Self::SurfaceSwim,
            "Dead" => Self::Dead,
            _ => return None,
        })
    }
}

/// A slugcat: the two-chunk [`PhysicalObject`] plus the movement state machine.
///
/// Drive it with [`Player::update`] at 40 Hz. Chunk positions (and `last_pos` for
/// interpolation) are in `body.chunks`, head first.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    /// The physics body: chunk [`HEAD`] (radius 9) and chunk [`HIPS`] (radius 8).
    pub body: PhysicalObject,
    /// Input history, `input[0]` newest.
    input: [Input; INPUT_HISTORY],
    /// `controller`: replaces the given input while set.
    controller: Option<PlayerController>,
    body_mode: BodyMode,
    animation: Animation,
    /// Wants to be upright (toggled by up/down presses).
    standing: bool,
    /// Facing: -1 or +1.
    flip_direction: i32,
    /// Facing before this tick's input was applied.
    last_flip_direction: i32,
    /// Ticks of stun left (`Creature.stun`).
    stun: i32,
    /// The stun that last played `UI_Slugcat_Stunned_Init` (`lastStun`).
    last_stun: i32,
    dead: bool,
    /// The main chunk's position at the first `Die`, until the game takes it for
    /// `GameOver`.
    died_at: Option<Vec2>,
    /// Fell out of the room: removed from the game (`Destroy`).
    destroyed: bool,

    /// Ticks before a shortcut can be entered (`shortcutDelay`).
    shortcut_delay: i32,
    /// Entrance being pulled into (`enteringShortCut`).
    entering_shortcut: Option<(i32, i32)>,
    /// Growth of the pull-in snap radius (`shortCutRadAdd`).
    short_cut_rad_add: f32,
    /// Set while inside a tube; the body is then out of the room.
    vessel: Option<ShortcutVessel>,

    /// Coyote time: ticks left in which a jump is still allowed.
    can_jump: i32,
    /// Jump-press buffer: ticks left in which a press still triggers a jump.
    want_to_jump: i32,
    /// Remaining hold-to-jump-higher boost.
    jump_boost: f32,
    /// Wall-jump window; its sign is the jump direction.
    can_wall_jump: i32,
    /// After a wall jump: forced horizontal direction, counting toward 0.
    jump_stun: i32,
    wall_slide_counter: i32,
    no_grab_counter: i32,
    pole_skip_penalty: i32,
    force_feet_to_horizontal_beam_tile: i32,
    time_since_in_corridor_mode: i32,
    /// Ticks of slowed running and climbing, counting toward 0.
    slow_movement_stun: i32,
    /// Ticks left of the slide-up hop on a vertical pole.
    slide_up_pole: i32,
    ledge_grab_counter: i32,

    /// Anti-slide pin of the hips while standing still.
    feet_stuck_pos: Option<Vec2>,
    /// Per-chunk horizontal speed cap chosen this tick.
    dynamic_run_speed: [f32; 2],

    super_launch_jump: i32,
    kill_super_launch_jump_counter: i32,
    simulate_hold_jump_button: i32,
    crawl_turn_delay: i32,
    upper_body_frames_on_ground: i32,
    upper_body_frames_off_ground: i32,
    lower_body_frames_on_ground: i32,
    lower_body_frames_off_ground: i32,
    animation_frame: i32,
    consistent_down_diagonal: i32,
    dive_force: f32,

    pullup_softlock_safety: i32,
    straight_up_on_horizontal_beam: bool,
    up_on_horizontal_beam_pos: Vec2,

    /// Running direction a Stand skid starts from (`slideDirection`).
    slide_direction: i32,
    init_slide_counter: i32,
    /// Ticks into the turn-around skid; 0 when not skidding.
    slide_counter: i32,
    /// Direction of a roll or belly slide (and after a whiplash flip); 0 otherwise.
    roll_direction: i32,
    roll_counter: i32,
    stop_rolling_counter: i32,
    exit_belly_slide_counter: i32,
    /// `longBellySlide`: a slide restarted by a throw backwards out of it.
    long_belly_slide: bool,
    whiplash_jump: bool,
    flip_from_slide: bool,
    rocket_jump_from_belly_slide: bool,
    /// Ticks left in which a landing may turn into a roll (`allowRoll`).
    allow_roll: i32,

    /// Consecutive ticks wedged in a corridor; corridor mode starts above 2.
    go_into_corridor_climb: i32,
    /// Dropping out of the bottom of a shaft in `Default` mode.
    corridor_drop: bool,
    /// Direction of a running corridor U-turn.
    corridor_turn_dir: Option<(i32, i32)>,
    corridor_turn_counter: i32,
    /// Ticks left in which a corridor shoot or slide is a "success".
    can_corridor_jump: i32,
    vertical_corridor_slide_counter: i32,
    horizontal_corridor_slide_counter: i32,
    /// Moving tail-first in a corridor; above 10 it slows the climb.
    backwards_counter: i32,
    /// After a successful shaft shoot: no beam grabs while jump and up stay held.
    shoot_up_counter: i32,
    /// `privSneak`: how long it has been low or still on a pole, 0 to 1.
    priv_sneak: f32,
    /// Beam tile to catch after crawling head-first out of a shaft with down held.
    drop_grab_tile: Option<(i32, i32)>,
    /// Largest `Player.Blink(n)` this tick, for the graphics; 0 when none.
    blink_request: i32,
    /// `mapInput`: this tick's input before the map button stripped it.
    map_input: Input,
    /// `exitsToBeDiscovered`: the room-exit entrances of the first room the map
    /// discovery saw, never refilled (as in the game).
    pub exits_to_be_discovered: Option<Vec<Vec2>>,
    pub added_spawn_room_to_discovery: bool,

    /// Breath left: 1 full, 0 out of air (below 0 briefly while heading up).
    air_in_lungs: f32,
    /// `outOfBreathFac`: [shortness of breath, last tick's, pant phase, last tick's].
    out_of_breath: [f32; 4],
    /// Came up for air with less than a third left: slow recovery until full.
    lungs_exhausted: bool,
    /// The head was under water (submersion above 0.9) last tick.
    submerged: bool,
    /// Drowning progress: death at 1.
    drown: f32,
    /// Strength of the current swim stroke.
    swim_force: f32,
    /// Stroke phase: 0..4 under water, also drives the swimming poses.
    swim_cycle: f32,
    /// Ticks before the next surface jump or deep-swim boost.
    water_jump_delay: i32,
    /// `waterFriction`, set by the swim animations, used by next tick's physics.
    water_friction: f32,
    /// `waterRetardationImmunity` from the dive force, used by next tick's physics.
    water_retardation_immunity: f32,

    /// Which footstep sound is next (`leftFoot`).
    left_foot: bool,
    /// The sliding / stunned loop and its sound (`slideLoop`, `slideLoopSound`).
    slide_loop: Option<LoopEmitter>,
    slide_loop_sound: Option<SoundId>,
    /// Slide loops let go (`alive = false`), still in their room until their next
    /// update destroys them.
    released_slide_loops: Vec<LoopEmitter>,
    /// Where a new slide loop's emitter joins.
    emitter_place: EmitterPlace,
    /// `room.PlaySound` calls on a chunk made where no generator is at hand; their
    /// draws are made before the next draw of the tick (same order as the game).
    sound_queue: Vec<QueuedSound>,
    /// `room.InGameNoise` calls made where nobody can hear them yet, each after the
    /// sounds queued before it; heard at the update's last flush.
    noise_queue: Vec<QueuedNoise>,

    /// `sleepCounter`: above 0 waking up in a shelter, below 0 lying down; 0 awake.
    sleep_counter: i32,
    sleep_curl_up: f32,
    sleep_when_still: bool,
    touched_no_input_counter: i32,
    force_sleep_counter: i32,
    ready_for_win: bool,
    /// True until the first shortcut spits the player out.
    still_in_start_shelter: bool,
    /// `slugcatStats` (the session's `characterStats`).
    stats: SlugcatStats,
    /// `aerobicLevel`: 0 rested, 1 out of breath.
    aerobic_level: f32,
    /// `exhausted` (only ever set while malnourished in the base game).
    exhausted: bool,

    /// The counters `GrabUpdate` keeps.
    pub(crate) grab: GrabState,
    /// `objectInStomach`.
    stomach: Option<StomachObject>,
    /// `Stun` or `Die` let go of everything (`LoseAllGrasps`); the game applies it.
    release_grasps: bool,
    /// `MovementUpdate` ran this tick (so `GrabUpdate`, its last step, is due).
    movement_ran: bool,
    /// The pull toward a shortcut this tick: target and snap radius, for the objects
    /// carried along.
    shortcut_pull: Option<(Vec2, f32)>,
    /// `roomSettings.CeilingDrips` of the room it updates in.
    ceiling_drips: f32,
    /// `WaterDrip`s made this tick, for the room to add.
    drips: Vec<WaterDrip>,
    /// The body as it was when this tick's shortcut pull began, where `Creature.Update`
    /// runs the graphics module's extra update.
    before_pull: Option<Box<Player>>,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    /// A Survivor at the origin, as the `Player` constructor leaves it (not standing,
    /// `Default` body mode, no animation, full lungs) but always facing right; the game's
    /// constructor picks the facing at random ([`Player::create`]).
    #[must_use]
    pub fn new() -> Self {
        Self {
            body: PhysicalObject::slugcat(),
            input: [Input::NONE; INPUT_HISTORY],
            controller: None,
            body_mode: BodyMode::Default,
            animation: Animation::None,
            standing: false,
            flip_direction: 1,
            last_flip_direction: 1,
            stun: 0,
            last_stun: 0,
            dead: false,
            died_at: None,
            destroyed: false,
            shortcut_delay: 0,
            entering_shortcut: None,
            short_cut_rad_add: 0.0,
            vessel: None,
            can_jump: 0,
            want_to_jump: 0,
            jump_boost: 0.0,
            can_wall_jump: 0,
            jump_stun: 0,
            wall_slide_counter: 0,
            no_grab_counter: 0,
            pole_skip_penalty: 0,
            force_feet_to_horizontal_beam_tile: 0,
            time_since_in_corridor_mode: 0,
            slow_movement_stun: 0,
            slide_up_pole: 0,
            ledge_grab_counter: 0,
            feet_stuck_pos: None,
            dynamic_run_speed: [0.0; 2],
            super_launch_jump: 0,
            kill_super_launch_jump_counter: 0,
            simulate_hold_jump_button: 0,
            crawl_turn_delay: 0,
            upper_body_frames_on_ground: 0,
            upper_body_frames_off_ground: 0,
            lower_body_frames_on_ground: 0,
            lower_body_frames_off_ground: 0,
            animation_frame: 0,
            consistent_down_diagonal: 0,
            dive_force: 0.0,
            pullup_softlock_safety: 0,
            straight_up_on_horizontal_beam: false,
            up_on_horizontal_beam_pos: Vec2::ZERO,
            slide_direction: 0,
            init_slide_counter: 0,
            slide_counter: 0,
            roll_direction: 0,
            roll_counter: 0,
            stop_rolling_counter: 0,
            exit_belly_slide_counter: 0,
            long_belly_slide: false,
            whiplash_jump: false,
            flip_from_slide: false,
            rocket_jump_from_belly_slide: false,
            allow_roll: 0,
            go_into_corridor_climb: 0,
            corridor_drop: false,
            corridor_turn_dir: None,
            corridor_turn_counter: 0,
            can_corridor_jump: 0,
            vertical_corridor_slide_counter: 0,
            horizontal_corridor_slide_counter: 0,
            backwards_counter: 0,
            shoot_up_counter: 0,
            priv_sneak: 0.0,
            drop_grab_tile: None,
            blink_request: 0,
            map_input: Input::NONE,
            exits_to_be_discovered: None,
            added_spawn_room_to_discovery: false,
            air_in_lungs: 1.0,
            out_of_breath: [0.0; 4],
            lungs_exhausted: false,
            submerged: false,
            drown: 0.0,
            swim_force: 0.0,
            swim_cycle: 0.0,
            water_jump_delay: 0,
            water_friction: crate::slugcat::PARAMS.water_friction,
            water_retardation_immunity: 0.0,
            left_foot: false,
            slide_loop: None,
            slide_loop_sound: None,
            released_slide_loops: Vec::new(),
            emitter_place: EmitterPlace::default(),
            sound_queue: Vec::new(),
            noise_queue: Vec::new(),
            sleep_counter: 0,
            sleep_curl_up: 0.0,
            sleep_when_still: false,
            touched_no_input_counter: 0,
            force_sleep_counter: 0,
            ready_for_win: false,
            still_in_start_shelter: true,
            stats: SlugcatCampaign::SURVIVOR.stats,
            aerobic_level: 0.0,
            exhausted: false,
            grab: GrabState::default(),
            stomach: None,
            release_grasps: false,
            movement_ran: false,
            shortcut_pull: None,
            ceiling_drips: 0.0,
            drips: Vec::new(),
            before_pull: None,
        }
    }

    /// The `Player` constructor: [`Player::new`] facing left or right with even odds.
    pub fn create(rng: &mut UnityRandom) -> Self {
        let mut player = Self::new();
        if rng.value() < 0.5 {
            player.flip_direction = -1;
            player.last_flip_direction = -1;
        }
        player
    }

    /// Puts the body on `tile` as [`place_in_room`] (tile middle plus a random unit
    /// offset per chunk, at rest). Movement state is kept.
    pub fn place(&mut self, tile: (i32, i32), rng: &mut UnityRandom) {
        place_in_room(&mut self.body, tile, rng);
        self.feet_stuck_pos = None;
    }

    /// The body chunks, head first. Same data as `body.chunks`.
    #[must_use]
    pub fn chunks(&self) -> &[BodyChunk] {
        &self.body.chunks
    }

    /// Head chunk.
    #[must_use]
    pub fn head(&self) -> &BodyChunk {
        &self.body.chunks[HEAD]
    }

    /// Hips chunk.
    #[must_use]
    pub fn hips(&self) -> &BodyChunk {
        &self.body.chunks[HIPS]
    }

    #[must_use]
    pub fn body_mode(&self) -> BodyMode {
        self.body_mode
    }

    #[must_use]
    pub fn animation(&self) -> Animation {
        self.animation
    }

    /// Whether the player wants to be upright.
    #[must_use]
    pub fn is_standing(&self) -> bool {
        self.standing
    }

    #[must_use]
    pub fn left_foot(&self) -> bool {
        self.left_foot
    }

    /// Facing: -1 left, +1 right.
    #[must_use]
    pub fn flip_direction(&self) -> i32 {
        self.flip_direction
    }

    /// Wants to be upright (`standing = value`, as room scripts set it).
    pub fn set_standing(&mut self, standing: bool) {
        self.standing = standing;
    }

    #[must_use]
    pub fn controller(&self) -> Option<PlayerController> {
        self.controller
    }

    /// `controller = value`.
    pub fn set_controller(&mut self, controller: Option<PlayerController>) {
        self.controller = controller;
    }

    /// The last [`INPUT_HISTORY`] inputs, newest first.
    #[must_use]
    pub fn input_history(&self) -> &[Input; INPUT_HISTORY] {
        &self.input
    }

    /// Frame counter of the current gait or animation (`animationFrame`), read by the graphics.
    #[must_use]
    pub fn animation_frame(&self) -> i32 {
        self.animation_frame
    }

    /// Crouch super-jump charge (`superLaunchJump`, 0..=20), read by the graphics.
    #[must_use]
    pub fn super_launch_jump(&self) -> i32 {
        self.super_launch_jump
    }

    /// `superLaunchJump = 0` (the guide's super-jump lesson).
    pub fn reset_super_launch_jump(&mut self) {
        self.super_launch_jump = 0;
    }

    /// Direction of a running corridor U-turn (`corridorTurnDir`), read by the graphics.
    #[must_use]
    pub fn corridor_turn_dir(&self) -> Option<(i32, i32)> {
        self.corridor_turn_dir
    }

    /// `Player.Blink(n)` raised this tick (largest `n`, 0 if none); the graphics close
    /// the eyes for at least that many ticks.
    #[must_use]
    pub fn blink_request(&self) -> i32 {
        self.blink_request
    }

    /// `MapInput`: the input the HUD map pans and changes layers with.
    #[must_use]
    pub fn map_input(&self) -> Input {
        self.map_input
    }

    /// `Player.Blink`.
    pub(crate) fn blink(&mut self, ticks: i32) {
        self.blink_request = self.blink_request.max(ticks);
    }

    /// One 40 Hz tick (`Player.Update`, the movement-relevant parts in the game's order):
    /// counters and the jump-press buffer, breathing (`LungUpdate`), reading `input`
    /// (ignored while stunned or dead), wall-slide braking, the stun and shortcut-delay
    /// countdowns and out-of-room destruction, physics (`PhysicalObject.Update`, which
    /// delivers terrain impacts mid-collision) or, while entering a shortcut, the pull-in
    /// instead, resetting `GoThroughFloors`, then the movement state machine unless
    /// stunned, dead or entering. Impulses chosen this tick act in the next tick's
    /// physics. A dead body keeps falling as plain physics. Does nothing while inside a
    /// shortcut (the body is out of the room; [`crate::ShortcutHandler`] moves it) or once
    /// destroyed.
    ///
    /// `water` is the room's [`Water`] (`None` for a room without water); update it first
    /// each tick, as the room does. The game's random draws (stun threshold, stacked-body
    /// nudge, corridor-turn jiggle, drowning flail and every sound's draws) come from
    /// `rng`; the `room.PlaySound` calls go to `sounds`.
    pub fn update(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        input: Input,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        if self.update_before_catch(room, water, input, rng, sounds) {
            self.update_after_catch(room, water, rng, sounds, &mut ());
        }
    }

    /// [`Player::update`] up to the stacked-body nudge, where `Player.Update` catches a
    /// bat fly within reach. Returns whether the rest of the update runs (not in a
    /// shortcut, not destroyed).
    pub fn update_before_catch(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        input: Input,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> bool {
        self.blink_request = 0;
        self.movement_ran = false;
        self.shortcut_pull = None;
        self.before_pull = None;
        if self.destroyed || self.vessel.is_some() {
            return false;
        }
        self.body.params = crate::slugcat::PARAMS;
        self.body.params.surface_friction = if self.is_conscious() {
            SURFACE_FRICTION
        } else {
            UNCONSCIOUS_SURFACE_FRICTION
        };
        self.body.params.water_friction = self.water_friction;
        self.body.params.water_retardation_immunity = self.water_retardation_immunity;
        self.update_counters_before_input(room, water, rng, sounds);
        if !self.dead {
            self.lung_update(room, water, rng, sounds);
        }
        self.update_out_of_breath();
        self.read_input(input);
        self.set_unconscious_modes();
        self.update_after_input(room, rng, sounds);
        true
    }

    /// The rest of [`Player::update`] after the catch: the camera clamp, the creature
    /// update, physics or the pull into a shortcut, then the movement state machine.
    pub fn update_after_catch(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        noises: &mut dyn NoiseSink,
    ) {
        self.clamp_to_cameras(room);
        self.update_after_clamp(room, water, rng, sounds, noises);
    }

    /// [`Player::update_after_catch`] after the camera clamp, where `Player.Update` has its
    /// shelter block: the creature update, physics or the pull, the state machine. Its
    /// noises go to `noises` as the game makes them.
    pub(crate) fn update_after_clamp(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        noises: &mut dyn NoiseSink,
    ) {
        self.creature_update(water);
        if self.destroyed {
            self.flush_sounds_and_noises(rng, sounds, noises);
            return;
        }
        if let Some(entrance) = self.entering_shortcut {
            self.before_pull = Some(Box::new(self.clone()));
            self.pull_into_shortcut(room, entrance);
        } else {
            self.short_cut_rad_add = shortcut::RAD_ADD_RESET;
            self.update_physics(room, water, rng, sounds, noises);
        }
        self.body.set_go_through_floors(false);
        if self.stun < 1 && !self.dead && self.entering_shortcut.is_none() && self.vessel.is_none()
        {
            self.movement_ran = true;
            self.movement_update(room, water, rng, sounds);
        }
        self.flush_sounds_and_noises(rng, sounds, noises);
    }

    /// Counters, the slide loop and the jump-press buffer. They run before the new input
    /// is read, so they see last tick's input as `input[0]`.
    fn update_counters_before_input(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        if self.pole_skip_penalty > 0 {
            self.pole_skip_penalty -= 1;
        }
        self.count_shoot_up();
        self.update_slide_loop(rng, sounds);
        if self.body_mode == BodyMode::CorridorClimb {
            self.time_since_in_corridor_mode = 0;
        } else {
            self.time_since_in_corridor_mode = self.time_since_in_corridor_mode.saturating_add(1);
        }
        self.update_allow_roll(room);
        if self.stun == 12 {
            self.play_sound(SoundId::UiSlugcatExitStun, HEAD, 1.0, 1.0);
        }
        self.update_jump_buffer(room);
        if self.body_mode == BodyMode::WallClimb {
            self.wall_slide_counter += 1;
        } else {
            self.wall_slide_counter = 0;
        }
        self.can_wall_jump -= self.can_wall_jump.signum();
        if self.no_grab_counter > 0 {
            self.no_grab_counter -= 1;
        }
        if self.water_jump_delay > 0 {
            self.water_jump_delay -= 1;
        }
        if self.force_feet_to_horizontal_beam_tile > 0 {
            self.force_feet_to_horizontal_beam_tile -= 1;
        }
        if self.can_jump > 0 {
            self.can_jump -= 1;
        }
        if self.slow_movement_stun > 0 {
            self.slow_movement_stun -= 1;
        }
        if self.backwards_counter > 0 {
            self.backwards_counter -= 1;
        }
        if self.vertical_corridor_slide_counter > 0 {
            self.vertical_corridor_slide_counter -= 1;
        }
        if self.horizontal_corridor_slide_counter > 0 {
            self.horizontal_corridor_slide_counter -= 1;
        }
        self.jump_stun -= self.jump_stun.signum();
        if self.grab.dont_grab_stuff > 0 {
            self.grab.dont_grab_stuff -= 1;
        }
        self.update_sneak(water);
    }

    /// `privSneak` rises while low (not standing, crawling, in a corridor, or still on a
    /// pole) and off the air, else falls; the head is then `1.5 * (1 - Sneak)` loud and
    /// the hips `0.7 * (1 - Sneak)`, times the slugcat's `loudnessFac`. The game's
    /// `ClimbIntoShortCut` body mode is never set; there is no zero-gravity body mode
    /// (`ZeroG`: `privSneak` 0.5, both chunks 0.5 loud).
    fn update_sneak(&mut self, water: Option<&Water>) {
        let low = !self.standing
            || matches!(self.body_mode, BodyMode::Crawl | BodyMode::CorridorClimb)
            || (self.animation == Animation::HangFromBeam && self.input[0].x == 0)
            || (self.animation == Animation::ClimbOnBeam && self.input[0].y == 0);
        self.priv_sneak = if low && self.body_mode != BodyMode::Default {
            (self.priv_sneak + 0.1).min(1.0)
        } else {
            (self.priv_sneak - 0.04).max(0.0)
        };
        let quiet = 1.0 - self.sneak(water);
        let fac = self.stats.loudness_fac;
        self.body.chunks[HEAD].loudness = 1.5 * quiet * fac;
        self.body.chunks[HIPS].loudness = 0.7 * quiet * fac;
    }

    /// `Sneak`.
    #[must_use]
    pub fn sneak(&self, water: Option<&Water>) -> f32 {
        crate::creature::PlayerVisibility {
            general: self.stats.general_visibility_bonus,
            stealth_in_sneak_mode: self.stats.visual_stealth_in_sneak_mode,
            priv_sneak: self.priv_sneak,
            submersion: self.body.submersion(water),
        }
        .sneak()
    }

    /// `privSneak`.
    #[must_use]
    pub fn priv_sneak(&self) -> f32 {
        self.priv_sneak
    }

    /// The jump-press edge sets the 5-tick buffer, or does the horizontal-pole hop.
    fn update_jump_buffer(&mut self, room: &Room) {
        let pressed = self.input[0].jump && !self.input[1].jump;
        if self.want_to_jump > 0 {
            self.want_to_jump -= 1;
            return;
        }
        if !pressed {
            return;
        }
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let no_contact = [head, hips]
            .iter()
            .all(|c| c.contact().x == 0 && c.contact().y == 0);
        let may_hop = self.standing
            && self.animation == Animation::None
            && (hips.last_pos.y - hips.pos.y).abs() < 20.0
            && hips.vel.y < 5.0
            && head.pos.y > hips.pos.y + 5.0
            && self.body_mode == BodyMode::Default
            && no_contact
            && !self.is_tile_solid(room, HIPS, -1, 0)
            && !self.is_tile_solid(room, HIPS, 1, 0);
        if !may_hop {
            self.want_to_jump = 5;
            return;
        }
        let beam_y = hips.pos.tile_middle().y;
        let near_beam_line = (beam_y - hips.pos.y)
            .abs()
            .min((beam_y - hips.last_pos.y).abs())
            < 7.5;
        let over_beam =
            room.tile_at_pos(hips.pos).horizontal_pole() && self.input[0].x != 0 && near_beam_line;
        if over_beam && self.input[0].y <= 0 && self.pole_skip_penalty < 1 {
            let lift = beam_y + 5.0 - hips.pos.y;
            let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
            head.vel.y = 4.5;
            hips.vel.y = 3.5;
            hips.pos.y += lift;
            head.pos.y += lift;
            self.jump_boost = 6.0;
            self.pole_skip_penalty = 6;
            self.play_sound(SoundId::SlugcatFromHorizontalPoleJump, HIPS, 1.0, 1.0);
            self.want_to_jump = 0;
            self.can_jump = 0;
        } else {
            self.pole_skip_penalty = (self.pole_skip_penalty + 12).min(30);
            self.want_to_jump = 5;
        }
    }

    /// `checkInput`: shift the history ring and store the new input (the controller's
    /// when one is set), or a neutral one while stunned (any stun tick) or dead.
    fn read_input(&mut self, input: Input) {
        self.input.copy_within(0..INPUT_HISTORY - 1, 1);
        self.input[0] = match self.controller {
            _ if self.stun != 0 || self.dead => Input::NONE,
            Some(controller) => controller.input(),
            None => input.normalized(),
        };
        self.map_input = self.input[0];
        // `standStillOnMapButton` is set in a story session (the port's only kind): holding
        // the map button, or sleeping, drops the other buttons and blinks.
        if self.input[0].map || self.sleeping() {
            self.input[0] = Input::NONE.with_map(self.input[0].map);
            self.blink(5);
        }
        let [now, prev, prev2, ..] = self.input;
        // Holding a charged crouch super jump freezes horizontal input.
        if self.super_launch_jump > 10 && now.jump && prev.jump && prev2.jump && now.y < 1 {
            self.input[0].x = 0;
        }
        let diag = self.input[0].down_diagonal();
        if diag != 0 && diag == self.input[1].down_diagonal() {
            self.consistent_down_diagonal += 1;
        } else {
            self.consistent_down_diagonal = 0;
        }
    }

    /// Wall-slide braking (not swimming, not in corridors), ceiling un-standing and the
    /// stacked-body nudge: the parts of `Player.Update` between reading input and the
    /// bat-fly catch.
    fn update_after_input(&mut self, room: &Room, rng: &mut UnityRandom, sounds: &mut Sounds) {
        self.flush_sounds(rng, sounds);
        let x = i32::from(self.input[0].x);
        let friction = self.body.params.surface_friction;
        let head_above = self.body.chunks[HEAD].pos.y > self.body.chunks[HIPS].pos.y;
        let brake = (1.0 - friction * if head_above { 2.0 } else { 0.5 }).clamp(0.0, 1.0);
        let in_corridor = self.body_mode == BodyMode::CorridorClimb;
        if self.body_mode != BodyMode::Swimming {
            for c in &mut self.body.chunks {
                let cx = i32::from(c.contact().x);
                if cx != 0 && x == cx && c.vel.y < 0.0 && !in_corridor {
                    c.vel.y *= brake;
                }
            }
        }

        if self.tile_pos(HEAD).1 <= self.tile_pos(HIPS).1 && self.is_tile_solid(room, HEAD, 0, 1) {
            self.standing = false;
        }

        // Head exactly below the hips and nothing moving sideways: kick the head
        // sideways by a random speed in [-1, 1] (`Random.Range(-1f, 1f)`).
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if head.pos.x == hips.pos.x
            && head.vel.x == 0.0
            && hips.vel.x == 0.0
            && head.pos.y < hips.pos.y
        {
            head.vel.x = rng.range_f32(-1.0, 1.0);
            hips.vel.x = -head.vel.x;
        }
    }

    /// Keeps the head within the horizontal span of the room's cameras.
    pub(crate) fn clamp_to_cameras(&mut self, room: &Room) {
        let xs = room.cameras().iter().map(|c| c[0]);
        let (Some(min), Some(max)) = (xs.clone().reduce(f32::min), xs.reduce(f32::max)) else {
            return;
        };
        let head_x = self.body.chunks[HEAD].pos.x;
        let clamp_to = if head_x < min + 20.0 {
            min + 20.0
        } else if head_x > max + 1366.0 {
            max + 1366.0
        } else {
            return;
        };
        for c in &mut self.body.chunks {
            c.pos.x = clamp_to;
        }
    }

    // ---- shared helpers ------------------------------------------------------------

    /// Tile of chunk `i`.
    fn tile_pos(&self, i: usize) -> (i32, i32) {
        self.body.chunks[i].pos.tile()
    }

    /// `PhysicalObject.IsTileSolid`: Solid, or a one-way Floor below a chunk that is not
    /// going through floors.
    #[must_use]
    pub fn is_tile_solid(&self, room: &Room, i: usize, dx: i32, dy: i32) -> bool {
        let (x, y) = self.tile_pos(i);
        match room.tile(x + dx, y + dy).terrain {
            Terrain::Solid => true,
            Terrain::Floor => dy < 0 && !self.body.chunks[i].go_through_floors,
            _ => false,
        }
    }

    fn input_x(&self) -> i32 {
        i32::from(self.input[0].x)
    }

    fn input_y(&self) -> i32 {
        i32::from(self.input[0].y)
    }
}
