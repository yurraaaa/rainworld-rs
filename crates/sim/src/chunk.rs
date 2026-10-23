//! A single circular body chunk: integration and tile collision (`BodyChunk.Update`).
//!
//! Behaviour: `rainworld-re/notes/physics-core.md` sections 3-4 and
//! `slope-collision.md` section 3, impact reports `falls.md` section 1, water
//! `water.md` section 2. Burrowing, terrain curves and `setPos` are not ported.
//!
//! Mirrors: BodyChunk.cs (Update)

use rw_assets::{Room, Slope, TILE_SIZE, Terrain, tile_centre, tile_position};

use crate::float_ext::FloatExt;
use crate::room_ext::RoomExt;
use crate::{Vec2, Water};

/// Default `restrictInRoomRange`: how far outside the room (px) a chunk may go.
pub const DEFAULT_RESTRICT_IN_ROOM_RANGE: f32 = 1200.0;

/// `EffectiveRoomGravity`: no gravity effects are ported.
const ROOM_GRAVITY: f32 = 1.0;

/// The game's emergency limit on tile checks per collision sweep.
const MAX_REPEATS: u32 = 100_000;

/// Owner-level scalars a chunk update reads (`PhysicalObject.gravity` etc.).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsParams {
    /// Subtracted from `vel.y` every tick (px/tick^2).
    pub gravity: f32,
    /// Velocity multiplier per tick in air.
    pub air_friction: f32,
    /// Fraction of speed kept on a terrain bounce.
    pub bounce: f32,
    /// Tangential friction on terrain contact (0.5 for a conscious player).
    pub surface_friction: f32,
    /// A flat-tile hit faster than this along its axis is reported (`impactTreshhold`).
    pub impact_threshold: f32,
    /// Upward push per tick at full submersion, times room gravity.
    pub buoyancy: f32,
    /// Velocity multiplier per tick fully under water (below 10 px/tick).
    pub water_friction: f32,
    /// How much of the extra drag on fast chunks under water is avoided (0..=1).
    pub water_retardation_immunity: f32,
}

/// A flat-tile hit reported to the owner (`PhysicalObject.TerrainImpact`).
///
/// Found before the bounce, from `vel` after this tick's gravity and drag. Slopes
/// never report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainImpact {
    /// Index of the chunk in its owner.
    pub chunk: usize,
    /// Side the surface is on: floor (0, -1), ceiling (0, 1), walls (-1, 0) / (1, 0).
    pub direction: Contact,
    /// Absolute velocity along `direction` (px/tick).
    pub speed: f32,
    /// The chunk did not touch that side on the previous tick.
    pub first_contact: bool,
}

/// Contact direction on each axis: -1, 0 or +1 (`IntVector2 contactPoint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Contact {
    /// -1 touching a wall on the left, +1 on the right.
    pub x: i8,
    /// -1 resting on a floor or floor slope, +1 against a ceiling.
    pub y: i8,
}

/// One circular body part.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyChunk {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub last_last_pos: Vec2,
    pub vel: Vec2,
    pub rad: f32,
    pub mass: f32,
    /// Scales `rad` for terrain collision (1.0 for everything ported so far).
    pub terrain_squeeze: f32,
    pub collide_with_terrain: bool,
    pub collide_with_slopes: bool,
    /// Ignore one-way `Floor` tiles.
    pub go_through_floors: bool,
    /// Pushed by other objects in the room's collision pass (`collideWithObjects`).
    pub collide_with_objects: bool,
    /// How far outside the room the chunk is allowed (px).
    pub restrict_in_room_range: f32,
    /// Ticks before this chunk can splash the water surface again (`splashStop`).
    pub splash_stop: i32,
    /// `setPos`: taken instead of `pos += vel` at the next update.
    pub set_pos: Option<Vec2>,
    /// How loud its terrain impacts are (`loudness`, 1 by default).
    pub loudness: f32,
    contact: Contact,
    last_contact: Contact,
    on_slope: i8,
}

