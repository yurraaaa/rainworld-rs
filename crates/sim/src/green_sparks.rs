//! `GreenSparks` (the `GreenSparks` room effect's spawner) and `GreenSparks.GreenSpark`
//! (a spark drifting through the room, in the play layer or on a depth, lighting dark
//! rooms). Every draw is on the shared stream. Base game only: the More Slugcats branch
//! (every x sign flipped, sparks entering at the right edge) is not ported (D29).
//! Sprites: `rw-graphics` `green_spark.rs`.
//!
//! Mirrors: GreenSparks.cs (GreenSparks, GreenSpark), Room.cs (NowViewed: GreenSparks)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::camera::CameraView;
use crate::insect::InsectWater;
use crate::shared_physics::TileRect;
use crate::world::AiMap;
use crate::{FloatExt, RoomExt, Vec2};

/// What a spark's `Update` reads of its room.
#[derive(Debug, Clone, Copy)]
pub struct SparkCx<'a> {
    pub room: &'a Room,
    pub room_index: i32,
    pub water: &'a InsectWater<'a>,
    pub camera: &'a CameraView,
    /// `Room.Darkness(pos)`: camera 0's palette darkness when it shows the room.
    pub darkness: f32,
    /// `Room.BeingViewed`.
    pub being_viewed: bool,
}

/// `GreenSparks.GreenSpark`.
#[derive(Debug, Clone, PartialEq)]
pub struct GreenSpark {
    pub update_order: u64,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub last_last_pos: Vec2,
    pub vel: Vec2,
    pub dir: Vec2,
    pub life: f32,
    pub life_time: f32,
    pub depth: f32,
    /// Its `LightSource`, by id, once it has one.
    pub light: Option<u64>,
}

impl GreenSpark {
    /// `col`.
    pub const COLOR: [f32; 3] = [0.0, 1.0, 1.0 / 85.0];

    /// The constructor: 3 draws in the play layer, else 3 (negative depth) or 4.
    #[must_use]
    pub fn new(pos: Vec2, rng: &mut UnityRandom) -> Self {
        let life_time = 600.0_f32.lerp(1200.0, rng.value());
        let depth = if rng.value() < 0.4 {
            0.0
        } else if rng.value() < 0.3 {
            -0.5 * rng.value()
        } else {
            rng.value().mathf_pow(1.5) * 3.0
        };
        Self {
            update_order: 0,
            slated: false,
            pos,
            last_pos: pos,
            last_last_pos: pos,
            vel: Vec2::ZERO,
            dir: Vec2::ZERO,
            life: 1.0,
            life_time,
            depth,
            light: None,
        }
    }

    /// `InPlayLayer`.
    #[must_use]
    pub fn in_play_layer(&self) -> bool {
        self.depth == 0.0
    }

    /// `Room.NowViewed`'s sparks for a `GreenSparks` effect of `amount`: `W * H *
    /// amount / 50` tries of two position draws, a third (`Pow(value, 1 - amount)`
    /// against the terrain proximity) on an open tile, and the constructor's draws on
    /// success. `ai_map` is the room's once `readyForAI`.
    #[must_use]
    pub fn seed_room(
        room: &Room,
        amount: f32,
        ai_map: Option<&AiMap>,
        rng: &mut UnityRandom,
    ) -> Vec<Self> {
        let mut made = Vec::new();
        let mut j = 0;
        while (j as f32) < ((room.width() * room.height()) as f32) * amount / 50.0 {
            let pos = Vec2::new(
                rng.value() * room.pixel_width(),
                rng.value() * room.pixel_height(),
            );
            let proximity = ai_map.map_or(5, |m| {
                let (x, y) = pos.tile();
                m.terrain_proximity(x, y)
            });
            if !room.is_solid_at(pos)
                && rng.value().mathf_pow(1.0 - amount) > proximity as f32 * 0.05
            {
                made.push(Self::new(pos, rng));
            }
            j += 1;
        }
        made
    }

    /// `Update` (one draw). Returns where its light should be and how bright when it
    /// has one (`depth <= 0` in a dark room); the caller makes, moves or removes the
    /// `LightSource` (kept in [`Self::light`]).
    pub fn update(&mut self, cx: &SparkCx, rng: &mut UnityRandom) -> Option<(Vec2, f32)> {
        let room = cx.room;
        self.vel *= 0.99;
        self.vel += Vec2::new(0.11, self.life.lerp_map(0.0, 0.5, -0.1, 0.05));
        self.vel += self.dir * 0.2;
        self.dir = (self.dir + Vec2::random_unit(rng) * 0.6).normalized();
        self.life -= 1.0 / self.life_time;
        self.last_last_pos = self.last_pos;
        self.last_pos = self.pos;
        self.pos += self.vel / (self.depth + 1.0);
        if self.in_play_layer() {
            self.collide_with_terrain(room);
            if cx.water.submerged(self.pos) {
                self.pos.y = cx.water.level(self.pos);
                self.life -= 0.025;
            }
        }
        let outside = self.pos.distance(Self::restrict_in_room(self.pos, room)) > 100.0;
        if self.life < 0.0 || (outside && !cx.camera.viewed(cx.room_index, room, self.pos, 400.0)) {
            self.slated = true;
        }
        let light = (self.depth <= 0.0 && cx.darkness > 0.0).then(|| {
            let alpha = 0.4 * self.life.inverse_lerp(0.0, 0.2) * self.depth.inverse_lerp(-0.6, 0.0);
            (self.pos, alpha)
        });
        if !cx.being_viewed {
            self.slated = true;
        }
        light
    }

