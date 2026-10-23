//! `WaterFall`: the sheet of falling water `Room.Loaded`'s tile parse makes from a room's
//! waterfall tiles (code 8 of the geometry file). It falls from its top tile to the
//! water, throws water-coloured drips off the terrain it passes and, in a room with
//! water, hits the surface, rolls bubbles up from it and pushes the bodies under it.
//! Made before the room has its water, so its first `strikeLevel` is -200 (the game's own
//! order, D30). Base game with Remix off and no DLC: the inverted-water and `PointSubmerged`
//! branches are left out (D30). The sprites are `rw-graphics`' (`water_fall.rs`).
//!
//! Mirrors: WaterFall.cs, Room.cs (LoadFromDataString: the waterfall tiles)

use rw_assets::{Room, TileFlags};
use rw_random::UnityRandom;

use crate::{BodyChunk, Bubble, FloatExt, Vec2, Water, WaterDrip};

/// What a waterfall adds to its room's update list, in `AddObject` order.
#[derive(Debug, Clone, PartialEq)]
pub enum FallObject {
    Drip(WaterDrip),
    Bubble(Bubble),
}

/// One bubble of the falling water: `bubbles[k, 0..3]` (position, last position,
/// velocity, and `x` size/life, `y` life time).
pub type FallBubble = [Vec2; 4];

/// One `WaterFall`.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterFall {
    /// Its place in the room's update list.
    pub update_order: u64,
    pub tile_pos: (i32, i32),
    pub pos: Vec2,
    pub width: i32,
    pub last_flow: f32,
    pub flow: f32,
    pub set_flow: f32,
    pub original_flow: f32,
    pub visual_density: f32,
    pub last_visual_dens: f32,
    pub top_pos: [f32; 3],
    pub bottom_pos: [f32; 3],
    pub bubbles_per_tile: i32,
    pub bubbles: Vec<FallBubble>,
    pub hit_terrain_tiles: Vec<(i32, i32)>,
}

/// A waterfall `Room.Loaded`'s tile parse makes: its top-left tile, flow and width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FallSpec {
    pub tile: (i32, i32),
    pub flow: f32,
    pub width: i32,
}

impl WaterFall {
    /// `strikeLevel` of a room without a `Water` object.
    pub const NO_WATER_STRIKE_LEVEL: f32 = -200.0;

    /// The waterfalls the tile parse makes: the tiles with code 8 are listed column by
    /// column from the left, each from the top down; then, until none is left, a tile with
    /// no listed tile to its left or above (else the last one) starts a fall as wide as
    /// the run of listed tiles to its right whose tile below is listed alike (a double-tile
    /// fall, flow 1, takes the tile below too; else flow 0.5).
    #[must_use]
    pub fn specs(room: &Room) -> Vec<FallSpec> {
        let mut list: Vec<(i32, i32)> = Vec::new();
        for x in 0..room.width() {
            for y in (0..room.height()).rev() {
                if room.tile(x, y).flags.contains(TileFlags::WATERFALL) {
                    list.push((x, y));
                }
            }
        }
        let mut specs = Vec::new();
        while !list.is_empty() {
            let mut start = list[0];
            for &t in &list {
                start = t;
                if !list.contains(&(t.0 - 1, t.1)) && !list.contains(&(t.0, t.1 + 1)) {
                    break;
                }
            }
            let double = list.contains(&(start.0, start.1 - 1));
            let mut width = 0;
            let mut x = start.0;
            while x < room.width()
                && list.contains(&(x, start.1))
                && list.contains(&(x, start.1 - 1)) == double
            {
                width += 1;
                list.retain(|&t| t != (x, start.1));
                if double {
                    list.retain(|&t| t != (x, start.1 - 1));
                }
                x += 1;
            }
            specs.push(FallSpec {
                tile: start,
                flow: if double { 1.0 } else { 0.5 },
                width,
            });
        }
        specs
    }

    /// The constructor. The room has no `Water` object yet, whether or not it will have
    /// one (`has_water` is `Room.water`, which decides the bubbles).
    #[must_use]
    pub fn new(update_order: u64, room: &Room, spec: FallSpec, has_water: bool) -> Self {
        let FallSpec { tile, flow, width } = spec;
        let pos = Vec2::middle_of_tile(tile) + Vec2::new(-10.0, 15.0);
        let bubbles_per_tile = if has_water {
            5.0_f32.lerp(10.0, flow) as i32
        } else {
            0
        };
        let bubbles = vec![[Vec2::ZERO; 4]; (bubbles_per_tile * width) as usize];
        let strike = Self::NO_WATER_STRIKE_LEVEL;
        let mut top_pos = [pos.y, pos.y, 0.0];
        let bottom_pos = [strike, strike, 0.0];
        if flow == 0.0 {
            top_pos[0] = bottom_pos[0];
            top_pos[1] = top_pos[0];
        }
        let mut hit_terrain_tiles = Vec::new();
        for i in tile.0..=tile.0 + width {
            let mut y = tile.1;
            while y as f32 > strike / 20.0 {
                if room.tile(i, y).is_solid() && !room.tile(i, y + 1).is_solid() {
                    hit_terrain_tiles.push((i, y));
                }
                y -= 1;
            }
        }
        Self {
            update_order,
            tile_pos: tile,
            pos,
            width,
            last_flow: flow,
            flow,
            set_flow: flow,
            original_flow: flow,
            visual_density: 0.0,
            last_visual_dens: 0.0,
            top_pos,
            bottom_pos,
            bubbles_per_tile,
            bubbles,
            hit_terrain_tiles,
        }
    }

