//! `SuperStructureProjector` (the `SuperStructureProjector` room effect, Five Pebbles'
//! superstructure): a holographic overlay of moving scan lines, cursors on a 15 px grid,
//! single glyphs that spread over free neighbour cells and glyph matrices. The projector
//! and each of its parts are objects of the room's update list; the parts are kept here
//! in creation order (their `update_order`, so found by binary search) and the game's side
//! (`game_projections.rs`) runs each one at its turn. Sprites are `rw-graphics`'
//! `projector_draw.rs`.
//!
//! The More Slugcats region `MS` hides the projector unless Moon's heart is active (DLC,
//! not ported, D35). Draws are the game's, in order; its quirks are kept (a reused glyph
//! keeps the old `pos` while the grid holds it at the new cell; `ScrambleAllGlyphs`
//! tests `pos > 0`).
//!
//! Mirrors: SuperStructureProjector.cs

use rw_random::UnityRandom;

use crate::{FloatExt, Vec2};

/// `glyphGrid`'s size.
pub const GRID_SIZE: (i32, i32) = (95, 55);

/// What the projector reads of the game at its turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectorFrame {
    /// `room.ElectricPower`.
    pub electric_power: f32,
    /// `room.BeingViewed`.
    pub being_viewed: bool,
    /// `game.cameras[0].currentCameraPosition` while camera 0 is in this room.
    pub camera_position: Option<usize>,
}

/// `SuperStructureProjector.ScanLine`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanLine {
    pub alignment: bool,
    pub pos: f32,
    pub last_pos: f32,
    pub speed: f32,
}

impl ScanLine {
    /// One draw (the speed).
    pub fn new(alignment: bool, pos: f32, dir: f32, rng: &mut UnityRandom) -> Self {
        let speed = 1.0f32.lerp(7.0, rng.value()) * dir;
        Self {
            alignment,
            pos: pos + speed.unity_sign(),
            last_pos: pos - speed.unity_sign(),
            speed,
        }
    }

    /// `Update` with the projector's grid movement this tick on the line's axis and the
    /// room's size on it; true if it called `Destroy`.
    pub fn update(&mut self, grid_move: Vec2, room_px: Vec2) -> bool {
        self.last_pos = self.pos;
        self.pos += self.speed;
        let (drift, extent) = if self.alignment {
            (grid_move.x, room_px.x)
        } else {
            (grid_move.y, room_px.y)
        };
        self.pos += drift;
        (self.pos < -200.0 && self.last_pos < -200.0 && self.speed < 0.0)
            || (self.pos > extent + 200.0 && self.last_pos > extent + 200.0 && self.speed > 0.0)
    }
}

/// `SuperStructureProjector.Cursor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cursor {
    pub g_pos: (i32, i32),
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub size: i32,
}

/// The state every `Glyph` has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    pub pos: (i32, i32),
    pub glyph_col: f32,
    pub life: i32,
    pub counter: i32,
}

/// `SuperStructureProjector.SingleGlyph`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SingleGlyph {
    pub glyph: Glyph,
    pub procreate: i32,
    pub selected: bool,
    /// `selected` when the constructor ran: the sprite's colour is set once, when its
    /// leaser is made, and a reused glyph keeps it.
    pub sprite_selected: bool,
    pub inactive: bool,
}

/// `SuperStructureProjector.GlyphMatrix`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphMatrix {
    pub glyph: Glyph,
    pub size: (i32, i32),
    pub max_size: (i32, i32),
}

/// What a projector part is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartKind {
    ScanLine(ScanLine),
    Cursor(Cursor),
    Single(SingleGlyph),
    Matrix(GlyphMatrix),
}

/// An object of the projector (`SuperStructureProjectorPart`) in the room's update list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectorPart {
    pub update_order: u64,
    pub slated: bool,
    pub kind: PartKind,
}

