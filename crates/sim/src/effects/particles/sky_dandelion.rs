//! `SkyDandelions` (the room effect's spawner), `SkyDandelions.SkyDandelion` (a seed
//! drifting up on the wind, in the play layer or on a depth) and `DandelionStalk` (the
//! stalk of a `DandelionPatch` dandelion, which a passing body blows loose). The
//! constructor of the spawner and every update draw from the shared stream; the patches
//! use `SeededRandom` (the shared state restored). Sprites: `rw-graphics`
//! `sky_dandelion.rs`.
//!
//! Mirrors: SkyDandelions.cs (SkyDandelions, SkyDandelion, DandelionStalk)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::creature::abstract_ai::SeededRandom;
use crate::game::camera::CameraView;
use crate::insect::InsectWater;
use crate::{FloatExt, RoomExt, Vec2};

/// A body chunk as the stalks read it (`bodyChunks[k]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkView {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub rad: f32,
}

/// `SkyDandelions.SkyDandelion`.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyDandelion {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub drift_dir: Vec2,
    pub last_health: f32,
    pub health: f32,
    pub depth: f32,
    pub stuck: bool,
}

impl SkyDandelion {
    /// The constructor: the drift angle, then (unless `play_layer_only`) the depth.
    /// 1 draw with `play_layer_only`, else 2 (depth 0) or 4.
    #[must_use]
    pub fn new(pos: Vec2, play_layer_only: bool, rng: &mut UnityRandom) -> Self {
        let drift_dir = Vec2::from_degrees(rng.value() * 360.0);
        let depth = if play_layer_only || rng.value() < 0.2 {
            0.0
        } else if rng.value() < 0.5 {
            -0.5 * rng.value()
        } else {
            rng.value().mathf_pow(1.5) * 5.0
        };
        Self {
            update_order: 0,
            slated: false,
            pos,
            last_pos: pos,
            vel: Vec2::ZERO,
            drift_dir,
            last_health: 1.0,
            health: 1.0,
            depth,
            stuck: false,
        }
    }

    /// `InPlayLayer`.
    #[must_use]
    pub fn in_play_layer(&self) -> bool {
        self.depth == 0.0
    }

    /// `Update` (one draw).
    pub fn update(&mut self, room: &Room, water: &InsectWater, rng: &mut UnityRandom) {
        self.vel *= 0.9;
        self.drift_dir =
            (self.drift_dir + Vec2::from_degrees(rng.value() * 360.0) * 0.2).normalized();
        if !self.stuck {
            self.vel += self.drift_dir * 0.1;
        }
        self.vel.y += 0.2;
        self.last_pos = self.pos;
        self.pos += self.vel / (self.depth + 1.0);
        self.last_health = self.health;
        if self.in_play_layer() {
            if room.is_solid_at(self.pos) {
                if !room.is_solid_at(self.last_pos) {
                    self.pos = self.last_pos;
                }
                self.health -= 1.0 / 30.0;
                if self.last_health < 0.0 {
                    self.slated = true;
                    return;
                }
            } else {
                self.health = 1.0_f32.min(self.health + 1.0 / 70.0);
            }
            if water.submerged(self.pos) {
                self.pos.y = water.level(self.pos);
            }
        }
        if self.pos.y > room.pixel_height() + 300.0 {
            self.slated = true;
        }
    }
}

/// `SkyDandelions.DandelionStalk`.
#[derive(Debug, Clone, PartialEq)]
pub struct DandelionStalk {
    pub update_order: u64,
    pub slated: bool,
    pub pos: Vec2,
    pub last_pos: Vec2,
    /// `dandelion`, by its place in the update list; `None` once blown loose.
    pub dandelion: Option<u64>,
    pub stalk_length: f32,
    pub segs: usize,
    pub stalk_dir: Vec2,
    pub tip: Vec2,
    pub last_tip: Vec2,
    pub straight_stalk: f32,
    pub last_straight_stalk: f32,
}

