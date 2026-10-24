//! The rain's places in `RainWorldGame.Update`: camera 0's shake with the rain's terms
//! (the end of `RoomCamera.Update`), `GlobalRain.Update` right after the cameras, and
//! `RainCycle.Update`'s `RainHit`, and `Room.RainDrownOutSounds` for the microphone.
//!
//! Mirrors: RainWorldGame.cs (Update), RoomCamera.cs (Update), RainCycle.cs (Update, RainHit), GlobalRain.cs (Update), Room.cs (RainDrownOutSounds)

use rw_assets::{ActiveMods, DangerType, RoomEffectType};

use super::RainWorldGame;
use super::screen_shake::RainShake;
use crate::FloatExt;
use crate::world::{PassiveRain, RainTick, RealizedRoom};

impl RainWorldGame {
    /// The rain's terms in camera 0's `Update`: its room's danger, shelter flag and
    /// `RumbleIntensity` with the cycle's shake values.
    #[must_use]
    pub fn rain_shake(&self) -> RainShake {
        let Some(room) = self.world.realized(self.camera_room) else {
            return RainShake::NONE;
        };
        let danger = room.settings.danger_type;
        let cycle = &self.world.rain_cycle;
        RainShake {
            rain_danger: room.settings.rain_danger(self.world.mods()),
            aerie_blizzard: danger == Some(DangerType::AerieBlizzard),
            shelter: self
                .world
                .abstract_room(self.camera_room)
                .is_some_and(|r| r.shelter()),
            rumble: room.settings.rumble_intensity,
            screen_shake: cycle.screen_shake(),
            micro_screen_shake: cycle.micro_screen_shake(),
        }
    }

    /// Camera 0's `Update` shake step, at the head of the tick.
    pub(crate) fn update_camera_shake(&mut self) {
        let rain = self.rain_shake();
        self.camera_shake.update(&rain, &mut self.rng);
    }

    /// `GlobalRain.Update`, right after the cameras and before the realizer.
    pub(crate) fn update_global_rain(&mut self) {
        let cycle = &self.world.rain_cycle;
        let passive = self.world.realized(self.camera_room).map(|r| PassiveRain {
            light_rain: r.settings.effect_amount(RoomEffectType::LightRain),
            heavy_rain: r.settings.effect_amount(RoomEffectType::HeavyRain),
            heavy_rain_flux: r.settings.effect_amount(RoomEffectType::HeavyRainFlux),
            bullet_rain: r.settings.effect_amount(RoomEffectType::BulletRain),
            bullet_rain_flux: r.settings.effect_amount(RoomEffectType::BulletRainFlux),
        });
        let tick = RainTick {
            time_until_rain: cycle.time_until_rain(),
            rain_approaching: cycle.rain_approaching(),
            story: self.is_story_session(),
            passive,
        };
        self.world
            .rain_cycle
            .global_rain
            .update(&tick, &mut self.rng);
    }

    /// `RainCycle.Update`'s `RainHit`, after the timers.
    pub(crate) fn update_rain_hit(&mut self) {
        self.world.rain_cycle.rain_hit(&mut self.rng);
    }

    /// `Room.RainDrownOutSounds` of camera 0's room, which `VirtualMicrophone.Update` takes
    /// from volume group 0.
    #[must_use]
    pub fn rain_drown_out_sounds(&self) -> f32 {
        self.world.realized(self.camera_room).map_or(0.0, |r| {
            let rain = r.room_rain.as_ref().map(|rain| rain.intensity);
            r.rain_drown_out_sounds(self.world.mods(), rain)
        })
    }
}

impl RealizedRoom {
    /// `Room.RainDrownOutSounds`: 0 without rain danger or a `RoomRain`, else its
    /// `intensity` over 0 to 0.6 (the base game; `room_rain` is that intensity).
    #[must_use]
    pub fn rain_drown_out_sounds(&self, mods: ActiveMods, room_rain: Option<f32>) -> f32 {
        if !self.settings.rain_danger(mods) {
            return 0.0;
        }
        room_rain.map_or(0.0, |intensity| intensity.inverse_lerp(0.0, 0.6))
    }
}

#[cfg(test)]
#[path = "tests/rain_turn.rs"]
mod tests;