/// `SuperStructureProjector`.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperStructureProjector {
    pub update_order: u64,
    pub slated: bool,
    /// `effect.amount`.
    pub amount: f32,
    pub tile_size: (i32, i32),
    /// `room.PixelWidth`, `PixelHeight`.
    pub room_px: Vec2,
    pub entire_room_size: (i32, i32),
    pub grid_pos: Vec2,
    pub last_grid_pos: Vec2,
    pub grid_speed: Vec2,
    pub int_offset: (i32, i32),
    pub visible: bool,
    pub last_cam_pos: usize,
    /// `parts`, in creation order.
    pub parts: Vec<ProjectorPart>,
    pub scan_lines: Vec<u64>,
    pub glyphs_list: Vec<u64>,
    pub cursors: Vec<u64>,
    pub inactive_glyphs_waiting_room: Vec<u64>,
    /// `glyphGrid` (95 x 55, `x` major): the glyph in each cell.
    glyph_grid: Vec<Option<u64>>,
}

/// Where a new object takes its place in the room's update list: increments and returns.
fn next_seq(seq: &mut u64) -> u64 {
    *seq += 1;
    *seq
}

impl Glyph {
    /// `Glyph`'s constructor: `glyphCol` and the life (two draws); the caller puts it in
    /// the grid.
    fn new(pos: (i32, i32), rng: &mut UnityRandom) -> Self {
        let glyph_col = rng.value();
        let life = rng.range_i32(20, 200).max(0);
        Self {
            pos,
            glyph_col,
            life,
            counter: 0,
        }
    }
}

impl SingleGlyph {
    /// `Reset`'s random part: the life, whether it spreads and when, whether it is
    /// selected (3 to 4 draws).
    fn reset_draws(&mut self, rng: &mut UnityRandom) {
        self.glyph.life = rng.range_i32(10, 600);
        self.procreate = if rng.value() < 0.5 {
            -1
        } else {
            rng.range_i32(1, self.glyph.life)
        };
        self.selected = rng.value() < 0.02;
        self.glyph.counter = 0;
    }
}

impl GlyphMatrix {
    /// `GlyphMatrix`'s constructor after `Glyph`'s: the maximum size, the size and the
    /// life (5 draws, fewer when a maximum is 1).
    fn new(glyph: Glyph, amount: f32, rng: &mut UnityRandom) -> Self {
        let max_x = 1.0f32.lerp(70.0, rng.value() * amount).to_int_cs();
        let max_y = 1.0f32.lerp(70.0, rng.value() * amount).to_int_cs();
        let size = (rng.range_i32(1, max_x), rng.range_i32(1, max_y));
        let mut glyph = glyph;
        glyph.life = rng.range_i32(10, 600);
        Self {
            glyph,
            size,
            max_size: (max_x, max_y),
        }
    }
}

impl SuperStructureProjector {
    /// `idealGlyphNumber`.
    #[must_use]
    pub fn ideal_glyph_number(&self) -> usize {
        (520.0 * self.amount).to_int_cs().max(0) as usize
    }

    /// `idealScanLineNumber`.
    #[must_use]
    pub fn ideal_scan_line_number(&self) -> usize {
        let (w, h) = self.tile_size;
        let both = ((w * h) as f32).lerp(w.max(h) as f32, 0.5);
        (both / 2000.0 * 10.0 * self.amount).to_int_cs().max(0) as usize
    }

    /// `idealCursorNumber`.
    #[must_use]
    pub fn ideal_cursor_number(&self) -> usize {
        let val = i32::from(self.amount > 0.1);
        let (w, h) = self.tile_size;
        val.max(((w * h) as f32 / 2000.0 * 1.2 * self.amount).to_int_cs())
            .max(0) as usize
    }

