//! The player: base slugcat movement on the two-chunk body (`Player`).
//!
//! Behaviour: `rainworld-re/notes/slugcat-movement.md` (tick order, input, body modes,
//! jumps), `horizontal-beams.md`, `vertical-poles.md`, `ledge-grab.md` and
//! `slope-collision.md` section 5. Stats are the
//! Survivor's (every `SlugcatStats` factor 1, no adrenaline, never exhausted). Fall
//! damage, stun and death: `falls.md`. Skid, flip, roll, belly slide and rocket jump:
//! `advanced-moves.md`. Shortcuts: `shortcuts.md`.
//! What is not ported is listed in `docs/DIVERGENCES.md`.

mod advanced;
mod animation;
mod body_mode;
mod falls;
mod input;
mod jump;
mod ledge;
mod movement;
mod poles;
mod shortcut;
#[cfg(test)]
mod tests;

use rand::Rng;
use rw_assets::{Room, Terrain};

pub use input::{INPUT_HISTORY, Input};

use crate::room_ext::RoomExt;
use crate::shortcut::ShortcutVessel;
use crate::{BodyChunk, PhysicalObject, Vec2, determine_safe_spawn_tile, place_in_room};

/// Index of the head chunk (`bodyChunks[0]`, `mainBodyChunk`).
pub const HEAD: usize = 0;
/// Index of the hips chunk (`bodyChunks[1]`).
pub const HIPS: usize = 1;

