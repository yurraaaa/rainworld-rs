//! `DeepProcessing`: a placed quad of Five Pebbles' screen-grab "data rain" (the shader is
//! `rw-render`'s), and `DeepProcessingLight`, the blue light fixture that follows the
//! room's electric power. The quad's `power` chases `Room.ElectricPower` in jumps now and
//! then, with a one in five chance of snapping to the extreme instead. Draws are on the
//! shared stream (D37); the sprites are `rw-graphics`'.
//!
//! Mirrors: DeepProcessing.cs, DeepProcessingLight.cs

use rw_assets::DeepProcessingData;
use rw_random::UnityRandom;

use crate::superstructure::electric::zap_coil::ZAP_BLUE;
use crate::{FloatExt, LightSource, LightTie, Vec2};

/// One `DeepProcessing`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepProcessing {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`.
    pub pos: Vec2,
    pub data: DeepProcessingData,
    /// `quad`: the position and the three handles added to it.
    pub quad: [Vec2; 4],
    /// `power`: starts at 1.
    pub power: f32,
}

impl DeepProcessing {
    /// The constructor (no draws).
    #[must_use]
    pub fn new(update_order: u64, pos: Vec2, data: DeepProcessingData) -> Self {
        let [h0, h1, h2] = data.quad.handles;
        Self {
            update_order,
            pos,
            data,
            quad: [
                pos,
                pos + Vec2::from(h0),
                pos + Vec2::from(h1),
                pos + Vec2::from(h2),
            ],
            power: 1.0,
        }
    }

    /// `Update` given `room.ElectricPower`: one draw; one time in fourteen the power
    /// moves toward the electric power (two more draws), by `1 / Lerp(1, 4, v)` at most,
    /// or on a one-in-five chance jumps to the far extreme (0 going down, 1 going up).
    pub fn update(&mut self, electric_power: f32, rng: &mut UnityRandom) {
        if rng.value() >= 1.0 / 14.0 {
            return;
        }
        if self.power > electric_power {
            let target = if rng.value() < 0.2 {
                0.0
            } else {
                electric_power
            };
            self.power = target.max(self.power - 1.0 / 1.0_f32.lerp(4.0, rng.value()));
        } else if self.power < electric_power {
            let target = if rng.value() < 0.2 {
                1.0
            } else {
                electric_power
            };
            self.power = target.min(self.power + 1.0 / 1.0_f32.lerp(4.0, rng.value()));
        }
    }
}

/// `DeepProcessingLight`: a blue `LightSource` whose radius and alpha follow the room's
/// electric power.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepProcessingLight {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`.
    pub pos: Vec2,
    /// `lightData.randomSeed`.
    pub random_seed: i32,
    /// Its `LightSource`'s id.
    pub light: u64,
}

impl DeepProcessingLight {
    /// The constructor's light: blue, tied to the fixture, alpha 1, unaffected by the
    /// palette's darkness.
    #[must_use]
    pub fn light_source(id: u64, room: i32, order: u64, pos: Vec2) -> LightSource {
        let mut l = LightSource::new(
            id,
            room,
            pos,
            false,
            ZAP_BLUE,
            Some(LightTie::Fixture(order)),
        );
        l.set_alpha = Some(1.0);
        l.affected_by_palette_darkness = 0.0;
        l
    }

    /// `Update` given `room.ElectricPower`: one draw for the radius' flicker.
    pub fn update(&self, lights: &mut [LightSource], electric_power: f32, rng: &mut UnityRandom) {
        let Some(l) = LightSource::find_mut(lights, self.light) else {
            return;
        };
        l.set_rad = Some(
            80.0_f32.lerp(800.0, self.random_seed as f32 / 100.0)
                * electric_power
                * 0.9_f32.lerp(1.1, rng.value()),
        );
        l.set_pos = Some(self.pos);
        l.set_alpha = Some(electric_power * 0.5);
    }
}

#[cfg(test)]
#[path = "tests/deep_processing.rs"]
mod tests;
