//! Frame timing: the simulation rate and Unity's `Time.deltaTime` as the process
//! manager receives it.

use std::time::{Duration, Instant};

/// Simulation rate (ticks per second, `MainLoopProcess.framesPerSecond`).
pub const TICK_RATE: u32 = 40;

/// Real time between frames, capped as Unity caps `Time.deltaTime`.
#[derive(Debug, Clone, Default)]
pub struct FrameClock {
    last: Option<Instant>,
}

impl FrameClock {
    /// Unity's default `Time.maximumDeltaTime` (1/3 s; the game's own setting in
    /// `globalgamemanagers` is not checked).
    pub const MAX_DELTA: Duration = Duration::from_millis(333);

    /// Seconds since the previous call (0 on the first).
    pub fn delta(&mut self, now: Instant) -> f32 {
        let elapsed = self.last.map_or(Duration::ZERO, |t| now - t);
        self.last = Some(now);
        elapsed.min(Self::MAX_DELTA).as_secs_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_is_zero_and_stalls_are_capped() {
        let mut clock = FrameClock::default();
        let t = Instant::now();
        assert_eq!(clock.delta(t), 0.0);
        assert!((clock.delta(t + Duration::from_millis(25)) - 0.025).abs() < 1e-6);
        assert!((clock.delta(t + Duration::from_secs(10)) - 0.333).abs() < 1e-6);
    }
}