    /// The play layer's wall: a solid tile costs life, a spark that entered it is put
    /// back on the tile's grown edge and bounces (0.3 of the speed), one that was
    /// already inside is lifted onto the tile's middle row plus 10.
    fn collide_with_terrain(&mut self, room: &Room) {
        if !room.is_solid_at(self.pos) {
            return;
        }
        self.life -= 0.025;
        if room.is_solid_at(self.last_pos) {
            self.pos.y = self.pos.tile_middle().y + 10.0;
            return;
        }
        let hit = room
            .ray_trace_tiles_first_solid(self.last_pos.tile(), self.pos.tile())
            .unwrap_or_else(|| self.pos.tile());
        let rect = TileRect::of_tile(hit).grow(2.0);
        let (corner, normal) = rect.rect_collision(self.pos, self.last_pos);
        self.pos = corner;
        let bounce = 0.3;
        if normal.x < 0.0 {
            self.vel.x = self.vel.x.abs() * bounce;
        } else if normal.x > 0.0 {
            self.vel.x = -self.vel.x.abs() * bounce;
        } else if normal.y < 0.0 {
            self.vel.y = self.vel.y.abs() * bounce;
        } else if normal.y > 0.0 {
            self.vel.y = -self.vel.y.abs() * bounce;
        }
    }

    /// `Custom.RestrictInRect(pos, room.RoomRect)`.
    fn restrict_in_room(pos: Vec2, room: &Room) -> Vec2 {
        Vec2::new(
            pos.x.clamp(0.0, room.pixel_width()),
            pos.y.clamp(0.0, room.pixel_height()),
        )
    }
}

/// `GreenSparks`, the spawner.
#[derive(Debug, Clone, PartialEq)]
pub struct GreenSparks {
    pub update_order: u64,
    /// The sparks it made, by their places in the update list (not the ones
    /// `NowViewed` seeded).
    pub sparks: Vec<u64>,
    pub tot_sparks: usize,
    pub wind: Vec2,
}

impl GreenSparks {
    /// The constructor: `Clamp((int)(W * H * Lerp(0, 0.1, amount)), 1, 300)` sparks.
    #[must_use]
    pub fn new(room: &Room, amount: f32) -> Self {
        let cells = (room.width() * room.height()) as f32;
        let count = (cells * 0.0_f32.lerp(0.1, amount))
            .to_int_cs()
            .clamp(1, 300);
        Self {
            update_order: 0,
            sparks: Vec::new(),
            tot_sparks: count as usize,
            wind: Vec2::ZERO,
        }
    }

    /// `Update`: its slated sparks are dropped and the others pushed by the wind, a
    /// spark added when under the cap, then the wind wanders (1 draw). `all` is the
    /// room's sparks in update order. The new spark, if any, still needs its place in
    /// the update list and [`Self::sparks`].
    pub fn update(
        &mut self,
        all: &mut [GreenSpark],
        room: &Room,
        room_index: i32,
        camera: &CameraView,
        rng: &mut UnityRandom,
    ) -> Option<GreenSpark> {
        let wind = self.wind;
        self.sparks.retain(|&order| {
            let Ok(i) = all.binary_search_by_key(&order, |s| s.update_order) else {
                return false;
            };
            if all[i].slated {
                return false;
            }
            all[i].vel += wind * 0.2;
            true
        });
        let added = if self.sparks.len() < self.tot_sparks {
            self.add_spark(room, room_index, camera, rng)
        } else {
            None
        };
        self.wind += Vec2::random_unit(rng) * 0.1;
        self.wind *= 0.98;
        self.wind = self.wind.clamp_magnitude(1.0);
        added
    }

    /// `AddSpark`: one draw picks the left edge or the bottom, a second the place on
    /// it; nothing comes of a solid or watery tile. The spark enters just off the
    /// camera's view.
    fn add_spark(
        &self,
        room: &Room,
        room_index: i32,
        camera: &CameraView,
        rng: &mut UnityRandom,
    ) -> Option<GreenSpark> {
        let (w, h) = (room.width(), room.height());
        let tile = if rng.value() < h as f32 / w as f32 {
            (0, rng.range_i32(0, h))
        } else {
            (rng.range_i32(0, w), 0)
        };
        if room.is_solid_tile(tile.0, tile.1) || room.water_int(tile.0, tile.1) != 0 {
            return None;
        }
        let mut at = Vec2::middle_of_tile(tile);
        let centre = Vec2::new(room.pixel_width() / 2.0, room.pixel_height() / 2.0);
        for _ in 0..10 {
            if !camera.viewed(room_index, room, at, 200.0) {
                break;
            }
            at += centre.direction_to(at) * 100.0;
        }
        Some(GreenSpark::new(at, rng))
    }
}
