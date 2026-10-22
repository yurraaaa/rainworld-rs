//! `SharedPhysics`' point collision with the tile grid (`TerrainCollisionData` through
//! `VerticalCollision`, `SlopesVertically`, `HorizontalCollision`) and its exact ray
//! traces between two room positions, as the cosmetic insects use them. `room.terrain`
//! (terrain curves) is not ported, so its snapping branch never runs (D3).
//!
//! Mirrors: SharedPhysics.cs (TerrainCollisionData, VerticalCollision, SlopesVertically, HorizontalCollision, SolidFloor, ExactTerrainRayTracePos, ExactTerrainRayTrace, RayTraceTilesForTerrainReturnFirstSolid, RayTraceTilesForTerrain), Custom.cs (RectCollision)

use rw_assets::{Room, Slope, Terrain};

use crate::Vec2;

/// `SharedPhysics.TerrainCollisionData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainCollisionData {
    pub pos: Vec2,
    pub last_pos: Vec2,
    pub vel: Vec2,
    pub rad: f32,
    pub contact_point: (i32, i32),
    pub go_through_floors: bool,
}

impl TerrainCollisionData {
    #[must_use]
    pub fn new(pos: Vec2, last_pos: Vec2, vel: Vec2, rad: f32, go_through_floors: bool) -> Self {
        Self {
            pos,
            last_pos,
            vel,
            rad,
            contact_point: (0, 0),
            go_through_floors,
        }
    }

    /// `VerticalCollision`, `SlopesVertically`, then `HorizontalCollision`.
    #[must_use]
    pub fn collide(self, room: &Room) -> Self {
        self.vertical(room).slopes_vertically(room).horizontal(room)
    }

    fn solid(room: &Room, x: i32, y: i32) -> bool {
        room.tile(x, y).terrain == Terrain::Solid
    }

    fn last_pos_solid(&self, room: &Room) -> bool {
        let (x, y) = self.last_pos.tile();
        Self::solid(room, x, y)
    }

    /// `SolidFloor`: Solid, or a one-way floor below a last position above it when not
    /// going through floors.
    fn solid_floor(&self, room: &Room, x: i32, y: i32) -> bool {
        match room.tile(x, y).terrain {
            Terrain::Solid => true,
            Terrain::Floor if !self.go_through_floors => {
                Vec2::new(0.0, self.last_pos.y).tile().1 > y
            }
            _ => false,
        }
    }

    /// `HorizontalCollision`.
    #[must_use]
    pub fn horizontal(mut self, room: &Room) -> Self {
        let last_tile = self.last_pos.tile();
        let y_top = Vec2::new(0.0, self.pos.y + self.rad - 1.0).tile().1;
        let y_bottom = Vec2::new(0.0, self.pos.y - self.rad + 1.0).tile().1;
        if self.vel.x > 0.0 {
            let x_to = Vec2::new(self.pos.x + self.rad + 0.01, 0.0).tile().0;
            let x_from = Vec2::new(self.last_pos.x + self.rad, 0.0).tile().0;
            for i in x_from..=x_to {
                for j in y_bottom..=y_top {
                    if Self::solid(room, i, j)
                        && !Self::solid(room, i - 1, j)
                        && (last_tile.0 < i || self.last_pos_solid(room))
                    {
                        self.pos.x = i as f32 * 20.0 - self.rad;
                        self.vel.x = 0.0;
                        self.contact_point.0 = 1;
                        return self;
                    }
                }
            }
        } else if self.vel.x < 0.0 {
            let x_to = Vec2::new(self.pos.x - self.rad - 0.01, 0.0).tile().0;
            let x_from = Vec2::new(self.last_pos.x - self.rad, 0.0).tile().0;
            let mut i = x_from;
            while i >= x_to {
                for j in y_bottom..=y_top {
                    if Self::solid(room, i, j)
                        && !Self::solid(room, i + 1, j)
                        && (last_tile.0 > i || self.last_pos_solid(room))
                    {
                        self.pos.x = (i as f32 + 1.0) * 20.0 + self.rad;
                        self.vel.x = 0.0;
                        self.contact_point.0 = -1;
                        return self;
                    }
                }
                i -= 1;
            }
        }
        self
    }

