//! `RainWorldGame.RequestHeavyAi`: at most three heavy AI updates a tick, and while any
//! request was turned away for want of room, the AIs that ran since stay skipped. The
//! game keeps this in statics; the port keeps it in the game (a new game starts with
//! the list of the last tick, which no AI can read before the first tick's reset: a
//! lizard flees only from its second update). Lizards call it while fleeing.
//!
//! Mirrors: RainWorldGame.cs (RequestHeavyAi, MaxConcurrentHeavyAi, Update)

use super::RainWorldGame;
use crate::creature::EntityId;

/// `_concurrentHeavyAi`, `_concurrentHeavyAiDelayedExceptLastRunAis`, `_lastRunAiIds`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeavyAiThrottle {
    pub concurrent: i32,
    pub delayed_except_last_run: i32,
    pub last_run: Vec<EntityId>,
}

impl HeavyAiThrottle {
    pub const MAX_CONCURRENT: i32 = 3;

    /// `RequestHeavyAi(creature)`: no for an AI in the last-run list; else yes (and
    /// listed) while fewer than three ran this tick; else no, counted as delayed.
    pub fn request(&mut self, id: EntityId) -> bool {
        if self.last_run.contains(&id) {
            return false;
        }
        if self.concurrent < Self::MAX_CONCURRENT {
            self.last_run.push(id);
            self.concurrent += 1;
            return true;
        }
        self.delayed_except_last_run += 1;
        false
    }

    /// The end of `RainWorldGame.Update`: the list is cleared only when nothing waited.
    pub fn end_of_tick(&mut self) {
        if self.delayed_except_last_run <= 0 {
            self.last_run.clear();
        }
        self.concurrent = 0;
        self.delayed_except_last_run = 0;
    }
}

impl RainWorldGame {
    /// The heavy-AI throttle as it stands.
    #[must_use]
    pub fn heavy_ai(&self) -> &HeavyAiThrottle {
        &self.heavy_ai
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: i32) -> EntityId {
        EntityId::new(-1, n)
    }

    /// One heavy AI alone runs every tick (nothing waits, so the list clears at each
    /// tick's end); asked twice in one tick, the second is turned away without counting
    /// as delayed.
    #[test]
    fn a_lone_heavy_ai_runs_every_tick() {
        let mut h = HeavyAiThrottle::default();
        let mut runs = Vec::new();
        for _ in 0..3 {
            runs.push(h.request(id(1)));
            runs.push(h.request(id(1)));
            assert_eq!(h.delayed_except_last_run, 0);
            h.end_of_tick();
        }
        assert_eq!(runs, [true, false, true, false, true, false]);
    }

    /// Three a tick; the fourth is delayed, which keeps the three listed (and skipped)
    /// next tick, when the fourth runs and nothing waits, so the list clears.
    #[test]
    fn three_a_tick_and_the_delayed_keep_the_list() {
        let mut h = HeavyAiThrottle::default();
        let first: Vec<bool> = (1..=4).map(|n| h.request(id(n))).collect();
        assert_eq!(first, [true, true, true, false]);
        assert_eq!(h.delayed_except_last_run, 1);
        h.end_of_tick();
        assert_eq!(h.last_run, [id(1), id(2), id(3)]);
        let second: Vec<bool> = (1..=4).map(|n| h.request(id(n))).collect();
        assert_eq!(second, [false, false, false, true]);
        h.end_of_tick();
        assert!(h.last_run.is_empty());
        let third: Vec<bool> = (1..=4).map(|n| h.request(id(n))).collect();
        assert_eq!(third, [true, true, true, false]);
    }
}
