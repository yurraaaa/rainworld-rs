//! The room's water: the wavy surface simulation (`Water` with its `DefaultSurface`) that
//! physics reads through `Room.FloatWaterLevel`.
//!
//! It lives in the simulation because chunk submersion, buoyancy and drag, swimming and
//! breathing all read the surface height at a chunk's x, and bodies crossing or moving in
//! the surface push it. Behaviour: `rainworld-re/notes/water.md` section 1 and the
//! decompiled `Water` class; base game with Remix off. Not ported (docs/DIVERGENCES.md
//! D11): `WaterCutoff` and `AirPocket` placed objects (no base-game room uses them), the
//! unused `WaterViscosity` effect, rain rumble and flooding (no rain cycle yet), and the
//! cosmetic splashes, bubbles and sounds.

use rw_assets::{Room, RoomSettings, TILE_SIZE, Terrain};
use rw_random::UnityRandom;

use crate::float_ext::FloatExt;
use crate::{BodyChunk, Vec2};

/// One point of the surface (`Water.SurfacePoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct SurfacePoint {
    /// x on the 20 px grid; y the still water level.
    default_pos: Vec2,
    /// Offset from `default_pos`: smoothed wave height plus the travelling sines.
    pos: Vec2,
    last_default_pos: Vec2,
    last_pos: Vec2,
    /// Wave-equation displacement, clamped to +-40 px.
    height: f32,
    last_height: f32,
    next_height: f32,
}

impl SurfacePoint {
    fn new(default_pos: Vec2) -> Self {
        Self {
            default_pos,
            pos: Vec2::ZERO,
            last_default_pos: default_pos,
            last_pos: Vec2::ZERO,
            height: 0.0,
            last_height: 0.0,
            next_height: 0.0,
        }
    }

    fn room_pos(&self) -> Vec2 {
        self.default_pos + self.pos
    }

    /// `DrawPos`: between last tick's and this tick's position.
    fn draw_pos(&self, time_stacker: f32) -> Vec2 {
        (self.last_default_pos + self.last_pos).lerp(self.room_pos(), time_stacker)
    }
}

/// The room's water body and its surface (`Water`).
///
/// Points are 20 px apart from 220 px left of the room to 220 px right of it. Each has
/// a front (`[i][0]`, the physical surface) and a back point half a step to the right
/// (`[i][1]`, drawn only; its wave height stays 0).
#[derive(Debug, Clone, PartialEq)]
pub struct Water {
    points: Vec<[SurfacePoint; 2]>,
    /// `fWaterLevel`: still water level in px (constant without floods).
    level: f32,
    /// `Room.defaultWaterLevel`: tile row of the surface.
    level_row: i32,
    /// `Room.waterInFrontOfTerrain`.
    in_front: bool,
    sin_counter: f32,
    wave_amplitude: f32,
    wave_speed: f32,
    wave_length: f32,
    roll_back_length: f32,
    roll_back_amp: f32,
}

impl Water {
    /// Surface point spacing (`triangleWidth`), px.
    pub const SPACING: f32 = 20.0;
    /// How far the surface reaches past each side of the room (`leftMargin`), px.
    pub const MARGIN: f32 = 220.0;
    /// Wave height limit, px.
    const MAX_HEIGHT: f32 = 40.0;

    /// The water of `room` (`Room.AddWater`), or `None` for a room without water.
    #[must_use]
    pub fn new(room: &Room, settings: &RoomSettings) -> Option<Self> {
        let row = room.water_level()?;
        let level = row as f32 * TILE_SIZE + TILE_SIZE / 2.0;
        let width = Self::MARGIN + room.pixel_width() + Self::MARGIN;
        let total = (width / Self::SPACING) as usize + 1;
        let points = (0..total)
            .map(|i| {
                [0.0, 0.5].map(|offset| {
                    SurfacePoint::new(Vec2::new(
                        -Self::MARGIN + (i as f32 + offset) * Self::SPACING,
                        level,
                    ))
                })
            })
            .collect();
        let mut water = Self {
            points,
            level,
            level_row: row,
            in_front: room.water_in_front(),
            sin_counter: 0.0,
            wave_amplitude: 0.0,
            wave_speed: 0.0,
            wave_length: 0.0,
            roll_back_length: 0.0,
            roll_back_amp: 0.0,
        };
        water.apply_settings(settings);
        Some(water)
    }