    /// `VerticalCollision` (no terrain curves).
    #[must_use]
    pub fn vertical(mut self, room: &Room) -> Self {
        let last_tile = self.last_pos.tile();
        let x_left = Vec2::new(self.pos.x - self.rad + 1.0, 0.0).tile().0;
        let x_right = Vec2::new(self.pos.x + self.rad - 1.0, 0.0).tile().0;
        if self.vel.y > 0.0 {
            let y_to = Vec2::new(0.0, self.pos.y + self.rad + 0.01).tile().1;
            let y_from = Vec2::new(0.0, self.last_pos.y + self.rad).tile().1;
            for i in y_from..=y_to {
                for j in x_left..=x_right {
                    if Self::solid(room, j, i)
                        && !Self::solid(room, j, i - 1)
                        && (last_tile.1 < i || self.last_pos_solid(room))
                    {
                        self.pos.y = i as f32 * 20.0 - self.rad;
                        self.vel.y = 0.0;
                        self.contact_point.1 = 1;
                        return self;
                    }
                }
            }
        } else if self.vel.y < 0.0 {
            let y_to = Vec2::new(0.0, self.pos.y - self.rad - 0.01).tile().1;
            let y_from = Vec2::new(0.0, self.last_pos.y - self.rad).tile().1;
            let mut i = y_from;
            while i >= y_to {
                for k in x_left..=x_right {
                    if self.solid_floor(room, k, i)
                        && !self.solid_floor(room, k, i + 1)
                        && (last_tile.1 > i || self.last_pos_solid(room))
                    {
                        self.pos.y = (i as f32 + 1.0) * 20.0 + self.rad;
                        self.vel.y = 0.0;
                        self.contact_point.1 = -1;
                        return self;
                    }
                }
                i -= 1;
            }
        }
        self
    }

    /// `SlopesVertically`.
    #[must_use]
    pub fn slopes_vertically(mut self, room: &Room) -> Self {
        let (tx, ty) = self.pos.tile();
        let middle = self.pos.tile_middle();
        let mut offset = (0, 0);
        let mut slope = room.identify_slope(tx, ty);
        if room.tile(tx, ty).terrain != Terrain::Slope {
            if room.identify_slope(tx - 1, ty).is_some() && self.pos.x - self.rad <= middle.x - 10.0
            {
                slope = room.identify_slope(tx - 1, ty);
                offset.0 = -1;
            } else if room.identify_slope(tx + 1, ty).is_some()
                && self.pos.x + self.rad >= middle.x + 10.0
            {
                slope = room.identify_slope(tx + 1, ty);
                offset.0 = 1;
            } else if self.pos.y - self.rad < middle.y - 10.0 {
                if room.identify_slope(tx, ty - 1).is_some() {
                    slope = room.identify_slope(tx, ty - 1);
                    offset.1 = -1;
                }
            } else if self.pos.y + self.rad > middle.y + 10.0
                && room.identify_slope(tx, ty + 1).is_some()
            {
                slope = room.identify_slope(tx, ty + 1);
                offset.1 = 1;
            }
        }
        let Some(slope) = slope else {
            return self;
        };
        let v = Vec2::middle_of_tile((tx + offset.0, ty + offset.1));
        let (h, side) = match slope {
            Slope::UpLeft => (self.pos.x - (v.x - 10.0) + (v.y - 10.0), -1),
            Slope::UpRight => (20.0 - (self.pos.x - (v.x - 10.0)) + (v.y - 10.0), -1),
            Slope::DownLeft => (20.0 - (self.pos.x - (v.x - 10.0)) + (v.y - 10.0), 1),
            Slope::DownRight => (self.pos.x - (v.x - 10.0) + (v.y - 10.0), 1),
        };
        if side == -1 && self.pos.y <= h + self.rad + self.rad {
            self.pos.y = h + self.rad + self.rad;
        } else if side == 1 && self.pos.y >= h - self.rad - self.rad {
            self.pos.y = h - self.rad - self.rad;
        }
        self
    }
}

/// The tiles on the line between two room positions, walked as `SharedPhysics`' exact
/// ray traces walk them (`RayTraceTilesForTerrain(Room, Vector2, Vector2)`).
#[derive(Debug, Clone, Copy)]
pub struct TileWalk {
    x: i32,
    y: i32,
    step_x: i32,
    step_y: i32,
    dt_dx: f32,
    dt_dy: f32,
    next_x: f32,
    next_y: f32,
    left: i32,
}

impl TileWalk {
    #[must_use]
    pub fn new(a: Vec2, b: Vec2) -> Self {
        let (ax, ay, bx, by) = (a.x / 20.0, a.y / 20.0, b.x / 20.0, b.y / 20.0);
        let (dx, dy) = ((bx - ax).abs(), (by - ay).abs());
        let (x, y) = (ax.floor() as i32, ay.floor() as i32);
        let (dt_dx, dt_dy) = (1.0 / dx, 1.0 / dy);
        let mut left = 1;
        let (step_x, next_x) = if dx == 0.0 {
            (0, dt_dx)
        } else if bx > ax {
            left += bx.floor() as i32 - x;
            (1, ((ax.floor() as i32 + 1) as f32 - ax) * dt_dx)
        } else {
            left += x - bx.floor() as i32;
            (-1, (ax - ax.floor() as i32 as f32) * dt_dx)
        };
        let (step_y, next_y) = if dy == 0.0 {
            (0, dt_dy)
        } else if by > ay {
            left += by.floor() as i32 - y;
            (1, ((ay.floor() as i32 + 1) as f32 - ay) * dt_dy)
        } else {
            left += y - by.floor() as i32;
            (-1, (ay - ay.floor() as i32 as f32) * dt_dy)
        };
        Self {
            x,
            y,
            step_x,
            step_y,
            dt_dx,
            dt_dy,
            next_x,
            next_y,
            left,
        }
    }
}

