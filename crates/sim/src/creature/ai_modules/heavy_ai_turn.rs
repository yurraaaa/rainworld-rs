//! `RainWorldGame.RequestHeavyAi`: at most three heavy AI updates a tick, and while any
//! request was turned away for want of room, the AIs that ran since stay skipped. The
//! game keeps this in statics; the port keeps it in the game (a new game starts with
//! the list of the last tick, which no AI can read before the first tick's reset: a
//! lizard flees only from its second update). Lizards call it while fleeing.
//!
//! Mirrors: RainWorldGame.cs (RequestHeavyAi, MaxConcurrentHeavyAi, Update)

use crate::creature::EntityId;
use crate::game::RainWorldGame;

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
#[path = "tests/heavy_ai_turn.rs"]
mod tests;
