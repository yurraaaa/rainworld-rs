//! `SuperStructureFuses`: a grid of blinking lights (a `fuseSize` px square each, 10 by
//! default) over a placed tile rectangle. Each cell carries five numbers (`lights`: the
//! brightness now and two ticks ago, a mode, and the mode's timer); the cells fade and
//! wobble, jump into fixed modes by chance, and malfunction over a random sub-rectangle
//! in spells. Offscreen (no camera sees a corner within 40 px) the grid does not update
//! at all. In a `BrokenZeroG` room the grid fades with gravity. Hundreds of draws a tick
//! on the shared stream (D36). Sprites: `rw-graphics`.
//!
//! Mirrors: SuperStructureFuses.cs

use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// The cells of a grid `lights[j, k, 0..5]`, brightness, last brightness, the one
/// before, mode, mode timer.
type Cell = [f32; 5];

/// What the constructor reads of the room and world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FuseSetup {
    /// `placedObject.pos`.
    pub pos: Vec2,
    /// `IntRect(left, bottom, right, top)` in tiles.
    pub rect: [i32; 4],
    /// `regionParams.fuseSize` (10 without a region).
    pub size: f32,
    /// The tile depth: 0 for rectangles with only solid tiles, 1 with a free tile in
    /// front of a wall, 2 with one in the open.
    pub depth: i32,
    /// `broken`.
    pub broken: f32,
    /// `BrokenZeroG` amount above 0.
    pub gravity_dependent: bool,
}

/// One `SuperStructureFuses`.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperStructureFuses {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// `placedObject.pos`, what `pos` follows.
    pub placed_pos: Vec2,
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub rect: [i32; 4],
    pub broken: f32,
    pub size: f32,
    /// Cells across and up.
    pub width: usize,
    pub height: usize,
    lights: Vec<Cell>,
    pub depth: i32,
    pub debug_mode: i32,
    pub lightness: f32,
    pub last_lightness: f32,
    pub mal_function: i32,
    pub smoothed_malfunction: f32,
    pub malfunction_rect: [i32; 4],
    pub power: f32,
    pub power_flicker: f32,
    pub gravity_dependent: bool,
    pub culled: bool,
}

impl SuperStructureFuses {
    /// The constructor (no draws): `rect.Width * (int)(20 / size)` cells across (the
    /// rectangle's width is `|right - left|`, so a one-tile-wide rectangle has none).
    #[must_use]
    pub fn new(update_order: u64, s: FuseSetup) -> Self {
        let per_tile = (20.0 / s.size).to_int_cs().max(0);
        let [left, bottom, right, top] = s.rect;
        let width = ((right - left).abs() * per_tile) as usize;
        let height = ((top - bottom).abs() * per_tile) as usize;
        Self {
            update_order,
            placed_pos: s.pos,
            pos: s.pos,
            last_pos: Vec2::ZERO,
            rect: s.rect,
            broken: s.broken,
            size: s.size,
            width,
            height,
            lights: vec![[0.0; 5]; width * height],
            depth: s.depth,
            debug_mode: 0,
            lightness: 0.0,
            last_lightness: 0.0,
            mal_function: 0,
            smoothed_malfunction: 0.0,
            malfunction_rect: [0; 4],
            power: 1.0,
            power_flicker: 1.0,
            gravity_dependent: s.gravity_dependent,
            culled: false,
        }
    }

    /// The constructor's `broken`: the `CorruptionSpores` amount; outside an arena session, in
    /// a region other than Five Pebbles' (`SS`) and the Underhang (`UW`), 1.
    #[must_use]
    pub fn broken_for(spores: f32, arena: bool, region: Option<&str>) -> f32 {
        if !arena && region.is_some_and(|n| n != "SS" && n != "UW") {
            1.0
        } else {
            spores
        }
    }

    /// The cell `lights[j, k]`.
    #[must_use]
    pub fn cell(&self, j: usize, k: usize) -> Cell {
        self.lights[j * self.height + k]
    }

    fn cell_mut(&mut self, j: usize, k: usize) -> &mut Cell {
        &mut self.lights[j * self.height + k]
    }

    /// `GetFloatRect.GetCorner(i)`: the rectangle's pixels, corners in the game's order
    /// (top left, top right, bottom right, bottom left).
    #[must_use]
    pub fn corner(&self, i: usize) -> Vec2 {
        let [left, bottom, right, top] = self.rect;
        let (l, b) = (left as f32 * 20.0, bottom as f32 * 20.0);
        let (r, t) = (right as f32 * 20.0 + 20.0, top as f32 * 20.0 + 20.0);
        match i {
            0 => Vec2::new(l, t),
            1 => Vec2::new(r, t),
            2 => Vec2::new(r, b),
            3 => Vec2::new(l, b),
            _ => Vec2::ZERO,
        }
    }