/// Conscious player surface friction, re-applied every tick.
const SURFACE_FRICTION: f32 = 0.5;
/// Surface friction while stunned or dead.
const UNCONSCIOUS_SURFACE_FRICTION: f32 = 0.3;
/// `runspeedFac` of the Survivor.
const RUN_SPEED_FAC: f32 = 1.0;
/// `poleClimbSpeedFac` of the Survivor.
const POLE_CLIMB_SPEED_FAC: f32 = 1.0;
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
    Dead,
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
    dead: bool,
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
    whiplash_jump: bool,
    flip_from_slide: bool,
    rocket_jump_from_belly_slide: bool,
    /// Ticks left in which a landing may turn into a roll (`allowRoll`).
    allow_roll: i32,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    /// A Survivor at the origin, as the `Player` constructor leaves it: not standing,
    /// `Default` body mode, no animation, facing right (the game picks the facing at
    /// random). Place it with [`Player::place`] or create it with [`Player::spawn`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            body: PhysicalObject::slugcat(),
            input: [Input::NONE; INPUT_HISTORY],
            body_mode: BodyMode::Default,
            animation: Animation::None,
            standing: false,
            flip_direction: 1,
            last_flip_direction: 1,
            stun: 0,
            dead: false,
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
            whiplash_jump: false,
            flip_from_slide: false,
            rocket_jump_from_belly_slide: false,
            allow_roll: 0,
        }
    }

    /// A new player on a spawn tile chosen by [`determine_safe_spawn_tile`], placed by
    /// [`place_in_room`]; both draw from `rng`.
    pub fn spawn<R: Rng + ?Sized>(room: &Room, rng: &mut R) -> Self {
        let tile = determine_safe_spawn_tile(room, rng);
        let mut player = Self::new();
        player.place(tile, rng);
        player
    }

    /// Puts the body on `tile` as [`place_in_room`] (tile middle plus a random unit
    /// offset per chunk, at rest). Movement state is kept.
    pub fn place<R: Rng + ?Sized>(&mut self, tile: (i32, i32), rng: &mut R) {
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

    /// Facing: -1 left, +1 right.
    #[must_use]
    pub fn flip_direction(&self) -> i32 {
        self.flip_direction
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

    /// One 40 Hz tick (`Player.Update`, the movement-relevant parts in the game's order):
    /// counters and the jump-press buffer, reading `input` (ignored while stunned or
    /// dead), wall-slide braking, the stun and shortcut-delay countdowns and
    /// out-of-room destruction, physics (`PhysicalObject.Update`, which delivers terrain
    /// impacts mid-collision) or, while entering a shortcut, the pull-in instead, resetting
    /// `GoThroughFloors`, then the movement state machine unless stunned, dead or
    /// entering. Impulses chosen this tick act in the next tick's physics. A dead body
    /// keeps falling as plain physics. Does nothing while inside a shortcut (the body
    /// is out of the room; [`crate::ShortcutHandler`] moves it) or once destroyed.
    pub fn update(&mut self, room: &Room, input: Input) {
        if self.destroyed || self.vessel.is_some() {
            return;
        }
        self.body.params = crate::slugcat::PARAMS;
        self.body.params.surface_friction = if self.is_conscious() {
            SURFACE_FRICTION
        } else {
            UNCONSCIOUS_SURFACE_FRICTION
        };
        self.update_counters_before_input(room);
        self.read_input(input);
        self.set_unconscious_modes();
        self.update_after_input(room);
        self.creature_update(room);
        if self.destroyed {
            return;
        }
        if let Some(entrance) = self.entering_shortcut {
            self.pull_into_shortcut(room, entrance);
        } else {
            self.short_cut_rad_add = shortcut::RAD_ADD_RESET;
            self.update_physics(room);
        }
        self.body.set_go_through_floors(false);
        if self.stun < 1 && !self.dead && self.entering_shortcut.is_none() && self.vessel.is_none()
        {
            self.movement_update(room);
        }
    }

    /// Counters and the jump-press buffer. They run before the new input is read, so
    /// they see last tick's input as `input[0]`.
    fn update_counters_before_input(&mut self, room: &Room) {
        if self.pole_skip_penalty > 0 {
            self.pole_skip_penalty -= 1;
        }
        // No corridor mode is ported, so this only counts up.
        self.time_since_in_corridor_mode = self.time_since_in_corridor_mode.saturating_add(1);
        self.update_allow_roll(room);
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
        if self.force_feet_to_horizontal_beam_tile > 0 {
            self.force_feet_to_horizontal_beam_tile -= 1;
        }
        if self.can_jump > 0 {
            self.can_jump -= 1;
        }
        if self.slow_movement_stun > 0 {
            self.slow_movement_stun -= 1;
        }
        self.jump_stun -= self.jump_stun.signum();
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
            self.want_to_jump = 0;
            self.can_jump = 0;
        } else {
            self.pole_skip_penalty = (self.pole_skip_penalty + 12).min(30);
            self.want_to_jump = 5;
        }
    }

    /// `checkInput`: shift the history ring and store the new input, or a neutral one
    /// while stunned (any stun tick) or dead.
    fn read_input(&mut self, input: Input) {
        self.input.copy_within(0..INPUT_HISTORY - 1, 1);
        self.input[0] = if self.stun == 0 && !self.dead {
            input.normalized()
        } else {
            Input::NONE
        };
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

    /// Wall-slide braking, ceiling un-standing, the stacked-body nudge and the camera
    /// clamp: the parts of `Player.Update` between reading input and physics.
    fn update_after_input(&mut self, room: &Room) {
        let x = i32::from(self.input[0].x);
        let friction = self.body.params.surface_friction;
        let head_above = self.body.chunks[HEAD].pos.y > self.body.chunks[HIPS].pos.y;
        let brake = (1.0 - friction * if head_above { 2.0 } else { 0.5 }).clamp(0.0, 1.0);
        for c in &mut self.body.chunks {
            let cx = i32::from(c.contact().x);
            if cx != 0 && x == cx && c.vel.y < 0.0 {
                c.vel.y *= brake;
            }
        }

        if self.tile_pos(HEAD).1 <= self.tile_pos(HIPS).1 && self.is_tile_solid(room, HEAD, 0, 1) {
            self.standing = false;
        }

        // Head exactly below the hips and nothing moving sideways: the game kicks the
        // head sideways by a random speed in [-1, 1]; the port uses the facing
        // (docs/DIVERGENCES.md D3).
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if head.pos.x == hips.pos.x
            && head.vel.x == 0.0
            && hips.vel.x == 0.0
            && head.pos.y < hips.pos.y
        {
            head.vel.x = 0.5 * self.flip_direction as f32;
            hips.vel.x = -head.vel.x;
        }

        self.clamp_to_cameras(room);
    }

    /// Keeps the head within the horizontal span of the room's cameras.
    fn clamp_to_cameras(&mut self, room: &Room) {
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
    fn is_tile_solid(&self, room: &Room, i: usize, dx: i32, dy: i32) -> bool {
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
