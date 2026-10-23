//! `CreatureLooker`: which tracked creature a creature's graphics look at, chosen over
//! its AI's tracker. Its owner (`ILookingAtCreatures`) supplies the forced look
//! creature, the interest bonus and looking at nothing. Decompile: `CreatureLooker.cs`.
//!
//! The looker reads the tracker through [`LookCandidate`]s, a copy taken when it runs:
//! the reads that change a representation (the `VisualContact` getter clearing the
//! field, `BestGuessForPosition` settling a dirty guess) are recorded on the copy and
//! applied back to the tracker afterwards, as the game makes them in place.
//!
//! Mirrors: CreatureLooker.cs

use std::cell::Cell;

use rw_random::UnityRandom;

use crate::creature::{
    CreatureId, Relationship, RelationshipType, RepKey, Sighting, WorldCoordinate,
};
use crate::{FloatExt, Vec2};

/// A representation as the looker reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct LookCandidate {
    pub key: RepKey,
    pub creature: CreatureId,
    /// In the tracker's list (a dropped one is only read as the current look target).
    pub listed: bool,
    best_guess: WorldCoordinate,
    /// Whether `BestGuessForPosition` was read.
    pub guessed: Cell<bool>,
    /// The raw `visualContact`, as the getter leaves it.
    pub visual_field: Cell<bool>,
    /// The getter's check: realized in the looker's room.
    pub in_view: bool,
    pub lowest_generation: i32,
    /// `TicksSinceSeen`.
    pub ticks_since_seen: i32,
    pub body_size: f32,
    pub scaryness: f32,
    /// The looker's template's relationship to it (`CreatureRelationship`).
    pub relationship: Relationship,
    /// The realized creature's main chunk.
    pub main_pos: Option<Vec2>,
    pub main_vel: Option<Vec2>,
    /// The owner's `CreatureInterestBonus` factor.
    pub interest: f32,
    /// One of the looker's grasps holds it.
    pub grabbed: bool,
}

/// What a candidate is built from.
#[derive(Debug, Clone, PartialEq)]
pub struct LookCandidateParts {
    pub key: RepKey,
    pub creature: CreatureId,
    pub listed: bool,
    pub best_guess: WorldCoordinate,
    pub visual_field: bool,
    pub in_view: bool,
    pub lowest_generation: i32,
    pub ticks_since_seen: i32,
    pub body_size: f32,
    pub scaryness: f32,
    pub relationship: Relationship,
    pub main_pos: Option<Vec2>,
    pub main_vel: Option<Vec2>,
    pub interest: f32,
    pub grabbed: bool,
}

impl LookCandidate {
    #[must_use]
    pub fn new(p: LookCandidateParts) -> Self {
        Self {
            key: p.key,
            creature: p.creature,
            listed: p.listed,
            best_guess: p.best_guess,
            guessed: Cell::new(false),
            visual_field: Cell::new(p.visual_field),
            in_view: p.in_view,
            lowest_generation: p.lowest_generation,
            ticks_since_seen: p.ticks_since_seen,
            body_size: p.body_size,
            scaryness: p.scaryness,
            relationship: p.relationship,
            main_pos: p.main_pos,
            main_vel: p.main_vel,
            interest: p.interest,
            grabbed: p.grabbed,
        }
    }

    /// The `VisualContact` getter: false (and the field cleared) out of view.
    pub fn visual_contact(&self) -> bool {
        if !self.in_view {
            self.visual_field.set(false);
        }
        self.visual_field.get()
    }

    /// `BestGuessForPosition()`.
    pub fn best_guess(&self) -> WorldCoordinate {
        self.guessed.set(true);
        self.best_guess
    }

    /// `EstimatedChanceOfFinding`.
    #[must_use]
    pub fn chance_of_finding(&self) -> f32 {
        Sighting {
            visual_contact: self.visual_field.get(),
            lowest_generation: self.lowest_generation,
            ticks_since_seen: self.ticks_since_seen,
        }
        .estimated_chance_of_finding()
    }

