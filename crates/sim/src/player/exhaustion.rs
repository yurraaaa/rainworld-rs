//! The slugcat's stats, `aerobicLevel` and malnourished exhaustion: the `Player`
//! constructor's and `SetMalnourished`'s chunk masses, `AerobicIncrease`
//! (`Player.AerobicIncrease`) and the `Player.Update` block after the map discovery, base
//! game: More Slugcats' Saint, wounds and Gourmand and the Watcher's camouflage are
//! not ported.
//!
//! Mirrors: Player.cs (Player ctor, SetMalnourished, AerobicIncrease, Update)

use rw_random::UnityRandom;

use super::{Animation, Player};
use crate::Sounds;
use crate::campaign::SlugcatStats;
use crate::math::float_ext::FloatExt;

impl Player {
    /// `slugcatStats`.
    #[must_use]
    pub fn stats(&self) -> &SlugcatStats {
        &self.stats
    }

    /// `Player.Malnourished` (`slugcatStats.malnourished`; no creature malnourishes in the
    /// base game).
    #[must_use]
    pub fn malnourished(&self) -> bool {
        self.stats.malnourished
    }

    #[must_use]
    pub fn aerobic_level(&self) -> f32 {
        self.aerobic_level
    }

    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// The session's `characterStats` becoming this player's `slugcatStats` (the
    /// constructor, or `SetMalnourished`): each chunk weighs `0.7 * bodyWeightFac / 2`.
    pub fn set_stats(&mut self, stats: SlugcatStats) {
        self.stats = stats;
        let mass = 0.7 * stats.body_weight_fac / 2.0;
        for c in &mut self.body.chunks {
            c.mass = mass;
        }
    }

    /// `AerobicIncrease`: `f / 9` more, at most 1.
    pub(crate) fn aerobic_increase(&mut self, f: f32) {
        self.aerobic_level = (self.aerobic_level + f / 9.0).min(1.0);
    }

    /// The malnourished exhaustion (out of breath at `aerobicLevel` 1 until it falls
    /// below 0.4: slowed, now and then stunned for 7 ticks or knocked off its feet), then
    /// the aerobic recovery toward `1 - airInLungs`. Runs before this tick's input is
    /// read, so `input[0]` is last tick's.
    pub fn exhaustion_update(&mut self, rng: &mut UnityRandom, sounds: &mut Sounds) {
        if self.malnourished() {
            if self.aerobic_level == 1.0 {
                self.exhausted = true;
            } else if self.aerobic_level < 0.4 {
                self.exhausted = false;
            }
            if self.exhausted {
                let slow = self.aerobic_level.lerp_map(0.7, 0.4, 6.0, 0.0) as i32;
                self.slow_movement_stun = self.slow_movement_stun.max(slow);
                if self.aerobic_level > 0.9 && rng.value() < 0.05 {
                    self.stun(7, rng, sounds);
                }
                if self.aerobic_level > 0.9 && rng.value() < 0.1 {
                    self.standing = false;
                }
                if !self.lungs_exhausted || self.animation == Animation::SurfaceSwim {
                    self.swim_cycle += 0.05;
                }
            } else {
                let slow = self.aerobic_level.lerp_map_pow(1.0, 0.4, 2.0, 0.0, 2.0) as i32;
                self.slow_movement_stun = self.slow_movement_stun.max(slow);
            }
        } else {
            self.exhausted = false;
        }
        if self.lungs_exhausted {
            self.aerobic_level = 1.0;
        } else {
            let rate = if self.malnourished() { 1.2 } else { 1.0 };
            let still = self.input[0].x == 0 && self.input[0].y == 0;
            let ticks = if still { 400.0 } else { 1100.0 }
                * (1.0 + 3.0 * self.aerobic_level.inverse_lerp(0.9, 1.0));
            self.aerobic_level = (1.0 - self.air_in_lungs).max(self.aerobic_level - rate / ticks);
        }
    }

    /// `aerobicLevel = value` (tests and dev tools).
    pub fn set_aerobic_level(&mut self, value: f32) {
        self.aerobic_level = value;
    }
}