    /// The constructor: the scan lines, half the ideal glyph count of random glyphs, then
    /// `intOffset`; the parts take places in the update list first, the projector last
    /// (`AddObject` after the constructor).
    pub fn new(tile_size: (i32, i32), amount: f32, seq: &mut u64, rng: &mut UnityRandom) -> Self {
        let room_px = Vec2::new(tile_size.0 as f32 * 20.0, tile_size.1 as f32 * 20.0);
        let mut p = Self {
            update_order: 0,
            slated: false,
            amount,
            tile_size,
            room_px,
            entire_room_size: (
                (room_px.x / 15.0).floor() as i32 + 1,
                (room_px.y / 15.0).floor() as i32 + 1,
            ),
            grid_pos: Vec2::ZERO,
            last_grid_pos: Vec2::ZERO,
            grid_speed: Vec2::ZERO,
            int_offset: (0, 0),
            visible: true,
            last_cam_pos: 0,
            parts: Vec::new(),
            scan_lines: Vec::new(),
            glyphs_list: Vec::new(),
            cursors: Vec::new(),
            inactive_glyphs_waiting_room: Vec::new(),
            glyph_grid: vec![None; (GRID_SIZE.0 * GRID_SIZE.1) as usize],
        };
        for _ in 0..p.ideal_scan_line_number() {
            let flag = rng.value() > p.room_px.y / (p.room_px.y + p.room_px.x);
            let pos = rng.value() * if flag { p.room_px.x } else { p.room_px.y };
            let dir = if rng.value() < 0.5 { -1.0 } else { 1.0 };
            p.add_scan_line_part(flag, pos, dir, seq, rng);
        }
        for _ in 0..p.ideal_glyph_number() / 2 {
            p.add_random_glyph(seq, rng);
        }
        p.int_offset = (rng.range_i32(100, 1000), rng.range_i32(100, 1000));
        p.update_order = next_seq(seq);
        p
    }

    fn grid_index(pos: (i32, i32)) -> Option<usize> {
        ((0..GRID_SIZE.0).contains(&pos.0) && (0..GRID_SIZE.1).contains(&pos.1))
            .then(|| (pos.0 * GRID_SIZE.1 + pos.1) as usize)
    }

    /// `glyphGrid[x, y]`.
    #[must_use]
    pub fn grid_at(&self, pos: (i32, i32)) -> Option<u64> {
        Self::grid_index(pos).and_then(|i| self.glyph_grid[i])
    }

    fn set_grid(&mut self, pos: (i32, i32), to: Option<u64>) {
        if let Some(i) = Self::grid_index(pos) {
            self.glyph_grid[i] = to;
        }
    }

    /// The index of the part at this place in the update list.
    #[must_use]
    pub fn part_index(&self, order: u64) -> Option<usize> {
        self.parts
            .binary_search_by_key(&order, |p| p.update_order)
            .ok()
    }

    /// The part at this place in the update list.
    #[must_use]
    pub fn part(&self, order: u64) -> Option<&ProjectorPart> {
        self.part_index(order).map(|i| &self.parts[i])
    }

    fn push_part(&mut self, kind: PartKind, seq: &mut u64) -> u64 {
        let update_order = next_seq(seq);
        self.parts.push(ProjectorPart {
            update_order,
            slated: false,
            kind,
        });
        update_order
    }

    /// A `ScanLine` joins the room (one draw).
    fn add_scan_line_part(
        &mut self,
        alignment: bool,
        pos: f32,
        dir: f32,
        seq: &mut u64,
        rng: &mut UnityRandom,
    ) {
        let line = ScanLine::new(alignment, pos, dir, rng);
        let order = self.push_part(PartKind::ScanLine(line), seq);
        self.scan_lines.push(order);
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
        self.add_scan_line_part(flag, pos, if flag2 { 1.0 } else { -1.0 }, seq, rng);
    }

    /// `AddRandomGlyph`: a cell, then a matrix (one in twenty) or a single glyph.
    fn add_random_glyph(&mut self, seq: &mut u64, rng: &mut UnityRandom) {
        let pos = (rng.range_i32(0, GRID_SIZE.0), rng.range_i32(0, GRID_SIZE.1));
        if rng.value() < 0.05 {
            let glyph = Glyph::new(pos, rng);
            let matrix = GlyphMatrix::new(glyph, self.amount, rng);
            let order = self.push_part(PartKind::Matrix(matrix), seq);
            self.glyphs_list.push(order);
            self.set_grid(pos, Some(order));
        } else {
            self.add_single_glyph_at(pos, seq, rng);
        }
    }

