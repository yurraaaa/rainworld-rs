//! `RainCycle` and its `WaterLevelCycle` (base game, no precycles): built by the `World`
//! constructor with its draws, the timer ticked by `RainWorldGame.Update`. The rain
//! itself (`RainHit`, `GlobalRain`) is not ported.
//!
//! Mirrors: RainCycle.cs, WaterLevelCycle.cs

use rw_assets::SlugcatName;
use rw_random::UnityRandom;

use crate::FloatExt;

/// `rainWorld.setup.cycleTimeMin` / `cycleTimeMax` (seconds; `setup.txt` defaults).
pub const CYCLE_TIME_MIN: f32 = 400.0;
pub const CYCLE_TIME_MAX: f32 = 800.0;

/// `WaterLevelCycle.Stage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaterStage {
    #[default]
    Rise,
    Max,
    Fall,
    Min,
}

/// `WaterLevelCycle`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterLevelCycle {
    pub stage: WaterStage,
    pub stage_duration: f32,
    pub time_in_stage: f32,
}

impl WaterStage {
    /// `GetTimeRange` (seconds).
    fn time_range(self) -> (f32, f32) {
        match self {
            Self::Rise => (7.0, 7.0),
            Self::Max | Self::Min => (8.0, 10.0),
            Self::Fall => (8.0, 8.0),
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Rise => Self::Max,
            Self::Max => Self::Fall,
            Self::Fall => Self::Min,
            Self::Min => Self::Rise,
        }
    }
}

impl WaterLevelCycle {
    /// The constructor's three draws.
    pub fn new(rng: &mut UnityRandom) -> Self {
        let stage = if rng.value() < 0.5 {
            WaterStage::Max
        } else {
            WaterStage::Min
        };
        let (a, b) = stage.time_range();
        let stage_duration = rng.range_f32(a, b);
        let time_in_stage = rng.value() * stage_duration;
        Self {
            stage,
            stage_duration,
            time_in_stage,
        }
    }

    pub fn update(&mut self, rng: &mut UnityRandom) {
        self.time_in_stage += 0.025;
        if self.time_in_stage >= self.stage_duration {
            self.stage = self.stage.next();
            self.time_in_stage = 0.0;
            let (a, b) = self.stage.time_range();
            self.stage_duration = rng.range_f32(a, b);
        }
    }
}

/// `RainCycle`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RainCycle {
    pub cycle_length: i32,
    pub base_cycle_length: i32,
    pub timer: i32,
    pub pause: i32,
    pub sun_down_start_time: i32,
    pub day_night_counter: i32,
    pub rainbow_seed: i32,
    pub water_cycle: WaterLevelCycle,
    /// `storyMode`: the session is a story session.
    pub story_mode: bool,
    /// `startUpTicks`: the cycle's sunrise length (2400 after a story's first cycle).
    pub start_up_ticks: i32,
    /// `brokenAntiGrav`: made by the first loaded room with `BrokenZeroG`.
    pub broken_anti_grav: Option<crate::BrokenAntiGravity>,
}

impl RainCycle {
    /// The `World` constructor's story branch: the cycle's minutes, then `RainCycle`'s
    /// constructor (six draws in all). `cycleStartUp` is on and no story start is a
    /// `Dev` start, so cycles after the first get the 2400-tick start-up.
    pub fn new_story(character: SlugcatName, cycle_number: i32, rng: &mut UnityRandom) -> Self {
        let v = rng.value();
        let t = if character == SlugcatName::Yellow {
            0.35 + 0.65 * v.mathf_pow(1.2)
        } else {
            v
        };
        Self {
            story_mode: true,
            start_up_ticks: if cycle_number > 0 { 2400 } else { 0 },
            ..Self::new(CYCLE_TIME_MIN.lerp(CYCLE_TIME_MAX, t) / 60.0, rng)
        }
    }

    /// `new RainCycle(world, minutes)`.
    pub fn new(minutes: f32, rng: &mut UnityRandom) -> Self {
        let cycle_length = (minutes * 40.0 * 60.0) as i32;
        let sun_down_start_time =
            (cycle_length as f32).lerp(CYCLE_TIME_MAX * 60.0, rng.range_f32(0.02, 0.045)) as i32;
        let rainbow_seed = rng.range_i32(0, 10000);
        Self {
            cycle_length,
            base_cycle_length: cycle_length,
            timer: 0,
            pause: 0,
            sun_down_start_time,
            day_night_counter: 0,
            rainbow_seed,
            water_cycle: WaterLevelCycle::new(rng),
            story_mode: false,
            start_up_ticks: 0,
            broken_anti_grav: None,
        }
    }

