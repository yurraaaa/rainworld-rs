//! Tile lookups by [`Vec2`] room-pixel positions on the foreign [`Room`] type.

use rw_assets::{Room, Shortcut, Slope, Terrain, Tile};

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

    /// Whether `(x, y)` is part of a shortcut tube: shortcut-flagged or an entrance.
    fn is_in_shortcut_tube(&self, x: i32, y: i32) -> bool;

    /// Slope of the tile at `(tx, ty)` shifted by `(dx, dy)` (`Room.IdentifySlope`).
    fn slope_near(&self, tx: i32, ty: i32, offset: (i32, i32)) -> Option<Slope>;

    /// Every tile coordinate, top row first, left to right.
    fn scan_top_down(&self) -> impl Iterator<Item = (i32, i32)> + '_;

    /// Tile coordinates flagged with shortcut `kind`, in scan order.
    fn tiles_with_shortcut(&self, kind: Shortcut) -> impl Iterator<Item = (i32, i32)> + '_;
}

impl RoomExt for Room {
    fn tile_at_pos(&self, pos: Vec2) -> Tile {
        self.tile_at(pos.to_array())
    }

    fn is_solid_tile(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).is_solid()
    }

    fn is_in_shortcut_tube(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
            .is_some_and(|t| t.shortcut != Shortcut::None || t.terrain == Terrain::ShortcutEntrance)
    }

    fn slope_near(&self, tx: i32, ty: i32, (dx, dy): (i32, i32)) -> Option<Slope> {
        self.identify_slope(tx + dx, ty + dy)
    }

    fn scan_top_down(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..self.height())
            .rev()
            .flat_map(move |y| (0..self.width()).map(move |x| (x, y)))
    }

    fn tiles_with_shortcut(&self, kind: Shortcut) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.scan_top_down()
            .filter(move |&(x, y)| self.tile(x, y).shortcut == kind)
    }
}
