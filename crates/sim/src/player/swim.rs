//! Water: entering the `Swimming` body mode, the `DeepSwim` and `SurfaceSwim`
//! animations, `GravitateToOpening`, breath and drowning (`LungUpdate`).
//!
//! Behaviour: `rainworld-re/notes/water.md` sections 3-5, base game with Remix off
//! (docs/DIVERGENCES.md D11), Survivor stats.

use rw_assets::Room;
use rw_random::UnityRandom;

use super::{Animation, BodyMode, HEAD, HIPS, Player, ROOM_GRAVITY};
use crate::float_ext::FloatExt;
use crate::{Vec2, Water};

/// `SlugcatStats.drownThreshold`: below this much air the slugcat struggles.
pub(super) const DROWN_THRESHOLD: f32 = 1.0 / 3.0;
/// `swimBoostMinAir`: a deep-swim boost needs more air than this.
const SWIM_BOOST_MIN_AIR: f32 = 0.5;
/// `swimBoostCost` with Remix off.
const SWIM_BOOST_COST: f32 = 0.2;
/// `swimBoostForce`.
const SWIM_BOOST_FORCE: f32 = 3.0;
/// `swimBoostCooldown`, ticks.
const SWIM_BOOST_COOLDOWN: i32 = 20;
/// `swimForceFac` and `lungsFac` of the Survivor.
const SWIM_FORCE_FAC: f32 = 1.0;
const LUNGS_FAC: f32 = 1.0;
/// `RainWorldGame.setupValues.lungs`: 128 in a release build (`setup.txt` is read
/// only with dev tools).
const SETUP_LUNGS: f32 = 128.0;
/// Ticks of being out of air (one `drown` step each) before death.
const DROWN_TICKS: f32 = 120.0;

/// Animations during which a beam or ledge holds the body, so it cannot surface-swim.
const HELD_ANIMATIONS: [Animation; 8] = [
    Animation::BeamTip,
    Animation::ClimbOnBeam,
    Animation::GetUpOnBeam,
    Animation::GetUpToBeamTip,
    Animation::HangFromBeam,
    Animation::StandOnBeam,
    Animation::LedgeGrab,
    Animation::HangUnderVerticalBeam,
];

impl Player {
    /// Breath left: 1 full, 0 empty.
    #[must_use]
    pub fn air_in_lungs(&self) -> f32 {
        self.air_in_lungs
    }

    /// Came up short of breath and still recovering (`lungsExhausted`).
    #[must_use]
    pub fn lungs_exhausted(&self) -> bool {
        self.lungs_exhausted
    }

    /// Drowning progress, 0..1 (death at 1).
    #[must_use]
    pub fn drown(&self) -> f32 {
        self.drown
    }

    /// Swim stroke phase (`swimCycle`), read by the graphics.
    #[must_use]
    pub fn swim_cycle(&self) -> f32 {
        self.swim_cycle
    }

    /// Ticks before the next surface jump or boost (`waterJumpDelay`).
    #[must_use]
    pub fn water_jump_delay(&self) -> i32 {
        self.water_jump_delay
    }

    fn sub(&self, i: usize, water: Option<&Water>) -> f32 {
        self.body.chunks[i].submersion(water)
    }

    /// The water block of `MovementUpdate`: with either chunk more than 20% under
    /// water (not wall-climbing unless the head is fully under, not in a corridor),
    /// swim deep when the head is well below the surface, else swim at the surface when
    /// nothing holds the body up. Holding down (or a down-diagonal) dives. Once at the
    /// surface, a plain re-check goes deep only when diving or 80 px down.
    pub(super) fn classify_swimming(&mut self, room: &Room, water: Option<&Water>) {
        let Some(w) = water else {
            return;
        };
        let (head_sub, hips_sub) = (self.sub(HEAD, water), self.sub(HIPS, water));
        if self.body_mode == BodyMode::WallClimb && head_sub != 1.0 {
            return;
        }
        if !(head_sub > 0.2 || hips_sub > 0.2) || self.body_mode == BodyMode::CorridorClimb {
            return;
        }
        let diving = self.input_y() < 0 || self.input[0].down_diagonal() != 0;
        let head = self.body.chunks[HEAD].pos;
        let level = w.level_at(head.x);
        let deep = head.y < level - 80.0;
        let under = head.y < level - if diving { 10.0 } else { 30.0 };
        let hips_needed = if diving { -1.0 } else { 0.6 };
        if (self.animation != Animation::SurfaceSwim || diving || deep)
            && under
            && hips_sub > hips_needed
        {
            self.body_mode = BodyMode::Swimming;
            self.animation = Animation::DeepSwim;
        } else if (!self.is_tile_solid(room, HIPS, 0, -1) || hips_sub == 1.0)
            && !HELD_ANIMATIONS.contains(&self.animation)
        {
            self.body_mode = BodyMode::Swimming;
            self.animation = Animation::SurfaceSwim;
        }
    }

