//! Zero gravity: the `ZeroG` and `BrokenZeroG` room effects. [`AntiGravity`] is the room
//! object that sets `Room.gravity`; [`BrokenAntiGravity`] is the world's flickering
//! switch (`RainCycle.brokenAntiGrav`) the `BrokenZeroG` rooms follow. Base game
//! (Moon-heart, Pebbles' energy and filtration power branches are DLC).
//!
//! Mirrors: AntiGravity.cs (AntiGravity, BrokenAntiGravity)

use rw_random::UnityRandom;

use crate::math::float_ext::{FloatExt, MathfSin, SCurve};

/// The dev setup's `gravityFlickerCycleMin` and `gravityFlickerCycleMax` (seconds).
pub const FLICKER_CYCLE_MIN: i32 = 8;
pub const FLICKER_CYCLE_MAX: i32 = 18;

/// What one [`BrokenAntiGravity::update`] asks of the camera's room when it has
/// `BrokenZeroG`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrokenTick {
    /// The switch flipped this tick: its new state (`on`).
    pub switched: Option<bool>,
    /// A transition is running (`0 < progress < 1`): the room's screen shakes.
    pub shaking: bool,
}

/// `AntiGravity.BrokenAntiGravity`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrokenAntiGravity {
    pub counter: i32,
    pub on: bool,
    pub from: f32,
    pub to: f32,
    pub progress: f32,
    pub lights: f32,
    pub lights_get_to: f32,
    pub cycle_min: i32,
    pub cycle_max: i32,
}

impl BrokenAntiGravity {
    /// The constructor: one draw for the initial state.
    pub fn new(cycle_min: i32, cycle_max: i32, rng: &mut UnityRandom) -> Self {
        let on = rng.value() < 0.5;
        let to = if on { 1.0 } else { 0.0 };
        Self {
            counter: 0,
            on,
            from: to,
            to,
            progress: 0.0,
            lights: to,
            lights_get_to: 0.0,
            cycle_min,
            cycle_max,
        }
    }

    /// `CurrentAntiGravity`.
    #[must_use]
    pub fn current_anti_gravity(&self) -> f32 {
        self.from
            .lerp(self.to, self.progress.inverse_lerp(0.35, 1.0).s_curve(0.5))
    }

    /// `CurrentLightsOn`.
    #[must_use]
    pub fn current_lights_on(&self) -> f32 {
        self.lights
    }

    /// `Update`, once a tick from `RainCycle.Update`. The room calls for the camera's
    /// room (the switch sound, the screen movement) are in the result.
    pub fn update(&mut self, rng: &mut UnityRandom) -> BrokenTick {
        let mut tick = BrokenTick::default();
        self.counter -= 1;
        if self.counter < 1 {
            self.on = !self.on;
            self.counter = rng.range_i32(self.cycle_min * 40, self.cycle_max * 40);
            self.from = self.to;
            self.to = if self.on { 1.0 } else { 0.0 };
            self.progress = 0.0;
            tick.switched = Some(self.on);
        }
        if self.progress < 1.0 {
            self.progress = (self.progress + 1.0 / 120.0).min(1.0);
            if rng.value() < 0.125 {
                let exponent = self.progress.lerp_map(0.0, 0.6, 1.0, 0.0);
                let base = rng.value() * self.progress.inverse_lerp(0.0, 0.5).mathf_pow(0.5);
                self.lights_get_to = self.from.lerp(self.to, base.mathf_pow(exponent));
            }
        } else {
            self.lights_get_to = self.to;
        }
        self.lights = self
            .lights
            .lerp_and_tick(self.lights_get_to, 0.15, 0.000_833_333_35);
        tick.shaking = self.progress > 0.0 && self.progress < 1.0;
        tick
    }

    /// The screen shake amount of the running transition for a room with `amount` of
    /// `BrokenZeroG`.
    #[must_use]
    pub fn shake(&self, amount: f32) -> f32 {
        amount * 0.5 * (self.progress * std::f32::consts::PI).mathf_sin()
    }
}

/// `AntiGravity`, the room object.
#[derive(Debug, Clone, PartialEq)]
pub struct AntiGravity {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `active`: Five Pebbles' oracle scenes switch it off (not ported).
    pub active: bool,
}

impl AntiGravity {
    #[must_use]
    pub fn new(update_order: u64) -> Self {
        Self {
            update_order,
            active: true,
        }
    }

    /// `Update`: the room's new `gravity` from its `ZeroG` and `BrokenZeroG` amounts and
    /// the world's switch (if a `BrokenZeroG` room made one).
    #[must_use]
    pub fn gravity(zero_g: f32, broken_zero_g: f32, broken: Option<&BrokenAntiGravity>) -> f32 {
        let mut num = zero_g;
        if num < 1.0 {
            num = 0.0_f32.lerp(0.85, num);
        }
        if let Some(b) = broken {
            num = num.max(broken_zero_g * b.current_anti_gravity());
        }
        1.0 - num
    }
}

#[cfg(test)]
#[path = "tests/anti_gravity.rs"]
mod tests;
