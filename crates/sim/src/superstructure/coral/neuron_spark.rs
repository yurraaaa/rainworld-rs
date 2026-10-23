//! `NeuronSpark`: the flash two linked mycelia make where their tips meet. Its sprites
//! are `rw-graphics`' `cosmetic_graphics.rs`.
//!
//! Mirrors: NeuronSpark.cs (constructor, Update)

use rw_random::UnityRandom;

use crate::{FloatExt, SoundId, Sounds, Vec2};

/// A `CosmeticSprite` that plays its sound on its first update and lasts `lifeTime`
/// ticks.
#[derive(Debug, Clone, PartialEq)]
pub struct NeuronSpark {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub last_life: f32,
    pub life: f32,
    pub life_time: f32,
}

impl NeuronSpark {
    /// The constructor: `lifeTime = Lerp(1, 4, value)` (one draw).
    pub fn new(pos: Vec2, rng: &mut UnityRandom) -> Self {
        Self {
            pos,
            last_pos: pos,
            life_time: 1.0f32.lerp(4.0, rng.value()),
            life: 1.0,
            last_life: 1.0,
        }
    }

    /// `Update`: `CosmeticSprite.Update`, the spark's sound on the first one (the
    /// microphone's draws), and the fade; true once destroyed.
    pub fn update(&mut self, sounds: &mut Sounds, rng: &mut UnityRandom) -> bool {
        self.last_pos = self.pos;
        if self.life == 1.0 {
            sounds.play_at(SoundId::SsMyceliaSpark, self.pos, 1.0, 1.0, rng);
        }
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        self.last_life < 0.0
    }
}

#[cfg(test)]
#[path = "tests/neuron_spark.rs"]
mod tests;