    /// The wave parameters from the room settings (`Water.waveAmplitude` etc.).
    fn apply_settings(&mut self, s: &RoomSettings) {
        self.wave_amplitude = 1.0_f32.lerp(40.0, s.wave_amplitude);
        self.wave_speed = (-1.0_f32 / 30.0).lerp(1.0 / 30.0, s.wave_speed);
        self.wave_length = 50.0_f32.lerp(750.0, s.wave_length);
        self.roll_back_length = 2.0_f32.lerp(0.0, s.second_wave_length);
        self.roll_back_amp = s.second_wave_amplitude;
    }

    /// Tile row of the surface (`Room.defaultWaterLevel`).
    #[must_use]
    pub fn level_row(&self) -> i32 {
        self.level_row
    }

    /// Still water level in px (`fWaterLevel`, `20 * row + 10`).
    #[must_use]
    pub fn still_level(&self) -> f32 {
        self.level
    }

    /// Number of surface points.
    #[must_use]
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// Wave height of front point `i` (before the smoothing and sines are added).
    #[must_use]
    pub fn wave_height(&self, i: usize) -> f32 {
        self.points[i][0].height
    }

    /// Sets the wave height of front point `i` (as a splash would).
    pub fn set_wave_height(&mut self, i: usize, height: f32) {
        self.points[i][0].height = height;
    }

    /// Room position of front point `i` this tick.
    #[must_use]
    pub fn point(&self, i: usize) -> Vec2 {
        self.points[i][0].room_pos()
    }

    /// The surface height at `x` (`Room.FloatWaterLevel` / `DetailedWaterLevel`): linear
    /// between the two front points around `x`; past the ends, the end point's height.
    #[must_use]
    pub fn level_at(&self, x: f32) -> f32 {
        let a = self.previous_point(x);
        let b = (a + 1).min(self.points.len() - 1);
        let (pa, pb) = (self.points[a][0].room_pos(), self.points[b][0].room_pos());
        pa.y.lerp(pb.y, x.inverse_lerp(pa.x, pb.x))
    }

    /// `BodyChunk.submersion`: 0 with the surface at or below the chunk's bottom, 1 at or
    /// above its top, linear in between.
    #[must_use]
    pub fn submersion(&self, chunk: &BodyChunk) -> f32 {
        self.level_at(chunk.pos.x)
            .inverse_lerp(chunk.pos.y - chunk.rad, chunk.pos.y + chunk.rad)
    }

