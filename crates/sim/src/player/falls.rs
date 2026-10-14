//! Fall damage, stun and death: `Player.TerrainImpact`, `Stun`, `Die` and the
//! `Creature.Update` parts that count stun down and kill a body below the room.
//!
//! Behaviour: `rainworld-re/notes/falls.md`. Remix (MMF) is off: no fall into a
//! shortcut, no wall pounce (docs/DIVERGENCES.md D4).

use rw_assets::Room;

use super::{Animation, BodyMode, HEAD, Player};
use crate::{DEFAULT_RESTRICT_IN_ROOM_RANGE, TerrainImpact};

/// Floor impacts faster than this kill (Survivor; Gourmand 80).
pub(super) const FALL_DEATH_SPEED: f32 = 60.0;
/// Impacts faster than this stun (Survivor; Gourmand 40).
pub(super) const FALL_STUN_SPEED: f32 = 35.0;
/// `Stunned` holds from this many stun ticks up.
const STUNNED_FROM: i32 = 10;
/// `Player.Stun` un-stands the player above `Random.Range(5, 12)`; the port uses the
/// mean (docs/DIVERGENCES.md D4).
const UNSTAND_STUN: i32 = 8;

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

    /// `Player.Stun`: keeps the longer of the current and the new stun; a stun above
    /// 8 ticks also stops standing and releases the feet pin.
    pub fn stun(&mut self, ticks: i32) {
        if ticks > UNSTAND_STUN {
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
    pub(super) fn creature_update(&mut self, room: &Room) {
        if self.stun > 0 {
            self.stun -= 1;
        }
        if self.shortcut_delay > 0 {
            self.shortcut_delay -= 1;
        }
        if self.fell_out_of_room(room) {
            self.die();
            self.destroyed = true;
        }
    }

    /// Head below the out-of-room limit in a room whose water does not catch it.
    fn fell_out_of_room(&self, room: &Room) -> bool {
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
        let water_catches = room.water_level().is_some_and(|level| level >= -10);
        head.pos.y < limit && !water_catches
    }

    /// Hands this tick's terrain impacts to [`Player::terrain_impact`] in the game's
    /// order. They are found during physics but applied after it; that is exact while
    /// no handler branch changes chunk velocities (the landing roll will).
    pub(super) fn deliver_terrain_impacts(&mut self) {
        for i in 0..self.body.impacts().len() {
            let impact = self.body.impacts()[i];
            self.terrain_impact(impact);
        }
    }

    /// `Player.TerrainImpact` with Remix off: the landing roll pre-empts everything,
    /// otherwise a first contact kills on a floor above 60 or stuns above 35.
    pub(super) fn terrain_impact(&mut self, impact: TerrainImpact) {
        if self.lands_in_roll(&impact) {
            return;
        }
        if !impact.first_contact {
            return;
        }
        if impact.speed > FALL_DEATH_SPEED && impact.direction.y < 0 {
            self.die();
        } else if impact.speed > FALL_STUN_SPEED {
            self.stun(Self::fall_stun_ticks(impact.speed));
        }
    }

    /// Hook for the landing roll (`advanced-moves.md` section 5), which turns a
    /// down-diagonal landing into a roll and skips fall damage. Not ported yet: never
    /// rolls. Porting it needs `allowRoll` and the roll animation, and its velocity
    /// change must land mid-physics (see [`Player::deliver_terrain_impacts`]).
    fn lands_in_roll(&self, _impact: &TerrainImpact) -> bool {
        false
    }
}
