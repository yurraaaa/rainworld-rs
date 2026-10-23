//! The game's side of a room's `Lightning`: `Room.Loaded` making it, its turn in the
//! update list with the room calls it makes (`MakeBackgroundNoise`, `ScreenMovement`,
//! `PlaySound`), and its loop emitters' turns. Type: [`crate::Lightning`].
//!
//! Mirrors: Room.cs (Loaded: lightning, MakeBackgroundNoise, ScreenMovement)

use std::sync::Arc;

use rw_assets::{RoomEffectType, SoundTable};
use rw_random::UnityRandom;

use super::{RainWorldGame, RoomObject};
use crate::camera::CameraView;
use crate::item::LoopCx;
use crate::lightning::{LightningEnv, LightningRoom};
use crate::world::World;
use crate::{CameraShake, Lightning, ScreenMovement, Sounds};

/// What `Lightning.Update` calls on room `index` of the game.
struct LightningHost<'a> {
    world: &'a mut World,
    index: i32,
    camera: &'a CameraView,
    shake: &'a mut CameraShake,
    sounds: &'a mut Sounds,
    rng: &'a mut UnityRandom,
    triggers: &'a SoundTable,
    camera_room: i32,
    tick: u64,
    seq: &'a mut u64,
}

impl LightningRoom for LightningHost<'_> {
    fn rng(&mut self) -> &mut UnityRandom {
        self.rng
    }

    fn make_background_noise(&mut self, noise: f32) {
        if let Some(r) = self.world.realized_mut(self.index) {
            r.make_background_noise(noise);
        }
    }

    fn screen_movement(&mut self, m: ScreenMovement) {
        self.world
            .screen_movement(self.index, m, self.camera, self.shake, self.rng);
    }

    fn loop_cx(&mut self) -> LoopCx<'_> {
        LoopCx {
            room_index: self.index,
            sounds: self.sounds,
            rng: self.rng,
            triggers: self.triggers,
            camera_room: self.camera_room,
            tick: self.tick,
            update_seq: self.seq,
        }
    }
}

impl RainWorldGame {
    /// `Room.Loaded`'s `lightning` (the first `Lightning` or `BkgOnlyLightning` effect
    /// of the room), joining the update list.
    pub(super) fn add_lightning(&mut self, index: i32) {
        let Some(effect) = self.world.realized(index).and_then(|r| {
            r.settings.effects.iter().find_map(|e| {
                let bkg_only = e.kind == RoomEffectType::BkgOnlyLightning.value();
                (bkg_only || e.kind == RoomEffectType::Lightning.value())
                    .then_some((e.amount, bkg_only))
            })
        }) else {
            return;
        };
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.lightning = Some(Lightning::new(order, effect.0, effect.1));
        }
    }

    /// Room `index`'s `Lightning` and its loop emitters in its update list.
    pub(super) fn room_lightning_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let Some(l) = self
            .world
            .realized(index)
            .and_then(|r| r.lightning.as_ref())
        else {
            return Vec::new();
        };
        let mut objects: Vec<(u64, RoomObject)> = l
            .emitters()
            .map(|e| (e.update_order, RoomObject::LightningLoop(e.update_order)))
            .collect();
        objects.push((l.update_order, RoomObject::Lightning));
        objects
    }

    /// The `Lightning`'s `Update` in room `index`.
    pub(super) fn run_lightning(&mut self, index: i32) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let extra_loud_thunder = r.settings.effect_amount(RoomEffectType::ExtraLoudThunder);
        let Some(mut lightning) = r.lightning.take() else {
            return;
        };
        let cycle = &self.world.rain_cycle;
        let env = LightningEnv {
            extra_loud_thunder,
            time_until_rain: cycle.time_until_rain(),
            cycle_end_boost: LightningEnv::boost(cycle.cycle_length, cycle.timer),
        };
        let triggers = Arc::clone(self.sounds.table());
        let mut seq = self.world.update_seq();
        let (camera_room, tick) = (self.camera_room, self.tick);
        let sounds = if index == camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut host = LightningHost {
            world: &mut self.world,
            index,
            camera: &self.camera_view,
            shake: &mut self.camera_shake,
            sounds,
            rng: &mut self.rng,
            triggers: &triggers,
            camera_room,
            tick,
            seq: &mut seq,
        };
        lightning.update(env, &mut host);
        self.world.set_update_seq(seq);
        if let Some(r) = self.world.realized_mut(index) {
            r.lightning = Some(lightning);
        }
    }

    /// The `SoundEmitter.Update` of the `Lightning`'s loop emitter at `order`.
    pub(super) fn run_lightning_emitter(&mut self, index: i32, order: u64) {
        let triggers = Arc::clone(self.sounds.table());
        let mut seq = self.world.update_seq();
        let (camera_room, tick) = (self.camera_room, self.tick);
        let sounds = if index == camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let Some(l) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.lightning.as_mut())
        else {
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
        l.update_emitter(order, &mut cx);
        self.world.set_update_seq(seq);
    }
}

#[cfg(test)]
#[path = "game_lightning_tests.rs"]
mod tests;
