//! `SocialMemory` (52354): what a creature feels about others it met, by their ID. It
//! lives in the `CreatureState` of templates with `socialMemory` and is saved with it
//! (`Social<cC>...`). The abstract AI follows a creature it likes or hates strongly
//! (`MigrationBehavior`); the realized AIs' trackers read and change it.
//!
//! Mirrors: SocialMemory.cs

use rw_save::cs_float::CsFloat;

use super::EntityId;
use crate::FloatExt;

/// `SocialMemory.Relationship`.
#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub subject: EntityId,
    pub like: f32,
    pub fear: f32,
    pub know: f32,
    pub temp_like: f32,
    pub temp_fear: f32,
    /// `unrecognizedSaveStrings`: `key<rB>value` entries kept as read.
    pub unrecognized: Vec<String>,
}

impl Relationship {
    #[must_use]
    pub fn new(subject: EntityId) -> Self {
        Self {
            subject,
            like: 0.0,
            fear: 0.0,
            know: 0.0,
            temp_like: 0.0,
            temp_fear: 0.0,
            unrecognized: Vec::new(),
        }
    }

    /// `InfluenceLike`: damped by how well it is known.
    pub fn influence_like(&mut self, change: f32) {
        self.like = (self.like + change * 1.0_f32.lerp(0.1, self.know)).clamp(-1.0, 1.0);
    }

    pub fn influence_fear(&mut self, change: f32) {
        self.fear = (self.fear + change * 1.0_f32.lerp(0.1, self.know)).clamp(-1.0, 1.0);
    }

    pub fn influence_know(&mut self, amount_of_lerp_upwards: f32) {
        self.know = self.know.lerp(1.0, amount_of_lerp_upwards.min(0.9));
    }

    pub fn influence_temp_like(&mut self, change: f32) {
        self.temp_like = (self.temp_like + change).clamp(-1.0, 1.0);
    }

    pub fn influence_temp_fear(&mut self, change: f32) {
        self.temp_fear = (self.temp_fear + change).clamp(-1.0, 1.0);
    }

    /// `EvenOutTemps`: the temporary values step toward the lasting ones.
    pub fn even_out_temps(&mut self, speed: f32) {
        self.temp_like = if self.temp_like < self.like {
            self.like.min(self.temp_like + speed)
        } else {
            self.like.max(self.temp_like - speed)
        };
        self.temp_fear = if self.temp_fear < self.fear {
            self.fear.min(self.temp_fear + speed)
        } else {
            self.fear.max(self.temp_fear - speed)
        };
    }

    /// `ToString`: empty without like or fear (knowledge alone is not saved).
    #[must_use]
    pub fn save_string(&self) -> String {
        if self.like == 0.0 && self.fear == 0.0 {
            return String::new();
        }
        let mut text = format!("REL<rA>ID.{}.{}", self.subject.spawner, self.subject.number);
        if self.like != 0.0 {
            text.push_str(&format!("<rA>L<rB>{}", self.like.cs_string()));
        }
        if self.fear != 0.0 {
            text.push_str(&format!("<rA>F<rB>{}", self.fear.cs_string()));
        }
        if self.know != 0.0 {
            text.push_str(&format!("<rA>K<rB>{}", self.know.cs_string()));
        }
        for u in &self.unrecognized {
            text.push_str("<rA>");
            text.push_str(u);
        }
        text
    }

    /// `FromString`: the temporary values start at the lasting ones. `None` where the
    /// game would throw (a malformed ID or number).
    #[must_use]
    pub fn from_save_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split("<rA>").collect();
        let mut r = Self::new(EntityId::from_save_string(parts.get(1)?)?);
        for part in parts.iter().skip(2) {
            let kv: Vec<&str> = part.split("<rB>").collect();
            match kv[0] {
                "L" => {
                    r.like = kv.get(1)?.trim().parse().ok()?;
                    r.temp_like = r.like;
                }
                "F" => {
                    r.fear = kv.get(1)?.trim().parse().ok()?;
                    r.temp_fear = r.fear;
                }
                "K" => r.know = kv.get(1)?.trim().parse().ok()?,
                _ if !part.trim().is_empty() && kv.len() >= 2 => {
                    r.unrecognized.push((*part).to_owned());
                }
                _ => {}
            }
        }
        Some(r)
    }
}

