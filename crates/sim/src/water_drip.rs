//! `WaterDrip` (42265): a falling droplet, thrown off a wet ceiling by a creature's hard
//! terrain impact, shaken loose by the slugcat's bites, or dripping from a rain room's
//! ceiling on its own (`Room.Update`). A `CosmeticSprite` in its room's update list
//! whose `Update` draws on the shared stream; the sprites are `rw-graphics`'.
//!
//! Mirrors: WaterDrip.cs, Room.cs (Update)

use rw_assets::{DangerType, Room, Terrain};
use rw_random::UnityRandom;

use crate::world::RealizedRoom;
use crate::{FloatExt, TerrainImpact, Vec2, Water};

/// A realized room's `waterGlitterCycle` and its last value (`Room.Update` lowers it by
/// 1/60 a tick), read by `WaterShinyness`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterGlitter {
    pub last: f32,
    pub cycle: f32,
}

impl WaterGlitter {
    pub fn update(&mut self) {
        self.last = self.cycle;
        self.cycle -= 1.0 / 60.0;
    }

    /// `Room.WaterShinyness(pos, timeStacker)`.
    #[must_use]
    pub fn shinyness(self, pos: Vec2, time_stacker: f32) -> f32 {
        let num = pos.x - pos.y;
        let phase = -self.last.lerp(self.cycle, time_stacker) + num / 800.0;
        let sin = f64::from(phase * std::f32::consts::PI * 2.0).sin() as f32;
        (0.5 + sin * 0.5).mathf_pow(7.0)
    }
}

impl RealizedRoom {
    /// `Room.Update`'s ambient ceiling drip, after the water's update: in a viewed,
    /// non-shelter room with a rain danger type and ceiling tiles, `Random.value <
    /// gravity` (always true at gravity 1, still a draw), then a second draw against the
    /// chance that none of the ceiling tiles drips, then a drip at a random ceiling
    /// tile (tile, x jitter, `WaterDrip`'s life). Without a drip it makes 0 to 2 draws.
    pub fn ambient_ceiling_drip(
        &self,
        viewed: bool,
        shelter: bool,
        cycle_start_up: f32,
        rng: &mut UnityRandom,
    ) -> Option<WaterDrip> {
        if !viewed
            || shelter
            || self.settings.danger_type == Some(DangerType::None)
            || self.ceiling_tiles.is_empty()
        {
            return None;
        }
        if rng.value() >= self.gravity {
            return None;
        }
        let drips = self.settings.ceiling_drips;
        let chance = ((1.0 - cycle_start_up) * drips.inverse_lerp(0.0, 0.5))
            .max(drips.mathf_pow(7.0))
            * 0.05;
        let tiles = self.ceiling_tiles.len();
        let none = f64::from(1.0 - chance).powf(tiles as f64);
        if f64::from(rng.value()) <= none {
            return None;
        }
        let len = i32::try_from(tiles).unwrap_or(i32::MAX);
        let tile = self.ceiling_tiles[rng.range_i32(0, len) as usize];
        let pos = Vec2::middle_of_tile(tile) + Vec2::new((-10.0).lerp(10.0, rng.value()), 9.0);
        Some(WaterDrip::new(pos, Vec2::ZERO, false, rng))
    }
}

/// One `WaterDrip`.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterDrip {
    /// The port's id for it (traces, graphics).
    pub id: u64,
    pub room: i32,
    /// Its place in the room's update list.
    pub update_order: u64,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub last_last_pos: Vec2,
    pub last_last_last_pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub last_life: f32,
    pub life_time: i32,
    pub random_lightness: f32,
    pub last_random_lightness: f32,
    /// `mustExitTerrainOnceToBeDestroyedByTerrain` (never set by the ported callers).
    pub must_exit: bool,
    pub water_color: bool,
    pub slated: bool,
}

impl WaterDrip {
    /// The constructor: at rest history at `pos`, a life of `Random.Range(10, 120)` ticks.
    pub fn new(pos: Vec2, vel: Vec2, water_color: bool, rng: &mut UnityRandom) -> Self {
        Self {
            id: 0,
            room: -1,
            update_order: 0,
            pos,
            last_pos: pos,
            last_last_pos: pos,
            last_last_last_pos: pos,
            vel,
            life: 1.0,
            last_life: 1.0,
            life_time: rng.range_i32(10, 120),
            random_lightness: 0.0,
            last_random_lightness: 0.0,
            must_exit: false,
            water_color,
            slated: false,
        }
    }

    /// The drips of `Creature.TerrainImpact` (77767-77795) for a first contact of a chunk
    /// at `pos` (radius `rad`, mass `mass`) in a room with `ceiling_drips`: up to five
    /// when `speed` weighted by the mass exceeds 10, thrown back off the surface.
    pub fn from_impact(
        impact: &TerrainImpact,
        pos: Vec2,
        rad: f32,
        mass: f32,
        ceiling_drips: f32,
        rng: &mut UnityRandom,
    ) -> Vec<Self> {
        let speed = impact.speed;
        let num = speed * mass.lerp(1.0, 0.75);
        if num <= 10.0 {
            return Vec::new();
        }
        let count = 5.min(((1.0 + num.lerp(1.0, 0.8) * 0.4) * ceiling_drips) as i32);
        let dir = Vec2::new(f32::from(impact.direction.x), f32::from(impact.direction.y));
        let mut drips = Vec::new();
        for _ in 0..count {
            let a = Vec2::from_degrees(rng.value() * 360.0);
            let mut vel = a * rng.value() * speed.lerp(5.0, 0.5).clamp(3.0, 15.0);
            if impact.direction.x != 0 {
                vel.x = vel.x.abs() * -dir.x;
            }
            if impact.direction.y != 0 {
                vel.y = vel.y.abs() * -dir.y;
            }
            drips.push(Self::new(pos + dir * rad * 0.9, vel, false, rng));
        }
        drips
    }

    /// `WaterDrip.Update` and `CosmeticSprite.Update` (`gravity` is the room's): falls, fades, and
    /// is slated once its life is out, below the water or inside a Solid tile.
    pub fn update(
        &mut self,
        room: &Room,
        water: Option<&Water>,
        glitter: WaterGlitter,
        gravity: f32,
        rng: &mut UnityRandom,
    ) {
        self.last_last_last_pos = self.last_last_pos;
        self.last_last_pos = self.last_pos;
        self.vel.y -= 0.9 * gravity;
        self.last_life = self.life;
        self.life -= 1.0 / self.life_time as f32;
        let float_water_level = water.map_or(f32::MIN, |w| w.level_at(self.pos.x));
        let tile = room.tile_at(self.pos.to_array());
        if self.last_life <= 0.0
            || self.pos.y < float_water_level
            || (tile.terrain == Terrain::Solid && !self.must_exit)
        {
            self.slated = true;
        }
        if self.must_exit && !tile.is_solid() {
            self.must_exit = false;
        }
        self.last_random_lightness = self.random_lightness;
        let shiny = glitter.shinyness(self.pos, 1.0);
        self.random_lightness = rng.value().lerp(1.0, shiny * 0.5);
        self.last_pos = self.pos;
        self.pos += self.vel;
    }
}
