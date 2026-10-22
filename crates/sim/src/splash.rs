//! `Splash` (`SplashWater.Splash : WaterParticle`): a blob of water thrown up where a
//! body crosses the surface, a `CosmeticSprite` in its room's update list. It flies,
//! swells toward its `amount`, trails a linger point that hangs below it, and dies
//! within 60 ticks; falling back in it upsets the surface (`WaterfallHitSurface`) and
//! once plays `Splashing_Water_Into_Water_Surface`; hitting terrain it bounces, plays
//! `Splashing_Water_Into_Terrain` and sheds a water-coloured `WaterDrip`. Neither `new
//! Splash()` nor `Reset` draws; the update draws only for the drip and the surface. The
//! sprites are `rw-graphics`'. Behaviour: `rainworld-re/notes/splash.md`.
//! Remix off and no DLC: `PointSubmerged`'s inverted-water branch is left out (D11).
//!
//! Mirrors: SplashWater/Splash.cs, SplashWater/WaterParticle.cs

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::room_ext::RoomExt;
use crate::shared_physics::TileRect;
use crate::{FloatExt, SoundId, Sounds, Vec2, Water, WaterDrip};

/// One `Splash`.
#[derive(Debug, Clone, PartialEq)]
pub struct Splash {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    /// The radius it swells toward (`WaterParticle.amount`).
    pub amount: f32,
    pub rad: f32,
    pub last_rad: f32,
    pub life: f32,
    pub last_life: f32,
    /// Ticks a full life lasts (60).
    pub life_time: f32,
    /// Ticks until it may play a splash sound (`makeSoundCounter`, 4 at birth).
    pub make_sound_counter: i32,
    /// The point the sprites stretch down to.
    pub linger_pos: Vec2,
    pub last_linger_pos: Vec2,
    pub linger_pos_vel: Vec2,
}

impl Splash {
    /// `new Splash()` then `Reset(pos, vel, amount, initRad)`; `water_level` is
    /// `Room.FloatWaterLevel(pos)`. It starts `init_rad` along `vel` from `pos` (where
    /// `last_pos` stays) with its linger point under the surface.
    #[must_use]
    pub fn new(pos: Vec2, vel: Vec2, amount: f32, init_rad: f32, water_level: f32) -> Self {
        let linger_pos = Vec2::new(pos.x, water_level - 30.0 - init_rad);
        Self {
            pos: pos + vel.normalized() * init_rad,
            last_pos: pos,
            vel,
            amount,
            rad: init_rad,
            last_rad: 0.0,
            life: 1.0,
            last_life: 1.0,
            life_time: 60.0,
            make_sound_counter: 4,
            linger_pos,
            last_linger_pos: linger_pos,
            linger_pos_vel: Vec2::ZERO,
        }
    }

    /// `Splash.Update`: true once it is destroyed (`lastLife < 0` at its head: the second
    /// update after `life` went below 0, which still runs whole). A terrain hit returns the drip it sheds, which joins the
    /// room then. Draws: the drip's two (direction, then its life), and while it is
    /// under the surface `WaterfallHitSurface`'s per point (one, three on a hit), each
    /// sound's.
    pub fn update(
        &mut self,
        room: &Room,
        water: &mut Water,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> (bool, Option<WaterDrip>) {
        let destroyed = self.last_life < 0.0;
        let drip = self.particle_update(room, rng, sounds);
        self.last_linger_pos = self.linger_pos;
        self.linger_pos += self.linger_pos_vel;
        let floor = water.level_at(self.linger_pos.x) - 20.0;
        if floor > self.linger_pos.y {
            self.linger_pos.y = floor;
        }
        self.linger_pos_vel.y -= 0.9;
        if self.linger_pos.dist_less(self.pos, self.rad) {
            self.linger_pos = self.pos + self.pos.direction_to(self.linger_pos) * self.rad;
        }
        if self.pos.y < water.level_at(self.pos.x) {
            self.life -= 0.06;
            let flow = self.vel.y.inverse_lerp(-2.0, -8.0);
            water.waterfall_hit_surface(self.pos, 10.0, flow, rng);
            self.vel.y = self.vel.y.abs() * 0.4;
            let speed = self.vel.length();
            if self.make_sound_counter <= 0 && speed > 4.0 {
                let volume = speed.inverse_lerp(4.0, 14.0);
                let id = SoundId::SplashingWaterIntoWaterSurface;
                sounds.play_at(id, self.pos, volume, 1.0, rng);
                self.make_sound_counter = i32::MAX;
            }
        } else {
            self.vel.y -= 0.9;
        }
        (destroyed, drip)
    }

    /// `CosmeticSprite.Update` then `WaterParticle.Update`: moves, slows, falls, swells
    /// toward `amount` (losing life once past 1.5 times it) and, entering a Solid tile,
    /// stops at its edge, bounces and sheds a drip.
    fn particle_update(
        &mut self,
        room: &Room,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> Option<WaterDrip> {
        self.last_pos = self.pos;
        self.pos += self.vel;
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time;
        self.make_sound_counter = self.make_sound_counter.wrapping_sub(1);
        self.vel *= 0.97;
        self.vel.y -= 0.9;
        self.last_rad = self.rad;
        self.rad += self.vel.length().inverse_lerp(1.0, 20.0);
        if self.rad < self.amount {
            self.rad = self.rad.lerp(self.amount, 0.1);
        }
        if self.rad > self.amount * 1.5 {
            self.life -= 0.05;
        }
        if self.rad > self.amount * 2.0 {
            self.rad = self.amount * 2.0;
        }
        if !room.is_solid_at(self.pos) || room.is_solid_at(self.last_pos) {
            return None;
        }
        // The trace ends on the Solid tile `pos` is in, so it always finds one.
        let hit = room
            .ray_trace_tiles_first_solid(self.last_pos.tile(), self.pos.tile())
            .unwrap_or_else(|| self.pos.tile());
        let rect = TileRect::of_tile(hit).grow(2.0);
        let (corner, normal) = rect.rect_collision(self.pos, self.last_pos);
        self.pos = corner;
        let speed = self.vel.length();
        if self.make_sound_counter <= 0 && speed > 4.0 {
            let volume = speed.inverse_lerp(4.0, 16.0);
            sounds.play_at(
                SoundId::SplashingWaterIntoTerrain,
                self.pos,
                volume,
                1.0,
                rng,
            );
            self.make_sound_counter = 10;
        }
        if normal.x < 0.0 {
            self.vel.x = self.vel.x.abs() * 0.4;
        } else if normal.x > 0.0 {
            self.vel.x = -self.vel.x.abs() * 0.4;
        } else if normal.y < 0.0 {
            self.vel.y = self.vel.y.abs() * 0.4;
        } else if normal.y > 0.0 {
            self.vel.y = -self.vel.y.abs() * 0.4;
        }
        let speed = self.vel.length();
        let drip_vel = self.vel + Vec2::random_unit(rng) * speed;
        let drip = WaterDrip::new(self.pos, drip_vel, true, rng);
        self.life -= 0.02;
        Some(drip)
    }
}

#[cfg(test)]
#[path = "splash_tests.rs"]
mod tests;
