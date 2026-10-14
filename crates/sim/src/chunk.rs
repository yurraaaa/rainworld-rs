//! A single circular body chunk: integration and tile collision (`BodyChunk.Update`).
//!
//! Behaviour: `rainworld-re/notes/physics-core.md` sections 3-4 and
//! `slope-collision.md` section 3. Water, burrowing, terrain curves, `setPos` and the
//! `TerrainImpact` callback are not ported.

use rw_assets::{Room, Slope, TILE_SIZE, Terrain, tile_centre, tile_position};

use crate::Vec2;
use crate::room_ext::RoomExt;

/// Default `restrictInRoomRange`: how far outside the room (px) a chunk may go.
pub const DEFAULT_RESTRICT_IN_ROOM_RANGE: f32 = 1200.0;

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
    /// How far outside the room the chunk is allowed (px).
    pub restrict_in_room_range: f32,
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
            restrict_in_room_range: DEFAULT_RESTRICT_IN_ROOM_RANGE,
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

    /// Moves the chunk to `pos` with no history and no velocity (`HardSetPosition` + zero vel).
    pub fn reset_at(&mut self, pos: Vec2) {
        self.pos = pos;
        self.last_pos = pos;
        self.last_last_pos = pos;
        self.vel = Vec2::ZERO;
    }

    /// First half of `BodyChunk.Update`: gravity, air drag, history shift, `pos += vel`
    /// and the per-tick resets. Collision follows in [`BodyChunk::collide`].
    pub(crate) fn integrate(&mut self, p: &PhysicsParams) {
        if self.vel.y.is_nan() {
            self.vel.y = 0.0;
        }
        if self.vel.x.is_nan() {
            self.vel.x = 0.0;
        }
        self.vel.y -= p.gravity;
        self.vel *= p.air_friction;
        self.last_last_pos = self.last_pos;
        self.last_pos = self.pos;
        self.pos += self.vel;
        self.on_slope = 0;
        self.last_contact = self.contact;
    }

    /// Second half of `BodyChunk.Update`: vertical, slope and horizontal collision,
    /// then the room-range clamp. `lowest_last_y` is the lowest `last_pos.y` of all the
    /// owner's chunks (starting from the room's pixel height), read by one-way floors.
    pub(crate) fn collide(&mut self, room: &Room, p: &PhysicsParams, lowest_last_y: f32) {
        if self.collide_with_terrain {
            self.collide_vertical(room, p, lowest_last_y);
            if self.collide_with_slopes {
                self.collide_slopes(room, p);
            }
            self.collide_horizontal(room, p);
        } else {
            self.contact = Contact::default();
        }
        self.restrict_to_room(room);
    }

    /// The game's `SolidFloor`: Solid, or a one-way Floor that the whole body was above.
    fn is_solid_floor(&self, room: &Room, x: i32, y: i32, lowest_last_y: f32) -> bool {
        match room.tile(x, y).terrain {
            Terrain::Solid => true,
            Terrain::Floor => !self.go_through_floors && tile_y(lowest_last_y) > y,
            _ => false,
        }
    }

    fn collide_vertical(&mut self, room: &Room, p: &PhysicsParams, lowest_last_y: f32) {
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
            'rows: for row in first..=last {
                for col in x0..=x1 {
                    if room.is_solid_tile(col, row)
                        && !room.is_solid_tile(col, row - 1)
                        && (last_ty < row || last_solid)
                    {
                        self.pos.y = row as f32 * TILE_SIZE - r;
                        self.contact.y = 1;
                        self.vel.y = -self.vel.y.abs() * p.bounce;
                        if self.vel.y.abs() < bounce_threshold(p) {
                            self.vel.y = 0.0;
                        }
                        self.vel.x *= friction_factor(p);
                        break 'rows;
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        break 'rows;
                    }
                }
            }
        } else if self.pos.y < self.last_pos.y {
            let first = tile_y(self.last_pos.y - r);
            let last = tile_y(self.pos.y - r - 0.01);
            'rows: for row in (last..=first).rev() {
                for col in x0..=x1 {
                    if self.is_solid_floor(room, col, row, lowest_last_y)
                        && !self.is_solid_floor(room, col, row + 1, lowest_last_y)
                        && (last_ty > row || last_solid)
                    {
                        self.pos.y = (row as f32 + 1.0) * TILE_SIZE + r;
                        self.contact.y = -1;
                        self.vel.y = self.vel.y.abs() * p.bounce;
                        if self.vel.y < p.gravity || self.vel.y < bounce_threshold(p) {
                            self.vel.y = 0.0;
                        }
                        self.vel.x *= friction_factor(p);
                        break 'rows;
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        break 'rows;
                    }
                }
            }
        }
    }

    fn collide_horizontal(&mut self, room: &Room, p: &PhysicsParams) {
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
            'cols: for col in first..=last {
                for row in y0..=y1 {
                    if room.is_solid_tile(col, row)
                        && !room.is_solid_tile(col - 1, row)
                        && (last_tx < col || last_solid)
                    {
                        self.pos.x = col as f32 * TILE_SIZE - r;
                        self.contact.x = 1;
                        self.vel.x = -self.vel.x.abs() * p.bounce;
                        if self.vel.x.abs() < bounce_threshold(p) {
                            self.vel.x = 0.0;
                        }
                        self.vel.y *= friction_factor(p);
                        break 'cols;
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        break 'cols;
                    }
                }
            }
        } else if self.vel.x < 0.0 {
            let first = tile_x(self.last_pos.x - r);
            let last = tile_x(self.pos.x - r - 0.01);
            'cols: for col in (last..=first).rev() {
                for row in y0..=y1 {
                    if room.is_solid_tile(col, row)
                        && !room.is_solid_tile(col + 1, row)
                        && (last_tx > col || last_solid)
                    {
                        self.pos.x = (col as f32 + 1.0) * TILE_SIZE + r;
                        self.contact.x = -1;
                        self.vel.x = self.vel.x.abs() * p.bounce;
                        if self.vel.x.abs() < bounce_threshold(p) {
                            self.vel.x = 0.0;
                        }
                        self.vel.y *= friction_factor(p);
                        break 'cols;
                    }
                    checks += 1;
                    if checks > MAX_REPEATS {
                        break 'cols;
                    }
                }
            }
        }
    }

    /// `checkAgainstSlopesVertically`: lift onto (or push below) a 45 degree slope line.
    fn collide_slopes(&mut self, room: &Room, p: &PhysicsParams) {
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

    fn restrict_to_room(&mut self, room: &Room) {
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

/// Speed below which a bounce is cancelled: `1 + 9 * (1 - bounce)`.
fn bounce_threshold(p: &PhysicsParams) -> f32 {
    1.0 + 9.0 * (1.0 - p.bounce)
}

/// Multiplier on the tangential velocity at a flat-tile hit: `clamp(2 * friction, 0, 1)`.
fn friction_factor(p: &PhysicsParams) -> f32 {
    (p.surface_friction * 2.0).clamp(0.0, 1.0)
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
