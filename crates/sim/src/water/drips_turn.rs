//! The game's side of a room's ambient dripping: `Room.Update`'s ceiling drip and
//! `Room.Loaded`'s `DrippingSound` with its turns in the update list. Types:
//! [`crate::WaterDrip`], [`crate::DrippingSound`].
//!
//! Mirrors: Room.cs (Update: the ceiling drip; Loaded: DrippingSound)

use std::sync::Arc;

use crate::DrippingSound;
use crate::game::{RainWorldGame, RoomObject};
use crate::item::LoopCx;

impl RainWorldGame {
    /// `Room.Update`'s ceiling drip in room `index`, viewed when camera 0 shows it.
    pub(crate) fn ambient_ceiling_drip(&mut self, index: i32) {
        let viewed = self.camera_room == index;
        let shelter = self.world.abstract_room(index).is_some_and(|r| r.shelter());
        let start_up = self.world.rain_cycle.cycle_start_up();
        let Some(room) = self.world.realized(index) else {
            return;
        };
        if let Some(drip) = room.ambient_ceiling_drip(viewed, shelter, start_up, &mut self.rng) {
            self.world.add_drip(index, drip);
        }
    }

    /// `Room.Loaded`'s `DrippingSound`, after the trigger checkers.
    pub(crate) fn add_dripping_sound(&mut self, index: i32) {
        let shelter = self.world.abstract_room(index).is_some_and(|r| r.shelter());
        let start_up = self.world.rain_cycle.cycle_start_up();
        let wanted = self
            .world
            .realized(index)
            .is_some_and(|r| DrippingSound::wanted(&r.settings, shelter, start_up));
        if !wanted {
            return;
        }
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.dripping_sound = Some(DrippingSound::new(order));
        }
    }

    /// Room `index`'s `DrippingSound` and its loop's emitters in its update list.
    pub(crate) fn room_dripping_sound_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let Some(d) = self
            .world
            .realized(index)
            .and_then(|r| r.dripping_sound.as_ref())
        else {
            return Vec::new();
        };
        let mut objects: Vec<(u64, RoomObject)> = d
            .emitters()
            .map(|e| {
                (
                    e.update_order,
                    RoomObject::DrippingSoundLoop(e.update_order),
                )
            })
            .collect();
        if !d.slated {
            objects.push((d.update_order, RoomObject::DrippingSound));
        }
        objects
    }

    /// The `DrippingSound`'s `Update` in room `index`.
    pub(crate) fn run_dripping_sound(&mut self, index: i32) {
        let start_up = self.world.rain_cycle.cycle_start_up();
        self.with_dripping_sound(index, |d, cx| d.update(start_up, cx));
    }

    /// The `SoundEmitter.Update` of the `DrippingSound`'s emitter at `order`.
    pub(crate) fn run_dripping_sound_emitter(&mut self, index: i32, order: u64) {
        self.with_dripping_sound(index, |d, cx| d.update_emitter(order, cx));
    }

    /// `f` on room `index`'s `DrippingSound` with what its loop reads of the game; the
    /// sound leaves the room once destroyed with no emitter left.
    fn with_dripping_sound(
        &mut self,
        index: i32,
        f: impl FnOnce(&mut DrippingSound, &mut LoopCx<'_>),
    ) {
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
        let Some(d) = r.dripping_sound.as_mut() else {
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
        f(d, &mut cx);
        if d.finished() {
            r.dripping_sound = None;
        }
        self.world.set_update_seq(seq);
    }
}

#[cfg(test)]
#[path = "tests/drips_turn.rs"]
mod tests;
