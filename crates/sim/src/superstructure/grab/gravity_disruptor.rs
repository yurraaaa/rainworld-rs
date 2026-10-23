//! `GravityDisruptor`: a placed whirl in Five Pebbles' chambers. While its `power` is above
//! zero it swirls every body chunk and every zero-gravity speck within 450 px around it
//! (pushing them toward 260 px from the centre and round it), and its sixteen streaks
//! flicker. Twenty `DisruptorSpeck`s orbit it. In a `BrokenZeroG` room the power follows
//! the world's `CurrentAntiGravity`. The swirl over the room's bodies is the game's side
//! (`superstructure/grab/turn.rs`); the sprites are `rw-graphics`'. Draws are on the shared stream
//! (D37).
//!
//! Mirrors: GravityDisruptor.cs (GravityDisruptor, DisruptorSpeck)

use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// How many streaks the disruptor has (`lights.GetLength(0)`).
pub const STREAKS: usize = 16;

/// How many `DisruptorSpeck`s it makes.
pub const SPECKS: usize = 20;

/// Reach of the swirl in px.
const REACH: f32 = 450.0;

/// One `GravityDisruptor`.
#[derive(Debug, Clone, PartialEq)]
pub struct GravityDisruptor {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`, what `pos` follows.
    pub placed_pos: Vec2,
    pub pos: Vec2,
    pub last_pos: Vec2,
    /// 0, or 1 for a free tile in front of a wall, 2 for one in the open.
    pub depth: i32,
    pub debug_mode: i32,
    /// `lights[16, 4]`: the streaks' brightness now, a tick ago, two ticks ago, and the
    /// streak's own drift.
    pub lights: [[f32; 4]; STREAKS],
    pub point_dir: Vec2,
    pub get_to_point_dir: Vec2,
    pub dir_fac: f32,
    pub dir_fac_get_to: f32,
    pub power: f32,
    pub last_power: f32,
    /// `roomSpecks`: the update-list places of the `GenericZeroGSpeck`s `AIMapReady`
    /// found.
    pub room_specks: Vec<u64>,
}

impl GravityDisruptor {
    /// The constructor's first draws, the twenty specks: for each a random direction
    /// times `400 * value` (two draws), then the speck's own two ([`DisruptorSpeck::new`]).
    /// `orders` are the specks' places in the update list and `disruptor` the
    /// disruptor's, which comes after them.
    #[must_use]
    pub fn make_specks(
        pos: Vec2,
        orders: &[u64; SPECKS],
        disruptor: u64,
        rng: &mut UnityRandom,
    ) -> Vec<DisruptorSpeck> {
        orders
            .iter()
            .map(|&order| {
                let at = pos + Vec2::random_unit(rng) * (400.0 * rng.value());
                DisruptorSpeck::new(order, disruptor, at, pos, rng)
            })
            .collect()
    }

    /// The rest of the constructor, after the specks: a draw for each streak's drift,
    /// `pointDir` (one) and `dirFac`.
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, depth: i32, rng: &mut UnityRandom) -> Self {
        let mut lights = [[0.0; 4]; STREAKS];
        for l in &mut lights {
            l[3] = rng.value();
        }
        let point_dir = Vec2::random_unit(rng);
        let dir_fac = rng.value();
        Self {
            update_order,
            placed_pos: pos,
            pos,
            last_pos: pos,
            depth,
            debug_mode: 0,
            lights,
            point_dir,
            get_to_point_dir: point_dir,
            dir_fac,
            dir_fac_get_to: dir_fac,
            power: 1.0,
            last_power: 1.0,
            room_specks: Vec::new(),
        }
    }

    /// The first lines of `Update`: the position follows the placed object (a move, only
    /// the editor's, shows the debug colours for 80 ticks).
    pub fn follow(&mut self) {
        self.last_pos = self.pos;
        self.pos = self.placed_pos;
        if self.last_pos != self.pos {
            self.debug_mode = 80;
        }
        if self.debug_mode > 0 {
            self.debug_mode -= 1;
        }
    }

    /// The swirl's push on a body chunk at `at` moving at `vel`: the change of its
    /// position, which is also added to its velocity, and the change of its velocity
    /// round the centre; `None` without power or beyond 450 px. A chunk is pushed toward
    /// 260 px from the centre (half as hard from outside); the weight is 0 at 450 px and
    /// 1 inside 200.
    #[must_use]
    pub fn chunk_push(&self, at: Vec2, vel: Vec2) -> Option<(Vec2, Vec2)> {
        if self.power <= 0.0 || !self.pos.dist_less(at, REACH) {
            return None;
        }
        let radial = self.pos.direction_to(at);
        let tangent = radial.perpendicular();
        let side = (at + vel).distance_to_line(self.pos, at).unity_sign();
        let dist = self.pos.distance(at);
        let weight = dist.inverse_lerp(REACH, 200.0).mathf_pow(2.0);
        let inside = if dist < 260.0 { 1.0 } else { 0.5 };
        let push = radial * ((260.0 - dist) * 0.05 * weight * inside * self.power);
        Some((push, tangent * (side * 0.1 * weight * self.power)))
    }

    /// The swirl's change of velocity of a `GenericZeroGSpeck` at `at` moving at `vel`:
    /// round the centre only.
    #[must_use]
    pub fn speck_push(&self, at: Vec2, vel: Vec2) -> Option<Vec2> {
        if self.power <= 0.0 || !self.pos.dist_less(at, REACH) {
            return None;
        }
        let tangent = self.pos.direction_to(at).perpendicular();
        let weight = self
            .pos
            .distance(at)
            .inverse_lerp(REACH, 200.0)
            .mathf_pow(2.0);
        let side = (at + vel).distance_to_line(self.pos, at).unity_sign();
        Some(tangent * (side * 0.5 * weight * self.power))
    }

    /// The rest of `Update` after the swirl: the pointing direction and `dirFac` wander
    /// (a draw each, two more when they pick a new target), every streak (one or two
    /// draws, then one more when it re-rolls its drift), and the power: the world's
    /// `CurrentAntiGravity` in a `BrokenZeroG` room (`anti_gravity`), else unchanged.
    pub fn update(&mut self, anti_gravity: Option<f32>, rng: &mut UnityRandom) {
        self.point_dir = self.point_dir.slerp(self.get_to_point_dir, 0.01);
        if rng.value() < 0.0125 {
            self.get_to_point_dir = Vec2::random_unit(rng);
        }
        self.dir_fac = self.dir_fac.lerp(self.dir_fac_get_to, 0.01);
        if rng.value() < 0.0125 {
            self.dir_fac_get_to = rng.value();
        }
        for m in 0..STREAKS {
            self.update_streak(m, rng);
        }
        self.last_power = self.power;
        if let Some(p) = anti_gravity {
            self.power = p;
        }
    }

    /// One streak of `Update`'s loop.
    fn update_streak(&mut self, m: usize, rng: &mut UnityRandom) {
        let l = &mut self.lights;
        l[m][2] = l[m][1];
        l[m][1] = l[m][0];
        l[m][0] = (l[m][0]
            + (-1.0_f32).lerp(1.0, rng.value()) / 120.0
            + (-1.0_f32).lerp(1.0, l[m][3]) / 60.0)
            .clamp(0.0, 1.0);
        let facing = Vec2::from_degrees(m as f32 / 16.0 * 360.0).dot(self.point_dir);
        l[m][0] = l[m][0].lerp(
            facing.inverse_lerp(-1.0, 1.0).mathf_pow(1.5),
            facing.abs().mathf_pow(8.0)
                * 0.3
                * self.dir_fac
                * (0.5 - l[m][3]).abs().inverse_lerp(0.5, 0.0),
        );
        let next = if m < STREAKS - 1 { m + 1 } else { 0 };
        let previous = if m > 0 { m - 1 } else { STREAKS - 1 };
        let around = l[next][2] + l[previous][2];
        l[m][0] = l[m][0].lerp(around / 2.0, 0.05);
        if rng.value() < 0.005 {
            l[m][3] = rng.value();
        }
    }
}

/// `GravityDisruptor.DisruptorSpeck`: a pixel orbiting the disruptor at its own radius.
#[derive(Debug, Clone, PartialEq)]
pub struct DisruptorSpeck {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// The disruptor's place in the update list.
    pub disruptor: u64,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub my_float_speed: f32,
    pub my_orbit_rad: f32,
    pub new_orbit_rad: f32,
}

impl DisruptorSpeck {
    /// The constructor, two draws: a float speed, then a random direction.
    #[must_use]
    pub fn new(
        update_order: u64,
        disruptor: u64,
        pos: Vec2,
        disruptor_pos: Vec2,
        rng: &mut UnityRandom,
    ) -> Self {
        let my_float_speed = 2.0_f32.lerp(8.0, rng.value());
        let vel = Vec2::random_unit(rng) * my_float_speed;
        let orbit = disruptor_pos.distance(pos);
        Self {
            update_order,
            disruptor,
            pos,
            last_pos: pos,
            vel,
            my_float_speed,
            my_orbit_rad: orbit,
            new_orbit_rad: orbit,
        }
    }

    /// `Update` given the disruptor's position and power: one draw, two when the orbit
    /// radius to settle on changes.
    pub fn update(&mut self, disruptor_pos: Vec2, power: f32, rng: &mut UnityRandom) {
        self.last_pos = self.pos;
        self.pos += self.vel;
        if rng.value() < 0.01 {
            self.new_orbit_rad = 50.0_f32.lerp(400.0, rng.value());
        }
        self.my_orbit_rad = self.my_orbit_rad.lerp(self.new_orbit_rad, 0.01);
        let radial = disruptor_pos.direction_to(self.pos);
        let tangent = radial.perpendicular();
        let side = (self.pos + self.vel)
            .distance_to_line(disruptor_pos, self.pos)
            .unity_sign();
        let dist = disruptor_pos.distance(self.pos);
        let pull = radial * ((self.my_orbit_rad - dist) * 0.05);
        self.pos += pull;
        self.vel += pull;
        self.vel += tangent * (side * dist.lerp_map(400.0, 100.0, 0.01, 0.15));
        let settle = self.vel.normalized()
            * (self.my_float_speed / (self.my_orbit_rad.lerp(100.0, 0.5) * 0.01));
        self.vel = self.vel.lerp(settle, 0.01) * power;
    }
}

#[cfg(test)]
#[path = "tests/gravity_disruptor.rs"]
mod tests;
