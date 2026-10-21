//! `AgressionTracker`'s state (the game's spelling): an anger per tracked rival that
//! drifts toward a target set by distance, the prey's nearness and the room's
//! attraction. The relationship update that reads the world is `world/ai_modules.rs`.
//! Decompile: `AgressionTracker.cs`.
//!
//! Mirrors: AgressionTracker.cs

use super::RepKey;
use crate::FloatExt;

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

    #[must_use]
    pub fn reps(&self) -> Vec<RepKey> {
        self.creatures.iter().map(|c| c.rep).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anger_steps_toward_its_target() {
        let a = AgressionTracker::new(0.001, 0.001);
        assert!((a.step_anger(0.1, 0.5) - 0.101).abs() < 1e-7);
        assert_eq!(a.step_anger(0.5, 0.5), 0.5);
        assert_eq!(a.step_anger(0.1, 0.1005), 0.1005);
        assert!((a.step_anger(0.5, 0.0) - 0.499).abs() < 1e-7);
        let mut a = a;
        a.highest_anger = 0.35;
        assert_eq!(a.utility(), 0.0);
    }
}
