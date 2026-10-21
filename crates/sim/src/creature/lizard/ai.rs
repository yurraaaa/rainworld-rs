//! `LizardAI`'s state: its behaviour, its modules (path finder, tracker, noise, den
//! finder, prey, threat and aggression trackers, utility comparer, relationship, stuck
//! and discomfort trackers), the idle spot's counters and the moods the behaviours read.
//! The update is `RainWorldGame::lizard_ai_update` (`game_lizard_ai.rs`); the modules
//! that read the world are `world/ai_modules.rs`. Decompile: `LizardAI.cs`.
//!
//! Mirrors: LizardAI.cs (Behavior, LizardInjuryTracker, fields), ArtificialIntelligence.cs (fields)

use rw_assets::CreatureType;

use crate::FloatExt;
use crate::creature::{
    AgressionTracker, DenFinder, DiscomfortTracker, NoiseTracker, PreyTracker, RelationshipTracker,
    RepKey, StuckTracker, ThreatTracker, Tracker, UtilityComparer, WorldCoordinate,
};
use crate::float_ext::SCurve;

/// `LizardAI.Behavior`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LizardBehavior {
    Idle,
    Hunt,
    Flee,
    Travelling,
    EscapeRain,
    ReturnPrey,
    Injured,
    Fighting,
    Frustrated,
    ActingOutMission,
    Lurk,
    InvestigateSound,
    GoToSpitPos,
    FollowFriend,
}

impl LizardBehavior {
    const ALL: [Self; 14] = [
        Self::Idle,
        Self::Hunt,
        Self::Flee,
        Self::Travelling,
        Self::EscapeRain,
        Self::ReturnPrey,
        Self::Injured,
        Self::Fighting,
        Self::Frustrated,
        Self::ActingOutMission,
        Self::Lurk,
        Self::InvestigateSound,
        Self::GoToSpitPos,
        Self::FollowFriend,
    ];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Hunt => "Hunt",
            Self::Flee => "Flee",
            Self::Travelling => "Travelling",
            Self::EscapeRain => "EscapeRain",
            Self::ReturnPrey => "ReturnPrey",
            Self::Injured => "Injured",
            Self::Fighting => "Fighting",
            Self::Frustrated => "Frustrated",
            Self::ActingOutMission => "ActingOutMission",
            Self::Lurk => "Lurk",
            Self::InvestigateSound => "InvestigateSound",
            Self::GoToSpitPos => "GoToSpitPos",
            Self::FollowFriend => "FollowFriend",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.value() == value)
    }
}

/// `LizardAI`.
#[derive(Debug, Clone, PartialEq)]
pub struct LizardAi {
    /// The `LizardPather` (a number in the world's path finders).
    pub path_finder: Option<usize>,
    pub behavior: LizardBehavior,
    pub current_utility: f32,
    pub run_speed: f32,
    /// A dev tool's fixed `runSpeed` (MCP `set_destination`, D18): while set, the
    /// behaviours' bodies are skipped and the destination is the tool's.
    pub dev_run_speed: Option<f32>,
    pub stranded: bool,
    /// `ArtificialIntelligence.timeInRoom`.
    pub time_in_room: i32,
    /// `LizardAI`'s own `timeInRoom` (it hides the base's).
    pub lizard_time_in_room: i32,
    /// `ArtificialIntelligence.lastRoom`.
    pub last_room: i32,
    pub panic: i32,
    pub excitement: f32,
    pub fear: f32,
    pub hunger: f32,
    pub rain_fear: f32,
    pub last_distress_length: i32,
    pub idle_counter: i32,
    pub idle_spot_win_streak: i32,
    pub forbidden_idle_spot: WorldCoordinate,
    pub idle_restlessness: i32,
    pub unable_to_find_comfortable_position: i32,
    pub stuck: StuckTracker,
    pub discomfort: DiscomfortTracker,
    pub tracker: Tracker,
    pub noise: NoiseTracker,
    pub relationships: RelationshipTracker,
    pub den_finder: DenFinder,
    pub prey: PreyTracker,
    pub threat: ThreatTracker,
    pub agression: AgressionTracker,
    pub utility: UtilityComparer,
    pub focus_creature: Option<RepKey>,
    pub casual_aggression_target: Option<RepKey>,
    /// `usedToVultureMask` (no vulture masks are ported, so it stays 0).
    pub used_to_vulture_mask: i32,
}

