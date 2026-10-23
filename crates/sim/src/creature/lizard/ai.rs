//! `LizardAI`'s state: its behaviour, its modules (path finder, tracker, noise, den
//! finder, prey, threat and aggression trackers, utility comparer, mission,
//! relationship, stuck and discomfort trackers), the signals between lizards, the idle spot's counters and the moods the behaviours read.
//! The update is `RainWorldGame::lizard_ai_update` (`creature/lizard/ai_turn.rs`); the modules
//! that read the world are `creature/ai_modules/world.rs`. Decompile: `LizardAI.cs`.
//!
//! Mirrors: LizardAI.cs (Behavior, LizardCommunication, LizardInjuryTracker, fields), ArtificialIntelligence.cs (fields)

use rw_assets::CreatureType;

use crate::FloatExt;
use crate::creature::{
    AgressionTracker, CreatureId, DenFinder, DiscomfortTracker, FriendTracker, MissionTracker,
    NoiseTracker, PreyTracker, RelationshipTracker, RepKey, StuckTracker, ThreatTracker, Tracker,
    UtilityComparer, WorldCoordinate,
};
use crate::math::float_ext::SCurve;

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

/// `LizardAI.LizardCommunication` (the game's spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LizardCommunication {
    Agression,
    Submission,
}

impl LizardCommunication {
    pub const ALL: [Self; 2] = [Self::Agression, Self::Submission];

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::Agression => "Agression",
            Self::Submission => "Submission",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.value() == value)
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
    pub missions: MissionTracker,
    pub friend: FriendTracker,
    pub focus_creature: Option<RepKey>,
    pub casual_aggression_target: Option<RepKey>,
    /// `submittedTo`: the lizard it last submitted to.
    pub submitted_to: Option<CreatureId>,
    /// The last signal it sent, to whom and at which game tick (port-only, for MCP).
    pub last_signal: Option<(LizardCommunication, CreatureId, u64)>,
    /// `usedToVultureMask` (no vulture masks are ported, so it stays 0).
    pub used_to_vulture_mask: i32,
    pub attempt_bite_frames: i32,
    /// This tick's `RequestHeavyAi` answer while fleeing (port-only, for MCP; `None`
    /// when not asked).
    pub heavy_ai_granted: Option<bool>,
    /// What the last Flee update's `FleeTo` returned (port-only, for MCP).
    pub flee_to: Option<WorldCoordinate>,
}

impl LizardAi {
    /// The constructor's state for a lizard of `kind` (`has_den`: its abstract AI has
    /// a den position; else the den finder's mapping is reset by the caller) and the
    /// friend tracker's `tamingDifficlty`.
    #[must_use]
    pub fn new(kind: CreatureType, has_den: bool, taming_difficulty: f32) -> Self {
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
            missions: MissionTracker::default(),
            friend: FriendTracker::new(taming_difficulty),
            focus_creature: None,
            casual_aggression_target: None,
            submitted_to: None,
            last_signal: None,
            used_to_vulture_mask: 0,
            attempt_bite_frames: 0,
            heavy_ai_granted: None,
            flee_to: None,
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
        // A green lizard of middling dominance: `tamingDifficulty` 0.8.
        Self::new(CreatureType::GreenLizard, false, 0.8)
    }
}

#[cfg(test)]
#[path = "tests/ai.rs"]
mod tests;
