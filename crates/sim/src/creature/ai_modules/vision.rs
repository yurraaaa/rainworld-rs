//! What an AI sees: `ArtificialIntelligence.VisualScore` and `VisualContact`, a lizard's
//! `VisualScore` (the angle off its head), and what makes a body easier or harder to
//! see (`BodyChunk.VisibilityBonus`, `Player.VisibilityBonus`, `Player.Sneak`).
//! Decompile: `ArtificialIntelligence.VisualContact` / `VisualScore`,
//! `LizardAI.VisualScore`, `BodyChunk.VisibilityBonus`, `Player.Sneak` and `Player.VisibilityBonus`.
//!
//! Mirrors: ArtificialIntelligence.cs, LizardAI.cs, BodyChunk.cs, Player.cs

use crate::world::AiMap;
use crate::{FloatExt, RoomExt, Vec2};

/// A template's `visualRadius`, `waterVision`, `throughSurfaceVision` and
/// `movementBasedVision`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureVision {
    pub visual_radius: f32,
    pub water_vision: f32,
    pub through_surface_vision: f32,
    pub movement_based_vision: f32,
}

/// What the eyes of a realized AI read of its body and room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eyes {
    pub vision: CreatureVision,
    /// `VisionPoint` (the main chunk).
    pub point: Vec2,
    /// A lizard's `perfectVisionAngle` and `periferalVisionAngle` with its head
    /// (chunk 0) and neck (chunk 1).
    pub lizard_angles: Option<LizardSight>,
    /// `Blinded`.
    pub blinded: bool,
}

/// What `LizardAI.VisualScore` reads of the lizard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LizardSight {
    pub perfect: f32,
    pub periferal: f32,
    pub head: Vec2,
    pub neck: Vec2,
}

impl Eyes {
    /// `VisualScore(lookAtPoint, bonus)`: 0 beyond the radius (stretched by `bonus`),
    /// else falling linearly to it; less across the water surface and in deep water,
    /// 0.5 less on a narrow-space tile; a lizard loses up to 1 by the angle off its
    /// head. No base-game `VisionObscurer` is ported (the spore puff's).
    #[must_use]
    pub fn visual_score(&self, map: &AiMap, has_water: bool, look: Vec2, bonus: f32) -> f32 {
        let radius = self.vision.visual_radius * (1.0 + bonus);
        let mut score = 0.0;
        if self.point.dist_less(look, radius) {
            score = self.point.distance(look).inverse_lerp(radius, 0.0);
            if has_water {
                let (a, b) = (self.point.tile(), look.tile());
                let eye_deep = map.clamped_tile(a.0, a.1).deep_water();
                let look_deep = map.clamped_tile(b.0, b.1).deep_water();
                if eye_deep != look_deep {
                    score -= 1.0 - self.vision.through_surface_vision;
                }
                if eye_deep || look_deep {
                    score -= 1.0 - self.vision.water_vision;
                }
            }
            let t = look.tile();
            if map.tile(t.0, t.1).narrow_space {
                score -= 0.5;
            }
        }
        if let Some(l) = self.lizard_angles {
            let head = (l.head - l.neck).normalized();
            let dot = head.dot((look - l.neck).normalized());
            score -= dot.inverse_lerp(l.perfect, l.periferal);
        }
        score
    }

    /// `VisualContact(lookAtPoint, bonus)`: not blinded, a positive score and a clear
    /// line in the room (`Room.VisualContact`).
    #[must_use]
    pub fn visual_contact(
        &self,
        room: &rw_assets::Room,
        map: &AiMap,
        has_water: bool,
        look: Vec2,
        bonus: f32,
    ) -> bool {
        if self.blinded || self.visual_score(map, has_water, look, bonus) <= 0.0 {
            return false;
        }
        room.visual_contact(self.point, look)
    }
}

/// What `BodyChunk.VisibilityBonus` reads of a chunk and its owner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkVisibility {
    pub pos: Vec2,
    pub last_pos: Vec2,
    /// The owner's `VisibilityBonus`.
    pub owner_bonus: f32,
}

impl ChunkVisibility {
    /// `BodyChunk.VisibilityBonus(movementBasedVision)`: moving makes it easier to see
    /// (up to 10 px a tick), stillness harder, by the eyes' `movementBasedVision`; then
    /// the owner's bonus at up to half weight. -1 to 1.
    #[must_use]
    pub fn bonus(self, movement_based_vision: f32) -> f32 {
        let moved = self
            .last_pos
            .distance(self.pos)
            .inverse_lerp(1.0, 10.0)
            .mathf_pow(1.0);
        ((-movement_based_vision).lerp(1.0, moved) * movement_based_vision
            + self.owner_bonus * (1.0 - movement_based_vision * 0.5))
            .clamp(-1.0, 1.0)
    }
}

/// The parts of `Player.VisibilityBonus` the base game has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerVisibility {
    /// `slugcatStats.generalVisibilityBonus`.
    pub general: f32,
    /// `slugcatStats.visualStealthInSneakMode`.
    pub stealth_in_sneak_mode: f32,
    /// `privSneak`.
    pub priv_sneak: f32,
    /// `Submersion`.
    pub submersion: f32,
}

impl PlayerVisibility {
    /// `Sneak`: `privSneak` over its first half, square-rooted, pulled to 0.5 under water.
    #[must_use]
    pub fn sneak(self) -> f32 {
        self.priv_sneak
            .inverse_lerp(0.0, 0.5)
            .mathf_pow(0.5)
            .lerp(0.5, self.submersion)
    }

    /// `VisibilityBonus` with More Slugcats and Watcher off, and no mud (`Creature`'s
    /// base is 0).
    #[must_use]
    pub fn bonus(self) -> f32 {
        self.general - self.stealth_in_sneak_mode * self.sneak()
    }
}

#[cfg(test)]
#[path = "tests/vision.rs"]
mod tests;