impl DandelionStalk {
    /// The constructor for `dandelion` in room `room_index`: planted in the wall
    /// below it, leaning by `SeededRandom((int)pos.x + (int)pos.y + room index)`.
    /// `dandelion` is set once the dandelion has its place in the update list.
    #[must_use]
    pub fn new(
        dandelion: &SkyDandelion,
        room: &Room,
        room_index: i32,
        rng: &mut UnityRandom,
    ) -> Self {
        let mut pos = Vec2::new(dandelion.pos.x, dandelion.pos.tile_middle().y - 11.0);
        if !room.is_solid_at(pos) {
            pos.y -= 20.0;
        }
        let stalk_length = pos.distance(dandelion.pos);
        let segs = ((stalk_length / 2.0).to_int_cs()).clamp(2, 15) as usize;
        let seed = pos
            .x
            .to_int_cs()
            .wrapping_add(pos.y.to_int_cs())
            .wrapping_add(room_index);
        let stalk_dir = Vec2::from_degrees((-45.0_f32).lerp(45.0, rng.seeded_value(seed)));
        Self {
            update_order: 0,
            slated: false,
            pos,
            last_pos: pos,
            dandelion: None,
            stalk_length,
            segs,
            stalk_dir,
            tip: Vec2::ZERO,
            last_tip: Vec2::ZERO,
            straight_stalk: 0.0,
            last_straight_stalk: 0.0,
        }
    }

    /// `Update`: attached, the dandelion is pulled to the stalk's top and every body
    /// chunk in reach near a dandelion on screen may blow it loose (two draws per chunk
    /// in contact); loose, the tip returns to the root (1 draw) and the stalk goes.
    /// `dandelion` is the attached one.
    pub fn update(
        &mut self,
        dandelion: Option<&mut SkyDandelion>,
        room: &Room,
        room_index: i32,
        camera: &CameraView,
        chunks: &[ChunkView],
        rng: &mut UnityRandom,
    ) {
        self.last_pos = self.pos;
        let Some(d) = dandelion.filter(|_| self.dandelion.is_some()) else {
            self.update_loose(rng);
            return;
        };
        let top = self.pos + Vec2::new(0.0, self.stalk_length);
        d.vel += (top - d.pos) * 0.05;
        if !d.pos.dist_less(self.pos, self.stalk_length) {
            let dir = d.pos.direction_to(self.pos);
            let dist = d.pos.distance(self.pos);
            d.pos += dir * (dist - self.stalk_length);
            d.vel += dir * (dist - self.stalk_length);
        }
        if !camera.viewed(room_index, room, d.pos, 100.0) {
            return;
        }
        for c in chunks {
            if !d.pos.dist_less(c.pos, c.rad + 3.0) {
                continue;
            }
            let motion = c.pos - c.last_pos;
            d.vel = d.vel.lerp(motion, rng.value());
            if rng.value() < motion.length().inverse_lerp(1.0, 8.0) {
                d.vel += c.pos.direction_to(d.pos) * (motion.length() * 0.5);
                d.vel += motion * 0.5;
                d.stuck = false;
                self.tip = d.pos;
                self.last_tip = d.pos;
                self.dandelion = None;
                break;
            }
        }
    }

    fn update_loose(&mut self, rng: &mut UnityRandom) {
        self.last_tip = self.tip;
        self.tip = self.tip.lerp(self.pos, 0.1);
        self.last_straight_stalk = self.straight_stalk;
        self.straight_stalk = 1.0_f32.min(self.straight_stalk + rng.value() / 15.0);
        if self.tip.dist_less(self.pos, 1.0) {
            self.slated = true;
        }
    }
}

/// What `SkyDandelions` makes in its constructor, in `AddObject` order.
#[derive(Debug, Clone, PartialEq)]
pub enum MadeDandelion {
    /// A free dandelion drifting in the room.
    Free(SkyDandelion),
    /// A patch's stalk, then its dandelion (`stuck`).
    Planted(DandelionStalk, SkyDandelion),
}

/// A placed `DandelionPatch`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DandelionPatch {
    pub pos: Vec2,
    pub rad: f32,
}

/// `SkyDandelions`, the spawner a `SkyDandelions` effect adds.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyDandelions {
    pub update_order: u64,
    pub amount: f32,
    pub wait: f32,
}

impl SkyDandelions {
    /// `SpawnChance`: `ProximityToMiddleOfCycle` of the rain cycle and `globalRain.Intensity`.
    #[must_use]
    pub fn spawn_chance(&self, proximity_to_middle: f32, rain_intensity: f32) -> f32 {
        self.amount
            * (1.0 - proximity_to_middle).mathf_pow(0.15)
            * (1.0 - rain_intensity.mathf_pow(0.1))
    }

