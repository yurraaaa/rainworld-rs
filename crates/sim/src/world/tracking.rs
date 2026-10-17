//! What a realized AI's tracker answers about one creature, as other code asks it:
//! `ArtificialIntelligence.DynamicRelationship`, `StaticRelationship`, `LizardAI.
//! LikeOfPlayer` and `CurrentPlayerAggression`. Decompile: `ArtificialIntelligence.cs`
//! 368-398, `LizardAI.cs` 1463-1488.

use super::World;
use crate::FloatExt;
use crate::creature::{
    Community, CreatureId, CreatureRep, RealizedCreature, Relationship, Tracker,
};

impl World {
    /// The tracker of creature `ai`'s realized AI (a lizard's; the overseer's AI has
    /// none, a bat fly no AI).
    #[must_use]
    pub fn tracker_of(&self, ai: CreatureId) -> Option<&Tracker> {
        Some(&self.creature(ai).lizard()?.ai.tracker)
    }

    /// `realizedCreature.room` of creature `c`: `None` unrealized or in a shortcut (the
    /// player's as the game last reported its body).
    #[must_use]
    pub fn physical_room(&self, c: CreatureId) -> Option<i32> {
        let a = self.creature(c);
        match a.realized.as_ref()? {
            RealizedCreature::Player => self
                .player_body
                .as_ref()
                .filter(|b| b.id == c)
                .and_then(|b| b.room),
            RealizedCreature::Lizard(l) => l.room.filter(|_| !l.in_shortcut),
            RealizedCreature::Fly(f) => f.room.filter(|_| !f.in_shortcut),
            RealizedCreature::Overseer(o) => (o.in_room && !o.in_shortcut).then_some(a.pos.room),
        }
    }

    /// `StaticRelationship(other)`: the template's.
    #[must_use]
    pub fn static_relationship(&self, ai: CreatureId, other: CreatureId) -> Relationship {
        self.creature(ai)
            .template
            .relationship_to(self.creature(other).kind())
    }

    /// `DynamicRelationship(other)` over `tracker`: the representation's dynamic
    /// relationship, else the static one.
    #[must_use]
    pub fn dynamic_relationship_in(
        &self,
        ai: CreatureId,
        tracker: &Tracker,
        other: CreatureId,
    ) -> Relationship {
        tracker
            .representation_for(other)
            .and_then(CreatureRep::current_relationship)
            .unwrap_or_else(|| self.static_relationship(ai, other))
    }

    /// `DynamicRelationship(other)` of creature `ai`'s realized AI.
    #[must_use]
    pub fn dynamic_relationship(&self, ai: CreatureId, other: CreatureId) -> Relationship {
        match self.tracker_of(ai) {
            Some(t) => self.dynamic_relationship_in(ai, t, other),
            None => self.static_relationship(ai, other),
        }
    }

    /// `LizardAI.LikeOfPlayer(player)`: 0 without a representation; the session's like
    /// of the player for the lizard's community in this region, pulled toward
    /// `-spawnDataEvil` by its size, then toward the social memory's temporary like of
    /// the player by its size. The gift branch is taming (skipped).
    #[must_use]
    pub fn lizard_like_of_player(&self, ai: CreatureId, player: Option<&CreatureRep>) -> f32 {
        let Some(rep) = player else {
            return 0.0;
        };
        let c = self.creature(ai);
        let community = c.template.violence().community;
        let mut like = match rw_save::CommunityId::from_value(community.value()) {
            Some(id) if community != Community::None => {
                self.communities
                    .like_of_player(id, self.region_name(), self.session_difficulty)
            }
            _ => 0.0,
        };
        let evil = c.lizard().map_or(0.0, |l| l.spawn_data_evil);
        like = like.lerp(-evil, evil.abs());
        let subject = self.creature(rep.creature).id;
        let temp = c
            .state
            .social_memory
            .as_ref()
            .map_or(0.0, |m| m.temp_like(subject));
        like.lerp(temp, temp.abs())
    }

    /// `RealAI.CurrentPlayerAggression(player)`: the base `ArtificialIntelligence`'s 1
    /// for an overseer (`OverseerAI` does not override it); a bat fly has no abstract
    /// AI. A lizard's is 1 until its tracker relates to the player, then
    /// `InverseLerp(0.5, 0, LikeOfPlayer)`.
    #[must_use]
    pub fn current_player_aggression(&self, ai: CreatureId, player: CreatureId) -> Option<f32> {
        let c = self.creature(ai);
        if let Some(l) = c.lizard() {
            c.ai.as_ref().filter(|a| a.real_ai)?;
            let rep =
                l.ai.tracker
                    .representation_for(player)
                    .filter(|r| r.dynamic_relationship.is_some());
            return Some(match rep {
                None => 1.0,
                Some(r) => self
                    .lizard_like_of_player(ai, Some(r))
                    .inverse_lerp(0.5, 0.0),
            });
        }
        c.overseer_ai()
            .and_then(|a| a.real_ai.as_ref())
            .map(|_| 1.0)
    }
}
