//! `BulletDrip`: one of the heavy drops of the death rain's bullet rain. Its fields,
//! `Update` and the part of `Strike` that picks the column and the geometry (the draws,
//! in order); what a strike hits, the water entry and the sounds are the game side's
//! (`game/bullet_drip_turn.rs`). Type: [`BulletDrip`], kept in `RoomRain::bullet_drips`.
//!
//! Mirrors: BulletDrip.cs (BulletDrip, Update, Strike)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// `BulletDrip`.
#[derive(Debug, Clone, PartialEq)]
pub struct BulletDrip {
    pub update_order: u64,
    pub pos: Vec2,
    pub sky_pos: Vec2,
    pub counter: i32,
    pub last_falling: f32,
    pub falling: f32,
    pub move_tip: bool,
    pub fall_speed: f32,
    pub delay: i32,
}

impl BulletDrip {
    #[must_use]
    pub fn new(update_order: u64) -> Self {
        Self {
            update_order,
            pos: Vec2::default(),
            sky_pos: Vec2::default(),
            counter: 0,
            last_falling: 1.0,
            falling: 1.0,
            move_tip: false,
            fall_speed: 0.0,
            delay: 0,
        }
    }

    /// `Update`: the fall advances; once it has landed (`lastFalling >= 1`) the delay
    /// counts down. Returns whether `Strike` runs.
    pub fn update(&mut self) -> bool {
        self.last_falling = self.falling;
        self.falling = (self.falling + self.fall_speed).min(1.0);
        self.move_tip = false;
        if self.last_falling >= 1.0 {
            self.delay -= 1;
            return self.delay < 1;
        }
        false
    }

    /// `Strike` up to the trace: a column (`Range(0, TileWidth)`) whose top tile is solid
    /// or whose `rainReach` is the room's height returns at once, leaving `delay` as it is
    /// (so the next tick strikes again, user decision, D50). Otherwise `pos` is above the
    /// first solid tile (`-10` for a column without one) and `sky_pos` above the room, the
    /// fall restarts and `delay` is redrawn. Returns whether it got that far.
    pub fn strike_setup(
        &mut self,
        room: &Room,
        rain_reach: &[i32],
        rain_direction: f32,
        bullet_rain_density: f32,
        rng: &mut UnityRandom,
    ) -> bool {
        let column = rng.range_i32(0, room.width());
        if room.tile(column, room.height() - 1).is_solid() {
            return false;
        }
        let mut row = rain_reach[column as usize];
        if row >= room.height() {
            return false;
        }
        if row == 0 {
            row = -10;
        }
        self.pos =
            Vec2::middle_of_tile((column, row)) + Vec2::new((-10.0).lerp(10.0, rng.value()), 10.0);
        self.sky_pos = Vec2::middle_of_tile((column, row + room.height() + 50))
            + Vec2::new((-30.0).lerp(30.0, rng.value()) - 30.0 * rain_direction, 0.0);
        self.falling = 0.0;
        self.last_falling = 0.0;
        self.move_tip = true;
        self.fall_speed = 1.0 / 0.2_f32.lerp(1.8, rng.value());
        self.delay = rng.range_i32(0, 60 - (bullet_rain_density * 60.0) as i32);
        true
    }

    /// The tip's drawn position and the fall's: `DrawSprites` starts the tip at `skyPos`
    /// on the strike's tick.
    #[must_use]
    pub fn tip(&self, time_stacker: f32) -> Vec2 {
        if self.move_tip {
            self.sky_pos.lerp(self.pos, time_stacker)
        } else {
            self.pos
        }
    }

    /// The drop's end at `timeStacker`.
    #[must_use]
    pub fn head(&self, time_stacker: f32) -> Vec2 {
        self.sky_pos
            .lerp(self.pos, self.last_falling.lerp(self.falling, time_stacker))
    }
}

#[cfg(test)]
#[path = "tests/bullet_drip.rs"]
mod tests;