impl BodyChunk {
    /// A chunk at rest at `pos` (the game's constructor defaults).
    #[must_use]
    pub fn new(pos: Vec2, rad: f32, mass: f32) -> Self {
        Self {
            pos,
            last_pos: pos,
            last_last_pos: pos,
            vel: Vec2::ZERO,
            rad,
            mass,
            terrain_squeeze: 1.0,
            collide_with_terrain: true,
            collide_with_slopes: true,
            go_through_floors: false,
            collide_with_objects: true,
            restrict_in_room_range: DEFAULT_RESTRICT_IN_ROOM_RANGE,
            splash_stop: 0,
            set_pos: None,
            loudness: 1.0,
            contact: Contact::default(),
            last_contact: Contact::default(),
            on_slope: 0,
        }
    }

    /// Terrain contact found by this tick's collision (`ContactPoint`).
    #[must_use]
    pub fn contact(&self) -> Contact {
        self.contact
    }

    /// Contact of the previous tick (`lastContactPoint`).
    #[must_use]
    pub fn last_contact(&self) -> Contact {
        self.last_contact
    }

    /// x direction of the downhill side of the floor slope touched this tick, or 0.
    #[must_use]
    pub fn on_slope(&self) -> i8 {
        self.on_slope
    }

    /// Collision radius: `max(rad * terrain_squeeze, 1)`.
    #[must_use]
    pub fn terrain_rad(&self) -> f32 {
        (self.rad * self.terrain_squeeze).max(1.0)
    }

    /// `HardSetPosition`: moves the chunk to `pos` with no history; the velocity stays.
    /// A pending `setPos` (a move from outside the chunk's update) now lands on `pos`.
    pub fn hard_set_position(&mut self, pos: Vec2) {
        self.pos = pos;
        self.last_pos = pos;
        self.last_last_pos = pos;
        if self.set_pos.is_some() {
            self.set_pos = Some(pos);
        }
    }

    /// `MoveFromOutsideMyUpdate`: an owner that already updated with this `eu` is moved
    /// now, else at its next update (`setPos`).
    pub fn move_from_outside_my_update(&mut self, owner_even_update: bool, eu: bool, to: Vec2) {
        if owner_even_update == eu {
            self.pos = to;
        } else {
            self.set_pos = Some(to);
        }
    }

    /// `PhysicalObject.PushOutOf` for this chunk: out of the circle at `pos`, the
    /// overlap taken off its position and velocity.
    pub fn push_out_of(&mut self, pos: Vec2, rad: f32) {
        if !self.pos.dist_less(pos, rad + self.rad) {
            return;
        }
        let dist = self.pos.distance(pos);
        let dir = self.pos.direction_to(pos);
        let d = dir * (rad + self.rad - dist);
        self.pos -= d;
        self.vel -= d;
    }

    /// Moves the chunk to `pos` with no history and no velocity (`HardSetPosition` + zero vel).
    pub fn reset_at(&mut self, pos: Vec2) {
        self.pos = pos;
        self.last_pos = pos;
        self.last_last_pos = pos;
        self.vel = Vec2::ZERO;
    }

    /// First half of `BodyChunk.Update`: gravity, water or air drag, history shift,
    /// `pos += vel` (or `setPos`) and the per-tick resets. [`crate::PhysicalObject`] then runs the
    /// collision stages.
    pub(crate) fn integrate(&mut self, p: &PhysicsParams, water: Option<&Water>) {
        if self.vel.y.is_nan() {
            self.vel.y = 0.0;
        }
        if self.vel.x.is_nan() {
            self.vel.x = 0.0;
        }
        self.vel.y -= p.gravity;
        // The base game's test: the chunk's bottom at or below the surface.
        match water {
            Some(w) if self.pos.y - self.rad <= w.level_at(self.pos.x) => {
                self.water_drag(p, w.submersion(self));
            }
            _ => self.vel *= p.air_friction,
        }
        self.last_last_pos = self.last_pos;
        self.last_pos = self.pos;
        match self.set_pos.take() {
            Some(p) => self.pos = p,
            None => self.pos += self.vel,
        }
        self.on_slope = 0;
        self.last_contact = self.contact;
    }

