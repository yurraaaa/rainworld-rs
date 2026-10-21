//! `UtilityComparer` and the `FloatTweener` smoothers it uses: each compared module's
//! utility, weighted, optionally smoothed, and the highest of them. The modules'
//! utilities are taken as a snapshot ([`RawUtilities`]) since each is a pure read of
//! its module. Decompile: `UtilityComparer.cs`, `FloatTweener.cs`; the lizard's
//! weights are `LizardAI`'s constructor.
//!
//! Mirrors: UtilityComparer.cs, FloatTweener.cs

use rw_assets::CreatureType;

use crate::FloatExt;

/// `FloatTweener.TweenType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenType {
    None,
    Lerp,
    Tick,
}

/// `FloatTweener.FloatTweenBasic`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatTweenBasic {
    pub kind: TweenType,
    pub speed: f32,
}

impl FloatTweenBasic {
    #[must_use]
    pub fn lerp(speed: f32) -> Self {
        Self {
            kind: TweenType::Lerp,
            speed,
        }
    }

    /// `BaseTween`.
    #[must_use]
    pub fn tween(self, a: f32, b: f32) -> f32 {
        if a == b {
            return a;
        }
        match self.kind {
            TweenType::Lerp => a.lerp(b, self.speed),
            TweenType::Tick if a < b => (a + self.speed).min(b),
            TweenType::Tick => (a - self.speed).max(b),
            TweenType::None => b,
        }
    }
}

/// `FloatTweener.FloatTween` subclasses the AIs use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatTween {
    Basic(FloatTweenBasic),
    /// `FloatTweenUpAndDown`: `up` while rising toward the target, else `down`.
    UpAndDown {
        up: FloatTweenBasic,
        down: FloatTweenBasic,
    },
}

impl FloatTween {
    #[must_use]
    pub fn tween(self, a: f32, b: f32) -> f32 {
        match self {
            Self::Basic(t) => t.tween(a, b),
            Self::UpAndDown { up, down } => {
                if a < b {
                    up.tween(a, b)
                } else {
                    down.tween(a, b)
                }
            }
        }
    }
}

/// The `AIModule`s a lizard's comparer compares, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilityModule {
    ThreatTracker,
    PreyTracker,
    RainTracker,
    LizardInjuryTracker,
    AgressionTracker,
    MissionTracker,
    NoiseTracker,
    FriendTracker,
}

impl UtilityModule {
    pub const COUNT: usize = 8;

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The game's class name.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::ThreatTracker => "ThreatTracker",
            Self::PreyTracker => "PreyTracker",
            Self::RainTracker => "RainTracker",
            Self::LizardInjuryTracker => "LizardInjuryTracker",
            Self::AgressionTracker => "AgressionTracker",
            Self::MissionTracker => "MissionTracker",
            Self::NoiseTracker => "NoiseTracker",
            Self::FriendTracker => "FriendTracker",
        }
    }
}

/// Each compared module's `Utility()` now, by [`UtilityModule::index`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RawUtilities(pub [f32; UtilityModule::COUNT]);

impl RawUtilities {
    #[must_use]
    pub fn get(&self, m: UtilityModule) -> f32 {
        self.0[m.index()]
    }

    pub fn set(&mut self, m: UtilityModule, v: f32) {
        self.0[m.index()] = v;
    }
}

/// `UtilityComparer.UtilityTracker`.
#[derive(Debug, Clone, PartialEq)]
pub struct UtilityTracker {
    pub module: UtilityModule,
    pub smoother: Option<FloatTween>,
    pub weight: f32,
    pub continuation_bonus: f32,
    pub smoothed_utility: f32,
    pub exponent: f32,
}

impl UtilityTracker {
    fn new(module: UtilityModule, smoother: Option<FloatTween>, weight: f32, bonus: f32) -> Self {
        Self {
            module,
            smoother,
            weight,
            continuation_bonus: bonus,
            smoothed_utility: 0.0,
            exponent: 1.0,
        }
    }

    fn unsmoothed(&self, raw: &RawUtilities) -> f32 {
        raw.get(self.module).mathf_pow(self.exponent) * self.weight
    }

    /// `SmoothedUtility`: the smoothed value, or the weighted utility now without a
    /// smoother.
    #[must_use]
    pub fn smoothed(&self, raw: &RawUtilities) -> f32 {
        if self.smoother.is_some() {
            self.smoothed_utility
        } else {
            self.unsmoothed(raw)
        }
    }
}

/// `UtilityComparer`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UtilityComparer {
    pub trackers: Vec<UtilityTracker>,
    /// `highestUtilityTracker`, by index.
    pub highest: Option<usize>,
}

