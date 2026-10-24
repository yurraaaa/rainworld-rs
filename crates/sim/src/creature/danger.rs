//! What the threat music (`ThreatDetermination.ThreatOfCreature`) and the
//! guide's `CreatureDangerScore` read of a realized creature: its template's
//! `dangerousToPlayer`, `DangerPos`, and its realized AI's view of the player
//! (`CurrentPlayerAggression` and the tracker's representation).
//!
//! Every creature realized so far (the overseer, the bat fly and the green lizard)
//! uses the base `DangerPos` (`mainBodyChunk.pos`). The base-game overrides arrive with
//! their creatures: `Lizard.DangerPos` (the tongue's tip while it lashes out),
//! `Vulture.DangerPos` (a king vulture's shooting or charging tusk), and
//! `CurrentPlayerAggression` of `CicadaAI`, `DropBugAI`,
//! `BigNeedleWormAI` and `ScavengerAI`. `LizardAI`'s is in
//! `creature/ai_modules/tracker_world.rs`.
//!
//! Mirrors: ThreatDetermination.cs, Creature.cs, Lizard.cs, ArtificialIntelligence.cs

use rw_assets::CreatureType;

use super::{CreatureId, EntityId, RealizedCreature};
use crate::Vec2;
use crate::world::World;

/// The player's representation in a realized AI's `tracker`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackedRep {
    /// `VisualContact`.
    pub visual_contact: bool,
    /// `EstimatedChanceOfFinding`.
    pub estimated_chance_of_finding: f32,
}

/// A realized creature as the danger scores read it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureDanger {
    pub creature: CreatureId,
    pub id: EntityId,
    pub kind: CreatureType,
    pub dangerous_to_player: f32,
    pub dead: bool,
    pub danger_pos: Vec2,
    pub first_chunk_vel: Vec2,
    /// `None` without a realized AI with a tracker, or when it does not track the player.
    pub player_rep: Option<TrackedRep>,
    /// `RealAI.CurrentPlayerAggression(player)`; `None` without a realized AI.
    pub player_aggression: Option<f32>,
}

impl RealizedCreature {
    /// `DangerPos`; `None` for the player, whose body the game keeps. A lizard's is its
    /// head (the tongue's tip of the tongued breeds is not ported).
    #[must_use]
    pub fn danger_pos(&self) -> Option<Vec2> {
        match self {
            Self::Player => None,
            Self::Overseer(b) => Some(b.chunk().pos),
            Self::Fly(f) => Some(f.chunk().pos),
            Self::Lizard(l) => Some(l.head().pos),
        }
    }

    /// `firstChunk.vel`.
    #[must_use]
    pub fn first_chunk_vel(&self) -> Option<Vec2> {
        match self {
            Self::Player => None,
            Self::Overseer(b) => Some(b.chunk().vel),
            Self::Fly(f) => Some(f.chunk().vel),
            Self::Lizard(l) => Some(l.head().vel),
        }
    }

    /// `Creature.dead`.
    #[must_use]
    pub fn dead(&self) -> Option<bool> {
        match self {
            Self::Player => None,
            Self::Overseer(b) => Some(b.dead),
            Self::Fly(f) => Some(f.dead),
            Self::Lizard(l) => Some(l.dead),
        }
    }
}

impl World {
    /// The player's representation in creature `id`'s tracker, as `ThreatOfCreature`
    /// reads it: the `VisualContact` getter (false out of the tracker's room) and
    /// `EstimatedChanceOfFinding`. `OverseerAI` has no tracker, a bat fly no AI.
    #[must_use]
    pub fn player_tracker_rep(&self, id: CreatureId, player: CreatureId) -> Option<TrackedRep> {
        let t = self.tracker_of(id)?;
        let r = t.representation_for(player)?;
        let in_view = self.physical_room(player).is_some()
            && self.physical_room(player) == Some(self.creature(id).pos.room);
        let mut sighting = r.sighting(t.see_around_corners);
        sighting.visual_contact &= in_view;
        Some(TrackedRep {
            visual_contact: sighting.visual_contact,
            estimated_chance_of_finding: sighting.estimated_chance_of_finding(),
        })
    }

    /// Creature `id` as the danger scores read it, while it is realized (not the
    /// player), toward `player`.
    #[must_use]
    pub fn creature_danger(&self, id: CreatureId, player: CreatureId) -> Option<CreatureDanger> {
        let c = self.creature(id);
        let realized = c.realized.as_ref()?;
        Some(CreatureDanger {
            creature: id,
            id: c.id,
            kind: c.template.kind,
            dangerous_to_player: c.template.dangerous_to_player,
            dead: realized.dead()?,
            danger_pos: realized.danger_pos()?,
            first_chunk_vel: realized.first_chunk_vel()?,
            player_rep: self.player_tracker_rep(id, player),
            player_aggression: self.current_player_aggression(id, player),
        })
    }
}

impl crate::RainWorldGame {
    /// The realized creatures of the player's abstract room but the player, in the
    /// room's order.
    #[must_use]
    pub fn creatures_around_player(&self) -> Vec<CreatureDanger> {
        let player = self.player_creature();
        self.world()
            .room_creatures(self.player_pos().room)
            .iter()
            .filter(|&&c| c != player)
            .filter_map(|&c| self.world().creature_danger(c, player))
            .collect()
    }
}

#[cfg(test)]
#[path = "tests/danger.rs"]
mod tests;