impl LizardAi {
    /// The constructor's state for a lizard of `kind` (`has_den`: its abstract AI has
    /// a den position; else the den finder's mapping is reset by the caller).
    #[must_use]
    pub fn new(kind: CreatureType, has_den: bool) -> Self {
        Self {
            path_finder: None,
            behavior: LizardBehavior::Idle,
            current_utility: 0.0,
            run_speed: 0.0,
            dev_run_speed: None,
            stranded: false,
            time_in_room: 0,
            lizard_time_in_room: 0,
            last_room: -1,
            panic: 0,
            excitement: 0.5,
            fear: 0.0,
            hunger: 0.0,
            rain_fear: 0.0,
            last_distress_length: 0,
            idle_counter: 0,
            idle_spot_win_streak: 0,
            forbidden_idle_spot: WorldCoordinate::new(-1, -1, -1, -1),
            idle_restlessness: 0,
            unable_to_find_comfortable_position: 0,
            stuck: StuckTracker::lizard(),
            discomfort: DiscomfortTracker::default(),
            tracker: Tracker::lizard(),
            noise: NoiseTracker::default(),
            relationships: RelationshipTracker::default(),
            den_finder: DenFinder::new(has_den),
            prey: PreyTracker::lizard(kind == CreatureType::RedLizard),
            threat: ThreatTracker::new(3),
            agression: AgressionTracker::new(0.001, 0.001),
            utility: UtilityComparer::lizard(kind),
            focus_creature: None,
            casual_aggression_target: None,
            used_to_vulture_mask: 0,
        }
    }

    /// The representations the AI and its graphics refer to (kept when the tracker
    /// drops them from its list).
    #[must_use]
    pub fn held_reps(&self, look: Option<RepKey>) -> Vec<RepKey> {
        let mut held: Vec<RepKey> = self.focus_creature.into_iter().collect();
        held.extend(self.casual_aggression_target);
        held.extend(look);
        held.extend(self.noise.held_reps());
        held.extend(self.prey.reps());
        held.extend(self.prey.current);
        held.extend(self.threat.reps());
        held.extend(self.threat.most_threatening);
        held.extend(self.agression.reps());
        held.extend(self.agression.highest_target);
        held
    }

    /// `LizardAI.Update`'s `noiseTracker.hearingSkill`: 2 for black (and the Watcher's
    /// indigo) lizards, else 1.3 standing still down to 0.6 at a run speed of 0.6. The
    /// rot module's (More Slugcats, Watcher) is not ported.
    pub fn set_hearing_skill(&mut self, kind: CreatureType) {
        self.noise.hearing_skill =
            if matches!(kind, CreatureType::BlackLizard | CreatureType::IndigoLizard) {
                2.0
            } else {
                self.run_speed.lerp_map(0.0, 0.6, 1.3, 0.6)
            };
    }

    /// `LizardInjuryTracker.Utility` (Remix's no-arena-fleeing is DLC).
    #[must_use]
    pub fn injury_utility(health: f32) -> f32 {
        (1.0 - health.clamp(0.0, 1.0))
            .inverse_lerp(0.2, 0.9)
            .s_curve(0.01)
    }

    /// `CombinedFear`.
    #[must_use]
    pub fn combined_fear(&self) -> f32 {
        self.fear.max(self.rain_fear.mathf_pow(3.0))
    }

    /// `WantToStayInDenUntilEndOfCycle`: escaping the rain or injured (Remix's
    /// no-arena-fleeing is DLC).
    #[must_use]
    pub fn want_to_stay_in_den_until_end_of_cycle(&self) -> bool {
        matches!(
            self.behavior,
            LizardBehavior::EscapeRain | LizardBehavior::Injured
        )
    }
}

impl Default for LizardAi {
    fn default() -> Self {
        Self::new(CreatureType::GreenLizard, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `LizardInjuryTracker.Utility`: `SCurve(InverseLerp(0.2, 0.9, 1 - health), 0.01)`,
    /// 0 down to 80 % health, 1 at 10 %.
    #[test]
    fn injury_utility() {
        assert_eq!(LizardAi::injury_utility(1.0), 0.0);
        assert_eq!(LizardAi::injury_utility(0.8), 0.0);
        assert_eq!(LizardAi::injury_utility(0.1), 1.0);
        // 60 %: InverseLerp gives 2/7, the steep S-curve (k 0.01) keeps it near 0.
        let x = (0.4f32 - 0.2) / 0.7 * 2.0;
        let expect = 0.01 * x / (0.01 - x + 1.0) * 0.5;
        assert!((LizardAi::injury_utility(0.6) - expect).abs() < 1e-6);
    }

    #[test]
    fn behavior_names_round_trip() {
        for b in LizardBehavior::ALL {
            assert_eq!(LizardBehavior::from_value(b.value()), Some(b));
        }
    }
}
