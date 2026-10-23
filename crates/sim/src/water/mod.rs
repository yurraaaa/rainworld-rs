//! The room's water: the wavy surface simulation (`Water` with its `DefaultSurface`) that
//! physics reads through `Room.FloatWaterLevel`, and what `Water.Update` adds to the room
//! (the bottom bubbles, the bubbles of bodies crossing the surface and their
//! `BubbleEmitter`s).
//!
//! It lives in the simulation because chunk submersion, buoyancy and drag, swimming and
//! breathing all read the surface height at a chunk's x, bodies crossing or moving in
//! the surface push it, and its bubbles draw on the shared stream. Behaviour:
//! `rainworld-re/notes/water.md` section 1, `surveys/room-effects/water-effects.md` W4-W6
//! and the decompiled `Water` class; base game with Remix off. Not ported (docs/
//! divergences/ D11): `WaterCutoff` and `AirPocket` placed objects (no base-game room
//! uses them), the unused `WaterViscosity` effect and flooding (the rain's `RoomRain`
//! is not ported yet). The surface's rumble terms are [`SurfaceRumble`]. The `Splash`es it
//! makes are `splash.rs`'; they push the surface back through
//! [`Water::waterfall_hit_surface`].
//!
//! Mirrors: Water.cs, Room.cs (FloatWaterLevel)

pub(crate) mod bubble;
pub(crate) mod splash;
pub(crate) mod water_drip;
pub(crate) mod waterfall;

use rw_assets::{DangerType, Room, RoomSettings, TILE_SIZE, Terrain};
use rw_random::UnityRandom;

use crate::item::Entity;
use crate::math::float_ext::FloatExt;
use crate::{BodyChunk, Bubble, SoundId, Sounds, Splash, Vec2};

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

/// `Surface.Update`'s rain terms: in a room with `DangerType` other than `None` every
/// surface point draws three values a tick whatever the shake values are, and the
/// cycle's `ScreenShake` and `MicroScreenShake` times the room's `RumbleIntensity` move
/// the surface.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SurfaceRumble {
    /// `DangerType` is neither `None` nor (with a DLC) `Blizzard`.
    pub active: bool,
    /// `roomSettings.RumbleIntensity`.
    pub intensity: f32,
    /// `world.rainCycle.ScreenShake`, set by the caller each tick.
    pub screen_shake: f32,
    /// `world.rainCycle.MicroScreenShake`, set by the caller each tick.
    pub micro_screen_shake: f32,
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
    /// The rain's terms in the surface step.
    pub rumble: SurfaceRumble,
    bubble_emitters: Vec<BubbleEmitter>,
}

/// An object `Water.Update` adds to the room.
#[derive(Debug, Clone, PartialEq)]
pub enum WaterObject {
    Bubble(Bubble),
    Splash(Splash),
}

/// Whose body a chunk the water reads belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkOwner {
    /// The player's (its splash sounds play on the chunk).
    Player,
    /// A creature's or an item's.
    Entity(Entity),
    /// A coral stem's segment.
    Segment(crate::SegmentRef),
}

/// A body chunk the water's update reads: chunk `index` of `owner`.
#[derive(Debug)]
pub struct SurfaceChunk<'a> {
    pub owner: ChunkOwner,
    pub index: usize,
    pub chunk: &'a mut BodyChunk,
}

impl<'a> SurfaceChunk<'a> {
    /// The player's chunks in order.
    pub fn player(chunks: &'a mut [BodyChunk]) -> impl Iterator<Item = SurfaceChunk<'a>> {
        chunks
            .iter_mut()
            .enumerate()
            .map(|(index, chunk)| SurfaceChunk {
                owner: ChunkOwner::Player,
                index,
                chunk,
            })
    }
}

/// `Water.BubbleEmitter`: a chunk that went under trails bubbles while `amount` lasts.
#[derive(Debug, Clone, PartialEq)]
pub struct BubbleEmitter {
    pub owner: ChunkOwner,
    pub index: usize,
    /// The chunk as last read: the game keeps reading a chunk that left the water's
    /// update (its body left the room); the port keeps where it was (D11).
    pub pos: Vec2,
    pub vel: Vec2,
    pub rad: f32,
    pub amount: f32,
    pub decrease: f32,
}

impl BubbleEmitter {
    /// `Update`: `amount` drops by `decrease`, then `value * 40 < amount` makes a
    /// bubble within the chunk moving with it (four more draws).
    fn update(&mut self, rng: &mut UnityRandom) -> Option<Bubble> {
        self.amount -= self.decrease;
        if rng.value() * 40.0 >= self.amount {
            return None;
        }
        let dir = Vec2::from_degrees(rng.value() * 360.0);
        let pos = self.pos + dir * (rng.value() * self.rad);
        Some(Bubble::new(pos, self.vel, false, false, rng))
    }

