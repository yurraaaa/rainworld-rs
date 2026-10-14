//! Fall damage, stun and death: `Player.TerrainImpact`, `Stun`, `Die` and the
//! `Creature.Update` parts that count stun down and kill a body below the room.
//!
//! Behaviour: `rainworld-re/notes/falls.md`, landing roll `advanced-moves.md`
//! section 5. Remix (MMF) is off: no fall into a shortcut, no wall pounce
//! (docs/DIVERGENCES.md D4, D9).

use rw_assets::Room;
use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player};
use crate::float_ext::FloatExt;
use crate::{
    BodyChunk, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicalObject, TerrainImpact, TerrainImpactHandler,
    Water,
};

/// Floor impacts faster than this kill (Survivor; Gourmand 80).
pub(super) const FALL_DEATH_SPEED: f32 = 60.0;
/// Impacts faster than this stun (Survivor; Gourmand 40).
pub(super) const FALL_STUN_SPEED: f32 = 35.0;
/// `Stunned` holds from this many stun ticks up.
const STUNNED_FROM: i32 = 10;
/// `Player.Stun` un-stands the player when the stun exceeds `Random.Range(5, 12)`.
const UNSTAND_STUN_DRAW: (i32, i32) = (5, 12);

impl Player {
    /// Whether the player is dead. A dead body is still plain physics in
    /// [`Player::update`] (the app freezes it) unless it was destroyed.
    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// `Creature.Stunned`: at least 10 stun ticks left.
    #[must_use]
    pub fn is_stunned(&self) -> bool {
        self.stun >= STUNNED_FROM
    }

    /// `Creature.Consious`: neither stunned nor dead.
    #[must_use]
    pub fn is_conscious(&self) -> bool {
        !self.is_stunned() && !self.dead
    }

    /// Ticks of stun left. Input is ignored and movement skipped while it is above 0.
    #[must_use]
    pub fn stun_ticks(&self) -> i32 {
        self.stun
    }

    /// `Player.Stun`: keeps the longer of the current and the new stun; a stun longer
    /// than a random 5..=11 ticks also stops standing and releases the feet pin.
    pub fn stun(&mut self, ticks: i32, rng: &mut UnityRandom) {
        let threshold = rng.range_i32(UNSTAND_STUN_DRAW.0, UNSTAND_STUN_DRAW.1);
        self.stun_unstanding_above(ticks, threshold);
    }

    fn stun_unstanding_above(&mut self, ticks: i32, threshold: i32) {
        if ticks > threshold {
            self.standing = false;
            self.feet_stuck_pos = None;
        }
        self.stun = self.stun.max(ticks);
    }

    /// `Creature.Die`.
    pub fn die(&mut self) {
        self.dead = true;
    }

    /// Stun ticks for an impact at `speed`: `LerpMap(speed, 35, 60, 40, 140, 2.5)`,
    /// truncated.
    pub(super) fn fall_stun_ticks(speed: f32) -> i32 {
        let t = ((speed - FALL_STUN_SPEED) / (FALL_DEATH_SPEED - FALL_STUN_SPEED)).clamp(0.0, 1.0);
        (40.0 + 100.0 * t.powf(2.5)) as i32
    }

    /// The dead/stunned overrides of body mode and animation after reading input.
    pub(super) fn set_unconscious_modes(&mut self) {
        if self.dead {
            self.animation = Animation::Dead;
            self.body_mode = BodyMode::Dead;
        } else if self.stun > 0 {
            self.animation = Animation::None;
            self.body_mode = BodyMode::Stunned;
        }
    }

    /// Whether the body fell out of the room and was removed from the game. Nothing
    /// is left to simulate or draw.
    #[must_use]
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// The `Creature.Update` steps before physics: stun and shortcut-delay
    /// countdowns, then death and destruction below the room.
    pub(super) fn creature_update(&mut self, water: Option<&Water>) {
        if self.stun > 0 {
            self.stun -= 1;
        }
        if self.shortcut_delay > 0 {
            self.shortcut_delay -= 1;
        }
        if self.fell_out_of_room(water) {
            self.die();
            self.destroyed = true;
        }
    }

