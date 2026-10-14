//! `BodyPart`: a cosmetic point mass that is pushed out of terrain (base of the
//! tail segments, head, legs and hands).

use rw_assets::{Room, Slope, Terrain, tile_centre};
use rw_random::UnityRandom;
use rw_sim::Vec2;

/// `Custom.eightDirectionsAndZero`: the order terrain push-out and grip search visit tiles.
pub(crate) const EIGHT_DIRECTIONS_AND_ZERO: [(i32, i32); 9] = [
    (0, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// Half a tile, px.
const HALF_TILE: f32 = 10.0;

/// One cosmetic point mass (`BodyPart`).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPart {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub rad: f32,
    /// Velocity kept along a surface on terrain contact.
    pub surface_fric: f32,
    pub air_friction: f32,
    /// Touched terrain in the last [`BodyPart::push_out_of_terrain`].
    pub terrain_contact: bool,
}

/// Arguments of [`BodyPart::connect_to_point`] (`BodyPart.ConnectToPoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointConnection {
    pub point: Vec2,
    /// Allowed distance from `point`.
    pub rad: f32,
    /// Also correct when closer than `rad`.
    pub push: bool,
    /// Fraction of the distance added to the velocity toward `point`.
    pub elastic: f32,
    pub host_vel: Vec2,
    /// How much of the velocity relative to `host_vel` is removed.
    pub adapt_vel: f32,
    /// Fraction of `host_vel` added first.
    pub exaggerate_vel: f32,
}

impl BodyPart {
    /// A part at the origin; place it with [`BodyPart::reset`].
    #[must_use]
    pub fn new(rad: f32, surface_fric: f32, air_friction: f32) -> Self {
        Self {
            pos: Vec2::ZERO,
            last_pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            rad,
            surface_fric,
            air_friction,
            terrain_contact: false,
        }
    }

    /// `BodyPart.Reset`: at `point` plus a random 1 px offset, at rest.
    pub fn reset(&mut self, point: Vec2, rng: &mut UnityRandom) {
        self.pos = point + Vec2::random_unit(rng);
        self.last_pos = self.pos;
        self.vel = Vec2::ZERO;
    }

    /// `GenericBodyPart.Update`: move by `vel`, apply air friction, push out of terrain.
    pub fn update_free(&mut self, room: &Room) {
        self.last_pos = self.pos;
        self.pos += self.vel;
        self.vel *= self.air_friction;
        self.push_out_of_terrain(room);
    }

    /// `BodyPart.ConnectToPoint`.
    pub fn connect_to_point(&mut self, c: PointConnection) {
        let dist = self.pos.distance(c.point);
        if c.elastic > 0.0 {
            self.vel += self.pos.direction_to(c.point) * (dist * c.elastic);
        }
        self.vel += c.host_vel * c.exaggerate_vel;
        if c.push || dist >= c.rad {
            let v = self.pos.direction_to(c.point) * (c.rad - dist);
            self.pos -= v;
            self.vel -= v;
        }
        self.vel -= c.host_vel;
        self.vel *= 1.0 - c.adapt_vel;
        self.vel += c.host_vel;
    }

    /// `BodyPart.PushOutOfTerrain` without the burrow-terrain snap (no `room.terrain`).
    ///
    /// Visits the 9 tiles around `pos` in [`EIGHT_DIRECTIONS_AND_ZERO`] order, the tile
    /// of `pos` re-read each time since earlier pushes move it. Solid tiles push out
    /// along the axis whose neighbour is free (side chosen by `last_pos`), else away
    /// from the nearest corner point; slopes straight above or below (and the own
    /// tile) lift or lower `pos` onto the diagonal.
    pub fn push_out_of_terrain(&mut self, room: &Room) {
        self.terrain_contact = false;
        for (dx, dy) in EIGHT_DIRECTIONS_AND_ZERO {
            let (px, py) = self.pos.tile();
            let (tx, ty) = (px + dx, py + dy);
            match room.tile(tx, ty).terrain {
                Terrain::Solid => self.push_out_of_solid(room, tx, ty),
                Terrain::Slope if dx == 0 => self.push_out_of_slope(room, tx, ty),
                _ => {}
            }
        }
    }

    /// The Solid-tile case of [`BodyPart::push_out_of_terrain`].
    fn push_out_of_solid(&mut self, room: &Room, tx: i32, ty: i32) {
        let mid = Vec2::from(tile_centre(tx, ty));
        let (mut push_x, mut push_y) = (0.0, 0.0);
        if self.pos.y >= mid.y - HALF_TILE && self.pos.y <= mid.y + HALF_TILE {
            if self.last_pos.x < mid.x {
                if self.pos.x > mid.x - HALF_TILE - self.rad && !room.tile(tx - 1, ty).is_solid() {
                    push_x = mid.x - HALF_TILE - self.rad;
                }
            } else if self.pos.x < mid.x + HALF_TILE + self.rad && !room.tile(tx + 1, ty).is_solid()
            {
                push_x = mid.x + HALF_TILE + self.rad;
            }
        }
        if self.pos.x >= mid.x - HALF_TILE && self.pos.x <= mid.x + HALF_TILE {
            if self.last_pos.y < mid.y {
                if self.pos.y > mid.y - HALF_TILE - self.rad && !room.tile(tx, ty - 1).is_solid() {
                    push_y = mid.y - HALF_TILE - self.rad;
                }
            } else if self.pos.y < mid.y + HALF_TILE + self.rad && !room.tile(tx, ty + 1).is_solid()
            {
                push_y = mid.y + HALF_TILE + self.rad;
            }
        }
        // The game sets the velocity to `push - pos` after moving `pos`, i.e. to 0.
        if (self.pos.x - push_x).abs() < (self.pos.y - push_y).abs() && push_x != 0.0 {
            self.pos.x = push_x;
            self.vel.x = 0.0;
            self.vel.y *= self.surface_fric;
            self.terrain_contact = true;
            return;
        }
        if push_y != 0.0 {
            self.pos.y = push_y;
            self.vel.y = 0.0;
            self.vel.x *= self.surface_fric;
            self.terrain_contact = true;
            return;
        }
        let corner = Vec2::new(
            self.pos.x.clamp(mid.x - HALF_TILE, mid.x + HALF_TILE),
            self.pos.y.clamp(mid.y - HALF_TILE, mid.y + HALF_TILE),
        );
        let dist = self.pos.distance(corner);
        if dist < self.rad {
            let dir = self.pos.direction_to(corner);
            self.vel *= self.surface_fric;
            self.pos -= dir * (self.rad - dist);
            self.vel -= dir * (self.rad - dist);
            self.terrain_contact = true;
        }
    }

    /// The Slope-tile case of [`BodyPart::push_out_of_terrain`].
    fn push_out_of_slope(&mut self, room: &Room, tx: i32, ty: i32) {
        let mid = Vec2::from(tile_centre(tx, ty));
        let rising = mid.y + (mid.x - self.pos.x);
        let falling = mid.y - (mid.x - self.pos.x);
        let target = match room.identify_slope(tx, ty) {
            Some(Slope::UpLeft) if self.pos.y < falling + self.rad => falling + self.rad,
            Some(Slope::UpRight) if self.pos.y < rising + self.rad => rising + self.rad,
            Some(Slope::DownLeft) if self.pos.y > rising - self.rad => rising - self.rad,
            Some(Slope::DownRight) if self.pos.y > falling - self.rad => falling - self.rad,
            _ => return,
        };
        self.pos.y = target;
        self.vel.y = 0.0;
        self.vel.x *= self.surface_fric;
        self.terrain_contact = true;
    }
}