    /// `AddSingleGlyphAt`: a glyph from the waiting room is reset (keeping its `pos`), else
    /// a new one is made.
    pub fn add_single_glyph_at(&mut self, ps: (i32, i32), seq: &mut u64, rng: &mut UnityRandom) {
        if !self.inactive_glyphs_waiting_room.is_empty() {
            let order = self.inactive_glyphs_waiting_room.remove(0);
            if let Some(i) = self.part_index(order)
                && let PartKind::Single(s) = &mut self.parts[i].kind
            {
                s.inactive = false;
                s.reset_draws(rng);
                self.set_grid(ps, Some(order));
                self.glyphs_list.push(order);
            }
        } else {
            let glyph = Glyph::new(ps, rng);
            let mut single = SingleGlyph {
                glyph,
                procreate: 0,
                selected: false,
                sprite_selected: false,
                inactive: false,
            };
            let order = next_seq(seq);
            self.glyphs_list.push(order);
            self.set_grid(ps, Some(order));
            single.reset_draws(rng);
            single.sprite_selected = single.selected;
            self.parts.push(ProjectorPart {
                update_order: order,
                slated: false,
                kind: PartKind::Single(single),
            });
        }
    }

    /// `AddCursor`: two draws and the cursor's.
    fn add_cursor(&mut self, seq: &mut u64, rng: &mut UnityRandom) {
        let g_pos = (
            rng.range_i32(0, self.entire_room_size.0),
            rng.range_i32(0, self.entire_room_size.1),
        );
        let pos = self.grid_pos_at(g_pos, 1.0);
        let size = rng.range_i32(1, 4);
        let order = self.push_part(
            PartKind::Cursor(Cursor {
                g_pos,
                pos,
                last_pos: pos,
                size,
            }),
            seq,
        );
        self.cursors.push(order);
    }

    /// `GridPos`.
    #[must_use]
    pub fn grid_pos_at(&self, g_pos: (i32, i32), time_stacker: f32) -> Vec2 {
        Vec2::new(g_pos.0 as f32 * 15.0, g_pos.1 as f32 * 15.0)
            + self.last_grid_pos.lerp(self.grid_pos, time_stacker)
    }

    /// `UpdateOffset`'s `_gridOffset`.
    #[must_use]
    pub fn grid_offset(&self, time_stacker: f32) -> Vec2 {
        self.last_grid_pos.lerp(self.grid_pos, time_stacker)
            + Vec2::new(0.0, -1.0)
            + Vec2::new(self.int_offset.0 as f32, self.int_offset.1 as f32) * 15.0
    }

    /// `Glyph.Destroy` / the other parts' `Destroy`.
    fn destroy_part(&mut self, i: usize) {
        if let PartKind::Single(SingleGlyph { glyph, .. })
        | PartKind::Matrix(GlyphMatrix { glyph, .. }) = self.parts[i].kind
        {
            self.set_grid(glyph.pos, None);
        }
        self.parts[i].slated = true;
    }

    /// `Destroy`: every part, then the projector.
    pub fn destroy(&mut self) {
        for i in 0..self.parts.len() {
            self.destroy_part(i);
        }
        self.slated = true;
    }

    /// `Glyph.Move`.
    fn move_glyph(&mut self, order: u64, movement: (i32, i32)) {
        let Some(i) = self.part_index(order) else {
            return;
        };
        let (PartKind::Single(SingleGlyph { glyph, .. })
        | PartKind::Matrix(GlyphMatrix { glyph, .. })) = &mut self.parts[i].kind
        else {
            return;
        };
        let old = glyph.pos;
        glyph.pos.0 += movement.0;
        glyph.pos.1 += movement.1;
        if glyph.pos.0 < 0 {
            glyph.pos.0 += GRID_SIZE.0;
        } else if glyph.pos.0 >= GRID_SIZE.0 {
            glyph.pos.0 -= GRID_SIZE.0;
        }
        if glyph.pos.1 < 0 {
            glyph.pos.1 += GRID_SIZE.1;
        } else if glyph.pos.1 >= GRID_SIZE.1 {
            glyph.pos.1 -= GRID_SIZE.1;
        }
        let new = glyph.pos;
        self.set_grid(old, None);
        self.set_grid(new, Some(order));
    }

    fn move_all_glyphs(&mut self, movement: (i32, i32)) {
        for k in 0..self.glyphs_list.len() {
            self.move_glyph(self.glyphs_list[k], movement);
        }
    }

