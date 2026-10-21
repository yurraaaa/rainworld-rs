//! Tile lookups by [`Vec2`] room-pixel positions on the foreign [`Room`] type.
//!
//! Mirrors: -

use rw_assets::{Room, Slope, Tile};

use crate::Vec2;

/// `Room.GetTile(Vector2)` and friends for [`Vec2`] positions.
pub trait RoomExt {
    /// Tile containing `pos`, clamped at the room edges.
    fn tile_at_pos(&self, pos: Vec2) -> Tile;

    /// Whether the tile containing `pos` is `Solid` (`GetTile(pos).Solid`).
    fn is_solid_at(&self, pos: Vec2) -> bool {
        self.tile_at_pos(pos).is_solid()
    }

    /// Whether the tile at grid `(x, y)` is `Solid` (clamped at the room edges).
    fn is_solid_tile(&self, x: i32, y: i32) -> bool;

    /// Whether the three tiles below `(x - 1..=x + 1, y - 1)` are all Solid.
    fn has_ground_below(&self, x: i32, y: i32) -> bool {
        (-1..=1).all(|dx| self.is_solid_tile(x + dx, y - 1))
    }

    /// Slope of the tile at `(tx, ty)` shifted by `(dx, dy)` (`Room.IdentifySlope`).
    fn slope_near(&self, tx: i32, ty: i32, offset: (i32, i32)) -> Option<Slope>;

    /// `Room.VisualContact(a, b)`: neither end Solid and no Solid tile at each 20 px step
    /// between them (no terrain curves in the base game).
    fn visual_contact(&self, a: Vec2, b: Vec2) -> bool {
        if self.is_solid_at(a) || self.is_solid_at(b) {
            return false;
        }
        let dist = a.distance(b);
        let mut step = 20.0;
        while step < dist {
            if self.is_solid_at(a.lerp(b, step / dist)) {
                return false;
            }
            step += 20.0;
        }
        true
    }

    /// `Room.RayTraceTilesForTerrain`: no Solid tile on the line of tiles from `a` to
    /// `b`, ends included.
    fn ray_trace_tiles_for_terrain(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (mut dx, mut dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
        let (mut x, mut y) = a;
        let mut n = 1 + dx + dy;
        let sx = if b.0 > a.0 { 1 } else { -1 };
        let sy = if b.1 > a.1 { 1 } else { -1 };
        let mut err = dx - dy;
        dx *= 2;
        dy *= 2;
        while n > 0 {
            if self.is_solid_tile(x, y) {
                return false;
            }
            if err > 0 {
                x += sx;
                err -= dy;
            } else {
                y += sy;
                err += dx;
            }
            n -= 1;
        }
        true
    }

    /// `Room.RayTraceTilesList`: the line of tiles from `a` to `b` written over the
    /// start of `path`; returns their number.
    fn ray_trace_tiles_list(
        &self,
        a: (i32, i32),
        b: (i32, i32),
        path: &mut Vec<(i32, i32)>,
    ) -> usize {
        let (mut dx, mut dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
        let (mut x, mut y) = a;
        let mut n = 1 + dx + dy;
        let sx = if b.0 > a.0 { 1 } else { -1 };
        let sy = if b.1 > a.1 { 1 } else { -1 };
        let mut err = dx - dy;
        dx *= 2;
        dy *= 2;
        let mut i = 0;
        while n > 0 {
            if path.len() <= i {
                path.push((x, y));
            } else {
                path[i] = (x, y);
            }
            i += 1;
            if err > 0 {
                x += sx;
                err -= dy;
            } else {
                y += sy;
                err += dx;
            }
            n -= 1;
        }
        i
    }

    /// `Room.VisualContact` between two tiles' middles.
    fn visual_contact_tiles(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        self.visual_contact(Vec2::middle_of_tile(a), Vec2::middle_of_tile(b))
    }
}

impl RoomExt for Room {
    fn tile_at_pos(&self, pos: Vec2) -> Tile {
        self.tile_at(pos.to_array())
    }

    fn is_solid_tile(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).is_solid()
    }

    fn slope_near(&self, tx: i32, ty: i32, (dx, dy): (i32, i32)) -> Option<Slope> {
        self.identify_slope(tx + dx, ty + dy)
    }
}