    /// `DeepSwim`: no run force; a jump press with enough air boosts along the body
    /// (costing air, 20-tick cooldown; a press during the cooldown only restarts the
    /// stroke); holding a direction builds `swim_force`, advances the stroke cycle with
    /// a pulse at its end, and pushes the head that way (bent upward when short of
    /// breath); water friction follows how well the motion lines up with the body.
    pub(super) fn deep_swim(&mut self, water: Option<&Water>) {
        self.dynamic_run_speed = [0.0, 0.0];
        self.can_jump = 0;
        self.standing = false;
        self.body.set_go_through_floors(true);
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let axis = (head.pos - hips.pos).normalized();
        let alignment =
            (head.vel.normalized().dot(axis).abs() + hips.vel.normalized().dot(axis).abs()) / 2.0;
        let pressed = self.input[0].jump && !self.input[1].jump;
        if pressed && self.air_in_lungs > SWIM_BOOST_MIN_AIR {
            if self.water_jump_delay == 0 {
                self.swim_cycle = 2.7;
                let along = hips.pos.direction_to(head.pos);
                self.body.chunks[HEAD].vel += along * SWIM_BOOST_FORCE;
                self.air_in_lungs -= SWIM_BOOST_COST;
            } else {
                self.swim_cycle = 0.0;
            }
            self.water_jump_delay = SWIM_BOOST_COOLDOWN;
        }
        self.swim_cycle += 0.01;
        if self.input_x() != 0 || self.input_y() != 0 {
            self.swim_stroke(water, alignment);
        }
        self.water_friction = 0.92_f32.lerp(0.96, alignment);
        if self.body_mode != BodyMode::Swimming {
            self.animation = Animation::None;
        }
    }

    /// The held-direction part of `DeepSwim`.
    fn swim_stroke(&mut self, water: Option<&Water>, alignment: f32) {
        let (head, hips) = (&self.body.chunks[HEAD], &self.body.chunks[HIPS]);
        let turn = (head.last_pos - hips.last_pos).angle_to(head.pos - hips.pos);
        let mut target = 0.2 + turn.inverse_lerp(0.0, 12.0) * 0.8;
        if self.slow_movement_stun > 0 {
            target *= 0.5;
        }
        let rate = if target > self.swim_force { 0.7 } else { 0.05 };
        self.swim_force = self.swim_force.lerp(target, rate);
        self.swim_cycle += self.swim_force.lerp(1.0, 0.5) / 10.0;
        if self.air_in_lungs < 0.5 && self.air_in_lungs > 1.0 / 6.0 {
            self.swim_cycle += 0.05;
        }
        let head = &self.body.chunks[HEAD];
        if head.contact().x != 0 || head.contact().y != 0 {
            self.swim_force *= 0.5;
        }
        let head_sub = self.sub(HEAD, water);
        let along = hips.pos.direction_to(head.pos);
        let pulse = along * (0.7 * (self.swim_force * SWIM_FORCE_FAC).lerp(1.0, 0.5) * head_sub);
        if self.swim_cycle > 4.0 {
            self.swim_cycle = 0.0;
        } else if self.swim_cycle > 3.0 {
            self.body.chunks[HEAD].vel += pulse;
        }
        let mut dir = self.swim_dir();
        if self.air_in_lungs < DROWN_THRESHOLD {
            // Remix's breath leniency curve is off: the air itself sets the bend.
            let bend = self.air_in_lungs.inverse_lerp(DROWN_THRESHOLD, 0.0);
            dir = dir.slerp(Vec2::new(0.0, 1.0), bend);
        }
        let force = self.swim_force;
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        head.vel += dir * (0.5 * force * alignment.lerp(1.0, 0.5) * head_sub * SWIM_FORCE_FAC);
        hips.vel -= dir * (0.1 * head_sub);
        head.vel += along * (0.4 * force * alignment * head_sub * SWIM_FORCE_FAC);
        let speed = head.vel.length();
        if speed < 3.0 {
            let assist = speed.inverse_lerp(3.0, 1.5);
            head.vel += dir * (0.2 * assist);
            hips.vel -= dir * (0.1 * assist);
        }
    }

