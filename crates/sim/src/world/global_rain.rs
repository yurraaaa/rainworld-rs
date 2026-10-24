//! `GlobalRain`: the rain's direction (two draws a tick from the start of the game) and,
//! once the cycle's timer reaches its length, the `DeathRain` mode machine that fills
//! `Intensity`, `ShaderLight`, `ScreenShake`, `MicroScreenShake`, `RumbleSound`,
//! `bulletRainDensity`, `floodSpeed` and `flood`. The game keeps one per `RainWorldGame`
//! (`globalRain`) and `RainCycle` reads it through `world.game`; the port keeps it in the
//! `RainCycle` (`RainCycle::global_rain`), which every reader of these values has to hand.
//!
//! Not ported: arena sessions (`IsArenaSession`: the forced 70 % path, the 3.2 times faster
//! progression, the second flood arm), the precycle (More Slugcats, Watcher) arms and
//! the `Pulses` mode, Watcher's passive-rain scaling, the dev `ResetRain`; see D44 in
//! `docs/divergences/sim-rain.md`.
//!
//! Mirrors: GlobalRain.cs

use rw_random::UnityRandom;

use crate::FloatExt;
use crate::math::float_ext::MathfSin;

/// `GlobalRain.DeathRain.DeathRainMode`: the base game's values in registration order
/// (`Index`), then the More Slugcats one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeathRainMode {
    #[default]
    None,
    CalmBeforeStorm,
    GradeABuildUp,
    GradeAPlateu,
    GradeBBuildUp,
    GradeBPlateu,
    FinalBuildUp,
    Mayhem,
    AlternateBuildUp,
    /// More Slugcats; never registered in the base game.
    Pulses,
}

impl DeathRainMode {
    /// The base game's values, by `Index`.
    pub const BASE: [Self; 9] = [
        Self::None,
        Self::CalmBeforeStorm,
        Self::GradeABuildUp,
        Self::GradeAPlateu,
        Self::GradeBBuildUp,
        Self::GradeBPlateu,
        Self::FinalBuildUp,
        Self::Mayhem,
        Self::AlternateBuildUp,
    ];

    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::CalmBeforeStorm => "CalmBeforeStorm",
            Self::GradeABuildUp => "GradeABuildUp",
            Self::GradeAPlateu => "GradeAPlateu",
            Self::GradeBBuildUp => "GradeBBuildUp",
            Self::GradeBPlateu => "GradeBPlateu",
            Self::FinalBuildUp => "FinalBuildUp",
            Self::Mayhem => "Mayhem",
            Self::AlternateBuildUp => "AlternateBuildUp",
            Self::Pulses => "Pulses",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::BASE
            .into_iter()
            .chain([Self::Pulses])
            .find(|m| m.value() == value)
    }

    /// `ExtEnum.Index` (the `(int)` cast the game compares by).
    #[must_use]
    pub fn index(self) -> usize {
        Self::BASE
            .iter()
            .position(|&m| m == self)
            .unwrap_or(Self::BASE.len())
    }

    /// `values.GetEntry(Index + 1)`: the next registered value, `None` past the last.
    fn next_registered(self) -> Option<Self> {
        Self::BASE.get(self.index() + 1).copied()
    }
}

/// The camera room's effect amounts `GlobalRain.Update` reads before the rain
/// (`GetEffectAmount` of `LightRain`, `HeavyRain`, `HeavyRainFlux`, `BulletRain`,
/// `BulletRainFlux`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PassiveRain {
    pub light_rain: f32,
    pub heavy_rain: f32,
    pub heavy_rain_flux: f32,
    pub bullet_rain: f32,
    pub bullet_rain_flux: f32,
}

/// What `GlobalRain.Update` reads of the game and the cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainTick {
    /// `world.rainCycle.TimeUntilRain`.
    pub time_until_rain: i32,
    /// `world.rainCycle.RainApproaching`.
    pub rain_approaching: f32,
    /// `game.IsStorySession`.
    pub story: bool,
    /// The camera room's effects; `None` without a camera room.
    pub passive: Option<PassiveRain>,
}

/// `GlobalRain.DeathRain`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DeathRain {
    pub mode: DeathRainMode,
    pub time_in_this_mode: f32,
    pub progression: f32,
    pub calm_before_storm_sunlight: f32,
}

/// `GlobalRain`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalRain {
    pub last_rain_direction: f32,
    pub rain_direction: f32,
    pub rain_direction_get_to: f32,
    pub intensity: f32,
    pub shader_light: f32,
    pub screen_shake: f32,
    pub micro_screen_shake: f32,
    pub rumble_sound: f32,
    pub bullet_rain_density: f32,
    pub death_rain: Option<DeathRain>,
    pub flood: f32,
    pub flood_speed: f32,
    pub bullet_timer: i32,
    pub heavy_timer: i32,
    pub flood_lerp_speed: f32,
    pub water_flux_ticker: i32,
}

