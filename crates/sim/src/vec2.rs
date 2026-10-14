//! Minimal 2D float vector with the arithmetic the physics needs, in `f32` as the game.

use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// 2D vector in room pixels (y up).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// `(0, 0)`.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean length.
    #[must_use]
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Distance to `other`.
    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (other - self).length()
    }

    /// Unit vector, or zero when the length is at most `1e-5` (as Unity's `normalized`).
    #[must_use]
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len > 1e-5 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::ZERO
        }
    }

    /// Unit direction from `self` to `to`; `(0, 1)` when the points coincide
    /// (Unity's approximate `==`, squared distance below `1e-10`), as the game's `DirVec`.
    #[must_use]
    pub fn direction_to(self, to: Self) -> Self {
        let d = to - self;
        if d.x * d.x + d.y * d.y < 9.999_999_4e-11 {
            Self::new(0.0, 1.0)
        } else {
            d.normalized()
        }
    }

    /// As an array, for the `rw_assets` tile helpers.
    #[must_use]
    pub const fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }

    /// Linear interpolation: `self` at `t = 0`, `to` at `t = 1` (not clamped).
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        self + (to - self) * t
    }

    /// Tile containing this room-pixel position (`Room.GetTilePosition`).
    #[must_use]
    pub fn tile(self) -> (i32, i32) {
        rw_assets::tile_position(self.to_array())
    }

    /// Middle of the tile containing this position (`Room.MiddleOfTile(Vector2)`).
    #[must_use]
    pub fn tile_middle(self) -> Self {
        let (x, y) = self.tile();
        Self::from(rw_assets::tile_centre(x, y))
    }
}

impl From<[f32; 2]> for Vec2 {
    fn from([x, y]: [f32; 2]) -> Self {
        Self::new(x, y)
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, k: f32) -> Self {
        Self::new(self.x * k, self.y * k)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, k: f32) {
        *self = *self * k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_to_coincident_points_is_up() {
        assert_eq!(Vec2::ZERO.direction_to(Vec2::ZERO), Vec2::new(0.0, 1.0));
        assert_eq!(
            Vec2::new(1.0, 1.0).direction_to(Vec2::new(4.0, 5.0)),
            Vec2::new(0.6, 0.8)
        );
    }
}
