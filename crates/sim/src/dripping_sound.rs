//! `DrippingSound`: the start of a cycle's dripping loop (`Cycle_Start_Drips`), added by
//! `Room.Loaded` to a rain room with `CeilingDrips` while the cycle starts up. It plays
//! through a `DisembodiedDynamicSoundLoop` (pan 0) fading out between `CycleStartUp`
//! 0.75 and 1, and is destroyed at 1.
//!
//! Mirrors: DrippingSound.cs, DisembodiedDynamicSoundLoop.cs, DisembodiedLoopEmitter.cs

use rw_assets::{DangerType, RoomSettings};

use crate::item::{ChunkDynamicSoundLoop, LoopCx};
use crate::{FloatExt, LoopEmitter, SoundId};

/// One `DrippingSound` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct DrippingSound {
    pub update_order: u64,
    pub sound_loop: ChunkDynamicSoundLoop,
    /// Destroyed; kept while its emitters are still in the room.
    pub slated: bool,
}

impl DrippingSound {
    #[must_use]
    pub fn new(update_order: u64) -> Self {
        let mut sound_loop = ChunkDynamicSoundLoop::default();
        sound_loop.sound = Some(SoundId::CycleStartDrips);
        Self {
            update_order,
            sound_loop,
            slated: false,
        }
    }

    /// `Room.Loaded`'s condition for adding one.
    #[must_use]
    pub fn wanted(settings: &RoomSettings, shelter: bool, cycle_start_up: f32) -> bool {
        cycle_start_up < 1.0
            && settings.ceiling_drips > 0.0
            && settings.danger_type != Some(DangerType::None)
            && !shelter
    }

    /// `Update` in room `cx.room_index`: the loop, then its volume from `CycleStartUp`.
    pub fn update(&mut self, cycle_start_up: f32, cx: &mut LoopCx<'_>) {
        self.sound_loop
            .update_disembodied(Some(cx.room_index), 0.0, cx);
        let volume = cycle_start_up.inverse_lerp(1.0, 0.75).mathf_pow(0.5);
        self.sound_loop.set_volume(volume);
        // The `Volume` setter writes the emitter's volume at once.
        if let Some(e) = self.sound_loop.emitter {
            cx.sounds
                .update_emitter(e.sound, volume, self.sound_loop.pitch());
        }
        if cycle_start_up >= 1.0 {
            self.slated = true;
        }
    }

    /// `SoundEmitter.Update` of its emitter that joined the room at `order` (no owner
    /// check: a `DisembodiedLoopEmitter` lives while kept and heard); its sound is cut
    /// when it dies.
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

    /// Gone from the room: destroyed, with no emitter left.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.slated && self.emitters().next().is_none()
    }
}

#[cfg(test)]
#[path = "dripping_sound_tests.rs"]
mod tests;
