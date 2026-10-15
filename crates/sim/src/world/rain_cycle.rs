//! `RainCycle` and its `WaterLevelCycle` (base game, no precycles): built by the `World`
//! constructor with its draws, the timer ticked by `RainWorldGame.Update`. The rain
//! itself (`RainHit`, `GlobalRain`) is not ported.

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
}

impl RainCycle {
    /// The `World` constructor's story branch: the cycle's minutes, then `RainCycle`'s
    /// constructor (six draws in all).
    pub fn new_story(character: SlugcatName, rng: &mut UnityRandom) -> Self {
        let v = rng.value();
        let t = if character == SlugcatName::Yellow {
            0.35 + 0.65 * v.mathf_pow(1.2)
        } else {
            v
        };
        Self::new(CYCLE_TIME_MIN.lerp(CYCLE_TIME_MAX, t) / 60.0, rng)
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
        }
    }

    /// `TimeUntilRain`.
    #[must_use]
    pub fn time_until_rain(&self) -> i32 {
        self.cycle_length - self.timer
    }

    /// `Update` (the counter always may tick: no gates, ghosts or dialogs ported).
    pub fn update(&mut self, rng: &mut UnityRandom) {
        if self.pause > 0 {
            self.pause -= 1;
        } else {
            self.timer += 1;
        }
        if self.timer >= self.sun_down_start_time {
            self.day_night_counter += 1;
        }
        self.water_cycle.update(rng);
    }
}