    fn float_left(&self) -> f32 {
        self.tile_pos.0 as f32 * 20.0
    }

    fn float_right(&self) -> f32 {
        (self.tile_pos.0 + self.width) as f32 * 20.0
    }

    /// `startLevel`.
    #[must_use]
    pub fn start_level(&self) -> f32 {
        self.pos.y
    }

    /// `strikeLevel`: the still water level less 10 px (Remix off), under the fall's
    /// middle; -200 without a water object.
    #[must_use]
    pub fn strike_level(water: Option<&Water>) -> f32 {
        water.map_or(Self::NO_WATER_STRIKE_LEVEL, |w| w.still_level() - 10.0)
    }

    /// `floatLength`.
    #[must_use]
    pub fn float_length(&self, water: Option<&Water>) -> f32 {
        (self.start_level() - Self::strike_level(water)).abs()
    }

    fn falling_water_bottom(&self, water: Option<&Water>) -> f32 {
        self.start_level().min(Self::strike_level(water))
    }

    /// `Flooded`: the water is above where the fall starts.
    #[must_use]
    pub fn flooded(&self, water: Option<&Water>) -> bool {
        Self::strike_level(water) > self.start_level()
    }

    /// `WaterContact`.
    #[must_use]
    pub fn water_contact(&self, water: Option<&Water>) -> bool {
        self.flow > 0.0
            && !self.flooded(water)
            && self.bottom_pos[0] < Self::strike_level(water) + 10.0
    }

    /// `Update` up to the bodies' push: the fall's ends, the drips off the terrain, and,
    /// with `water` (`ConnectToWaterObject`), the surface hit, the bubbles and their
    /// upkeep. Returns the objects it added to the room, in `AddObject` order; draws as the
    /// game's.
    pub fn update(&mut self, water: Option<&mut Water>, rng: &mut UnityRandom) -> Vec<FallObject> {
        let mut made = Vec::new();
        self.last_flow = self.flow;
        self.last_visual_dens = self.visual_density;
        let bottom = self.falling_water_bottom(water.as_deref());
        if self.top_pos[0] == self.pos.y {
            self.visual_density = self.visual_density.lerp(self.flow, 0.1);
        }
        if self.top_pos[0] == self.pos.y || self.top_pos[0] <= bottom {
            self.flow = self.set_flow;
        }
        self.bottom_pos[1] = self.bottom_pos[0];
        self.bottom_pos[0] += self.bottom_pos[2];
        self.bottom_pos[2] += -0.9;
        if self.bottom_pos[0] < bottom {
            self.bottom_pos[0] = bottom;
            self.bottom_pos[2] = 0.0;
        }
        if self.flow == 0.0 {
            self.top_pos[1] = self.top_pos[0];
            self.top_pos[0] += self.top_pos[2];
            self.top_pos[2] += -0.9;
            if self.top_pos[0] < bottom {
                self.top_pos[0] = bottom;
                self.top_pos[2] = 0.0;
                self.visual_density = 0.0;
            }
        } else {
            self.top_pos = [self.pos.y, self.pos.y, 0.0];
            if self.last_flow == 0.0 {
                self.bottom_pos = [self.pos.y, self.pos.y, 0.0];
            }
        }
        if !self.flooded(water.as_deref()) {
            for i in 0..self.hit_terrain_tiles.len() {
                if rng.value() < self.flow && rng.value() < 1.0 / 3.0 {
                    let x = (-10.0_f32).lerp(10.0, rng.value());
                    let y = (-10.0_f32).lerp(10.0, rng.value());
                    let at = Vec2::middle_of_tile(self.hit_terrain_tiles[i]) + Vec2::new(x, y);
                    if at.y < self.top_pos[0] && at.y > self.bottom_pos[0] {
                        let dir = Vec2::from_degrees(360.0 * rng.value()) + Vec2::new(0.0, 1.0);
                        let speed = rng.value() * 7.0 * rng.value() * self.flow;
                        let mut drip = WaterDrip::new(at, dir * speed, true, rng);
                        drip.must_exit = true;
                        made.push(FallObject::Drip(drip));
                    }
                }
            }
        }
        if let Some(w) = water {
            self.update_water(w, rng, &mut made);
        }
        made
    }

