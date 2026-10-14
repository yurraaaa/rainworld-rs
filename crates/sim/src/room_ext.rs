//! Tile lookups by [`Vec2`] room-pixel positions on the foreign [`Room`] type.

use rw_assets::{Room, Slope, Tile};

use crate::Vec2;

/// `Room.GetTile(Vector2)` and friends for [`Vec2`] positions.
pub(crate) trait RoomExt {
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