    /// `SwimDir(true)` for digital input: the held direction, normalised.
    fn swim_dir(&self) -> Vec2 {
        Vec2::new(self.input_x() as f32, self.input_y() as f32).normalized()
    }

    /// `SurfaceSwim`: the head is held about 15 px above the surface (down sinks, up
    /// rises), the head paddles toward 2.7 px/tick with the hips trailing, and a jump
    /// press leaps out (once per 17 ticks; a press during the delay only hops).
    pub(super) fn surface_swim(&mut self, water: Option<&Water>) {
        self.can_jump = 0;
        self.swim_cycle += 0.025;
        self.water_friction = 0.96;
        self.swim_force *= 0.5;
        let (x, y) = (self.input_x(), self.input_y());
        let head_vy = self.body.chunks[HEAD].vel.y;
        let hold = y > -1
            && head_vy > -5.0
            && head_vy < 3.0
            && self.water_jump_delay == 0
            && !self.input[0].jump;
        let head_pos = self.body.chunks[HEAD].pos;
        let level = water.map_or(f32::MIN, |w| w.level_at(head_pos.x));
        let [head, hips] = self.body.chunk_pair_mut(HEAD, HIPS);
        if hold {
            head.vel.y *= 0.8;
            hips.vel.y *= 0.8;
            head.vel.y += ((head.pos.y - (level + 15.0)) * -0.1).clamp(-0.5, 1.5);
            hips.vel.y -= 0.5;
        } else if y == -1 {
            head.vel.y -= 0.2;
            hips.vel.y += 0.1;
        } else if y == 1 {
            head.vel.y += 0.5;
        }
        self.dynamic_run_speed = [2.7, 0.0];
        if x != 0 {
            hips.vel.x -= x as f32 * 0.2;
            self.swim_cycle += 1.0 / 30.0;
        }
        if self.input[0].jump && !self.input[1].jump {
            if self.water_jump_delay == 0 {
                if head.vel.y < 2.0 && hips.vel.y < 2.0 {
                    head.vel.y += 6.0;
                    hips.vel.y += 6.0;
                } else {
                    head.vel += hips.pos.direction_to(head.pos) * 4.0;
                    hips.vel.y += if hips.vel.y < 4.0 { 3.0 } else { 1.5 };
                }
                self.water_jump_delay = 17;
            } else {
                self.water_jump_delay = self.water_jump_delay.max(10);
                head.vel.y += 4.0;
                hips.vel.x *= 0.75;
            }
        }
        if self.body_mode != BodyMode::Swimming {
            self.animation = Animation::None;
        }
    }

    /// The `Swimming` body mode: steer into one-tile openings, fall through floors.
    pub(super) fn swimming_mode(&mut self, room: &Room) {
        self.gravitate_to_opening(room);
        self.body.set_go_through_floors(true);
    }

    /// `GravitateToOpening`: pressing one axis toward a one-tile hole (solid on both
    /// sides of it, not on both sides of the head) pulls the nearer chunk toward the
    /// hole, centres both across it and halves any speed above 2.
    fn gravitate_to_opening(&mut self, room: &Room) {
        let (x, y) = (self.input_x(), self.input_y());
        let (dx, dy) = if x != 0 && y == 0 {
            if self.is_tile_solid(room, HEAD, x, 0)
                || !self.is_tile_solid(room, HEAD, x, 1)
                || !self.is_tile_solid(room, HEAD, x, -1)
                || (self.is_tile_solid(room, HEAD, 0, 1) && self.is_tile_solid(room, HEAD, 0, -1))
            {
                return;
            }
            (x, 0)
        } else {
            if y == 0
                || x != 0
                || self.is_tile_solid(room, HEAD, 0, y)
                || !self.is_tile_solid(room, HEAD, 1, y)
                || !self.is_tile_solid(room, HEAD, -1, y)
                || (self.is_tile_solid(room, HEAD, 1, 0) && self.is_tile_solid(room, HEAD, -1, 0))
            {
                return;
            }
            (0, y)
        };
        let (tx, ty) = self.tile_pos(HEAD);
        let hole = Vec2::from(rw_assets::tile_centre(tx + dx, ty + dy));
        let mut nearest = HEAD;
        let mut best = f32::MAX;
        for i in [HEAD, HIPS] {
            let d = self.body.chunks[i].pos.distance(hole);
            if d < best {
                nearest = i;
                best = d;
            }
        }
        let c = &mut self.body.chunks[nearest];
        c.vel += c.pos.direction_to(hole) * 0.5_f32.lerp(1.0, ROOM_GRAVITY);
        let centring = 0.01_f32.lerp(0.1, ROOM_GRAVITY);
        for c in &mut self.body.chunks {
            let mid = c.pos.tile_middle();
            if dx != 0 {
                c.vel.y -= (c.pos.y - mid.y) * centring;
            } else {
                c.vel.x -= (c.pos.x - mid.x) * centring;
            }
            c.vel = c.vel.lerp(c.vel.clamp_length(2.0), 0.5);
        }
    }