    /// `Update`, given whether any camera sees a corner (`culled` is its negation) and
    /// `room.gravity`.
    pub fn update(&mut self, viewed: bool, gravity: f32, rng: &mut UnityRandom) {
        self.culled = !viewed;
        if self.culled {
            return;
        }
        if self.gravity_dependent {
            let target = if gravity > 0.5 { 0.0 } else { 1.0 };
            self.power = self.power.lerp(target, 0.2 * rng.value());
            if rng.value() < 0.1 {
                self.power_flicker = rng
                    .value()
                    .lerp(self.power, (self.power - 0.5).abs() * 2.0);
            }
        }
        self.last_lightness = self.lightness;
        self.lightness =
            (self.lightness + (-1.0f32).lerp(1.0, rng.value()) / 40.0).clamp(0.0, 1.0);
        self.lightness = self
            .lightness
            .min(self.smoothed_malfunction)
            .min(1.0 - self.power);
        for j in 0..self.width {
            for k in 0..self.height {
                self.update_cell(j, k, rng);
            }
        }
        if self.mal_function > 0 {
            self.mal_function -= 1;
        } else if rng.value().mathf_pow(self.broken) < 1.0 / 600.0f32.lerp(120.0, self.broken) {
            self.mal_function = rng.range_i32(20, 80);
            let (w, h) = (self.width as i32, self.height as i32);
            let mut r = [
                rng.range_i32(-10, w + 10),
                rng.range_i32(-10, h + 10),
                rng.range_i32(-10, w + 10),
                rng.range_i32(-10, h + 10),
            ];
            if r[0] > r[2] {
                r.swap(0, 2);
            }
            if r[1] > r[3] {
                r.swap(1, 3);
            }
            self.malfunction_rect = r;
        }
        let target = if self.mal_function > 0 { 1.0 } else { 0.0 };
        self.smoothed_malfunction = self.smoothed_malfunction.lerp(target, 0.02);
        self.last_pos = self.pos;
        self.pos = self.placed_pos;
        if self.last_pos != self.pos {
            self.debug_mode = 80;
        }
        if self.debug_mode > 0 {
            self.debug_mode -= 1;
        }
    }

    /// One cell's turn of the update loop.
    fn update_cell(&mut self, j: usize, k: usize, rng: &mut UnityRandom) {
        let smoothed = self.smoothed_malfunction;
        if rng.value().mathf_pow(0.5) <= smoothed {
            return;
        }
        let c = self.cell_mut(j, k);
        c[2] = c[1];
        c[1] = c[0];
        if c[3] > 0.0 {
            c[4] -= 1.0;
            if c[4] <= 0.0 {
                c[3] = 0.0;
            }
        }
        if rng.value().mathf_pow(0.25) < smoothed {
            c[0] = rng.value();
            c[3] = rng.range_i32(1, 6) as f32;
            c[4] = rng.range_i32(2, 10) as f32;
        }
        match c[3] as i32 {
            1 | 5 => {
                c[0] = 1.0;
                return;
            }
            2 => {
                c[0] = 0.0;
                return;
            }
            3 => {
                c[0] = rng.value();
                return;
            }
            4 => {
                if (c[4] as i32) % 12 > 5 {
                    c[0] = if c[0] == 0.0 { 1.0 } else { 0.0 };
                }
                return;
            }
            _ => {}
        }
        c[0] = (c[0] + (-1.0f32).lerp(1.0, rng.value()) / 10.0).clamp(0.0, 1.0);
        let (w, h) = (self.width, self.height);
        let mut sum = self.cell(j, k)[2];
        let mut count = 1;
        if j > 0 {
            sum += self.cell(j - 1, k)[2];
            count += 1;
        }
        if j < w - 1 {
            sum += self.cell(j + 1, k)[2];
            count += 1;
        }
        if k > 0 {
            sum += self.cell(j, k - 1)[2];
            count += 1;
        }
        if k < h - 1 {
            sum += self.cell(j, k + 1)[2];
            count += 1;
        }
        let broken = self.broken;
        let c = self.cell_mut(j, k);
        c[0] = c[0].lerp(sum / count as f32, 0.5);
        if rng.value() < 0.0025 {
            let low = if rng.value() < 1.0 / 3.0 { 1 } else { 2 };
            c[3] = rng.range_i32(low, 5) as f32;
            c[4] = 2.0f32.lerp(80.0, rng.value());
            if rng.value() < 1.0 / 8000.0f32.lerp(50.0, broken) {
                c[3] = 5.0;
                c[4] = 60.0f32.lerp(120.0, rng.value());
            }
        }
    }

    /// `Custom.InsideRect(new IntVector2(j, k), malfunctionRect)`.
    #[must_use]
    pub fn in_malfunction_rect(&self, j: usize, k: usize) -> bool {
        let [l, b, r, t] = self.malfunction_rect;
        let (j, k) = (j as i32, k as i32);
        j >= l && j <= r && k >= b && k <= t
    }
}

#[cfg(test)]
#[path = "ss_fuses_tests.rs"]
mod tests;
