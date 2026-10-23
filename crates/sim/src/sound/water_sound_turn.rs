//! The game's side of the water's surface loop: `Water.Update`'s head in `Room.Update`
//! and the loop's emitters in the update list. Type: [`crate::WaterSound`].
//!
//! Mirrors: Room.cs (Update: waterObject.Update)

use std::sync::Arc;

use crate::game::{RainWorldGame, RoomObject};
use crate::item::LoopCx;

impl RainWorldGame {
    /// Room `index`'s surface loop emitters in its update list.
    pub(crate) fn room_water_sound_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.water_sound.as_ref())
            .map(|w| {
                w.emitters()
                    .map(|e| (e.update_order, RoomObject::WaterSoundLoop(e.update_order)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `waterSounds` at the head of `Water.Update` in room `index`.
    pub(crate) fn run_water_sound(&mut self, index: i32) {
        self.with_water_sound(index, WaterSoundCall::Update);
    }

    /// The `SoundEmitter.Update` of the surface loop's emitter at `order`.
    pub(crate) fn run_water_sound_emitter(&mut self, index: i32, order: u64) {
        self.with_water_sound(index, WaterSoundCall::Emitter(order));
    }

    fn with_water_sound(&mut self, index: i32, call: WaterSoundCall) {
        let triggers = Arc::clone(self.sounds.table());
        let mut seq = self.world.update_seq();
        let (camera_room, tick) = (self.camera_room, self.tick);
        let sounds = if index == camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(w) = r.water_sound.as_mut() else {
            return;
        };
        let mut cx = LoopCx {
            room_index: index,
            sounds,
            rng: &mut self.rng,
            triggers: &triggers,
            camera_room,
            tick,
            update_seq: &mut seq,
        };
        match call {
            WaterSoundCall::Update => w.update(&r.settings, &mut cx),
            WaterSoundCall::Emitter(order) => w.update_emitter(order, &mut cx),
        }
        self.world.set_update_seq(seq);
    }
}

/// Which of the surface loop's turns runs.
#[derive(Debug, Clone, Copy)]
enum WaterSoundCall {
    Update,
    Emitter(u64),
}

#[cfg(test)]
#[path = "tests/water_sound_turn.rs"]
mod tests;
