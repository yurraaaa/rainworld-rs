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
    BodyChunk, DEFAULT_RESTRICT_IN_ROOM_RANGE, PhysicalObject, SoundId, Sounds, TerrainImpact,
    TerrainImpactHandler, Vec2, Water,
};

/// Floor impacts faster than this kill (Survivor; Gourmand 80).
pub(super) const FALL_DEATH_SPEED: f32 = 60.0;
/// Impacts faster than this stun (Survivor; Gourmand 40).
pub(super) const FALL_STUN_SPEED: f32 = 35.0;
/// `Stunned` holds from this many stun ticks up.
const STUNNED_FROM: i32 = 10;
/// `Player.Stun` un-stands the player when the stun exceeds `Random.Range(5, 12)`.
const UNSTAND_STUN_DRAW: (i32, i32) = (5, 12);
/// `Player.Stun` then drops a spear on the back above `Random.Range(40, 80)`; the draw
/// is made whether or not there is one.
const SPEAR_DROP_STUN_DRAW: (i32, i32) = (40, 80);

/// The player as the receiver of terrain impacts during its physics, with the
/// generator and sounds `TerrainImpact` and `Stun` use.
struct ImpactHandler<'a> {
    player: &'a mut Player,
    rng: &'a mut UnityRandom,
    sounds: &'a mut Sounds,
}

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

    /// `Player.Stun`: keeps the longer of the current and the new stun (1.5 times as
    /// long malnourished, twice exhausted); a stun longer
    /// than a random 5..=11 ticks also drops what the hands hold (`LoseAllGrasps`), stops
    /// standing and releases the feet pin. A new
    /// longer stun above 10 plays `UI_Slugcat_Stunned_Init` (silent in the shipped game).
    pub fn stun(&mut self, ticks: i32, rng: &mut UnityRandom, sounds: &mut Sounds) {
        self.flush_sounds(rng, sounds);
        let ticks = self.stun_multiplied(ticks);
        if ticks > rng.range_i32(UNSTAND_STUN_DRAW.0, UNSTAND_STUN_DRAW.1) {
            self.request_release_grasps();
            self.standing = false;
            self.feet_stuck_pos = None;
        }
        // No spear on the back to drop.
        let _ = rng.range_i32(SPEAR_DROP_STUN_DRAW.0, SPEAR_DROP_STUN_DRAW.1);
        if ticks > self.stun && ticks > 10 {
            self.play_sound(SoundId::UiSlugcatStunnedInit, HEAD, 1.0, 1.0);
            self.last_stun = ticks;
        }
        self.stun = self.stun.max(ticks);
        self.flush_sounds(rng, sounds);
    }

    /// `Mathf.RoundToInt(st * (exhausted ? 2 : 1.5))` when malnourished.
    #[must_use]
    pub fn stun_multiplied(&self, ticks: i32) -> i32 {
        if !self.malnourished() {
            return ticks;
        }
        let fac = if self.exhausted { 2.0 } else { 1.5 };
        (ticks as f32 * fac).round_ties_even() as i32
    }

    /// `Player.Die`: the first call asks the game for `GameOver` (see
    /// [`Player::take_died_at`]) and plays `UI_Slugcat_Die` (silent in the shipped game);
    /// its draws are made with the player's next ones. The hands let go.
    pub fn die(&mut self) {
        let main = self.body.chunks[HEAD].pos;
        self.die_with_main_at(main);
    }

    /// [`Player::die`] while the body is out for a physics step, `main` its main chunk's
    /// position.
    fn die_with_main_at(&mut self, main: Vec2) {
        if !self.dead {
            self.died_at = Some(main);
            self.play_sound(SoundId::UiSlugcatDie, HEAD, 1.0, 1.0);
        }
        self.dead = true;
        self.request_release_grasps();
    }

    /// The main chunk's position at the first `Die`, once: the game's `GameOver` call.
    pub fn take_died_at(&mut self) -> Option<Vec2> {
        self.died_at.take()
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
    pub(super) fn update_physics(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        self.flush_sounds(rng, sounds);
        let placeholder = PhysicalObject::new(Vec::new(), self.body.params);
        let mut body = std::mem::replace(&mut self.body, placeholder);
        let mut handler = ImpactHandler {
            player: self,
            rng,
            sounds,
        };
        body.update_with(room, water, &mut handler);
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
        self.play_sound(SoundId::SlugcatRollInit, HEAD, 1.0, 1.0);
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

impl Player {
    /// `Player.TerrainImpact` with Remix off: the landing roll pre-empts everything on
    /// this impact; otherwise a first contact kills on a floor above 60, stuns above 35,
    /// or plays a landing or bump sound by speed. Either way it ends in the base
    /// `Creature.TerrainImpact`'s ceiling drips.
    pub(super) fn terrain_impact_with(
        &mut self,
        chunks: &mut [BodyChunk],
        impact: TerrainImpact,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        let speed = impact.speed;
        if speed > 12.0 {
            self.blink((speed as i32).clamp(12, 60) / 2);
        }
        if self.lands_in_roll(&impact) {
            self.start_landing_roll(chunks);
        } else if impact.first_contact {
            self.first_contact_impact(impact, chunks[HEAD].pos, rng, sounds);
        }
        self.flush_sounds(rng, sounds);
        let c = &chunks[impact.chunk];
        self.impact_drips(c.pos, c.rad, c.mass, impact, rng);
    }

    /// A first contact that is not a landing roll: death on a floor above 60, a stun
    /// above 35, else a landing or bump sound by speed.
    fn first_contact_impact(
        &mut self,
        impact: TerrainImpact,
        main: Vec2,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        let speed = impact.speed;
        if speed > FALL_DEATH_SPEED && impact.direction.y < 0 {
            self.play_sound(SoundId::SlugcatTerrainImpactDeath, HEAD, 1.0, 1.0);
            self.die_with_main_at(main);
        } else if speed > FALL_STUN_SPEED {
            self.play_sound(SoundId::SlugcatTerrainImpactHard, HEAD, 1.0, 1.0);
            self.stun(Self::fall_stun_ticks(speed), rng, sounds);
        } else if impact.direction.y < 0 && self.is_conscious() {
            let id = if self.standing && impact.chunk == HIPS {
                SoundId::SlugcatFloorImpactStandard
            } else {
                SoundId::SlugcatFloorImpactStealthy
            };
            self.play_sound(id, HEAD, speed.inverse_lerp(8.0, 11.0), 1.0);
        } else if speed < 3.0 {
            let volume = speed.inverse_lerp(0.0, 2.0);
            self.play_sound(SoundId::SlugcatTerrainImpactLight, HEAD, volume, 3.0);
        } else if speed < 16.0 {
            self.play_sound(SoundId::SlugcatTerrainImpactMedium, HEAD, 1.0, 1.0);
        } else {
            self.play_sound(SoundId::SlugcatTerrainImpactHard, HEAD, 1.0, 1.0);
        }
    }
}

impl TerrainImpactHandler for ImpactHandler<'_> {
    fn terrain_impact(&mut self, chunks: &mut [BodyChunk], impact: TerrainImpact) {
        self.player
            .terrain_impact_with(chunks, impact, self.rng, self.sounds);
    }
}