    /// The constructor's dandelions: scattered over the room (4 draws each over open
    /// tiles, 2 over solid ones, with the depth draws above), then each patch's
    /// (seeded). `room_index` is `abstractRoom.index`.
    #[must_use]
    pub fn generate(
        &self,
        room: &Room,
        room_index: i32,
        patches: &[DandelionPatch],
        (proximity_to_middle, rain_intensity): (f32, f32),
        rng: &mut UnityRandom,
    ) -> Vec<MadeDandelion> {
        let mut made = Vec::new();
        let chance = self.spawn_chance(proximity_to_middle, rain_intensity);
        let (w, h) = (room.width(), room.height());
        let mut i = 0;
        while (i as f32) < ((w * h * 20) as f32) * chance / 1000.0 {
            let tile = (rng.range_i32(0, w), rng.range_i32(0, h));
            if !room.is_solid_tile(tile.0, tile.1) {
                let jitter = Vec2::new(-10.0 + 20.0 * rng.value(), -10.0 + 20.0 * rng.value());
                made.push(MadeDandelion::Free(SkyDandelion::new(
                    Vec2::middle_of_tile(tile) + jitter,
                    false,
                    rng,
                )));
            }
            i += 1;
        }
        let mut num = room_index;
        for patch in patches {
            self.plant_patch(room, room_index, patch, &mut num, &mut made, rng);
        }
        made
    }

    /// One patch: the grass-like tiles (open, solid below, open above, not walled in on
    /// both sides) within `Rad` get `3 * density` tries, each a seeded chance of a
    /// planted dandelion.
    fn plant_patch(
        &self,
        room: &Room,
        room_index: i32,
        patch: &DandelionPatch,
        num: &mut i32,
        made: &mut Vec<MadeDandelion>,
        rng: &mut UnityRandom,
    ) {
        let centre = patch.pos.tile();
        let reach = (patch.rad / 20.0).to_int_cs();
        for k in centre.0 - reach..=centre.0 + reach {
            for l in centre.1 - reach..=centre.1 + reach {
                if k <= 0 || k >= room.width() || l <= 0 || l >= room.height() {
                    continue;
                }
                if room.is_solid_tile(k, l)
                    || room.is_solid_tile(k, l + 1)
                    || !room.is_solid_tile(k, l - 1)
                    || (room.is_solid_tile(k - 1, l) && room.is_solid_tile(k + 1, l))
                {
                    continue;
                }
                let middle = Vec2::middle_of_tile((k, l));
                let density = patch
                    .pos
                    .distance(middle)
                    .inverse_lerp(patch.rad, patch.rad / 3.0)
                    * patch.rad.lerp_map(40.0, 200.0, 0.5, 1.0);
                let mut m = 0;
                while (m as f32) < 3.0 * density {
                    if rng.seeded_value(*num) < density.mathf_pow(0.5) {
                        let height = 0.1
                            + 0.9
                                * rng
                                    .seeded_value(*num + 1000)
                                    .mathf_pow(0.5_f32.lerp(2.5, density));
                        let at = Vec2::new(
                            -10.0 + rng.seeded_value(*num - 1000) * 20.0,
                            -10.0 + height * 30.0,
                        );
                        let mut dandelion = SkyDandelion::new(middle + at, true, rng);
                        dandelion.stuck = true;
                        let stalk = DandelionStalk::new(&dandelion, room, room_index, rng);
                        made.push(MadeDandelion::Planted(stalk, dandelion));
                    }
                    *num += 1;
                    m += 1;
                }
            }
        }
    }

    /// `Update`: the wait counts down; when it runs out it grows by `Lerp(30, 0.4,
    /// amount)` and `Random.value < SpawnChance` (1 draw) adds a dandelion below the
    /// room at a random x (1 draw, then the constructor's 2 or 4).
    pub fn update(
        &mut self,
        room: &Room,
        (proximity_to_middle, rain_intensity): (f32, f32),
        rng: &mut UnityRandom,
    ) -> Option<SkyDandelion> {
        if self.wait > 0.0 {
            self.wait -= 1.0;
            return None;
        }
        self.wait += 30.0_f32.lerp(0.4, self.amount);
        if rng.value() < self.spawn_chance(proximity_to_middle, rain_intensity) {
            let x = (-150.0_f32).lerp(room.pixel_width() + 150.0, rng.value());
            return Some(SkyDandelion::new(Vec2::new(x, -100.0), false, rng));
        }
        None
    }
}
