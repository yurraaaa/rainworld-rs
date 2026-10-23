//! `AboveCloudsView.DistantLightning`: a flash behind a distant building. It is the only
//! scene element with a turn of its own: one pass of the bolt's timers per tick, with
//! draws on the shared stream (D1): up to three, see [`DistantLightning::update`].
//! Its picture, whose brightness [`DistantLightning::light_intensity`] draws for on the
//! cosmetic stream, is in `rw-graphics` `background_scene.rs`.
//!
//! Mirrors: AboveCloudsView.cs (DistantLightning)

use rw_random::UnityRandom;

use crate::FloatExt;
use crate::math::float_ext::{MathfSin, SCurve};

/// The state of one bolt.
#[derive(Debug, Clone, PartialEq)]
pub struct DistantLightning {
    pub asset_name: &'static str,
    /// `minusDepthForLayering`: taken off the depth until the first turn.
    pub minus_depth_for_layering: f32,
    pub restored_depth: bool,
    pub wait: i32,
    pub tiny_thunder_wait: i32,
    pub tiny_thunder: i32,
    pub tiny_thunder_length: i32,
    pub thunder: i32,
    pub thunder_length: i32,
    pub random_level: f32,
    pub power: f32,
    pub random_level_change: i32,
    pub last_intensity: f32,
    pub intensity: f32,
    pub non_position_based_intensity: bool,
    pub intensity_multiplier: f32,
}

impl DistantLightning {
    #[must_use]
    pub fn new(asset_name: &'static str, minus_depth_for_layering: f32) -> Self {
        Self {
            asset_name,
            minus_depth_for_layering,
            restored_depth: false,
            wait: 0,
            tiny_thunder_wait: 5,
            tiny_thunder: 0,
            tiny_thunder_length: 0,
            thunder: 0,
            thunder_length: 0,
            random_level: 0.0,
            power: 0.0,
            random_level_change: 0,
            last_intensity: 0.0,
            intensity: 0.0,
            non_position_based_intensity: false,
            intensity_multiplier: 1.0,
        }
    }

    /// `ThunderFac`.
    #[must_use]
    pub fn thunder_fac(&self) -> f32 {
        1.0 - self.thunder as f32 / self.thunder_length as f32
    }

    /// `TinyThunderFac`.
    #[must_use]
    pub fn tiny_thunder_fac(&self) -> f32 {
        1.0 - self.tiny_thunder as f32 / self.tiny_thunder_length as f32
    }

    /// `Reset`: three draws.
    fn reset(&mut self, rng: &mut UnityRandom) {
        self.wait = (10.0_f32.lerp(440.0, rng.value()) * 1.5_f32.lerp(1.0, 1.0)).to_int_cs();
        self.power = 0.7_f32.lerp(1.0, rng.value());
        self.thunder_length = rng.range_i32(1, 10.0_f32.lerp(32.0, self.power).to_int_cs());
    }

    /// `Update`, whose depth fix-up (`restoredDepth`) is returned as the amount to add
    /// to the element's depth, on the first turn only. Draws on the shared stream:
    /// 2 when `randomLevelChange` runs out (every 1 to 5 ticks), 3 for a [`Self::reset`]
    /// when a bolt ends, 2 when `tinyThunderWait` runs out, 1 every tick `tinyThunder`
    /// is still above 0.
    pub fn update(&mut self, rng: &mut UnityRandom) -> f32 {
        let mut restore = 0.0;
        if !self.restored_depth {
            restore = self.minus_depth_for_layering;
            self.restored_depth = true;
        }
        self.random_level_change -= 1;
        if self.random_level_change < 1 {
            self.random_level_change = rng.range_i32(1, 6);
            self.random_level = rng.value();
        }
        if self.wait > 0 {
            self.wait -= 1;
            if self.wait < 1 {
                self.thunder = self.thunder_length;
            }
        } else {
            self.thunder -= 1;
            if self.thunder < 1 {
                self.reset(rng);
            }
        }
        if self.tiny_thunder_wait > 0 {
            self.tiny_thunder_wait -= 1;
            if self.tiny_thunder_wait < 1 {
                self.tiny_thunder_wait = rng.range_i32(10, 80);
                self.tiny_thunder_length = rng.range_i32(5, self.tiny_thunder_wait);
                self.tiny_thunder = self.tiny_thunder_length;
            }
        }
        self.last_intensity = self.intensity;
        let mut a = 0.0;
        let mut b = 0.0;
        if self.thunder > 0 {
            a = self.random_level.mathf_pow(
                3.0_f32.lerp(0.1, (self.thunder_fac() * std::f32::consts::PI).mathf_sin()),
            );
        }
        if self.tiny_thunder > 0 {
            self.tiny_thunder -= 1;
            b = rng.value().mathf_pow(3.0_f32.lerp(
                0.1,
                (self.tiny_thunder_fac() * std::f32::consts::PI).mathf_sin(),
            )) * 0.4;
        }
        self.intensity = a.max(b);
        restore
    }

    /// `LightIntensity(timeStacker)`: 1 draw on the cosmetic stream, and 2 more when
    /// that draw is under a third.
    pub fn light_intensity(&self, time_stacker: f32, rng: &mut UnityRandom) -> f32 {
        let mut num = self.last_intensity.lerp(self.intensity, time_stacker);
        if rng.value() < 1.0 / 3.0 {
            let target = if rng.value() < 0.5 { 1.0 } else { 0.0 };
            num = num.lerp(target, rng.value() * num);
        }
        num.s_curve(0.5) * self.intensity_multiplier
    }
}