    /// In water: a fast, shallow, downward skim along the surface bounces up
    /// (stone skipping); otherwise buoyancy, then drag blended from air to water
    /// friction by `submersion`, harder on chunks faster than 10 px/tick unless immune.
    fn water_drag(&mut self, p: &PhysicsParams, submersion: f32) {
        let v = self.vel;
        if v.x > v.y * 5.0 && v.x.abs() > 10.0 && v.y < 0.0 && submersion < 0.5 {
            self.vel.y *= -0.5;
            self.vel.x *= 0.75;
            return;
        }
        self.vel.y += p.buoyancy * ROOM_GRAVITY * submersion;
        let fast = (1.0 / (self.vel.length() - 10.0).max(1.0)).powf(0.5);
        let water = (p.water_friction * p.water_retardation_immunity).lerp(p.water_friction, fast);
        self.vel *= p.air_friction.lerp(water, submersion);
    }

    /// End of `BodyChunk.Update`: a fresh splash window (10) waits until the chunk is
    /// fully in or out of the water; after that it counts down every tick.
    pub(crate) fn count_splash_stop(&mut self, water: Option<&Water>) {
        if self.splash_stop == 10 {
            let s = self.submersion(water);
            if s == 0.0 || s == 1.0 {
                self.splash_stop -= 1;
            }
        } else if self.splash_stop > 0 {
            self.splash_stop -= 1;
        }
    }

    /// `CheckVerticalCollision` up to the hit: sweeps the rows passed this tick and snaps
    /// onto the first floor or ceiling found, returning its side. The owner is told of
    /// the hit ([`BodyChunk::impact`]) before [`BodyChunk::bounce_off`] finishes it.
    /// `lowest_last_y` is the lowest `last_pos.y` of all the owner's chunks (starting
    /// from the room's pixel height), read by one-way floors.
    pub(crate) fn sweep_vertical(&mut self, room: &Room, lowest_last_y: f32) -> Option<Contact> {
        self.contact.y = 0;
        let r = self.terrain_rad();
        let (_, last_ty) = tile_of(self.last_pos);
        let last_solid = room.tile_at(self.last_pos.to_array()).is_solid();
        let x0 = tile_x(self.pos.x - r + 1.0);
        let x1 = tile_x(self.pos.x + r - 1.0);
        let mut checks = 0;

        if self.pos.y > self.last_pos.y {
            let first = tile_y(self.last_pos.y + r);
            let last = tile_y(self.pos.y + r + 0.01);
            for row in first..=last {
                for col in x0..=x1 {
                    if room.is_solid_tile(col, row)
                        && !room.is_solid_tile(col, row - 1)
                        && (last_ty < row || last_solid)
                    {
                        self.pos.y = row as f32 * TILE_SIZE - r;
                        return Some(Contact { x: 0, y: 1 });
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        return None;
                    }
                }
            }
        } else if self.pos.y < self.last_pos.y {
            let first = tile_y(self.last_pos.y - r);
            let last = tile_y(self.pos.y - r - 0.01);
            for row in (last..=first).rev() {
                for col in x0..=x1 {
                    if self.is_solid_floor(room, col, row, lowest_last_y)
                        && !self.is_solid_floor(room, col, row + 1, lowest_last_y)
                        && (last_ty > row || last_solid)
                    {
                        self.pos.y = (row as f32 + 1.0) * TILE_SIZE + r;
                        return Some(Contact { x: 0, y: -1 });
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        return None;
                    }
                }
            }
        }
        None
    }

