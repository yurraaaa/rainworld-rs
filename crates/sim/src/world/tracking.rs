//! What a realized AI's tracker answers about one creature, as other code asks it:
//! `ArtificialIntelligence.DynamicRelationship`, `StaticRelationship`, `LizardAI.
//! LikeOfPlayer` and `CurrentPlayerAggression`; and how uncomfortable a tile is to it
//! (`DiscomfortTracker.DiscomfortOfTile`). Decompile: `ArtificialIntelligence.cs`
//! 368-398, `LizardAI.cs` 1463-1488, `DiscomfortTracker.cs`.
//!
//! Mirrors: ArtificialIntelligence.cs, LizardAI.cs, DiscomfortTracker.cs

use super::World;
use crate::creature::{
    Community, CreatureId, CreatureRep, PathFinder, RealizedCreature, Relationship,
    RelationshipType, Tracker, WorldCoordinate,
};
use crate::{FloatExt, Vec2};

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

    /// `DiscomfortTracker.DiscomfortOfTile(test)` of AI `ai` (`pf` its path finder): 0
    /// outside its room. Each tracked creature it is uncomfortable with makes the tiles
    /// near where it is believed to be (closer by its intensity, fading over 1000 ticks
    /// unseen, more where that creature can go) uncomfortable; with
    /// `uncomfortableWithUnknownNoises` (no lizard's) and a sound to examine, so do the
    /// unknown sources within 600 px. The uncomfortable item (`ItemTracker`, the
    /// scavengers') is not ported; last the tile's threat pulls it toward 1 (by
    /// `InverseLerp(0.1, 1, threat)^0.25`). More Slugcats' blizzard warmth and the Watcher's locusts are left out with
    /// their DLCs. The best guesses are read without settling them.
    #[must_use]
    pub fn discomfort_of_tile(
        &self,
        ai: CreatureId,
        test: WorldCoordinate,
        pf: &PathFinder,
    ) -> f32 {
        let Some(l) = self.creature(ai).lizard() else {
            return 0.0;
        };
        if test.room != self.creature(ai).pos.room || !test.tile_defined() {
            return 0.0;
        }
        let t = &l.ai.tracker;
        let mut num = 0.0f32;
        for rep in &t.reps {
            let Some(rel) = rep
                .current_relationship()
                .filter(|r| r.kind == RelationshipType::Uncomfortable)
            else {
                continue;
            };
            let guess = self.rep_best_guess(ai, &mut rep.clone(), Some(pf));
            if guess.room != test.room {
                continue;
            }
            let Some(map) = self.realized(test.room).and_then(|r| r.ai_map.as_deref()) else {
                continue;
            };
            let dx = (guess.tile.0 - test.tile.0) as f32;
            let dy = (guess.tile.1 - test.tile.1) as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            let template = self.creature(rep.creature).template;
            let mut v = dist.inverse_lerp(1.0 + rel.intensity * 40.0, 0.0);
            v *= (rep.ticks_since_seen(t.see_around_corners) as f32).inverse_lerp(1000.0, 50.0);
            v *= 0.5 + 0.5 * map.accessibility_for_creature(test.tile, &template);
            num = num.lerp(num.max(v.mathf_pow(0.5)), 0.5 + 0.5 * rel.intensity);
        }
        let unknown = l.ai.discomfort.uncomfortable_with_unknown_noises;
        let noise = &l.ai.noise;
        if unknown > 0.0 && noise.utility() > 0.0 {
            let mid = Vec2::middle_of_tile(test.tile);
            for s in noise.sources.iter().filter(|s| s.creature_rep.is_none()) {
                let mut v = s.pos.distance(mid).inverse_lerp(600.0, 0.0);
                v *= (s.age as f32).inverse_lerp(1000.0, 50.0);
                num = num.lerp(
                    num.max(v.mathf_pow(0.5)),
                    (0.5 + 0.5 * noise.utility()) * unknown,
                );
            }
        }
        let threat = self.threat_of_tile(ai, test).inverse_lerp(0.1, 1.0);
        num.lerp(1.0, threat.mathf_pow(0.25))
    }
}
