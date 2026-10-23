//! `AgressionTracker`'s state (the game's spelling): an anger per tracked rival that
//! drifts toward a target set by distance, the prey's nearness and the room's
//! attraction. The relationship update that reads the world is `world/ai_modules.rs`.
//! Decompile: `AgressionTracker.cs`.
//!
//! Mirrors: AgressionTracker.cs

use crate::FloatExt;
use crate::creature::RepKey;

/// `AgressionTracker.AngerRelationship`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngerRelationship {
    pub rep: RepKey,
    pub anger: f32,
    pub base_anger: f32,
}

/// `AgressionTracker`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgressionTracker {
    pub creatures: Vec<AngerRelationship>,
    pub highest_anger: f32,
    /// `highestAgressionTarget`'s representation.
    pub highest_target: Option<RepKey>,
    pub anger_speed_up: f32,
    pub anger_speed_down: f32,
}

impl AgressionTracker {
    #[must_use]
    pub fn new(anger_speed_up: f32, anger_speed_down: f32) -> Self {
        Self {
            creatures: Vec::new(),
            highest_anger: 0.0,
            highest_target: None,
            anger_speed_up,
            anger_speed_down,
        }
    }

    /// `Utility`.
    #[must_use]
    pub fn utility(&self) -> f32 {
        self.highest_anger.inverse_lerp(0.35, 1.0)
    }

    /// `AddCreature`: once per representation.
    pub fn add(&mut self, rep: RepKey, initial: f32, base: f32) {
        if self.creatures.iter().any(|c| c.rep == rep) {
            return;
        }
        self.creatures.push(AngerRelationship {
            rep,
            anger: initial,
            base_anger: base,
        });
    }

    /// `AngerRelationship.Update`'s step toward `target`.
    pub fn step_anger(&self, anger: f32, target: f32) -> f32 {
        if anger < target {
            (anger + self.anger_speed_up).clamp(0.0, target)
        } else if anger > target {
            (anger - self.anger_speed_down).clamp(target, 1.0)
        } else {
            anger
        }
    }

    /// `IncrementAnger`: the first relationship to `rep`, clamped to 0..1.
    pub fn increment_anger(&mut self, rep: Option<RepKey>, add: f32) {
        if let Some(c) = self.creatures.iter_mut().find(|c| Some(c.rep) == rep) {
            c.anger = (c.anger + add).clamp(0.0, 1.0);
        }
    }

    /// `SetAnger`: the anger, and the base anger unless `new_base` is -1, clamped.
    pub fn set_anger(&mut self, rep: Option<RepKey>, anger: f32, new_base: f32) {
        if let Some(c) = self.creatures.iter_mut().find(|c| Some(c.rep) == rep) {
            c.anger = anger.clamp(0.0, 1.0);
            if new_base != -1.0 {
                c.base_anger = new_base.clamp(0.0, 1.0);
            }
        }
    }

    /// `AgressionTarget`.
    #[must_use]
    pub fn target(&self) -> Option<RepKey> {
        self.highest_target
    }

    #[must_use]
    pub fn reps(&self) -> Vec<RepKey> {
        self.creatures.iter().map(|c| c.rep).collect()
    }
}

#[cfg(test)]
#[path = "tests/agression_tracker.rs"]
mod tests;