impl Default for GlobalRain {
    fn default() -> Self {
        Self {
            last_rain_direction: 0.0,
            rain_direction: 0.0,
            rain_direction_get_to: 0.0,
            intensity: 0.0,
            shader_light: -1.0,
            screen_shake: 0.0,
            micro_screen_shake: 0.0,
            rumble_sound: 0.0,
            bullet_rain_density: 0.0,
            death_rain: None,
            flood: 0.0,
            flood_speed: 0.0,
            bullet_timer: 0,
            heavy_timer: 0,
            flood_lerp_speed: 0.0,
            water_flux_ticker: 0,
        }
    }
}

impl DeathRain {
    /// `DeathRainUpdate`: advances the mode's progression and writes the values it owns;
    /// a finished mode hands over to the next at the end of the tick.
    fn update(&mut self, g: &mut GlobalRain, rain_approaching: f32, rng: &mut UnityRandom) {
        self.progression += 1.0 / self.time_in_this_mode;
        let mut finished = false;
        if self.progression > 1.0 {
            self.progression = 1.0;
            finished = true;
        }
        if self.mode == DeathRainMode::CalmBeforeStorm {
            g.rumble_sound = (g.rumble_sound - 0.025).max(0.0);
        } else {
            g.rumble_sound = g
                .rumble_sound
                .lerp(1.0 - rain_approaching.inverse_lerp(0.0, 0.6), 0.2);
        }
        let p = self.progression;
        match self.mode {
            DeathRainMode::CalmBeforeStorm => {
                g.intensity = p.inverse_lerp(0.15, 0.0).mathf_pow(1.5) * 0.24;
                g.shader_light = -1.0
                    + 0.3
                        * (p.inverse_lerp(0.03, 0.8) * std::f32::consts::PI).mathf_sin()
                        * self.calm_before_storm_sunlight;
                g.bullet_rain_density = p.inverse_lerp(0.3, 1.0).mathf_pow(8.0);
            }
            DeathRainMode::GradeABuildUp => {
                g.intensity = p * 0.6;
                g.micro_screen_shake = p * 1.5;
                g.bullet_rain_density = 1.0 - p;
            }
            DeathRainMode::GradeBBuildUp => {
                g.intensity = 0.6_f32.lerp(0.71, p);
                g.micro_screen_shake = 1.5_f32.lerp(2.1, p);
                g.screen_shake = p * 1.2;
            }
            DeathRainMode::FinalBuildUp => {
                g.intensity = 0.71_f32.lerp(1.0, p);
                g.micro_screen_shake = 2.1_f32.lerp(4.0, p.mathf_pow(1.2));
                g.screen_shake = 1.2_f32.lerp(3.0, p);
            }
            DeathRainMode::AlternateBuildUp => {
                g.intensity = 0.24_f32.lerp(0.6, p);
                g.micro_screen_shake = 1.0 + p * 0.5;
            }
            DeathRainMode::None
            | DeathRainMode::GradeAPlateu
            | DeathRainMode::GradeBPlateu
            | DeathRainMode::Mayhem
            | DeathRainMode::Pulses => {}
        }
        if finished {
            self.next_mode(g, rng);
        }
    }

    /// `NextDeathRainMode` (base game, no arena): the mode after this one and its length,
    /// with the draws of its entry.
    fn next_mode(&mut self, g: &mut GlobalRain, rng: &mut UnityRandom) {
        if self.mode == DeathRainMode::Mayhem {
            return;
        }
        if self.mode == DeathRainMode::None && rng.value() < 0.7 {
            self.mode = DeathRainMode::AlternateBuildUp;
        } else if self.mode == DeathRainMode::AlternateBuildUp {
            self.mode = DeathRainMode::GradeAPlateu;
        } else {
            self.mode = self.mode.next_registered().unwrap_or(DeathRainMode::Mayhem);
        }
        self.progression = 0.0;
        self.time_in_this_mode = match self.mode {
            DeathRainMode::CalmBeforeStorm => {
                let time = 400.0_f32.lerp(800.0, rng.value());
                self.calm_before_storm_sunlight = if rng.value() < 0.5 { 0.0 } else { rng.value() };
                time
            }
            DeathRainMode::GradeABuildUp => {
                g.shader_light = -1.0;
                6.0
            }
            DeathRainMode::GradeAPlateu => 400.0_f32.lerp(600.0, rng.value()),
            DeathRainMode::GradeBBuildUp | DeathRainMode::GradeBPlateu => {
                if rng.value() < 0.5 {
                    100.0
                } else {
                    50.0_f32.lerp(300.0, rng.value())
                }
            }
            DeathRainMode::FinalBuildUp => {
                if rng.value() < 0.5 {
                    300.0_f32.lerp(500.0, rng.value())
                } else {
                    100.0_f32.lerp(800.0, rng.value())
                }
            }
            DeathRainMode::AlternateBuildUp => 400.0_f32.lerp(1200.0, rng.value()),
            DeathRainMode::None | DeathRainMode::Mayhem | DeathRainMode::Pulses => {
                self.time_in_this_mode
            }
        };
    }
}