    /// Front surface points as the game draws them at `time_stacker` between ticks.
    pub fn draw_points(&self, time_stacker: f32) -> impl Iterator<Item = Vec2> + '_ {
        self.points.iter().map(move |p| p[0].draw_pos(time_stacker))
    }

    /// Back surface points (`[i][1]`, half a step right of the front ones) as drawn.
    pub fn back_draw_points(&self, time_stacker: f32) -> impl Iterator<Item = Vec2> + '_ {
        self.points.iter().map(move |p| p[1].draw_pos(time_stacker))
    }

    /// `Room.waterInFrontOfTerrain`.
    #[must_use]
    pub fn in_front(&self) -> bool {
        self.in_front
    }

    /// `Surface.PreviousPoint`: the last front point at or left of `x`.
    #[must_use]
    pub fn previous_point(&self, x: f32) -> usize {
        let last = self.points.len() as i32 - 1;
        let guess = ((x + 250.0) / Self::SPACING).floor() as i32 + 2;
        let mut i = guess.clamp(0, last) as usize;
        while i > 0 && x < self.points[i][0].room_pos().x {
            i -= 1;
        }
        i
    }

    /// `Surface.ClosestPoint`: the nearer of the two front points around `x`.
    fn closest_point(&self, x: f32) -> usize {
        let i = self.previous_point(x).min(self.points.len() - 2);
        let da = (self.points[i][0].room_pos().x - x).abs();
        let db = (self.points[i + 1][0].room_pos().x - x).abs();
        if da < db { i } else { i + 1 }
    }

    /// `Water.Update` (at the start of the room's tick, before any object moves): bodies
    /// crossing or moving in the surface push it, then the surface steps.
    pub fn update<'a>(
        &mut self,
        room: &Room,
        chunks: impl IntoIterator<Item = &'a mut BodyChunk>,
        rng: &mut UnityRandom,
    ) {
        for chunk in chunks {
            self.disturb(chunk);
        }
        self.step_surface(room, rng);
    }

    /// A chunk passing the surface faster than 3 px/tick splashes it (once per
    /// `splash_stop` window); one straddling it drags the two points around it.
    fn disturb(&mut self, chunk: &mut BodyChunk) {
        let level = self.level_at(chunk.pos.x);
        let (vel, rad) = (chunk.vel, chunk.rad);
        let entering = vel.y < -3.0 && chunk.last_pos.y > level && chunk.pos.y <= level;
        let leaving = vel.y > 3.0 && chunk.last_pos.y < level && chunk.pos.y >= level;
        if entering || leaving {
            let mut force = (vel.y * rad * chunk.mass.lerp(1.0, 0.3) / 3.0).lerp(10.0, 0.5);
            if force.abs() > vel.y.abs() {
                force = vel.y;
            }
            if chunk.splash_stop == 0 {
                chunk.splash_stop = 10;
                let i = self.closest_point(chunk.pos.x);
                self.points[i][0].height += force / 2.0;
                if i > 0 {
                    self.points[i - 1][0].height += force / 6.0;
                }
                if i < self.points.len() - 1 {
                    self.points[i + 1][0].height += force / 6.0;
                }
            }
        } else if chunk.pos.y - rad < level && chunk.pos.y + rad > level {
            let a = self.previous_point(chunk.pos.x);
            let b = (a + 1).min(self.points.len() - 1);
            self.points[a][0].height -= vel.x * rad * 0.03;
            self.points[b][0].height += vel.x * rad * 0.03;
            if vel.y.abs() > 2.0 {
                self.points[a][0].height += vel.y * rad * 0.01;
                self.points[b][0].height += vel.y * rad * 0.01;
            }
        }
    }

    /// `Surface.Update`: one step of the damped wave equation on the front heights
    /// (`R = 0.45`, ends reflecting, 0.75 damping where the surface is inside a solid
    /// tile, 0.95 for water in front of the terrain), a tiny random jitter, 1% loss,
    /// removal of two thirds of the mean height, then each point's offset: heights smoothed with
    /// neighbours that stick out plus the travelling sine and its roll-back wave.
    fn step_surface(&mut self, room: &Room, rng: &mut UnityRandom) {
        let r = 1.0_f32 * 0.0045 / (0.0005 * Self::SPACING);
        let r2 = r * r;
        let total = self.points.len();
        let mut sum = 0.0;
        for i in 0..total {
            let h = self.points[i][0].height;
            let last = self.points[i][0].last_height;
            let mut next = if i == 0 || i == total - 1 {
                let n = if i == 0 { i + 1 } else { i - 1 };
                (2.0 * h + (r - 1.0) * last + 2.0 * r2 * (self.points[n][0].height - h)) / (1.0 + r)
            } else {
                let next = r2 * (self.points[i - 1][0].height + self.points[i + 1][0].height)
                    + 2.0 * (1.0 - r2) * h
                    - last;
                let at = self.points[i][0].default_pos + Vec2::new(0.0, h);
                if room.tile_at(at.to_array()).terrain == Terrain::Solid {
                    next * if self.in_front { 0.95 } else { 0.75 }
                } else {
                    next
                }
            };
            next += (-self.wave_amplitude).lerp(self.wave_amplitude, rng.value()) * 0.005;
            next *= 0.99;
            self.points[i][0].next_height = next;
            sum += h;
            for j in 0..2 {
                self.place_point(i, j);
            }
        }
        let mean = sum / (total as f32 * 1.5);
        for l in 0..total {
            let p = self.points[l][0];
            let mut h = p.next_height - mean;
            if l > 0 && l < total - 1 {
                let around = self.points[l - 1][0]
                    .next_height
                    .lerp(self.points[l + 1][0].next_height, 0.5);
                h = h.lerp(around, 0.01);
            }
            let p = &mut self.points[l][0];
            p.last_height = p.height;
            p.height = h.clamp(-Self::MAX_HEIGHT, Self::MAX_HEIGHT);
        }
        self.sin_counter -= self.wave_speed;
    }

    /// The offset of point `[i][j]` this tick from the (not yet stepped) heights.
    fn place_point(&mut self, i: usize, j: usize) {
        let total = self.points.len() as i32;
        let phase = -(i as f32) * Self::SPACING / self.wave_length;
        let h = self.points[i][j].height;
        let mut sum = h * 3.0;
        let mut count = 3.0;
        for k in [-1_i32, 1] {
            let far = i as i32 + 2 * k;
            if far > 0 && far < total {
                let near = self.points[(i as i32 + k) as usize][j].height;
                if (h - near).abs() > (h - self.points[far as usize][j].height).abs() {
                    sum += near;
                    count += 1.0;
                }
            }
        }
        let (front, back) = if j == 1 { (1.0, -1.0) } else { (-1.0, 1.0) };
        let wave =
            Vec2::from_degrees((phase + self.sin_counter * front) * 360.0) * self.wave_amplitude;
        let roll_back =
            Vec2::from_degrees((phase + self.sin_counter * back) * 360.0 * self.roll_back_length)
                * (self.wave_amplitude * self.roll_back_amp);
        let level = self.level;
        let p = &mut self.points[i][j];
        p.last_pos = p.pos;
        p.last_default_pos = p.default_pos;
        p.pos = Vec2::new(0.0, sum / count) + wave + roll_back;
        // `TargetWaterLevel`: flat without `WaterCutoff` objects.
        p.default_pos.y = level;
    }
}

