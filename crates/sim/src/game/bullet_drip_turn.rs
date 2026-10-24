//! The game's side of `BulletDrip`s: `RoomRain.Update`'s upkeep of their count and each
//! drip's turn in the room's update list. A strike traces from the sky to the ground
//! against the bodies of collision layer 1, then plays `Small_Object_Into_Water_Fast`
//! (after `WaterfallHitSurface`) when it lands under the water surface or
//! `Bullet_Drip_Strike` otherwise, and pushes down (and stuns) what it hit. The Watcher's
//! `rippleLayer` test and `rippleFade` (1) are the DLC's (D50).
//!
//! Mirrors: BulletDrip.cs (Update, Strike), RoomRain.cs (Update: bulletDrips),
//! SharedPhysics.cs (TraceProjectileAgainstBodyChunks)

use crate::ChunkOwner;
use crate::game::{RainWorldGame, RoomObject};
use crate::sound::SoundId;
use crate::world::BulletDrip;

impl RainWorldGame {
    /// `RoomRain.Update`: one drip added, or the oldest destroyed, per tick until the
    /// count is `(int)(TileWidth * bulletRainDensity * RainIntensity)`.
    pub(crate) fn update_bullet_drip_count(&mut self, index: i32) {
        let density = self.world.rain_cycle.global_rain.bullet_rain_density;
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let Some(rain) = r.room_rain.as_ref() else {
            return;
        };
        let wanted = (r.room.width() as f32 * density * r.settings.rain_intensity) as i32;
        let count = rain.bullet_drips.len() as i32;
        if count < wanted {
            let order = self.world.next_update_seq();
            if let Some(rain) = self
                .world
                .realized_mut(index)
                .and_then(|r| r.room_rain.as_mut())
            {
                rain.bullet_drips.push(BulletDrip::new(order));
            }
        } else if count > wanted
            && let Some(rain) = self
                .world
                .realized_mut(index)
                .and_then(|r| r.room_rain.as_mut())
        {
            rain.bullet_drips.remove(0);
        }
    }

    /// Room `index`'s drips in its update list.
    pub(crate) fn room_bullet_drip_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.room_rain.as_ref())
            .map(|rain| {
                rain.bullet_drips
                    .iter()
                    .map(|d| (d.update_order, RoomObject::BulletDrip(d.update_order)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `BulletDrip.Update` of the drip at `order` in room `index`.
    pub(crate) fn run_bullet_drip(&mut self, index: i32, order: u64) {
        let global = &self.world.rain_cycle.global_rain;
        let (direction, density) = (global.rain_direction, global.bullet_rain_density);
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let room = r.room.clone();
        let Some(rain) = r.room_rain.as_mut() else {
            return;
        };
        let Some(drip) = rain
            .bullet_drips
            .iter_mut()
            .find(|d| d.update_order == order)
        else {
            return;
        };
        if !drip.update() {
            return;
        }
        let mut struck = drip.clone();
        let reach = rain.rain_reach.clone();
        if struck.strike_setup(&room, &reach, direction, density, &mut self.rng) {
            self.bullet_drip_lands(index, &mut struck);
        }
        if let Some(d) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.room_rain.as_mut())
            .and_then(|rain| {
                rain.bullet_drips
                    .iter_mut()
                    .find(|d| d.update_order == order)
            })
        {
            *d = struck;
        }
    }

    /// `Strike` from the trace on: the layer-1 chunk the drop's path meets first (no
    /// draws), the water entry or the strike sound, then the hit chunk's push and stun.
    fn bullet_drip_lands(&mut self, index: i32, drip: &mut BulletDrip) {
        let player_seq = self.player_here(index).then_some(self.player_seq);
        let mut hit: Option<(ChunkOwner, usize, f32)> = None;
        let mut best = f32::MAX;
        for (layer, _, owner) in self.world.physical_objects(index, player_seq) {
            if layer != 1 {
                continue;
            }
            for i in 0.. {
                let Some(chunk) = self.electric_chunk(owner, i) else {
                    break;
                };
                let t = drip.sky_pos.circles_collision_time(
                    chunk.pos,
                    drip.pos - drip.sky_pos,
                    0.5,
                    chunk.rad,
                );
                if t > 0.0 && t < 1.0 && t < best {
                    best = t;
                    hit = Some((owner, i, t));
                }
            }
        }
        let submerged = self
            .world
            .realized(index)
            .and_then(|r| r.water.as_ref())
            .map(|w| w.level_at(drip.pos.x))
            .filter(|&level| drip.pos.y < level);
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let sound = if let Some(level) = submerged {
            drip.pos.y = level - 30.0;
            if let Some(w) = self
                .world
                .realized_mut(index)
                .and_then(|r| r.water.as_mut())
            {
                w.waterfall_hit_surface(drip.pos, 0.0, 1.0, &mut self.rng);
            }
            SoundId::SmallObjectIntoWaterFast
        } else {
            SoundId::BulletDripStrike
        };
        sounds.play_at(sound, drip.pos, 1.0, 1.0, &mut self.rng);
        let Some((owner, i, t)) = hit else {
            return;
        };
        drip.pos = drip.sky_pos.lerp(drip.pos, t);
        if let Some(chunk) = self.electric_chunk_mut(owner, i) {
            chunk.vel.y -= 2.0 / chunk.mass;
        }
        if let Some(id) = self.rain_creature(owner) {
            let ticks = self.rng.range_i32(0, 4);
            self.creature_stun(id, ticks);
        }
    }
}

#[cfg(test)]
#[path = "tests/bullet_drip_turn.rs"]
mod tests;
