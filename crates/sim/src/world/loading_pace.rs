//! Headless ticking at the game's frame rate while a room loads (user, 2026-10-24, D14):
//! a tick without a window takes microseconds, so a preparer thread's few milliseconds
//! would span a hundred ticks where the game's 40 Hz frames give it one or two. A driver
//! that runs ticks faster than real time (MCP, integration tests) waits before a tick
//! until a frame has passed since the last one, but only while a room is loading.
//!
//! Mirrors: - (port-only: headless pacing of threaded loading)

use std::time::{Duration, Instant};

use super::World;

/// When the previous paced tick ran.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadingPace {
    last: Option<Instant>,
}

impl LoadingPace {
    /// One frame at 40 Hz.
    pub const FRAME: Duration = Duration::from_millis(25);

    /// Call before each tick: sleeps out the rest of a frame since the previous tick
    /// while `world` has a room loading.
    pub fn before_tick(&mut self, world: &World) {
        if world.rooms_loading()
            && let Some(wait) = self.last.and_then(|t| Self::FRAME.checked_sub(t.elapsed()))
        {
            std::thread::sleep(wait);
        }
        self.last = Some(Instant::now());
    }
}