    /// Reads its chunk again when it is among `chunks`.
    fn follow(&mut self, chunks: &[SurfaceChunk<'_>]) {
        if let Some(c) = chunks
            .iter()
            .find(|c| c.owner == self.owner && c.index == self.index)
        {
            self.pos = c.chunk.pos;
            self.vel = c.chunk.vel;
            self.rad = c.chunk.rad;
        }
    }
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
            rumble: SurfaceRumble::default(),
            bubble_emitters: Vec::new(),
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
        self.rumble.active =
            s.danger_type != Some(DangerType::None) && s.danger_type != Some(DangerType::Blizzard);
        self.rumble.intensity = s.rumble_intensity;
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

    /// `SurfaceLeftAndRightBoundries`: the first and last front points' still x.
    #[must_use]
    pub fn surface_bounds(&self) -> (f32, f32) {
        let last = self.points.len() - 1;
        (
            self.points[0][0].default_pos.x,
            self.points[last][0].default_pos.x,
        )
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

    /// `Water.WaterfallHitSurface(sourcePos, width, flow)` on the main surface (the only
    /// one without `AirPocket`s): each front point from the one at or left of `x - width
    /// / 2` to the one past `x + width / 2` gets a kick with chance `flow` (a draw), its
    /// height by `1.9 * Lerp(-1, 0.5, value) * Lerp(0.6, 1, flow)` (down at the two end
    /// points) and its offset `5` px in a random direction (two more draws).
    pub fn waterfall_hit_surface(
        &mut self,
        source: Vec2,
        width: f32,
        flow: f32,
        rng: &mut UnityRandom,
    ) {
        let first = self.previous_point(source.x - width / 2.0);
        let last = (self.previous_point(source.x + width / 2.0) + 1).min(self.points.len() - 1);
        for i in first..=last {
            if rng.value() >= flow {
                continue;
            }
            let edge = if i == first || i == last { -1.0 } else { 1.0 };
            let kick = 1.9 * (-1.0_f32).lerp(0.5, rng.value()) * edge * 0.6_f32.lerp(1.0, flow);
            self.points[i][0].height += kick;
            self.points[i][0].pos += Vec2::from_degrees(rng.value() * 360.0) * 5.0;
        }
    }

    /// The live `BubbleEmitter`s, oldest first.
    #[must_use]
    pub fn bubble_emitters(&self) -> &[BubbleEmitter] {
        &self.bubble_emitters
    }

    /// `Water.Update` (at the start of the room's tick, before any object moves): the
    /// bottom-bubble roll, bodies crossing or moving in the surface push it (with the
    /// splash sound, splashes and bubbles), the bubble emitters, then the surface steps.
    /// `chunks` are the room's physical objects' in `physicalObjects` order (collision
    /// layer, then joining order). Returns the splashes and bubbles it made, in
    /// `AddObject` order.
    pub fn update<'a>(
        &mut self,
        room: &Room,
        chunks: impl IntoIterator<Item = SurfaceChunk<'a>>,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) -> Vec<WaterObject> {
        // `waterSounds.Update()` ran just before (`RainWorldGame::run_water_sound`).
        let mut made: Vec<WaterObject> = self
            .bottom_bubble(room, rng)
            .map(WaterObject::Bubble)
            .into_iter()
            .collect();
        let mut chunks: Vec<SurfaceChunk<'a>> = chunks.into_iter().collect();
        let mut splashes = 0;
        for chunk in &mut chunks {
            self.disturb(chunk, &mut splashes, rng, sounds, &mut made);
        }
        for i in (0..self.bubble_emitters.len()).rev() {
            let e = &mut self.bubble_emitters[i];
            if e.amount <= 0.0 {
                self.bubble_emitters.remove(i);
                continue;
            }
            e.follow(&chunks);
            made.extend(e.update(rng).map(WaterObject::Bubble));
        }
        self.step_surface(room, rng);
        made
    }

    /// The roll at the head of `Water.Update` (two draws every tick): a random tile
    /// below the still surface; on air over a solid tile a growing bottom bubble (three
    /// more draws).
    fn bottom_bubble(&self, room: &Room, rng: &mut UnityRandom) -> Option<Bubble> {
        let x = rng.range_i32(0, room.width());
        let y = rng.range_i32(0, self.level_row);
        if room.tile(x, y).terrain != Terrain::Air || room.tile(x, y - 1).terrain != Terrain::Solid
        {
            return None;
        }
        let pos = Vec2::middle_of_tile((x, y)) + Vec2::new((-10.0).lerp(10.0, rng.value()), -10.0);
        Some(Bubble::new(pos, Vec2::ZERO, true, false, rng))
    }

    /// A chunk passing the surface faster than 3 px/tick splashes it (once per
    /// `splash_stop` window, with `ThroughSurfaceSound`), throws splashes and, going down,
    /// starts a bubble emitter with a burst of bubbles; one straddling it drags the two
    /// points around it. `splashes` is `Water.Update`'s splash and bubble count: from
    /// 10 on, crossing chunks make no more small splashes or bubbles.
    fn disturb(
        &mut self,
        surface: &mut SurfaceChunk<'_>,
        splashes: &mut i32,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
        made: &mut Vec<WaterObject>,
    ) {
        let player_chunk = (surface.owner == ChunkOwner::Player).then_some(surface.index);
        let chunk = &mut *surface.chunk;
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
                Self::through_surface_sound(player_chunk, chunk, rng, sounds);
            }
            // `|vel.y| * rad * mass`, which sizes the splashes.
            let weight = vel.y.abs() * rad * chunk.mass;
            if force.abs() > 5.0 {
                let pos = Vec2::new(chunk.pos.x, level + if force > 0.0 { 20.0 } else { -20.0 });
                let speed = force.abs().lerp_map(10.0, 40.0, 4.0, 14.0);
                let amount = weight.lerp_map(4.0, 300.0, 7.0, 90.0);
                let level = self.level_at(pos.x);
                let splash = Splash::new(pos, Vec2::new(vel.x * 0.5, speed), amount, rad, level);
                made.push(WaterObject::Splash(splash));
                *splashes += 1;
            }
            if *splashes >= 10 {
                return;
            }
            let small = (force * if force < 0.0 { 0.25 } else { 0.55 }).abs() as i32;
            for _ in 0..small {
                let x = chunk.pos.x + (-rad).lerp(rad, rng.value());
                let dir = Vec2::from_degrees(-45.0 + 90.0 * rng.value());
                let push = if force > 0.0 { 0.4 } else { 0.3 };
                let speed = rng.value() * (vel.y.abs() * push + 10.0);
                let pos = Vec2::new(x, level + 5.0);
                let splash_vel = Vec2::new(vel.x, 0.0) + dir * speed;
                let amount = weight.lerp_map(7.0, 200.0, 2.0, 10.0);
                let level = self.level_at(pos.x);
                made.push(WaterObject::Splash(Splash::new(
                    pos, splash_vel, amount, rad, level,
                )));
                *splashes += 1;
            }
            if vel.y < 0.0 && self.bubble_emitters.len() < 10 {
                let amount =
                    ((1.0 + vel.length()) * force.abs().lerp(5.0, 0.75) * 0.2).clamp(3.0, 25.0);
                self.bubble_emitters.push(BubbleEmitter {
                    owner: surface.owner,
                    index: surface.index,
                    pos: chunk.pos,
                    vel,
                    rad,
                    amount,
                    decrease: 0.4,
                });
                for _ in 0..(force * 0.75).abs() as i32 {
                    let x = chunk.pos.x + (-rad).lerp(rad, rng.value());
                    let dir = Vec2::from_degrees(135.0 + 90.0 * rng.value());
                    let speed = rng.value() * ((force * 1.5).abs() + 10.0);
                    let pos = Vec2::new(x, level - 5.0);
                    let bubble = Bubble::new(pos, dir * speed, false, false, rng);
                    made.push(WaterObject::Bubble(bubble));
                    *splashes += 1;
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

    /// `Water.ThroughSurfaceSound`: only going down, a fast and a slow splash on the
    /// chunk, sized by its mass.
    fn through_surface_sound(
        index: Option<usize>,
        chunk: &BodyChunk,
        rng: &mut UnityRandom,
        sounds: &mut Sounds,
    ) {
        let vy = chunk.vel.y;
        if vy >= -1.0 {
            return;
        }
        let slow = vy.inverse_lerp(-1.0, -50.0).sin();
        let fast = vy.inverse_lerp(-30.0, -60.0);
        let (fast_id, slow_id) = if chunk.mass < 0.1 {
            (
                SoundId::SmallObjectIntoWaterFast,
                SoundId::SmallObjectIntoWaterSlow,
            )
        } else if chunk.mass < 0.8 {
            (
                SoundId::MediumObjectIntoWaterFast,
                SoundId::MediumObjectIntoWaterSlow,
            )
        } else {
            (
                SoundId::LargeObjectIntoWaterFast,
                SoundId::LargeObjectIntoWaterSlow,
            )
        };
        for (id, volume) in [(fast_id, fast), (slow_id, slow)] {
            match index {
                Some(i) => sounds.play_on_chunk(id, i, false, volume, 1.0, rng),
                None => sounds.play_on_object(id, chunk.pos, false, volume, 1.0, rng),
            };
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
            if self.rumble.active {
                next += (-1.0_f32).lerp(1.0, rng.value())
                    * self.rumble.screen_shake
                    * self.rumble.intensity;
            }
            self.points[i][0].next_height = next;
            sum += h;
            for j in 0..2 {
                self.place_point(i, j, rng);
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
    fn place_point(&mut self, i: usize, j: usize, rng: &mut UnityRandom) {
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
        let rumble = if self.rumble.active {
            Vec2::from_degrees(rng.value() * 360.0)
                * (self.rumble.micro_screen_shake * 4.0 * self.rumble.intensity)
        } else {
            Vec2::ZERO
        };
        let roll_back =
            Vec2::from_degrees((phase + self.sin_counter * back) * 360.0 * self.roll_back_length)
                * (self.wave_amplitude * self.roll_back_amp);
        let level = self.level;
        let p = &mut self.points[i][j];
        p.last_pos = p.pos;
        p.last_default_pos = p.default_pos;
        p.pos = Vec2::new(0.0, sum / count) + wave + rumble + roll_back;
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

pub(crate) mod drips_turn;
#[cfg(test)]
mod tests;
pub(crate) mod waterfall_turn;
pub(crate) mod world;