    /// Head below the out-of-room limit in a room whose water does not catch it.
    fn fell_out_of_room(&self, water: Option<&Water>) -> bool {
        let head = &self.body.chunks[HEAD];
        let mut limit = -head.restrict_in_room_range + 1.0;
        if head.restrict_in_room_range == DEFAULT_RESTRICT_IN_ROOM_RANGE {
            let floor = if self.body_mode == BodyMode::WallClimb {
                -250.0
            } else {
                -500.0
            };
            limit = limit.max(floor);
        }
        // `!room.water || room.defaultWaterLevel < -10` (inverted water is DLC).
        let water_catches = water.is_some_and(|w| w.level_row() >= -10);
        head.pos.y < limit && !water_catches
    }

    /// `PhysicalObject.Update` with the player as the owner that receives terrain
    /// impacts mid-collision. The body is moved out for the call, so
    /// [`TerrainImpactHandler::terrain_impact`] reaches the chunks only through its
    /// argument.
    pub(super) fn update_physics(&mut self, room: &Room, water: Option<&Water>) {
        let placeholder = PhysicalObject::new(Vec::new(), self.body.params);
        let mut body = std::mem::replace(&mut self.body, placeholder);
        body.update_with(room, water, self);
        self.body = body;
    }

    /// The landing-roll test that comes first in `Player.TerrainImpact`: a down-diagonal
    /// held for 2 (speed above 24) or 7 consecutive ticks, a recent airborne moment
    /// (`allowRoll`) and a floor hit above 12 (any speed from a flip or a belly-slide
    /// rocket jump).
    fn lands_in_roll(&self, impact: &TerrainImpact) -> bool {
        let rocket_from_slide =
            self.animation == Animation::RocketJump && self.rocket_jump_from_belly_slide;
        let held_for = if impact.speed > 24.0 { 1 } else { 6 };
        self.input[0].down_diagonal() != 0
            && self.animation != Animation::Roll
            && (impact.speed > 12.0 || self.animation == Animation::Flip || rocket_from_slide)
            && impact.direction.y < 0
            && self.allow_roll > 0
            && self.consistent_down_diagonal > held_for
    }

    /// Starts the landing roll toward the held down-diagonal, steering both chunks'
    /// x velocity 70% of the way to `9 * input.x`.
    fn start_landing_roll(&mut self, chunks: &mut [BodyChunk]) {
        if self.animation == Animation::RocketJump && self.rocket_jump_from_belly_slide {
            chunks[HIPS].vel.y += 3.0;
            chunks[HIPS].pos.y += 3.0;
            chunks[HEAD].vel.y -= 3.0;
            chunks[HEAD].pos.y -= 3.0;
        }
        self.animation = Animation::Roll;
        self.roll_direction = i32::from(self.input[0].down_diagonal());
        self.roll_counter = 0;
        let target = 9.0 * f32::from(self.input[0].x);
        for c in &mut chunks[HEAD..=HIPS] {
            c.vel.x = c.vel.x.lerp(target, 0.7);
        }
        self.standing = false;
    }
}

impl TerrainImpactHandler for Player {
    /// `Player.TerrainImpact` with Remix off: the landing roll pre-empts everything on
    /// this impact; otherwise a first contact kills on a floor above 60 or stuns above 35.
    fn terrain_impact(&mut self, chunks: &mut [BodyChunk], impact: TerrainImpact) {
        if impact.speed > 12.0 {
            self.blink((impact.speed as i32).clamp(12, 60) / 2);
        }
        if self.lands_in_roll(&impact) {
            self.start_landing_roll(chunks);
            return;
        }
        if !impact.first_contact {
            return;
        }
        if impact.speed > FALL_DEATH_SPEED && impact.direction.y < 0 {
            self.die();
        } else if impact.speed > FALL_STUN_SPEED {
            // Fall stuns are at least 40 ticks, longer than any draw of the un-standing
            // threshold, so the draw cannot matter and is not made.
            self.stun_unstanding_above(Self::fall_stun_ticks(impact.speed), UNSTAND_STUN_DRAW.1);
        }
    }
}