/// `SocialMemory`: the relationships in the order they were made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SocialMemory {
    pub relationships: Vec<Relationship>,
}

impl SocialMemory {
    /// `GetRelationship`.
    #[must_use]
    pub fn get(&self, subject: EntityId) -> Option<&Relationship> {
        self.relationships
            .iter()
            .find(|r| r.subject.same_entity(subject))
    }

    /// `GetOrInitiateRelationship`.
    pub fn get_or_initiate(&mut self, subject: EntityId) -> &mut Relationship {
        let i = match self
            .relationships
            .iter()
            .position(|r| r.subject.same_entity(subject))
        {
            Some(i) => i,
            None => {
                self.relationships.push(Relationship::new(subject));
                self.relationships.len() - 1
            }
        };
        &mut self.relationships[i]
    }

    /// `DiscardRelationship`: every relationship with that ID.
    pub fn discard(&mut self, subject: EntityId) {
        self.relationships
            .retain(|r| !r.subject.same_entity(subject));
    }

    /// `GetLike`: 0 for a stranger.
    #[must_use]
    pub fn like(&self, subject: EntityId) -> f32 {
        self.get(subject).map_or(0.0, |r| r.like)
    }

    #[must_use]
    pub fn temp_like(&self, subject: EntityId) -> f32 {
        self.get(subject).map_or(0.0, |r| r.temp_like)
    }

    #[must_use]
    pub fn know(&self, subject: EntityId) -> f32 {
        self.get(subject).map_or(0.0, |r| r.know)
    }

    pub fn even_out_all_temps(&mut self, speed: f32) {
        for r in &mut self.relationships {
            r.even_out_temps(speed);
        }
    }

    /// `CycleTick`, once per cycle the region slept: weak likes and fears fade (below
    /// 0.5 by up to 15 %), knowledge below 0.5 by up to 85 %, and a relationship with
    /// both like and fear under 0.25 is forgotten.
    pub fn cycle_tick(&mut self) {
        for i in (0..self.relationships.len()).rev() {
            let r = &mut self.relationships[i];
            r.like *= r.like.abs().lerp_map(0.5, 0.0, 1.0, 0.85);
            r.fear *= r.fear.abs().lerp_map(0.5, 0.0, 1.0, 0.85);
            r.know *= r.know.lerp_map(0.5, 0.0, 1.0, 0.15);
            if r.like.abs() < 0.25 && r.fear.abs() < 0.25 {
                self.relationships.remove(i);
            }
        }
    }

    /// `ToString`: the saved relationships joined by `<smA>`; an unsaved one (no like or
    /// fear) leaves out its separator too, so a trailing one can remain, as in the game.
    #[must_use]
    pub fn save_string(&self) -> String {
        let mut text = String::new();
        let last = self.relationships.len().saturating_sub(1);
        for (i, r) in self.relationships.iter().enumerate() {
            let s = r.save_string();
            if !s.is_empty() {
                text.push_str(&s);
                if i < last {
                    text.push_str("<smA>");
                }
            }
        }
        text
    }

    /// `FromString`: the `REL` entries; `None` where the game would throw.
    #[must_use]
    pub fn from_save_string(s: &str) -> Option<Self> {
        let mut m = Self::default();
        for entry in s.split("<smA>") {
            if entry.split("<rA>").next() == Some("REL") {
                m.relationships.push(Relationship::from_save_string(entry)?);
            }
        }
        Some(m)
    }
}

#[cfg(test)]
#[path = "tests/social_memory.rs"]
mod tests;