    /// `CheckHorizontalCollision` up to the hit: as [`BodyChunk::sweep_vertical`] for
    /// walls, by the direction of `vel.x`.
    pub(crate) fn sweep_horizontal(&mut self, room: &Room) -> Option<Contact> {
        self.contact.x = 0;
        let r = self.terrain_rad();
        let (last_tx, _) = tile_of(self.last_pos);
        let last_solid = room.tile_at(self.last_pos.to_array()).is_solid();
        let y0 = tile_y(self.pos.y - r + 1.0);
        let y1 = tile_y(self.pos.y + r - 1.0);
        let mut checks = 0;

        if self.vel.x > 0.0 {
            let first = tile_x(self.last_pos.x + r);
            let last = tile_x(self.pos.x + r + 0.01);
            for col in first..=last {
                for row in y0..=y1 {
                    if room.is_solid_tile(col, row)
                        && !room.is_solid_tile(col - 1, row)
                        && (last_tx < col || last_solid)
                    {
                        self.pos.x = col as f32 * TILE_SIZE - r;
                        return Some(Contact { x: 1, y: 0 });
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        return None;
                    }
                }
            }
        } else if self.vel.x < 0.0 {
            let first = tile_x(self.last_pos.x - r);
            let last = tile_x(self.pos.x - r - 0.01);
            for col in (last..=first).rev() {
                for row in y0..=y1 {
                    if room.is_solid_tile(col, row)
                        && !room.is_solid_tile(col + 1, row)
                        && (last_tx > col || last_solid)
                    {
                        self.pos.x = (col as f32 + 1.0) * TILE_SIZE + r;
                        return Some(Contact { x: -1, y: 0 });
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        return None;
                    }
                }
            }
        }
        None
    }

    /// The `TerrainImpact` call for a hit on `side`, if the velocity into it exceeds the
    /// threshold. Read before the bounce.
    pub(crate) fn impact(
        &self,
        side: Contact,
        p: &PhysicsParams,
        index: usize,
    ) -> Option<TerrainImpact> {
        let speed = f32::from(side.x) * self.vel.x + f32::from(side.y) * self.vel.y;
        if speed <= p.impact_threshold {
            return None;
        }
        let last = self.last_contact;
        let first_contact = if side.y != 0 {
            last.y != side.y
        } else {
            last.x != side.x
        };
        Some(TerrainImpact {
            chunk: index,
            direction: side,
            speed,
            first_contact,
        })
    }

    /// The rest of a flat-tile hit on `side`: contact, bounce (cancelled below
    /// `1 + 9 * (1 - bounce)`, and below gravity on floors) and tangential friction.
    pub(crate) fn bounce_off(&mut self, side: Contact, p: &PhysicsParams) {
        let threshold = 1.0 + 9.0 * (1.0 - p.bounce);
        let friction = (p.surface_friction * 2.0).clamp(0.0, 1.0);
        if side.y != 0 {
            self.contact.y = side.y;
            self.vel.y = -f32::from(side.y) * self.vel.y.abs() * p.bounce;
            let small = if side.y < 0 {
                self.vel.y < p.gravity || self.vel.y < threshold
            } else {
                self.vel.y.abs() < threshold
            };
            if small {
                self.vel.y = 0.0;
            }
            self.vel.x *= friction;
        } else {
            self.contact.x = side.x;
            self.vel.x = -f32::from(side.x) * self.vel.x.abs() * p.bounce;
            if self.vel.x.abs() < threshold {
                self.vel.x = 0.0;
            }
            self.vel.y *= friction;
        }
    }

    /// A chunk's contacts as a test needs them.
    #[cfg(test)]
    pub(crate) fn set_contacts(&mut self, contact: Contact, last_contact: Contact) {
        self.contact = contact;
        self.last_contact = last_contact;
    }

    /// Clears the contact of a chunk that does not collide with terrain.
    pub(crate) fn clear_contact(&mut self) {
        self.contact = Contact::default();
    }

    /// The game's `SolidFloor`: Solid, or a one-way Floor that the whole body was above.
    fn is_solid_floor(&self, room: &Room, x: i32, y: i32, lowest_last_y: f32) -> bool {
        match room.tile(x, y).terrain {
            Terrain::Solid => true,
            Terrain::Floor => !self.go_through_floors && tile_y(lowest_last_y) > y,
            _ => false,
        }
    }

