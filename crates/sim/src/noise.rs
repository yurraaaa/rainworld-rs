//! `Noise.InGameNoise` and where noises come from: `PhysicalObject.TerrainImpact`'s
//! noise and the sink an updating object hands its `room.InGameNoise` calls to. Who hears
//! them is `world/hearing.rs`. Decompile: `Noise/InGameNoise.cs`, `PhysicalObject.cs`
//! `TerrainImpact`, `Room.cs` `InGameNoise`.
//!
//! Mirrors: Noise/InGameNoise.cs, PhysicalObject.cs (TerrainImpact), Room.cs (InGameNoise)

use rw_random::UnityRandom;

use crate::item::Entity;
use crate::{BodyChunk, FloatExt, TerrainImpact, TerrainImpactHandler, Vec2};

/// `Noise.InGameNoise`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InGameNoise {
    pub pos: Vec2,
    pub strength: f32,
    /// `sourceObject`.
    pub source: Option<Entity>,
    pub interesting: f32,
}

impl InGameNoise {
    /// `noise == default(InGameNoise)`, which `Room.InGameNoise` ignores.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.pos == Vec2::ZERO
            && self.strength == 0.0
            && self.source.is_none()
            && self.interesting == 0.0
    }
}

impl TerrainImpact {
    /// `PhysicalObject.TerrainImpact`'s noise: on a first contact faster than 4 whose
    /// speed times the chunk's loudness and half-weighted mass passes 0.5, just off the
    /// chunk toward the surface, 350 to 925 strong. `interesting` is 4 for a rock or a
    /// spear, else 1.
    #[must_use]
    pub fn noise(
        &self,
        chunk: &BodyChunk,
        source: Entity,
        interesting: f32,
    ) -> Option<InGameNoise> {
        let (pos, strength) = self.noise_at(chunk)?;
        Some(InGameNoise {
            pos,
            strength,
            source: Some(source),
            interesting,
        })
    }

    /// Where [`Self::noise`] is made and how strong it is.
    #[must_use]
    pub fn noise_at(&self, chunk: &BodyChunk) -> Option<(Vec2, f32)> {
        let loud = self.speed * chunk.loudness * chunk.mass.lerp(1.0, 0.5);
        if !self.first_contact || self.speed <= 4.0 || loud <= 0.5 {
            return None;
        }
        let dir = Vec2::new(f32::from(self.direction.x), f32::from(self.direction.y));
        let strength = 350.0.lerp(100.0.lerp(1500.0, loud.inverse_lerp(0.5, 20.0)), 0.5);
        Some((chunk.pos + dir * (chunk.rad * 0.9), strength))
    }
}

/// The noises of an object's terrain impacts in one physics update (where, how strong).
#[derive(Debug, Default)]
pub struct ImpactNoises(pub Vec<(Vec2, f32)>);

impl TerrainImpactHandler for ImpactNoises {
    fn terrain_impact(&mut self, chunks: &mut [BodyChunk], impact: TerrainImpact) {
        self.0.extend(impact.noise_at(&chunks[impact.chunk]));
    }
}

/// Where an updating object's noises go: `room.InGameNoise(new InGameNoise(pos,
/// strength, this, interesting))` from inside its update, heard at once.
pub trait NoiseSink {
    fn in_game_noise(&mut self, pos: Vec2, strength: f32, interesting: f32, rng: &mut UnityRandom);
}

/// Nobody hears (tests, and objects updated without a room).
impl NoiseSink for () {
    fn in_game_noise(&mut self, _: Vec2, _: f32, _: f32, _: &mut UnityRandom) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Contact;
    use crate::creature::CreatureId;

    fn impact(speed: f32, first_contact: bool) -> TerrainImpact {
        TerrainImpact {
            chunk: 1,
            direction: Contact { x: 0, y: -1 },
            speed,
            first_contact,
        }
    }

    /// `PhysicalObject.cs` 629-631: a chunk of mass 0.3 and loudness 0.7 landing at 10:
    /// `10 * 0.7 * Lerp(0.3, 1, 0.5)` = 4.55, so
    /// `Lerp(350, Lerp(100, 1500, InverseLerp(0.5, 20, 4.55)), 0.5)` at 0.9 of the radius
    /// below the chunk.
    #[test]
    fn terrain_impact_noise() {
        let mut c = BodyChunk::new(Vec2::new(100.0, 50.0), 9.0, 0.3);
        c.loudness = 0.7;
        let n = impact(10.0, true)
            .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
            .expect("loud enough");
        let x = 4.55f32;
        let t = (x - 0.5) / 19.5;
        let expected = (350.0 + (100.0 + 1400.0 * t)) / 2.0;
        assert!((n.strength - expected).abs() < 1e-3, "{}", n.strength);
        assert_eq!(n.pos, Vec2::new(100.0, 50.0 - 8.1));
        assert_eq!(n.source, Some(Entity::Creature(CreatureId(3))));
        assert_eq!(n.interesting, 1.0);
        assert!(
            impact(10.0, false)
                .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
                .is_none()
        );
        assert!(
            impact(4.0, true)
                .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
                .is_none()
        );
        c.loudness = 0.0;
        assert!(
            impact(10.0, true)
                .noise(&c, Entity::Creature(CreatureId(3)), 1.0)
                .is_none()
        );
    }

    #[test]
    fn default_noise() {
        let n = InGameNoise {
            pos: Vec2::ZERO,
            strength: 0.0,
            source: None,
            interesting: 0.0,
        };
        assert!(n.is_default());
        assert!(!InGameNoise { strength: 1.0, ..n }.is_default());
    }
}
