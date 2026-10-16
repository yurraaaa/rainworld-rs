//! What the threat music (`ThreatDetermination.ThreatOfCreature`, 73996) and the
//! guide's `CreatureDangerScore` (231884) read of a realized creature: its template's
//! `dangerousToPlayer`, `DangerPos`, and its realized AI's view of the player
//! (`CurrentPlayerAggression` and the tracker's representation).
//!
//! Every creature realized so far (the overseer, the bat fly and the green lizard)
//! uses the base `DangerPos` (`mainBodyChunk.pos`). The base-game overrides arrive with
//! their creatures: `Lizard.DangerPos` (197872, the tongue's tip while it lashes out),
//! `Vulture.DangerPos` (284462, a king vulture's shooting or charging tusk), and
//! `CurrentPlayerAggression` of `CicadaAI` (150153), `DropBugAI` (178192), `LizardAI`
//! (202014), `BigNeedleWormAI` (213919) and `ScavengerAI` (255506).

use rw_assets::CreatureType;

use super::{AbstractCreature, CreatureId, EntityId, RealizedCreature};
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

impl AbstractCreature {
    /// `abstractAI.RealAI.CurrentPlayerAggression(player)`: the base
    /// `ArtificialIntelligence`'s 1 for an overseer (`OverseerAI` does not override it);
    /// a bat fly has no abstract AI (`FlyAI` is no `ArtificialIntelligence`). `LizardAI`'s
    /// is 1 while its tracker has no relationship with the player, as the stand-in AI's
    /// never has (the tracker is lizard step 6).
    #[must_use]
    pub fn current_player_aggression(&self) -> Option<f32> {
        if self.lizard().is_some() {
            return self.ai.as_ref().filter(|a| a.real_ai).map(|_| 1.0);
        }
        self.overseer_ai()
            .and_then(|a| a.real_ai.as_ref())
            .map(|_| 1.0)
    }

    /// The player's representation in `abstractAI.RealAI.tracker`: `OverseerAI` has no
    /// tracker module, a bat fly no AI.
    #[must_use]
    pub fn player_tracker_rep(&self) -> Option<TrackedRep> {
        None
    }
}

impl World {
    /// Creature `id` as the danger scores read it, while it is realized (not the
    /// player).
    #[must_use]
    pub fn creature_danger(&self, id: CreatureId) -> Option<CreatureDanger> {
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
            player_rep: c.player_tracker_rep(),
            player_aggression: c.current_player_aggression(),
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
            .filter_map(|&c| self.world().creature_danger(c))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rw_assets::{ActiveMods, CreatureType as T};

    use crate::creature::CreatureTemplate;

    /// `dangerousToPlayer` as `InitStaticWorld` (125287-126484) and the lizard breeds'
    /// `danger` (305845-306552, set at 307309) leave it; children inherit (305458).
    #[test]
    fn dangerous_to_player_per_template() {
        let table = [
            (T::StandardGroundCreature, 0.0),
            (T::Slugcat, 0.0),
            (T::LizardTemplate, 0.0),
            (T::PinkLizard, 0.45),
            (T::GreenLizard, 0.45),
            (T::BlueLizard, 0.35),
            (T::YellowLizard, 0.4),
            (T::WhiteLizard, 0.5),
            (T::RedLizard, 0.8),
            (T::BlackLizard, 0.45),
            (T::Salamander, 0.4),
            (T::CyanLizard, 0.65),
            (T::Fly, 0.0),
            (T::Leech, 0.05),
            (T::SeaLeech, 0.07),
            (T::Snail, 0.0),
            (T::Vulture, 0.7),
            (T::KingVulture, 0.9),
            (T::GarbageWorm, 0.0),
            (T::LanternMouse, 0.0),
            (T::CicadaA, 0.0),
            (T::CicadaB, 0.0),
            (T::Spider, 0.1),
            (T::JetFish, 0.0),
            (T::BigEel, 0.8),
            (T::Deer, 0.0),
            (T::TubeWorm, 0.0),
            (T::DaddyLongLegs, 0.85),
            (T::BrotherLongLegs, 0.75),
            (T::TentaclePlant, 0.5),
            (T::PoleMimic, 0.5),
            (T::MirosBird, 0.6),
            (T::TempleGuard, 0.0),
            (T::Centipede, 0.3),
            (T::RedCentipede, 0.9),
            (T::SmallCentipede, 0.0),
            (T::Centiwing, 0.4),
            (T::Scavenger, 0.65),
            (T::Overseer, 0.0),
            (T::VultureGrub, 0.0),
            (T::Hazer, 0.0),
            (T::EggBug, 0.0),
            (T::BigSpider, 0.35),
            (T::SpitterSpider, 0.5),
            (T::BigNeedleWorm, 0.5),
            (T::SmallNeedleWorm, 0.0),
            (T::DropBug, 0.4),
        ];
        assert_eq!(table.len(), 47);
        for (kind, danger) in table {
            let t = CreatureTemplate::of(kind, ActiveMods::NONE);
            assert_eq!(t.kind, kind);
            assert_eq!(t.dangerous_to_player, danger, "{kind:?}");
        }
    }
}