    /// `checkAgainstSlopesVertically`: lift onto (or push below) a 45 degree slope line.
    pub(crate) fn collide_slopes(&mut self, room: &Room, p: &PhysicsParams) {
        // `slopeRad`; the game's later `slopeRad = TerrainRad - 1` write is dead.
        let r = self.terrain_rad();
        let (tx, ty) = tile_of(self.pos);
        let [mid_x, mid_y] = tile_centre(tx, ty);
        let mut found = room.identify_slope(tx, ty).map(|s| (s, (0, 0)));
        if room.tile(tx, ty).terrain != Terrain::Slope {
            // Priority: left, right, below, above, each gated by the chunk's extent
            // reaching that edge of its tile. The "below" branch does not fall through
            // to "above" when its slope is broken.
            found = if let Some(s) = room
                .slope_near(tx, ty, (-1, 0))
                .filter(|_| self.pos.x - r <= mid_x - 10.0)
            {
                Some((s, (-1, 0)))
            } else if let Some(s) = room
                .slope_near(tx, ty, (1, 0))
                .filter(|_| self.pos.x + r >= mid_x + 10.0)
            {
                Some((s, (1, 0)))
            } else if self.pos.y - r < mid_y - 10.0 {
                room.slope_near(tx, ty, (0, -1)).map(|s| (s, (0, -1)))
            } else if self.pos.y + r > mid_y + 10.0 {
                room.slope_near(tx, ty, (0, 1)).map(|s| (s, (0, 1)))
            } else {
                None
            };
        }
        let Some((slope, (dx, dy))) = found else {
            return;
        };

        // Bottom-left corner of the slope tile; the surface line extends past its edges.
        let [cx, cy] = tile_centre(tx + dx, ty + dy);
        let (bx, by) = (cx - 10.0, cy - 10.0);
        let lx = self.pos.x - bx;
        match slope {
            Slope::UpLeft => self.rest_on_slope(lx + by, -1, r, p),
            Slope::UpRight => self.rest_on_slope(20.0 - lx + by, 1, r, p),
            Slope::DownLeft => self.hang_under_slope(20.0 - lx + by, r, p),
            Slope::DownRight => self.hang_under_slope(lx + by, r, p),
        }
    }

    fn rest_on_slope(&mut self, surface: f32, downhill: i8, r: f32, p: &PhysicsParams) {
        let rest = surface + r + r;
        if self.pos.y <= rest {
            self.pos.y = rest;
            self.contact.y = -1;
            self.vel.x *= 1.0 - p.surface_friction;
            self.vel.x += self.vel.y.abs()
                * (0.5 - p.surface_friction).clamp(0.0, 0.5)
                * f32::from(downhill)
                * 0.2;
            self.vel.y = 0.0;
            self.on_slope = downhill;
        }
    }

    fn hang_under_slope(&mut self, surface: f32, r: f32, p: &PhysicsParams) {
        let limit = surface - r - r;
        if self.pos.y >= limit {
            self.pos.y = limit;
            self.contact.y = 1;
            self.vel.y = 0.0;
            self.vel.x *= 1.0 - p.surface_friction;
        }
    }

    pub(crate) fn restrict_to_room(&mut self, room: &Room) {
        let range = self.restrict_in_room_range;
        if self.pos.x < -range {
            self.vel.x = 0.0;
            self.pos.x = -range;
        } else if self.pos.x > room.pixel_width() + range {
            self.vel.x = 0.0;
            self.pos.x = room.pixel_width() + range;
        }
        if self.pos.y < -range {
            self.vel.y = 0.0;
            self.pos.y = -range;
        } else if self.pos.y > room.pixel_height() + range {
            self.vel.y = 0.0;
            self.pos.y = room.pixel_height() + range;
        }
    }
}

fn tile_of(pos: Vec2) -> (i32, i32) {
    tile_position(pos.to_array())
}

fn tile_x(x: f32) -> i32 {
    tile_position([x, 0.0]).0
}

fn tile_y(y: f32) -> i32 {
    tile_position([0.0, y]).1
}
