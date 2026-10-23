//! `PreyTracker`'s state: the remembered prey (by representation), the current one,
//! the frustration of a prey the path finder cannot get to. What reads the world
//! (`Update`, `Attractiveness`, `Reachable`, `Utility`, `DistanceEstimation`) is
//! `creature/ai_modules/world.rs`. Decompile: `PreyTracker.cs`.
//!
//! Mirrors: PreyTracker.cs

use crate::FloatExt;
use crate::creature::{RepKey, WorldCoordinate};

/// `PreyTracker.TrackedPrey`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedPrey {
    pub rep: RepKey,
    pub unreachable_counter: i32,
    pub at_position_but_cant_see_counter: i32,
    /// `default(WorldCoordinate)` at first: room 0, tile (0, 0), node 0.
    pub last_best_guess_pos: WorldCoordinate,
}

impl TrackedPrey {
    #[must_use]
    pub fn new(rep: RepKey) -> Self {
        Self {
            rep,
            unreachable_counter: 0,
            at_position_but_cant_see_counter: 0,
            last_best_guess_pos: WorldCoordinate::new(0, 0, 0, 0),
        }
    }
}

/// `PreyTracker`.
#[derive(Debug, Clone, PartialEq)]
pub struct PreyTracker {
    pub max_remembered_creatures: usize,
    pub persistance_bias: f32,
    pub prey: Vec<TrackedPrey>,
    /// `currentPrey`'s representation.
    pub current: Option<RepKey>,
    pub sure_to_get_prey_distance: f32,
    pub sure_to_lose_prey_distance: f32,
    pub frustration: f32,
    pub frustration_speed: f32,
    pub give_up_on_unreachable_prey: i32,
    pub give_up_on_ghost_generation: i32,
}

impl PreyTracker {
    /// The constructor (`successEstimationDistReliance` is stored nowhere).
    #[must_use]
    pub fn new(
        max_remembered: usize,
        persistance_bias: f32,
        sure_get: f32,
        sure_lose: f32,
    ) -> Self {
        Self {
            max_remembered_creatures: max_remembered,
            persistance_bias,
            prey: Vec::new(),
            current: None,
            sure_to_get_prey_distance: sure_get,
            sure_to_lose_prey_distance: sure_lose,
            frustration: 0.0,
            frustration_speed: 0.0125,
            give_up_on_unreachable_prey: 400,
            give_up_on_ghost_generation: 50,
        }
    }

    /// `LizardAI`'s `PreyTracker(this, 5, 2, 3, 70, 0.5)`; a red lizard gives up on
    /// unreachable prey after 1800 ticks.
    #[must_use]
    pub fn lizard(red: bool) -> Self {
        let mut p = Self::new(5, 2.0, 3.0, 70.0);
        if red {
            p.give_up_on_unreachable_prey = 1800;
        }
        p
    }

    /// `MostAttractivePrey`.
    #[must_use]
    pub fn most_attractive(&self) -> Option<RepKey> {
        self.current
    }

    /// `ForgetPrey`: every entry of a representation of `reps`.
    pub fn forget(&mut self, reps: &[RepKey]) {
        self.prey.retain(|p| !reps.contains(&p.rep));
    }

    /// `InverseLerp(giveUpOnGhostGeneration, giveUpOnGhostGeneration / 2, generation)`.
    #[must_use]
    pub fn ghost_generation_fac(&self, lowest_generation: i32) -> f32 {
        let g = self.give_up_on_ghost_generation;
        (lowest_generation as f32).inverse_lerp(g as f32, (g / 2) as f32)
    }

    /// `Utility`'s distance term: halfway between the sure-to-get to sure-to-lose falloff
    /// and a quarter of the way into it over the distance.
    #[must_use]
    pub fn distance_term(&self, dist: f32) -> f32 {
        let (get, lose) = (
            self.sure_to_get_prey_distance,
            self.sure_to_lose_prey_distance,
        );
        (1.0 - dist.inverse_lerp(get, lose)).lerp(get.lerp(lose, 0.25) / dist, 0.5)
    }

    /// The frustration's step: up when the prey is in the room and out of the path
    /// finder's reach, else down four times as fast.
    pub fn step_frustration(&mut self, rising: bool) {
        let d = if rising {
            self.frustration_speed
        } else {
            -self.frustration_speed * 4.0
        };
        self.frustration = (self.frustration + d).clamp(0.0, 1.0);
    }

    #[must_use]
    pub fn reps(&self) -> Vec<RepKey> {
        self.prey.iter().map(|p| p.rep).collect()
    }
}

#[cfg(test)]
#[path = "tests/prey_tracker.rs"]
mod tests;
