//! `RelationshipTracker`, the `AIModule` that keeps a dynamic relationship for each
//! tracked creature: its type and intensity this tick, the module that tracks it for
//! that type, and the AI's per-creature state. The relationship itself lives in its
//! representation (`trackerRep.dynamicRelationship`); its `Update` runs in
//! `game_tracker.rs`. Decompile: `RelationshipTracker.cs`.

use super::{Relationship, RelationshipType, RepKey};

/// The `AIModule` a relationship is sorted into (`ModuleToTrackRelationship`). The
/// modules themselves are lizard step 7; until then only which one is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingModule {
    PreyTracker,
    ThreatTracker,
    AgressionTracker,
}

impl TrackingModule {
    /// `LizardAI.ModuleToTrackRelationship`.
    #[must_use]
    pub fn for_lizard(r: Relationship) -> Option<Self> {
        match r.kind {
            RelationshipType::Afraid => Some(Self::ThreatTracker),
            RelationshipType::Eats | RelationshipType::Attacks => Some(Self::PreyTracker),
            RelationshipType::AgressiveRival => Some(Self::AgressionTracker),
            _ => None,
        }
    }

    /// The game's class name.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::PreyTracker => "PreyTracker",
            Self::ThreatTracker => "ThreatTracker",
            Self::AgressionTracker => "AgressionTracker",
        }
    }
}

/// `LizardAI.LizardTrackState`: what a lizard remembers of a player it tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LizardTrackState {
    /// The player held a spear when last seen.
    pub spear: bool,
    /// 1 a vulture mask, 2 a king's.
    pub vulture_mask: i32,
}

/// `RelationshipTracker.TrackedCreatureState` of the AIs ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedCreatureState {
    Lizard(LizardTrackState),
}

/// `RelationshipTracker.DynamicRelationship`.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicRelationship {
    pub current: Relationship,
    pub tracked_by_module: Option<TrackingModule>,
    pub tracked_by_module_weight: f32,
    pub state: Option<TrackedCreatureState>,
}

impl DynamicRelationship {
    /// The constructor's fields for `initial`, sorted into `module`.
    #[must_use]
    pub fn new(
        initial: Relationship,
        module: Option<TrackingModule>,
        state: Option<TrackedCreatureState>,
    ) -> Self {
        Self {
            current: initial,
            tracked_by_module: module,
            tracked_by_module_weight: 1.0,
            state,
        }
    }

    /// The lizard's track state of a player, if this is one.
    pub fn lizard_state_mut(&mut self) -> Option<&mut LizardTrackState> {
        match self.state.as_mut()? {
            TrackedCreatureState::Lizard(s) => Some(s),
        }
    }
}

/// `RelationshipTracker`: the order of its `relationships` (each by its
/// representation).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RelationshipTracker {
    pub relationships: Vec<RepKey>,
}
