//! The water's calm surface loop (`Water.waterSounds`): a `RectangularDynamicSoundLoop`
//! playing `Water_Surface_Calm_LOOP` on the one-tile band below the surface's resting
//! line, across the room. Its owner, the `WaterSoundObject`, never leaves the room.
//!
//! Mirrors: Water.cs (constructor and Update: waterSounds), RectangularDynamicSoundLoop.cs, DynamicSoundLoop.cs

use rw_assets::{Room, RoomEffectType, RoomSettings};

use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::{LoopEmitter, SoundId, SoundRect};

/// `Water.waterSounds` of a room with water.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterSound {
    pub rect: SoundRect,
    pub sound_loop: ChunkDynamicSoundLoop,
}

impl WaterSound {
    /// The `Water` constructor's loop; `None` for a room without water. The band runs
    /// from row `defaultWaterLevel - 1`'s bottom to row `defaultWaterLevel`'s bottom.
    /// The `waterInverted` branch (DLCShared) is not ported.
    #[must_use]
    pub fn new(room: &Room) -> Option<Self> {
        let level = room.water_level()? as f32;
        let mut sound_loop = ChunkDynamicSoundLoop::default();
        sound_loop.sound = Some(SoundId::WaterSurfaceCalmLoop);
        Some(Self {
            rect: SoundRect {
                left: 0.0,
                bottom: (level - 1.0) * 20.0,
                right: room.pixel_width(),
                top: level * 20.0,
            },
            sound_loop,
        })
    }

    /// The head of `Water.Update` in room `cx.room_index`: the loop, then a
    /// `SilenceWater` effect sets the volume to `1 - amount`.
    pub fn update(&mut self, settings: &RoomSettings, cx: &mut LoopCx<'_>) {
        self.sound_loop
            .update_rect(Some(cx.room_index), self.rect, cx);
        let silence = settings.effect_amount(RoomEffectType::SilenceWater);
        if silence > 0.0 {
            let volume = 1.0 - silence;
            self.sound_loop.set_volume(volume);
            // The `Volume` setter writes the emitter's volume at once.
            if let Some(e) = self.sound_loop.emitter {
                cx.sounds
                    .update_emitter(e.sound, volume, self.sound_loop.pitch());
            }
        }
    }

    /// `SoundEmitter.Update` of its `RectSoundEmitter` that joined the room at `order`
    /// (no owner check); its sound is cut when it dies.
    pub fn update_emitter(&mut self, order: u64, cx: &mut LoopCx<'_>) {
        let Some(e) = self.sound_loop.emitter_mut(order).filter(|e| !e.slated) else {
            return;
        };
        let room = e.room;
        e.update(Some(room), cx.camera_room, cx.tick);
        if e.slated {
            cx.sounds.stop_emitter(e.sound);
        }
        self.sound_loop.released.retain(|e| !e.slated);
    }

    /// The emitters still in the room's update list.
    pub fn emitters(&self) -> impl Iterator<Item = &LoopEmitter> {
        self.sound_loop.emitters()
    }

    /// The emitter the loop holds, playing for the camera's microphone.
    #[must_use]
    pub fn playing(&self) -> bool {
        self.sound_loop
            .emitter
            .is_some_and(|e| e.playing && !e.slated)
    }
}

#[cfg(test)]
#[path = "tests/water_sound.rs"]
mod tests;