impl BodyChunk {
    /// `submersion` in a room with `water`, 0 without (the game's level is then
    /// `float.MinValue`).
    #[must_use]
    pub fn submersion(&self, water: Option<&Water>) -> f32 {
        water.map_or(0.0, |w| w.submersion(self))
    }
}

#[cfg(test)]
mod tests {
    //! Expected values are derived by hand from `water.md` sections 1-2 and the
    //! `Water` / `BodyChunk.Update` formulas quoted in the comments.

    use rw_assets::{Room, RoomSettings};
    use rw_random::UnityRandom;

    use crate::tests::room_with_water;
    use crate::{BodyChunk, PhysicalObject, Vec2, Water, slugcat};

    const EPS: f32 = 1e-3;

    /// 20 x 10 open tiles, water row 3: still level 70, 43 surface points.
    fn pool() -> (Room, Water) {
        let row = ".".repeat(20);
        let room = room_with_water(&[row.as_str(); 10], 3);
        let water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
        (room, water)
    }

    fn chunk_at(y: f32, vel: Vec2) -> PhysicalObject {
        let mut c = BodyChunk::new(Vec2::new(100.0, y), 9.0, slugcat::CHUNK_MASS);
        c.vel = vel;
        PhysicalObject::new(vec![c], slugcat::PARAMS)
    }

