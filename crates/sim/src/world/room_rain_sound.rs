//! `RoomRain`'s seven `DisembodiedDynamicSoundLoop`s (pan 0, `startAtRandomTime` off): which
//! exist for the room's `DangerType`, the clip and volume group of each, and the volume
//! formulas `Update` sets before each loop's own `Update`. The loops start when their volume
//! first passes 0 (`DynamicSoundLoop.Update`, [`ChunkDynamicSoundLoop`]).
//!
//! Not ported: `rippleFade` (Watcher, 1 in the base game) times the first five volumes, and
//! the `Blizzard` type's missing flooding loop (D46).
//!
//! Mirrors: RoomRain.cs (RoomRain constructor, Update), DynamicSoundLoop.cs, DisembodiedDynamicSoundLoop.cs

use rw_assets::DangerType;

use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::math::float_ext::MathfSin;
use crate::{FloatExt, LoopEmitter, SoundId};

/// What `Update` reads for the loops' volumes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainSoundInput {
    /// The room's `intensity`.
    pub intensity: f32,
    /// `globalRain.RumbleSound`.
    pub rumble_sound: f32,
    /// `roomSettings.RumbleIntensity`.
    pub rumble_intensity: f32,
    /// `roomSettings.RainIntensity`.
    pub rain_intensity: f32,
    /// `world.rainCycle.TimeUntilRain`.
    pub time_until_rain: i32,
    /// `globalRain.floodSpeed`.
    pub flood_speed: f32,
    /// The room has the `VoidSea` effect.
    pub void_sea: bool,
    /// `cameras[0].ScreenShake` when camera 0 is in this room.
    pub camera_screen_shake: Option<f32>,
}

/// The loops of a `RoomRain`; a missing one was not made for the room's `DangerType`.
#[derive(Debug, Clone, PartialEq)]
pub struct RainLoops {
    pub normal: Option<ChunkDynamicSoundLoop>,
    pub heavy: Option<ChunkDynamicSoundLoop>,
    pub death: ChunkDynamicSoundLoop,
    pub rumble: ChunkDynamicSoundLoop,
    pub distant: ChunkDynamicSoundLoop,
    pub flooding: Option<ChunkDynamicSoundLoop>,
    pub screen_shake: ChunkDynamicSoundLoop,
}

impl RainLoops {
    /// The microphone's volume group of the rain loops.
    const RAIN_GROUP: usize = 3;

    fn made(sound: SoundId, group: usize) -> ChunkDynamicSoundLoop {
        let mut sound_loop = ChunkDynamicSoundLoop::default();
        sound_loop.sound = Some(sound);
        sound_loop.volume_group = group;
        sound_loop
    }

    /// The constructor's loops for `danger` (a shelter's is `Flood`).
    #[must_use]
    pub fn new(danger: DangerType) -> Self {
        let flood = danger == DangerType::Flood;
        let rain = danger != DangerType::Flood;
        let (death, distant) = if flood {
            (
                SoundId::DeathRainHeardFromUndergroundLoop,
                SoundId::DeathRainApproachingHeardFromUndergroundLoop,
            )
        } else {
            (SoundId::DeathRainLoop, SoundId::DeathRainApproachingLoop)
        };
        let flooding = !matches!(danger, DangerType::Rain | DangerType::AerieBlizzard);
        Self {
            normal: rain.then(|| Self::made(SoundId::NormalRainLoop, Self::RAIN_GROUP)),
            heavy: rain.then(|| Self::made(SoundId::HeavyRainLoop, Self::RAIN_GROUP)),
            death: Self::made(death, Self::RAIN_GROUP),
            rumble: Self::made(SoundId::DeathRainRumbleLoop, Self::RAIN_GROUP),
            distant: Self::made(distant, 0),
            flooding: flooding.then(|| {
                Self::made(
                    SoundId::FlashFloodLoop,
                    if flood { Self::RAIN_GROUP } else { 0 },
                )
            }),
            screen_shake: Self::made(SoundId::ScreenShakeLoop, 0),
        }
    }

    /// `Volume = v; Update()`.
    fn step(sound_loop: &mut ChunkDynamicSoundLoop, volume: f32, cx: &mut LoopCx<'_>) {
        sound_loop.set_volume(volume);
        sound_loop.update_disembodied(Some(cx.room_index), 0.0, cx);
    }

