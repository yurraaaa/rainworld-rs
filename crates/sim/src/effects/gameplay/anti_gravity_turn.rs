//! The game's side of zero gravity: `Room.Loaded` adding the room's `AntiGravity`, its
//! turn in the update list (the room's `gravity`), and the world's
//! `brokenAntiGrav.Update` from `RainCycle.Update` with the camera room's switch sound
//! and screen movement. Types: [`crate::effects::gameplay::anti_gravity`].
//!
//! Mirrors: Room.cs (Loaded: AntiGravity), AntiGravity.cs (Update, BrokenAntiGravity.Update calls)

use rw_assets::RoomEffectType;

use crate::effects::gameplay::anti_gravity::{FLICKER_CYCLE_MAX, FLICKER_CYCLE_MIN};
use crate::game::{RainWorldGame, RoomObject};
use crate::{AntiGravity, BrokenAntiGravity, ScreenMovement, SoundId, Vec2};

impl RainWorldGame {
    /// `Room.gravity` of realized room `index` (1 for an unrealized one).
    pub(crate) fn room_gravity(&self, index: i32) -> f32 {
        self.world.realized(index).map_or(1.0, |r| r.gravity)
    }

    /// `Room.Loaded`'s `ZeroG` / `BrokenZeroG` effect: one `AntiGravity` per room, whose
    /// constructor makes the world's `BrokenAntiGravity` for the first `BrokenZeroG` room.
    pub(crate) fn add_anti_gravity(&mut self, index: i32) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        if r.anti_gravity.is_some() {
            return;
        }
        let broken = r.settings.effect_amount(RoomEffectType::BrokenZeroG) > 0.0;
        if broken && self.world.rain_cycle.broken_anti_grav.is_none() {
            self.world.rain_cycle.broken_anti_grav = Some(BrokenAntiGravity::new(
                FLICKER_CYCLE_MIN,
                FLICKER_CYCLE_MAX,
                &mut self.rng,
            ));
        }
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.anti_gravity = Some(AntiGravity::new(order));
        }
    }

    /// The gravity `VirtualMicrophone.Update` fades the ambience (volume group 2) by: the
    /// camera room's, in a `BrokenZeroG` room of region SS.
    #[must_use]
    pub fn ambience_gravity(&self) -> Option<f32> {
        let r = self.world.realized(self.camera_room)?;
        (self.world.region_name() == Some("SS")
            && r.settings.effect_amount(RoomEffectType::BrokenZeroG) > 0.0)
            .then_some(r.gravity)
    }

    /// The room's `AntiGravity` in its update list.
    pub(crate) fn room_anti_gravity_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.anti_gravity.as_ref())
            .map(|a| (a.update_order, RoomObject::AntiGravity))
            .into_iter()
            .collect()
    }

    /// `AntiGravity.Update` of room `index`.
    pub(crate) fn run_anti_gravity(&mut self, index: i32) {
        let broken = self.world.rain_cycle.broken_anti_grav;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        if !r.anti_gravity.as_ref().is_some_and(|a| a.active) {
            return;
        }
        r.gravity = AntiGravity::gravity(
            r.settings.effect_amount(RoomEffectType::ZeroG),
            r.settings.effect_amount(RoomEffectType::BrokenZeroG),
            broken.as_ref(),
        );
        let gravity = r.gravity;
        if self.pos().room == index
            && let Some(p) = self.player.as_mut()
        {
            p.set_room_gravity(gravity);
        }
    }

    /// `brokenAntiGrav.Update` (from `RainCycle.Update`, after the timers): the switch
    /// sound and the screen movement go to the camera's room if it has `BrokenZeroG`.
    pub(crate) fn update_broken_anti_gravity(&mut self) {
        let Some(mut broken) = self.world.rain_cycle.broken_anti_grav else {
            return;
        };
        let tick = broken.update(&mut self.rng);
        self.world.rain_cycle.broken_anti_grav = Some(broken);
        let room = self.camera_room;
        let amount = self.world.realized(room).map_or(0.0, |r| {
            r.settings.effect_amount(RoomEffectType::BrokenZeroG)
        });
        if amount <= 0.0 {
            return;
        }
        if let Some(on) = tick.switched {
            let id = if on {
                SoundId::BrokenAntiGravitySwitchOn
            } else {
                SoundId::BrokenAntiGravitySwitchOff
            };
            self.sounds
                .play_disembodied(id, 0.0, amount, 1.0, &mut self.rng);
        }
        if tick.shaking {
            let m = ScreenMovement {
                source: None,
                bump: Vec2::ZERO,
                shake: broken.shake(amount),
            };
            self.world.screen_movement(
                room,
                m,
                &self.camera_view,
                &mut self.camera_shake,
                &mut self.rng,
            );
        }
    }
}

#[cfg(test)]
#[path = "tests/anti_gravity_turn.rs"]
mod tests;