    /// The projector's `Update`.
    pub fn update(&mut self, frame: &ProjectorFrame, seq: &mut u64, rng: &mut UnityRandom) {
        self.last_grid_pos = self.grid_pos;
        self.grid_pos += self.grid_speed;
        self.grid_speed = (self.grid_speed + Vec2::random_unit(rng)).clamp_magnitude(5.0);
        if rng.value() < 0.1 {
            self.visible = rng.value() < frame.electric_power;
        }
        if self.grid_pos.x < -15.0 {
            self.move_all_glyphs((-2, 0));
            self.grid_pos.x += 30.0;
            self.last_grid_pos.x += 30.0;
            self.int_offset.0 -= 2;
        }
        if self.grid_pos.x > 15.0 {
            self.move_all_glyphs((2, 0));
            self.grid_pos.x -= 30.0;
            self.last_grid_pos.x -= 30.0;
            self.int_offset.0 += 2;
        }
        if self.grid_pos.y < -15.0 {
            self.move_all_glyphs((0, -2));
            self.grid_pos.y += 30.0;
            self.last_grid_pos.y += 30.0;
            self.int_offset.1 -= 2;
        }
        if self.grid_pos.y > 15.0 {
            self.move_all_glyphs((0, 2));
            self.grid_pos.y -= 30.0;
            self.last_grid_pos.y -= 30.0;
            self.int_offset.1 += 2;
        }
        self.prune();
        if self.scan_lines.len() < self.ideal_scan_line_number() {
            self.add_scan_line(seq, rng);
        }
        if self.glyphs_list.len() < self.ideal_glyph_number() {
            self.add_random_glyph(seq, rng);
        }
        if self.cursors.len() < self.ideal_cursor_number() {
            self.add_cursor(seq, rng);
        }
        if !frame.being_viewed {
            self.destroy();
        }
        if let Some(current) = frame.camera_position {
            if current != self.last_cam_pos {
                self.scramble_all_glyphs(rng);
            }
            self.last_cam_pos = current;
        }
    }

    /// Takes the destroyed parts (and the glyphs gone inactive) off the projector's lists.
    fn prune(&mut self) {
        let slated: Vec<u64> = self
            .parts
            .iter()
            .filter(|p| p.slated)
            .map(|p| p.update_order)
            .collect();
        self.parts.retain(|p| !p.slated);
        self.scan_lines.retain(|o| !slated.contains(o));
        self.cursors.retain(|o| !slated.contains(o));
        let parts = &self.parts;
        let inactive = |o: &u64| {
            parts
                .binary_search_by_key(o, |p| p.update_order)
                .ok()
                .is_some_and(|i| {
                    matches!(
                        parts[i].kind,
                        PartKind::Single(SingleGlyph { inactive: true, .. })
                    )
                })
        };
        self.glyphs_list
            .retain(|o| !slated.contains(o) && !inactive(o));
    }

    /// `ScrambleAllGlyphs` (two draws per glyph).
    pub fn scramble_all_glyphs(&mut self, rng: &mut UnityRandom) {
        for k in 0..self.glyphs_list.len() {
            let order = self.glyphs_list[k];
            let Some(i) = self.part_index(order) else {
                continue;
            };
            let (PartKind::Single(SingleGlyph { glyph, .. })
            | PartKind::Matrix(GlyphMatrix { glyph, .. })) = &mut self.parts[i].kind
            else {
                continue;
            };
            let old = glyph.pos;
            glyph.pos = (rng.range_i32(0, GRID_SIZE.0), rng.range_i32(0, GRID_SIZE.1));
            let new = glyph.pos;
            if old.0 > 0 && old.1 > 0 && old.0 < GRID_SIZE.0 && old.1 < GRID_SIZE.1 {
                self.set_grid(old, None);
            }
            self.set_grid(new, Some(order));
        }
    }

    /// The `Update` of the part at `order`; nothing for a part that is not (or no longer)
    /// there or is slated.
    pub fn update_part(&mut self, order: u64, seq: &mut u64, rng: &mut UnityRandom) {
        let Some(i) = self.part_index(order) else {
            return;
        };
        if self.parts[i].slated {
            return;
        }
        match self.parts[i].kind {
            PartKind::ScanLine(_) => self.update_scan_line(i),
            PartKind::Cursor(_) => self.update_cursor(i, rng),
            PartKind::Single(_) => self.update_single(i, seq, rng),
            PartKind::Matrix(_) => self.update_matrix(i, rng),
        }
    }