    /// `LungUpdate`, before this tick's input is read (`input[0]` is last tick's). With
    /// the head under water air drains (faster when exhausted, slower holding straight
    /// up while short of breath); out of air with the body under water the slugcat is
    /// stunned and drowns over 120 ticks, unless it is holding up within reach of open
    /// surface, which allows 0.3 more; short of breath it flails and struggles. Above
    /// water air refills (slowly after coming up short of breath).
    pub(super) fn lung_update(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        rng: &mut UnityRandom,
    ) {
        // `airInLungs = min(airInLungs, 1 - rainDeath)`: no rain.
        if self.sub(HEAD, water) > 0.9 {
            self.breathe_under_water(room, water, rng);
        } else {
            if self.submerged && self.air_in_lungs < DROWN_THRESHOLD {
                self.lungs_exhausted = true;
            }
            if !self.lungs_exhausted && self.air_in_lungs > 0.9 {
                self.air_in_lungs = 1.0;
            }
            if self.air_in_lungs <= 0.0 {
                self.air_in_lungs = 0.0;
            }
            self.air_in_lungs += 1.0 / if self.lungs_exhausted { 240.0 } else { 60.0 };
            if self.air_in_lungs >= 1.0 {
                self.air_in_lungs = 1.0;
                self.lungs_exhausted = false;
                self.drown = 0.0;
            }
            self.submerged = false;
            if self.lungs_exhausted && self.animation != Animation::SurfaceSwim {
                self.swim_cycle += 0.1;
            }
        }
        if self.lungs_exhausted {
            self.slow_movement_stun = self.slow_movement_stun.max(5);
            if self.drown > 0.0 {
                self.slow_movement_stun = self.slow_movement_stun.max(10);
            }
        }
    }

    fn breathe_under_water(&mut self, room: &Room, water: Option<&Water>, rng: &mut UnityRandom) {
        if !self.submerged {
            self.swim_force = self.body.chunks[HEAD].vel.x.abs().inverse_lerp(0.0, 8.0);
            self.swim_cycle = 0.0;
        }
        let input = self.input[0];
        let holding_up = input.y == 1 && input.x == 0 && self.air_in_lungs < DROWN_THRESHOLD;
        let ticks = 40.0
            * if self.lungs_exhausted { 4.5 } else { 9.0 }
            * if holding_up { 1.5 } else { 1.0 }
            * (SETUP_LUNGS / 100.0);
        self.air_in_lungs -= 1.0 / ticks * LUNGS_FAC;
        let floor = if self.heading_for_air(room, water) {
            -0.3
        } else {
            0.0
        };
        if self.air_in_lungs <= floor && self.sub(HEAD, water) == 1.0 && self.sub(HIPS, water) > 0.5
        {
            self.air_in_lungs = 0.0;
            self.stun(10, rng);
            self.drown += 1.0 / DROWN_TICKS;
            if self.drown >= 1.0 {
                self.die();
            }
        } else if self.air_in_lungs < DROWN_THRESHOLD {
            self.slow_movement_stun = self.slow_movement_stun.max(1);
            if rng.value() < 0.5 {
                let dir = Vec2::random_unit(rng);
                self.body.chunks[HEAD].vel += dir * rng.value();
            }
            if input.y < 1 {
                let keep = 1.0_f32.lerp(0.9, self.air_in_lungs.inverse_lerp(0.0, DROWN_THRESHOLD));
                self.body.chunks[HIPS].vel *= keep;
            }
        }
        self.submerged = true;
    }

    /// Out of air but holding up, less than 200 px under the surface, with no solid
    /// tile between the head and the surface row: the game lets the air go to -0.3
    /// before drowning starts.
    fn heading_for_air(&self, room: &Room, water: Option<&Water>) -> bool {
        let Some(w) = water else {
            return false;
        };
        let head = self.body.chunks[HEAD].pos;
        if !(self.air_in_lungs <= 0.0
            && self.input[0].y == 1
            && w.level_at(head.x) - head.y < 200.0)
        {
            return false;
        }
        let (tx, ty) = head.tile();
        (ty..=w.level_row()).all(|row| !room.tile(tx, row).is_solid())
    }
}