impl Iterator for TileWalk {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<(i32, i32)> {
        if self.left <= 0 {
            return None;
        }
        let tile = (self.x, self.y);
        if self.next_y < self.next_x {
            self.y += self.step_y;
            self.next_y += self.dt_dy;
        } else {
            self.x += self.step_x;
            self.next_x += self.dt_dx;
        }
        self.left -= 1;
        Some(tile)
    }
}

/// The exact ray traces of `SharedPhysics` on a room.
pub trait ExactRayTrace {
    /// `RayTraceTilesForTerrain(room, A, B)`: no Solid tile on the walk.
    fn ray_trace_tiles_for_terrain_exact(&self, a: Vec2, b: Vec2) -> bool;
    /// `ExactTerrainRayTracePos`: where the line from `a` to `b` enters its first Solid
    /// tile (the corner of `RectCollision`'s rectangle), or `None`.
    fn exact_terrain_ray_trace_pos(&self, a: Vec2, b: Vec2) -> Option<Vec2>;
}

impl ExactRayTrace for Room {
    fn ray_trace_tiles_for_terrain_exact(&self, a: Vec2, b: Vec2) -> bool {
        TileWalk::new(a, b).all(|(x, y)| !self.tile(x, y).is_solid())
    }

    fn exact_terrain_ray_trace_pos(&self, a: Vec2, b: Vec2) -> Option<Vec2> {
        let tile = TileWalk::new(a, b).find(|&(x, y)| self.tile(x, y).is_solid())?;
        let m = Vec2::middle_of_tile(tile);
        Some(TileRect::new(m - Vec2::new(10.0, 10.0), m + Vec2::new(10.0, 10.0)).collision(b, a))
    }
}

/// A `FloatRect` of a tile.
#[derive(Debug, Clone, Copy)]
struct TileRect {
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
}

impl TileRect {
    fn new(lower_left: Vec2, upper_right: Vec2) -> Self {
        Self {
            left: lower_left.x,
            bottom: lower_left.y,
            right: upper_right.x,
            top: upper_right.y,
        }
    }

    fn inside(&self, v: Vec2) -> bool {
        v.x > self.left && v.x < self.right && v.y > self.bottom && v.y < self.top
    }

    /// `Custom.RectCollision(pos, lastPos, rect)`'s lower-left corner: the crossing of
    /// the rectangle's edges nearest `last_pos`, or `pos`.
    fn collision(&self, pos: Vec2, last_pos: Vec2) -> Vec2 {
        let left = last_pos.vertical_cross_point(pos, self.left);
        let right = last_pos.vertical_cross_point(pos, self.right);
        let (mut bottom, mut top) = (
            last_pos.horizontal_cross_point(pos, self.bottom),
            last_pos.horizontal_cross_point(pos, self.top),
        );
        if last_pos.x == pos.x {
            bottom = Vec2::new(last_pos.x, self.bottom);
            top = Vec2::new(last_pos.x, self.top);
        }
        let mut dst = if self.inside(last_pos) {
            f32::MAX
        } else {
            last_pos.distance(pos)
        };
        let mut hit = pos;
        for (p, vertical) in [(left, true), (right, true), (bottom, false), (top, false)] {
            let on_edge = if vertical {
                p.y >= self.bottom && p.y <= self.top
            } else {
                p.x >= self.left && p.x <= self.right
            };
            if on_edge && last_pos.dist_less(p, dst) {
                hit = p;
                dst = last_pos.distance(p);
            }
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_along_a_row_counts_every_tile() {
        let tiles: Vec<_> = TileWalk::new(Vec2::new(5.0, 5.0), Vec2::new(65.0, 5.0)).collect();
        assert_eq!(tiles, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn a_diagonal_walk_steps_one_axis_at_a_time() {
        let tiles: Vec<_> = TileWalk::new(Vec2::new(5.0, 5.0), Vec2::new(25.0, 35.0)).collect();
        assert_eq!(tiles, vec![(0, 0), (0, 1), (1, 1)]);
    }

    #[test]
    fn rect_collision_takes_the_entered_edge() {
        let r = TileRect::new(Vec2::new(20.0, 0.0), Vec2::new(40.0, 20.0));
        let hit = r.collision(Vec2::new(30.0, 10.0), Vec2::new(5.0, 10.0));
        assert_eq!(hit, Vec2::new(20.0, 10.0));
    }
}