impl GlobalRain {
    /// `OutsidePushAround`.
    #[must_use]
    pub fn outside_push_around(&self) -> f32 {
        self.intensity.inverse_lerp(0.35, 0.7).mathf_pow(0.8)
    }

    /// `InsidePushAround`.
    #[must_use]
    pub fn inside_push_around(&self) -> f32 {
        self.intensity.inverse_lerp(0.63, 0.98).mathf_pow(3.5)
    }

    /// `AnyPushAround`.
    #[must_use]
    pub fn any_push_around(&self) -> bool {
        self.outside_push_around() > 0.0 || self.inside_push_around() > 0.0
    }

    /// `InitDeathRain`: `new DeathRain`, whose constructor picks the first mode.
    pub fn init_death_rain(&mut self, rng: &mut UnityRandom) {
        let mut death_rain = DeathRain::default();
        death_rain.next_mode(self, rng);
        self.death_rain = Some(death_rain);
    }

    /// `Update`, once a tick from the start of the game (unless paused), right after the
    /// cameras and before the realizer.
    pub fn update(&mut self, tick: &RainTick, rng: &mut UnityRandom) {
        if rng.value() < 0.025 {
            self.rain_direction_get_to = (-1.0_f32).lerp(1.0, rng.value());
        }
        self.last_rain_direction = self.rain_direction;
        self.rain_direction = self.rain_direction.lerp(self.rain_direction_get_to, 0.01);
        if self.rain_direction < self.rain_direction_get_to {
            self.rain_direction = (self.rain_direction + 0.0125).min(self.rain_direction_get_to);
        } else if self.rain_direction > self.rain_direction_get_to {
            self.rain_direction = (self.rain_direction - 0.0125).max(self.rain_direction_get_to);
        }
        self.water_flux_ticker += 1;
        self.flood_lerp_speed = 0.2;
        if let Some(mut death_rain) = self.death_rain.take() {
            death_rain.update(self, tick.rain_approaching, rng);
            // The base game compares the modes' indices: `AlternateBuildUp` (8) counts as
            // after `GradeABuildUp` (2).
            if tick.story && death_rain.mode.index() > DeathRainMode::GradeABuildUp.index() {
                self.flood_speed = (self.flood_speed + 0.0025).min(0.8);
            }
            self.flood += self.flood_speed;
            self.death_rain = Some(death_rain);
            return;
        }
        self.intensity = (tick.time_until_rain as f32).inverse_lerp(600.0, 200.0) * 0.24;
        self.bullet_rain_density = 0.0;
        if let Some(passive) = tick.passive {
            self.update_passive(passive);
        }
    }

    /// The part of `Update` that reads the camera room's rain effects.
    fn update_passive(&mut self, passive: PassiveRain) {
        let mut heavy = passive.heavy_rain;
        if passive.heavy_rain_flux > 0.0 {
            let (hold, fade) = (1200.0 * passive.heavy_rain_flux, 60.0);
            self.heavy_timer = (self.heavy_timer + 1) % (fade * 2.0 + hold * 2.0).to_int_cs();
            heavy *= Self::pulse(self.heavy_timer as f32, hold, fade);
        }
        if heavy > 0.0 {
            self.intensity = (1.0 + heavy * 4.0) * 0.24;
            self.rumble_sound = heavy * 0.2;
            self.screen_shake = heavy;
        } else if passive.light_rain > 0.0 {
            self.intensity = passive.light_rain * 0.24;
        }
        let mut bullet = passive.bullet_rain;
        if passive.bullet_rain_flux > 0.0 {
            let (hold, fade) = (1200.0 * passive.bullet_rain_flux, 60.0);
            self.bullet_timer = (self.bullet_timer + 1) % (fade * 2.0 + hold * 2.0).to_int_cs();
            bullet *= Self::pulse(self.bullet_timer as f32, hold, fade);
        }
        if bullet > 0.0 {
            self.bullet_rain_density = bullet;
        }
    }

    /// The flux timers' envelope: up over `fade`, held for `hold`, down over `fade`, then
    /// off for `hold + fade` (the value the amount is multiplied by).
    fn pulse(timer: f32, hold: f32, fade: f32) -> f32 {
        if timer < fade {
            timer / fade
        } else if timer >= fade + hold && timer < fade * 2.0 + hold {
            1.0 - (timer - (hold + fade)) / fade
        } else if timer >= fade * 2.0 + hold {
            0.0
        } else {
            1.0
        }
    }
}

#[cfg(test)]
#[path = "tests/global_rain.rs"]
mod tests;
