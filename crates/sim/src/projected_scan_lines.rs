//! `ProjectedScanLines` (the `ProjectedScanLines` room effect): the scan lines of
//! [`crate::projector`] alone, slower (speed 0.5 to 3.5), without the grid. Each line is an
//! object of the room's update list, kept here in creation order (found by binary
//! search); the game's side is `game_projections.rs`, the sprites `rw-graphics`'
//! `projected_scan_lines_draw.rs`.
//!
//! Mirrors: ProjectedScanLines.cs

use rw_random::UnityRandom;

use crate::FloatExt;
use crate::Vec2;
use crate::projector::UpdateSeq;

/// `ProjectedScanLines.ScanLine`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedScanLine {
    pub update_order: u64,
    pub slated: bool,
    pub alignment: bool,
    pub pos: f32,
    pub last_pos: f32,
    pub speed: f32,
}

impl ProjectedScanLine {
    /// One draw (the speed).
    fn new(update_order: u64, alignment: bool, pos: f32, dir: f32, rng: &mut UnityRandom) -> Self {
        let speed = 0.5f32.lerp(3.5, rng.value()) * dir;
        Self {
            update_order,
            slated: false,
            alignment,
            pos: pos + speed.unity_sign(),
            last_pos: pos - speed.unity_sign(),
            speed,
        }
    }

    /// `Update` in a room `room_px` big.
    pub fn update(&mut self, room_px: Vec2) {
        self.last_pos = self.pos;
        self.pos += self.speed;
        let extent = if self.alignment { room_px.x } else { room_px.y };
        if (self.pos < -200.0 && self.last_pos < -200.0 && self.speed < 0.0)
            || (self.pos > extent + 200.0 && self.last_pos > extent + 200.0 && self.speed > 0.0)
        {
            self.slated = true;
        }
    }
}

/// `ProjectedScanLines`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedScanLines {
    pub update_order: u64,
    pub slated: bool,
    pub amount: f32,
    pub tile_size: (i32, i32),
    /// `room.PixelWidth`, `PixelHeight`.
    pub room_px: Vec2,
    pub visible: bool,
    /// Every line in the room, in creation order.
    pub lines: Vec<ProjectedScanLine>,
    /// `scanLines`: the lines the projector counts and destroys.
    pub scan_lines: Vec<u64>,
}

impl ProjectedScanLines {
    /// `idealScanLineNumber`.
    #[must_use]
    pub fn ideal_scan_line_number(&self) -> usize {
        let (w, h) = self.tile_size;
        let both = ((w * h) as f32).lerp(w.max(h) as f32, 0.5);
        (both / 2000.0 * 10.0 * self.amount).to_int_cs().max(0) as usize
    }

    /// The constructor (the lines take places in the update list first, the object
    /// last).
    pub fn new(tile_size: (i32, i32), amount: f32, seq: &mut u64, rng: &mut UnityRandom) -> Self {
        let mut p = Self {
            update_order: 0,
            slated: false,
            amount,
            tile_size,
            room_px: Vec2::new(tile_size.0 as f32 * 20.0, tile_size.1 as f32 * 20.0),
            visible: true,
            lines: Vec::new(),
            scan_lines: Vec::new(),
        };
        for _ in 0..p.ideal_scan_line_number() {
            let flag = rng.value() > p.room_px.y / (p.room_px.y + p.room_px.x);
            let pos = rng.value() * if flag { p.room_px.x } else { p.room_px.y };
            let dir = if rng.value() < 0.5 { -1.0 } else { 1.0 };
            p.add_line(flag, pos, dir, seq, rng);
        }
        p.update_order = seq.next_seq();
        p
    }

    fn add_line(
        &mut self,
        alignment: bool,
        pos: f32,
        dir: f32,
        seq: &mut u64,
        rng: &mut UnityRandom,
    ) {
        let line = ProjectedScanLine::new(seq.next_seq(), alignment, pos, dir, rng);
        self.scan_lines.push(line.update_order);
        self.lines.push(line);
    }

    /// `AddScanLine` (three draws and the line's).
    fn add_scan_line(&mut self, seq: &mut u64, rng: &mut UnityRandom) {
        let flag = rng.value() > self.room_px.y / (self.room_px.y + self.room_px.x);
        let flag2 = rng.value() < 0.5;
        let pos = if flag2 {
            0.0
        } else if flag {
            self.room_px.x
        } else {
            self.room_px.y
        };
        self.add_line(flag, pos, if flag2 { 1.0 } else { -1.0 }, seq, rng);
    }

    /// The index of the line at this place in the update list.
    #[must_use]
    pub fn line_index(&self, order: u64) -> Option<usize> {
        self.lines
            .binary_search_by_key(&order, |l| l.update_order)
            .ok()
    }

    /// `Update`; `being_viewed` is `room.BeingViewed` and `electric_power`
    /// `room.ElectricPower`.
    pub fn update(
        &mut self,
        electric_power: f32,
        being_viewed: bool,
        seq: &mut u64,
        rng: &mut UnityRandom,
    ) {
        if rng.value() < 0.1 {
            self.visible = rng.value() < electric_power;
        }
        let lines = &self.lines;
        self.scan_lines.retain(|&o| {
            lines
                .binary_search_by_key(&o, |l| l.update_order)
                .ok()
                .is_some_and(|i| !lines[i].slated)
        });
        if self.scan_lines.len() < self.ideal_scan_line_number() {
            self.add_scan_line(seq, rng);
        }
        if !being_viewed {
            self.destroy();
        }
    }

    /// `Destroy`: the lines it counts, then itself.
    pub fn destroy(&mut self) {
        for k in 0..self.scan_lines.len() {
            if let Some(i) = self.line_index(self.scan_lines[k]) {
                self.lines[i].slated = true;
            }
        }
        self.slated = true;
    }

    /// The `Update` of the line at `order` (none for one slated).
    pub fn update_line(&mut self, order: u64) {
        let room_px = self.room_px;
        if let Some(i) = self.line_index(order)
            && !self.lines[i].slated
        {
            self.lines[i].update(room_px);
        }
    }

    /// Drops the lines `Destroy` slated (the room's update list lets them go).
    pub fn remove_slated(&mut self) {
        self.lines.retain(|l| !l.slated);
    }
}

#[cfg(test)]
#[path = "projected_scan_lines_tests.rs"]
mod tests;