    /// `normalRainSound.Volume`: `0.1 + 0.9 * Clamp01(Sin(InverseLerp(0.001, 0.7, I) *
    /// pi))^1.5` while the room rains.
    #[must_use]
    pub fn normal_volume(intensity: f32) -> f32 {
        if intensity > 0.0 {
            0.1 + 0.9
                * (intensity.inverse_lerp(0.001, 0.7) * std::f32::consts::PI)
                    .mathf_sin()
                    .clamp(0.0, 1.0)
                    .mathf_pow(1.5)
        } else {
            0.0
        }
    }

    /// `deathRainSound.Volume`.
    #[must_use]
    pub fn death_volume(intensity: f32) -> f32 {
        intensity.inverse_lerp(0.35, 0.75).mathf_pow(0.8)
    }

    /// `heavyRainSound.Volume`, `death` being `deathRainSound.Volume` before this tick's
    /// update.
    #[must_use]
    pub fn heavy_volume(intensity: f32, death: f32) -> f32 {
        intensity.inverse_lerp(0.12, 0.5).mathf_pow(0.85) * (1.0 - death).mathf_pow(0.3)
    }

    /// The volume setters and `Update`s of `RoomRain.Update`'s last block, in order.
    pub fn update(&mut self, input: &RainSoundInput, cx: &mut LoopCx<'_>) {
        let i = input.intensity;
        if let Some(normal) = &mut self.normal {
            Self::step(normal, Self::normal_volume(i), cx);
        }
        if let Some(heavy) = &mut self.heavy {
            Self::step(heavy, Self::heavy_volume(i, self.death.volume()), cx);
        }
        Self::step(&mut self.death, Self::death_volume(i), cx);
        Self::step(
            &mut self.rumble,
            input.rumble_sound * input.rumble_intensity,
            cx,
        );
        let approach = (input.time_until_rain as f32).inverse_lerp(1400.0, 0.0);
        Self::step(&mut self.distant, approach * input.rain_intensity, cx);
        if let Some(flooding) = self.flooding.as_mut().filter(|_| !input.void_sea) {
            Self::step(flooding, input.flood_speed.inverse_lerp(0.01, 0.5), cx);
        }
        let shake = input
            .camera_screen_shake
            .map_or(0.0, |s| s * (1.0 - self.rumble.volume()));
        Self::step(&mut self.screen_shake, shake, cx);
    }

    fn each_mut(&mut self) -> impl Iterator<Item = &mut ChunkDynamicSoundLoop> {
        [
            self.normal.as_mut(),
            self.heavy.as_mut(),
            Some(&mut self.death),
            Some(&mut self.rumble),
            Some(&mut self.distant),
            self.flooding.as_mut(),
            Some(&mut self.screen_shake),
        ]
        .into_iter()
        .flatten()
    }

    /// Every loop that exists, in the constructor's order.
    pub fn each(&self) -> impl Iterator<Item = &ChunkDynamicSoundLoop> {
        [
            self.normal.as_ref(),
            self.heavy.as_ref(),
            Some(&self.death),
            Some(&self.rumble),
            Some(&self.distant),
            self.flooding.as_ref(),
            Some(&self.screen_shake),
        ]
        .into_iter()
        .flatten()
    }

    /// `SoundEmitter.Update` of the loop emitter that joined the room at `order`; its sound
    /// is cut when it dies.
    pub fn update_emitter(&mut self, order: u64, cx: &mut LoopCx<'_>) {
        for sound_loop in self.each_mut() {
            let Some(e) = sound_loop.emitter_mut(order).filter(|e| !e.slated) else {
                continue;
            };
            let room = e.room;
            e.update(Some(room), cx.camera_room, cx.tick);
            if e.slated {
                cx.sounds.stop_emitter(e.sound);
            }
            sound_loop.released.retain(|e| !e.slated);
            return;
        }
    }

    /// The loop emitters still in the room's update list.
    pub fn emitters(&self) -> impl Iterator<Item = &LoopEmitter> {
        self.each().flat_map(ChunkDynamicSoundLoop::emitters)
    }
}

#[cfg(test)]
#[path = "tests/room_rain_sound.rs"]
mod tests;