    /// `TimeUntilRain`.
    #[must_use]
    pub fn time_until_rain(&self) -> i32 {
        self.cycle_length - self.timer
    }

    /// `AmountLeft`: the share of the cycle before the rain, below 0 after it.
    #[must_use]
    pub fn amount_left(&self) -> f32 {
        (self.cycle_length - self.timer) as f32 / self.cycle_length as f32
    }

    /// `RainGameOver`.
    #[must_use]
    pub fn rain_game_over(&self) -> bool {
        self.timer >= self.cycle_length
    }

    /// `RainApproaching`: 1 until the last 2400 ticks (story; 400 otherwise), then
    /// down to 0 at the rain.
    #[must_use]
    pub fn rain_approaching(&self) -> f32 {
        let until = self.time_until_rain() as f32;
        if self.story_mode {
            until.inverse_lerp(0.0, 2400.0)
        } else {
            until.inverse_lerp(0.0, 400.0)
        }
    }

    /// `LightChangeBecauseOfRain`.
    #[must_use]
    pub fn light_change_because_of_rain(&self) -> f32 {
        self.rain_approaching().inverse_lerp(0.4, 1.0)
    }

    /// `CycleStartUp`: 0 to 1 over `start_up_ticks`, 1 without a start-up.
    #[must_use]
    pub fn cycle_start_up(&self) -> f32 {
        if self.start_up_ticks <= 0 {
            1.0
        } else {
            (self.timer as f32).inverse_lerp(0.0, self.start_up_ticks as f32)
        }
    }

    /// `ProximityToMiddleOfCycle`: 0 at mid-cycle, 1 at its start and end.
    #[must_use]
    pub fn proximity_to_middle_of_cycle(&self) -> f32 {
        let half = self.cycle_length as f32 / 2.0;
        (self.timer as f32 - half).abs() / half
    }

    /// `ShaderLight`: the level shader's light before clouds (1 full sun, -1 none).
    /// After the rain hits it is `GlobalRain.ShaderLight`, which stays at its initial
    /// -1 until the rain is ported.
    #[must_use]
    pub fn shader_light(&self) -> f32 {
        if self.rain_game_over() {
            return -1.0;
        }
        if self.story_mode {
            -1.0 + self
                .cycle_start_up()
                .lerp(1.0 - self.proximity_to_middle_of_cycle(), 0.2)
                * 2.0
                * self.light_change_because_of_rain().inverse_lerp(0.4, 1.0)
        } else {
            (self.time_until_rain() as f32).lerp_map(200.0, 880.0, -1.0, 1.0)
        }
    }

    /// `RainDarkPalette`: how far the palette has turned to its rain-dark half.
    #[must_use]
    pub fn rain_dark_palette(&self) -> f32 {
        if self.story_mode {
            self.light_change_because_of_rain().inverse_lerp(1.0, 0.0)
        } else {
            (self.time_until_rain() as f32).inverse_lerp(1000.0, 400.0)
        }
    }

    /// `dayNightCounter` after ticking up to `timer` from the cycle start.
    #[must_use]
    pub fn day_night_counter_at(&self, timer: i32) -> i32 {
        (timer - self.sun_down_start_time + 1).max(0)
    }

    /// `Update` (the counter always may tick: no gates, ghosts or dialogs ported).
    pub fn update(&mut self, rng: &mut UnityRandom) {
        self.update_timers();
        self.water_cycle.update(rng);
    }

    /// `RainCycle.Update` up to `brokenAntiGrav.Update`; the water cycle follows it.
    pub fn update_timers(&mut self) {
        if self.pause > 0 {
            self.pause -= 1;
        } else {
            self.timer += 1;
        }
        if self.timer >= self.sun_down_start_time {
            self.day_night_counter += 1;
        }
    }
}

#[cfg(test)]
#[path = "tests/rain_cycle.rs"]
mod tests;
