//! Fixed-timestep clock: whole ticks to simulate plus the render interpolation fraction.

use std::time::Duration;

/// Simulation rate (ticks per second).
pub const TICK_RATE: u32 = 40;

/// Longest frame time accounted for; anything beyond is dropped so a stall (window
/// drag, breakpoint) does not trigger a burst of catch-up ticks.
const MAX_FRAME: Duration = Duration::from_millis(250);

/// Accumulates real time and hands out fixed ticks.
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    tick: Duration,
    accumulator: Duration,
}

impl Default for FixedTimestep {
    fn default() -> Self {
        Self::new(TICK_RATE)
    }
}

impl FixedTimestep {
    /// A clock ticking `rate` times per second.
    ///
    /// # Panics
    /// If `rate` is 0.
    #[must_use]
    pub fn new(rate: u32) -> Self {
        assert!(rate > 0, "tick rate must be positive");
        Self {
            tick: Duration::from_secs(1) / rate,
            accumulator: Duration::ZERO,
        }
    }

    /// Adds `elapsed` real time (capped) and returns how many ticks to run now.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accumulator += elapsed.min(MAX_FRAME);
        let mut ticks = 0;
        while self.accumulator >= self.tick {
            self.accumulator -= self.tick;
            ticks += 1;
        }
        ticks
    }

    /// Fraction of a tick accumulated since the last tick, in `0..1`; the render
    /// interpolation factor between the previous and the latest simulated state.
    #[must_use]
    pub fn alpha(&self) -> f32 {
        self.accumulator.as_secs_f32() / self.tick.as_secs_f32()
    }

    /// Drops accumulated time (e.g. after a respawn).
    pub fn reset(&mut self) {
        self.accumulator = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: Duration = Duration::from_millis(25);

    #[test]
    fn whole_ticks_and_remainder() {
        let mut t = FixedTimestep::new(40);
        assert_eq!(t.advance(Duration::from_millis(60)), 2);
        assert!((t.alpha() - 0.4).abs() < 1e-4);
        assert_eq!(t.advance(Duration::from_millis(15)), 1);
        assert!(t.alpha().abs() < 1e-4);
    }

    #[test]
    fn short_frames_accumulate() {
        let mut t = FixedTimestep::new(40);
        let frame = Duration::from_micros(8_333); // 120 Hz
        let ticks: u32 = (0..12).map(|_| t.advance(frame)).sum();
        // 12 * 8.333 ms = 99.996 ms: 3 whole ticks.
        assert_eq!(ticks, 3);
        assert!(t.alpha() > 0.99 && t.alpha() < 1.0);
    }

    #[test]
    fn long_stall_is_capped() {
        let mut t = FixedTimestep::new(40);
        assert_eq!(t.advance(Duration::from_secs(5)), 10);
    }

    #[test]
    fn reset_clears_alpha() {
        let mut t = FixedTimestep::new(40);
        t.advance(TICK / 2);
        t.reset();
        assert_eq!(t.alpha(), 0.0);
    }
}