impl UtilityComparer {
    /// `LizardAI`'s compared modules: threat (up by half, down by a tick of 0.0025),
    /// prey (lerp 0.15, weight 0.6), rain 0.9, injury 0.9 (red 0.4), aggression 0.5,
    /// mission 0.8, noise 0.2, friend 0.9. The lurk tracker (white, salamander) is
    /// those breeds'.
    #[must_use]
    pub fn lizard(kind: CreatureType) -> Self {
        let threat = FloatTween::UpAndDown {
            up: FloatTweenBasic::lerp(0.5),
            down: FloatTweenBasic {
                kind: TweenType::Tick,
                speed: 0.0025,
            },
        };
        let injury = if kind == CreatureType::RedLizard {
            0.4
        } else {
            0.9
        };
        let trackers = vec![
            UtilityTracker::new(UtilityModule::ThreatTracker, Some(threat), 1.0, 1.0),
            UtilityTracker::new(
                UtilityModule::PreyTracker,
                Some(FloatTween::Basic(FloatTweenBasic::lerp(0.15))),
                0.6,
                1.0,
            ),
            UtilityTracker::new(UtilityModule::RainTracker, None, 0.9, 1.0),
            UtilityTracker::new(UtilityModule::LizardInjuryTracker, None, injury, 1.0),
            UtilityTracker::new(UtilityModule::AgressionTracker, None, 0.5, 1.2),
            UtilityTracker::new(UtilityModule::MissionTracker, None, 0.8, 1.0),
            UtilityTracker::new(UtilityModule::NoiseTracker, None, 0.2, 1.2),
            UtilityTracker::new(UtilityModule::FriendTracker, None, 0.9, 1.2),
        ];
        Self {
            trackers,
            highest: None,
        }
    }

    /// `Update`: each tracker smoothed, the first with the strictly highest smoothed
    /// utility kept.
    pub fn update(&mut self, raw: &RawUtilities) {
        let mut best = f32::MIN;
        self.highest = None;
        for (i, t) in self.trackers.iter_mut().enumerate() {
            if let Some(s) = t.smoother {
                t.smoothed_utility = s.tween(t.smoothed_utility, t.unsmoothed(raw));
            }
            let u = t.smoothed(raw);
            if u > best {
                best = u;
                self.highest = Some(i);
            }
        }
    }

    #[must_use]
    pub fn tracker(&self, m: UtilityModule) -> Option<&UtilityTracker> {
        self.trackers.iter().find(|t| t.module == m)
    }

    pub fn tracker_mut(&mut self, m: UtilityModule) -> Option<&mut UtilityTracker> {
        self.trackers.iter_mut().find(|t| t.module == m)
    }

    /// `GetSmoothedNonWeightedUtility`.
    #[must_use]
    pub fn smoothed_non_weighted(&self, m: UtilityModule, raw: &RawUtilities) -> f32 {
        self.tracker(m).map_or(0.0, |t| t.smoothed(raw) / t.weight)
    }

    /// `HighestUtilityModule`.
    #[must_use]
    pub fn highest_module(&self) -> Option<UtilityModule> {
        self.highest.map(|i| self.trackers[i].module)
    }

    /// `HighestUtility`.
    #[must_use]
    pub fn highest_utility(&self, raw: &RawUtilities) -> f32 {
        self.highest.map_or(0.0, |i| self.trackers[i].smoothed(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `UtilityComparer.Update`: the threat smoother rises halfway, falls by 0.0025 a
    /// tick; prey lerps by 0.15 and weighs 0.6; the highest is strictly the largest,
    /// the first on a tie.
    #[test]
    fn lizard_comparer_smooths_and_picks_the_highest() {
        let mut c = UtilityComparer::lizard(CreatureType::GreenLizard);
        let mut raw = RawUtilities::default();
        c.update(&raw);
        assert_eq!(c.highest_module(), Some(UtilityModule::ThreatTracker));
        assert_eq!(c.highest_utility(&raw), 0.0);
        raw.set(UtilityModule::ThreatTracker, 1.0);
        raw.set(UtilityModule::PreyTracker, 1.0);
        c.update(&raw);
        assert_eq!(
            c.tracker(UtilityModule::ThreatTracker)
                .unwrap()
                .smoothed_utility,
            0.5
        );
        let prey = c
            .tracker(UtilityModule::PreyTracker)
            .unwrap()
            .smoothed_utility;
        assert!((prey - 0.6 * 0.15).abs() < 1e-6);
        raw.set(UtilityModule::ThreatTracker, 0.0);
        c.update(&raw);
        let threat = c
            .tracker(UtilityModule::ThreatTracker)
            .unwrap()
            .smoothed_utility;
        assert!((threat - 0.4975).abs() < 1e-6);
        assert!(
            (c.smoothed_non_weighted(UtilityModule::PreyTracker, &raw)
                - (0.09 + (0.6 - 0.09) * 0.15) / 0.6)
                .abs()
                < 1e-6
        );
        // Unsmoothed trackers read the utility now.
        raw.set(UtilityModule::RainTracker, 1.0);
        assert!((c.smoothed_non_weighted(UtilityModule::RainTracker, &raw) - 1.0).abs() < 1e-6);
        c.update(&raw);
        assert_eq!(c.highest_module(), Some(UtilityModule::RainTracker));
        assert!((c.highest_utility(&raw) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn tick_tween_stops_at_the_target() {
        let t = FloatTweenBasic {
            kind: TweenType::Tick,
            speed: 0.3,
        };
        assert_eq!(t.tween(0.0, 0.2), 0.2);
        assert_eq!(t.tween(1.0, 0.0), 0.7);
    }
}