    /// `Update`'s `water != null` block.
    fn update_water(
        &mut self,
        water: &mut Water,
        rng: &mut UnityRandom,
        made: &mut Vec<FallObject>,
    ) {
        let width = self.width as f32;
        if self.water_contact(Some(water)) {
            water.waterfall_hit_surface(
                self.pos + Vec2::new(width * 10.0, 0.0),
                width * 20.0,
                self.flow,
                rng,
            );
        }
        let strike = Self::strike_level(Some(water));
        for j in self.tile_pos.0..self.tile_pos.0 + self.width {
            if self.water_contact(Some(water)) && rng.value() > self.flow / 2.0 {
                let column = Vec2::middle_of_tile((j, 0));
                if rng.value() < 0.5 {
                    let x = (-10.0_f32).lerp(10.0, rng.value());
                    let at = column + Vec2::new(x, strike - 20.0);
                    let dir = Vec2::from_degrees(160.0 + rng.value() * 40.0);
                    let vel = dir * (rng.value() * 20.0);
                    made.push(FallObject::Bubble(Bubble::new(at, vel, false, false, rng)));
                }
                if rng.value() < 0.2 {
                    let x = (-10.0_f32).lerp(10.0, rng.value());
                    let mut at = column + Vec2::new(x, strike);
                    at.y = water.level_at(at.x) + 2.0;
                    let centre = self.tile_pos.0 as f32 * 20.0 + width * 20.0 / 2.0;
                    let dir = Vec2::from_degrees(-45.0 + rng.value() * 90.0);
                    let vel =
                        Vec2::new(at.x - centre, 0.0) * 0.4 / width + dir * (rng.value() * 10.0);
                    made.push(FallObject::Drip(WaterDrip::new(at, vel, true, rng)));
                }
            }
            let per = self.bubbles_per_tile;
            for k in (j - self.tile_pos.0) * per..(j - self.tile_pos.0 + 1) * per {
                let b = &mut self.bubbles[k as usize];
                b[1] = b[0];
                b[0] += b[2];
                b[2] *= 0.9;
                b[2].y += 0.2;
                b[3].x -= 1.0 / b[3].y;
                let above = b[0].y > water.level_at(b[0].x);
                if b[3].x <= 0.0 || above {
                    self.reset_bubble(k as usize, j, strike, water, rng);
                }
            }
        }
    }

    /// `ResetBubble`: a bubble born below the fall's strike, rising and drifting.
    fn reset_bubble(
        &mut self,
        k: usize,
        tile_x: i32,
        strike: f32,
        water: &Water,
        rng: &mut UnityRandom,
    ) {
        let x = (-10.0_f32).lerp(10.0, rng.value());
        let at = Vec2::middle_of_tile((tile_x, 0)) + Vec2::new(x, strike - 25.0);
        let up = Vec2::from_degrees(160.0 + rng.value() * 40.0);
        let up = up * (rng.value() * 8.0_f32.lerp(12.0, self.flow));
        let drift = Vec2::from_degrees(rng.value() * 360.0);
        let vel = up + drift * (rng.value() * 2.0);
        let life_time = rng.range_i32(5, 20);
        let b = &mut self.bubbles[k];
        b[0] = at;
        b[2] = vel;
        b[3].y = life_time as f32;
        b[1] = b[0];
        b[3].x = 1.0;
        if !self.water_contact(Some(water)) {
            let b = &mut self.bubbles[k];
            b[0] = Vec2::new(-1000.0, 1000.0);
            b[1] = Vec2::new(-1000.0, 1000.0);
        }
    }

    /// `Update`'s last block: the bodies in the water under the fall are pushed down in
    /// its width and toward its sides by the edges (`chunks` of the room's bodies except a
    /// player holding up; `water` is the room's).
    pub fn push_chunks<'a>(
        &self,
        water: &Water,
        chunks: impl IntoIterator<Item = &'a mut BodyChunk>,
    ) {
        let (left, right) = (self.float_left(), self.float_right());
        for c in chunks {
            let level = water.level_at(c.pos.x);
            if !(c.pos.y > level - 120.0
                && c.pos.y < level + 40.0
                && c.submersion(Some(water)) > 0.0)
            {
                continue;
            }
            let num = c
                .pos
                .y
                .inverse_lerp(level - 120.0, level - 40.0)
                .mathf_pow(2.0)
                * c.submersion(Some(water)).mathf_pow(0.1)
                * self.flow;
            if c.pos.x > left && c.pos.x < right {
                c.vel.y -= num * 0.3 / c.mass;
            }
            for n in 0..2 {
                let side = if n == 0 { -1.0 } else { 1.0 };
                let mut edge = if n == 0 {
                    if c.pos.x < left {
                        c.pos.x.inverse_lerp(left - 40.0, left)
                    } else {
                        c.pos.x.inverse_lerp(left + 20.0, left)
                    }
                } else if c.pos.x > right {
                    c.pos.x.inverse_lerp(right + 40.0, right)
                } else {
                    c.pos.x.inverse_lerp(right - 20.0, right)
                };
                edge *= (c.pos.y.inverse_lerp(level - 60.0, level - 10.0) - 0.5) * 2.0;
                c.vel.x -= num * 0.17 * side * edge / c.mass;
            }
        }
    }
}

#[cfg(test)]
#[path = "waterfall_tests.rs"]
mod tests;