    fn update_scan_line(&mut self, i: usize) {
        let grid_move = self.grid_pos - self.last_grid_pos;
        let room_px = self.room_px;
        if let PartKind::ScanLine(line) = &mut self.parts[i].kind
            && line.update(grid_move, room_px)
        {
            self.parts[i].slated = true;
        }
    }

    fn update_cursor(&mut self, i: usize, rng: &mut UnityRandom) {
        let PartKind::Cursor(mut c) = self.parts[i].kind else {
            return;
        };
        c.last_pos = c.pos;
        c.pos += (self.grid_pos_at(c.g_pos, 1.0) - c.pos).clamp_magnitude(12.0);
        if rng.value() < 0.1 {
            c.g_pos.0 += rng.range_i32(-1, 2);
        }
        if rng.value() < 0.1 {
            c.g_pos.1 += rng.range_i32(-1, 2);
        }
        if c.g_pos.0 < 0
            || c.g_pos.0 >= self.entire_room_size.0
            || c.g_pos.1 < 0
            || c.g_pos.1 >= self.entire_room_size.1
        {
            self.parts[i].slated = true;
        }
        self.parts[i].kind = PartKind::Cursor(c);
    }

    /// The glyph state of a single glyph or matrix.
    fn glyph_mut(&mut self, i: usize) -> Option<&mut Glyph> {
        match &mut self.parts[i].kind {
            PartKind::Single(SingleGlyph { glyph, .. })
            | PartKind::Matrix(GlyphMatrix { glyph, .. }) => Some(glyph),
            _ => None,
        }
    }

    /// `Glyph.Update`: the counter and what the end of life does (a single glyph goes
    /// inactive, a matrix is destroyed).
    fn update_glyph(&mut self, i: usize) {
        let Some(glyph) = self.glyph_mut(i) else {
            return;
        };
        glyph.counter += 1;
        if glyph.counter < glyph.life {
            return;
        }
        let pos = glyph.pos;
        let order = self.parts[i].update_order;
        match &mut self.parts[i].kind {
            PartKind::Single(s) => {
                if !s.inactive {
                    s.inactive = true;
                    self.inactive_glyphs_waiting_room.push(order);
                    self.set_grid(pos, None);
                }
            }
            _ => self.destroy_part(i),
        }
    }

    fn update_single(&mut self, i: usize, seq: &mut u64, rng: &mut UnityRandom) {
        let PartKind::Single(s) = self.parts[i].kind else {
            return;
        };
        if s.inactive {
            return;
        }
        self.update_glyph(i);
        let PartKind::Single(s) = self.parts[i].kind else {
            return;
        };
        if s.glyph.counter == s.procreate
            || (s.glyph.counter == 10
                && rng.value() > 1.0 / 3.0
                && self.glyphs_list.len() < self.ideal_glyph_number())
        {
            self.procreate(s.glyph.pos, seq, rng);
        }
    }

    /// `SingleGlyph.Procreate`: a glyph in a random free neighbour cell.
    fn procreate(&mut self, pos: (i32, i32), seq: &mut u64, rng: &mut UnityRandom) {
        const FOUR_DIRECTIONS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];
        let free: Vec<(i32, i32)> = FOUR_DIRECTIONS
            .iter()
            .map(|d| (pos.0 + d.0, pos.1 + d.1))
            .filter(|&item| Self::grid_index(item).is_some() && self.grid_at(item).is_none())
            .collect();
        if !free.is_empty() {
            let at = rng.range_i32(0, free.len() as i32) as usize;
            self.add_single_glyph_at(free[at], seq, rng);
        }
    }

    fn update_matrix(&mut self, i: usize, rng: &mut UnityRandom) {
        self.update_glyph(i);
        let PartKind::Matrix(mut m) = self.parts[i].kind else {
            return;
        };
        if rng.value() < 0.05 {
            if rng.value() < 0.5 {
                let step = if rng.value() < 0.5 { -1 } else { 1 };
                m.size.0 = (m.size.0 + step).clamp(1, m.max_size.0.max(1));
            } else {
                let step = if rng.value() < 0.5 { -1 } else { 1 };
                m.size.1 = (m.size.1 + step).clamp(1, m.max_size.1.max(1));
            }
        }
        self.parts[i].kind = PartKind::Matrix(m);
    }
}

#[cfg(test)]
#[path = "projector_tests.rs"]
mod tests;
