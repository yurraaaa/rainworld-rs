//! `RainTracker.Utility` (base game: no locusts, Remix's daddy and deer branches and
//! precycles are DLC): 0 for night creatures and those ignoring the cycle; in a story
//! session it rises from 0 at 4000 ticks before the rain to 1 at 800 (an S-curve),
//! elsewhere from 400 to 80. Read off the world's `RainCycle` timer (user decision
//! 2026-10-21: as the game does, though no rain falls yet). Decompile: `RainTracker.cs`.
//!
//! Mirrors: RainTracker.cs

use crate::FloatExt;
use crate::math::float_ext::SCurve;

/// `RainTracker` (no state of its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RainTracker;

impl RainTracker {
    #[must_use]
    pub fn utility(night_or_ignores_cycle: bool, story: bool, time_until_rain: i32) -> f32 {
        if night_or_ignores_cycle {
            return 0.0;
        }
        let t = time_until_rain as f32;
        let x = if story {
            t.inverse_lerp(800.0, 4000.0).s_curve(0.1)
        } else {
            t.inverse_lerp(80.0, 400.0).s_curve(0.5)
        };
        x.inverse_lerp(1.0, 0.0)
    }
}

#[cfg(test)]
#[path = "tests/rain_tracker.rs"]
mod tests;