    fn close(actual: f32, expected: f32, what: &str) {
        assert!(
            (actual - expected).abs() < EPS,
            "{what}: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn layout_and_flat_level() {
        let (_, w) = pool();
        // (220 + 400 + 220) / 20 + 1 points from x = -220; level 20 * 3 + 10.
        assert_eq!(w.point_count(), 43);
        assert_eq!(w.point(0), Vec2::new(-220.0, 70.0));
        assert_eq!(w.point(42), Vec2::new(620.0, 70.0));
        assert_eq!(w.level_at(-1000.0), 70.0);
        assert_eq!(w.level_at(133.0), 70.0);
    }

    #[test]
    fn submersion_at_several_depths() {
        // InverseLerp(y - 9, y + 9, 70).
        let (_, w) = pool();
        for (y, expected) in [
            (79.0, 0.0),
            (90.0, 0.0),
            (70.0, 0.5),
            (65.5, 0.75),
            (61.0, 1.0),
        ] {
            let c = BodyChunk::new(Vec2::new(100.0, y), 9.0, 0.35);
            close(w.submersion(&c), expected, &format!("y {y}"));
        }
        let c = BodyChunk::new(Vec2::new(100.0, 70.0), 9.0, 0.35);
        assert_eq!(c.submersion(None), 0.0);
    }

    #[test]
    fn buoyancy_and_drag_per_tick() {
        let (room, w) = pool();
        // Fully under, at rest: vy = -0.9 + 0.95 = 0.05 (the net upward acceleration),
        // times water friction 0.96.
        let mut o = chunk_at(40.0, Vec2::ZERO);
        o.update_with(&room, Some(&w), &mut ());
        close(o.chunks[0].vel.y, 0.05 * 0.96, "full");
        close(o.chunks[0].pos.y, 40.0 + 0.048, "moved");
        // Half under: vy = -0.9 + 0.95 * 0.5, times lerp(0.999, 0.96, 0.5) = 0.9795.
        let mut o = chunk_at(70.0, Vec2::ZERO);
        o.update_with(&room, Some(&w), &mut ());
        close(o.chunks[0].vel.y, -0.425 * 0.9795, "half");
        // Bottom just above the surface: air only, -0.9 * 0.999.
        let mut o = chunk_at(79.5, Vec2::ZERO);
        o.update_with(&room, Some(&w), &mut ());
        close(o.chunks[0].vel.y, -0.9 * 0.999, "dry");
    }

    #[test]
    fn fast_chunks_drag_harder_unless_immune() {
        let (room, w) = pool();
        // vy = -20 - 0.9 + 0.95 = -19.95; (1 / (19.95 - 10))^0.5 = 0.317021;
        // immunity 0: lerp(0, 0.96, 0.317021) = 0.304340.
        let mut o = chunk_at(30.0, Vec2::new(0.0, -20.0));
        o.update_with(&room, Some(&w), &mut ());
        close(o.chunks[0].vel.y, -19.95 * 0.304_340, "no immunity");
        // Immunity 0.85: lerp(0.96 * 0.85, 0.96, 0.317021) = 0.861651.
        let mut o = chunk_at(30.0, Vec2::new(0.0, -20.0));
        o.params.water_retardation_immunity = 0.85;
        o.update_with(&room, Some(&w), &mut ());
        close(o.chunks[0].vel.y, -19.95 * 0.861_651, "immune");
    }

    #[test]
    fn skim_bounces_off_the_surface() {
        let (room, w) = pool();
        // Submersion 0.25 (y 74.5): vel (12, -1) -> gravity -1.9 -> vy * -0.5, vx * 0.75.
        let mut o = chunk_at(74.5, Vec2::new(12.0, -1.0));
        o.update_with(&room, Some(&w), &mut ());
        close(o.chunks[0].vel.y, 0.95, "vy");
        close(o.chunks[0].vel.x, 9.0, "vx");
        // Not fast enough sideways (|vx| 9): ordinary buoyancy and drag instead.
        let mut o = chunk_at(74.5, Vec2::new(9.0, -1.0));
        o.update_with(&room, Some(&w), &mut ());
        assert!(o.chunks[0].vel.y < 0.0);
    }

    #[test]
    fn single_disturbance_propagates() {
        let (room, mut w) = pool();
        // No sines or jitter, to follow the wave equation alone.
        w.wave_amplitude = 0.0;
        w.set_wave_height(20, 10.0);
        let mut rng = UnityRandom::new(1);
        w.update(&room, std::iter::empty(), &mut rng);
        // R = 0.45, R^2 = 0.2025. next[20] = 2 (1 - R^2) 10 * 0.99 = 15.7905,
        // next[19, 21] = R^2 10 * 0.99 = 2.00475; mean term 10 / (43 * 1.5) = 0.155039;
        // interior heights blend 1% toward the mean of their neighbours' next values.
        close(w.wave_height(20), 15.499_154, "centre");
        close(w.wave_height(19), 1.910_167, "left");
        close(w.wave_height(21), 1.910_167, "right");
        close(w.wave_height(22), -0.143_465, "two away");
        close(w.wave_height(30), -0.153_488, "far");
        close(w.wave_height(0), -0.155_039, "end");
        // Positions use the heights before the step: the peak alone (3 * 10 / 3), its
        // neighbours averaged with it (10 / 4).
        close(w.point(20).y, 80.0, "peak level");
        close(w.point(21).y, 72.5, "shoulder level");
        close(w.level_at(w.point(20).x + 10.0), 76.25, "between");
        w.update(&room, std::iter::empty(), &mut rng);
        // Tick 2: next = 0.99 (R^2 (h19 + h21) + 2 (1 - R^2) h20 - 10), minus the new
        // mean term, blended 1% toward next[21].
        close(w.wave_height(20), 15.044_812, "centre tick 2");
    }

    #[test]
    fn entering_chunk_splashes_once() {
        let (room, mut w) = pool();
        w.wave_amplitude = 0.0;
        // Crossing down at -20: force = lerp(-20 * 9 * lerp(0.35, 1, 0.3) / 3, 10, 0.5)
        // = -11.35; the closest point (x 100 -> index 16) gets half, neighbours a sixth.
        let mut c = BodyChunk::new(Vec2::new(100.0, 65.0), 9.0, 0.35);
        c.last_pos = Vec2::new(100.0, 85.0);
        c.vel = Vec2::new(0.0, -20.0);
        let mut chunks = [c];
        w.update(&room, chunks.iter_mut(), &mut UnityRandom::new(1));
        assert_eq!(chunks[0].splash_stop, 10);
        // Point 16 had height -5.675 when the surface stepped; its neighbours'
        // -1.891667 differ from it less than the next ones' 0, so it is not smoothed.
        close(w.point(16).y, 70.0 - 5.675, "splash point");
        close(
            w.point(15).y,
            70.0 + (-1.891_667 * 3.0 - 5.675) / 4.0,
            "beside",
        );
        // splash_stop blocks a second splash (the chunk crossing again, fully under).
        chunks[0].pos.y = 50.0;
        let before = w.wave_height(16);
        w.disturb(&mut chunks[0]);
        assert_eq!(w.wave_height(16), before);
    }
}