    /// `ReevaluateLookObject`'s score for a looker at `at` (after the room check), with
    /// `coin` its random interest when it drew one.
    fn score(&self, at: WorldCoordinate, coin: Option<f32>, favored: bool, bonus: f32) -> f32 {
        let mut s = 1.0
            + self.body_size.lerp(1.0, 0.5)
                * self.relationship.intensity
                * self.chance_of_finding()
                * self.scaryness.lerp(1.0, 0.7);
        if let Some(c) = coin {
            s += c;
        }
        if self.visual_contact()
            && let Some(v) = self.main_vel
        {
            s *= 1.0 + v.length();
        }
        let g = self.best_guess().tile;
        let d = Vec2::new(g.0 as f32, g.1 as f32)
            .distance(Vec2::new(at.tile.0 as f32, at.tile.1 as f32));
        s /= d.lerp(1.0, 0.6);
        if favored {
            s *= bonus;
        }
        if self.relationship.kind == RelationshipType::Ignores {
            s /= 2.0;
        }
        self.best_guess();
        s *= self.interest;
        if self.grabbed {
            s = f32::MIN;
        }
        s
    }
}

/// `CreatureLooker`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureLooker {
    pub look_creature: Option<RepKey>,
    pub look_focus_delay: i32,
    pub chance_to_look_at_nothing: f32,
    pub frames_between_focus_change: i32,
}

impl CreatureLooker {
    #[must_use]
    pub fn new(chance_to_look_at_nothing: f32, frames_between_focus_change: i32) -> Self {
        Self {
            look_creature: None,
            look_focus_delay: 0,
            chance_to_look_at_nothing,
            frames_between_focus_change,
        }
    }

    /// `Update`: the forced creature, else with the delay run out a
    /// `1 / framesBetweenFocusChange` chance to look again: over `chanceToLookAtNothing`
    /// a reevaluation, else nothing. Returns whether the owner should `LookAtNothing`.
    pub fn update(
        &mut self,
        candidates: &[LookCandidate],
        forced: Option<RepKey>,
        at: WorldCoordinate,
        rng: &mut UnityRandom,
    ) -> bool {
        if self.look_focus_delay > 0 {
            self.look_focus_delay -= 1;
        }
        if forced.is_some() {
            self.look_creature = forced;
        } else if self.look_focus_delay < 1
            && rng.value() < 1.0 / self.frames_between_focus_change as f32
        {
            self.look_focus_delay = self.frames_between_focus_change;
            if rng.value() > self.chance_to_look_at_nothing {
                self.reevaluate(candidates, None, 0.0, at, rng);
                return false;
            }
            self.look_creature = None;
            return true;
        }
        false
    }

    /// `ReevaluateLookObject(favorThisCreature, bonus)` for a looker at `at`: the listed
    /// creatures in its room scored by size, relationship, how well it knows where they
    /// are, speed and distance (a coin of random interest without a favourite).
    pub fn reevaluate(
        &mut self,
        candidates: &[LookCandidate],
        favor: Option<RepKey>,
        bonus: f32,
        at: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        self.look_creature = None;
        let mut best = 0.0;
        let random_interest = rng.value() < 0.5 && favor.is_none();
        for c in candidates.iter().filter(|c| c.listed) {
            if c.best_guess().room != at.room {
                continue;
            }
            let coin = random_interest.then(|| rng.value());
            let s = c.score(at, coin, favor == Some(c.key), bonus);
            if s > best {
                best = s;
                self.look_creature = Some(c.key);
            }
        }
    }

    /// The candidate looked at.
    #[must_use]
    pub fn target<'a>(&self, candidates: &'a [LookCandidate]) -> Option<&'a LookCandidate> {
        let k = self.look_creature?;
        candidates.iter().find(|c| c.key == k)
    }
}

#[cfg(test)]
#[path = "tests/looker.rs"]
mod tests;
