//! Minimal 2D float vector with the arithmetic the physics needs, in `f32` as the game.

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::FloatExt;

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

    /// `Custom.PerpendicularVector`: the unit vector turned a quarter turn
    /// counter-clockwise (zero for a zero vector).
    #[must_use]
    pub fn perpendicular(self) -> Self {
        let n = self.normalized();
        Self::new(-n.y, n.x)
    }

    /// `Custom.RNV`: a unit vector at a random angle (`DegToVec(Random.value * 360)`).
    pub fn random_unit(rng: &mut rw_random::UnityRandom) -> Self {
        Self::from_degrees(rng.value() * 360.0)
    }

    /// `Custom.DegToVec`: unit vector `deg` degrees clockwise from straight up.
    #[must_use]
    pub fn from_degrees(deg: f32) -> Self {
        // `Mathf.Sin`/`Cos` are `(float)Math.Sin`/`Cos` of the double.
        let rad = f64::from(deg.to_radians());
        Self::new(rad.sin() as f32, rad.cos() as f32)
    }

    /// Scaled down to length `max` if longer (`Vector2.ClampMagnitude`).
    #[must_use]
    pub fn clamp_length(self, max: f32) -> Self {
        let sq = self.x * self.x + self.y * self.y;
        if sq > max * max {
            self.normalized() * max
        } else {
            self
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

    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// `Vector2.Angle`: the unsigned angle between the two directions in degrees; 0 when
    /// either is (nearly) zero.
    #[must_use]
    pub fn angle_to(self, other: Self) -> f32 {
        let denominator =
            ((self.x * self.x + self.y * self.y) * (other.x * other.x + other.y * other.y)).sqrt();
        if denominator < 1e-15 {
            return 0.0;
        }
        (self.dot(other) / denominator)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    }

    /// `Vector3.Slerp` of the two as `Vector3`s (`z` = 0), back to 2D.
    #[must_use]
    pub fn slerp(self, to: Self, t: f32) -> Self {
        crate::Vec3::from_xy(self)
            .slerp(crate::Vec3::from_xy(to), t)
            .xy()
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

    /// `Room.MiddleOfTile(IntVector2)`.
    #[must_use]
    pub fn middle_of_tile((x, y): (i32, i32)) -> Self {
        Self::from(rw_assets::tile_centre(x, y))
    }

    /// `Custom.DistLess`.
    #[must_use]
    pub fn dist_less(self, other: Self, dist: f32) -> bool {
        (self - other).length() < dist
    }

    /// `Custom.MoveTowards`: at most `max` closer to `to`.
    #[must_use]
    pub fn move_towards(self, to: Self, max: f32) -> Self {
        if self.dist_less(to, max) {
            return to;
        }
        self + self.direction_to(to) * max
    }

    /// `Custom.VecToDeg`: degrees clockwise from straight up.
    #[must_use]
    pub fn to_degrees(self) -> f32 {
        let a = f64::from(self.x).atan2(f64::from(self.y)) as f32;
        a / (std::f32::consts::PI * 2.0) * 360.0
    }

    /// `Custom.DistanceToLine(self, l2, l1)`: signed distance to the line through both.
    #[must_use]
    pub fn distance_to_line(self, l2: Self, l1: Self) -> f32 {
        let dy = l2.y - l1.y;
        let dx = l2.x - l1.x;
        (dy * self.x - dx * self.y + l2.x * l1.y - l2.y * l1.x)
            / (dy.mathf_pow(2.0) + dx.mathf_pow(2.0)).sqrt()
    }

    /// `Custom.CirclesCollisionTime`: when, as a fraction of `vel`, a circle of radius
    /// `r1` moving from here first touches one of radius `r2` at `other` (NaN when it
    /// never does), with the game's 0.001 offsets.
    #[must_use]
    pub fn circles_collision_time(self, other: Self, vel: Self, r1: f32, r2: f32) -> f32 {
        let (x1, y1, x2, y2, vx, vy) = (self.x, self.y, other.x, other.y, vel.x, vel.y);
        let (e1, e2) = (0.001f32, 0.001f32);
        let num3 = -x1 * vx - y1 * vy + vx * x2 + vy * y2 + x1 * e1 - x2 * e1 + y1 * e2 - y2 * e2;
        let num5 = vx.mathf_pow(2.0) + vy.mathf_pow(2.0) - 2.0 * vx * e1 + e1.mathf_pow(2.0)
            - 2.0 * vy * e2
            + e2.mathf_pow(2.0);
        let num6 = x1.mathf_pow(2.0) + y1.mathf_pow(2.0) - r1.mathf_pow(2.0) - 2.0 * x1 * x2
            + x2.mathf_pow(2.0)
            - 2.0 * y1 * y2
            + y2.mathf_pow(2.0)
            - 2.0 * r1 * r2
            - r2.mathf_pow(2.0);
        let root = ((-2.0 * num3).mathf_pow(2.0) - 4.0 * num5 * num6).sqrt();
        (2.0 * num3 - root) / (2.0 * num5)
    }

    /// `Custom.VerticalCrossPoint`: where the line through `self` and `b` crosses `x`.
    #[must_use]
    pub fn vertical_cross_point(self, b: Self, x: f32) -> Self {
        if self.y == b.y {
            return Self::new(x, self.y);
        }
        let k = (self.y - b.y) / (self.x - b.x);
        Self::new(x, self.y - self.x * k + k * x)
    }

    /// `Custom.HorizontalCrossPoint`: where the line through `self` and `b` crosses `y`.
    #[must_use]
    pub fn horizontal_cross_point(self, b: Self, y: f32) -> Self {
        if self.x == b.x {
            return Self::new(self.x, y);
        }
        let k = (self.y - b.y) / (self.x - b.x);
        Self::new((y - (self.y - self.x * k)) / k, y)
    }

    /// `Custom.LineIntersection` of the line through `self` and `b1` with the line
    /// through `a2` and `b2`.
    #[must_use]
    pub fn line_intersection(self, b1: Self, a2: Self, b2: Self) -> Self {
        if self.x == b1.x {
            return a2.vertical_cross_point(b2, self.x);
        }
        if a2.x == b2.x {
            return self.vertical_cross_point(b1, a2.x);
        }
        if self.y == b1.y {
            return a2.horizontal_cross_point(b2, self.y);
        }
        if a2.y == b2.y {
            return self.horizontal_cross_point(b1, a2.y);
        }
        let k1 = (self.y - b1.y) / (self.x - b1.x);
        let c1 = self.y - self.x * k1;
        let k2 = (a2.y - b2.y) / (a2.x - b2.x);
        let c2 = a2.y - a2.x * k2;
        let x = (c1 - c2) / (k2 - k1);
        Self::new(x, c2 + k2 * x)
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

impl Div<f32> for Vec2 {
    type Output = Self;
    fn div(self, k: f32) -> Self {
        Self::new(self.x / k, self.y / k)
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

/// `IntVector2` methods on tile coordinates.
pub trait TileExt {
    /// `IntVector2.FloatDist`.
    fn float_dist(self, other: Self) -> f32;
    /// `Custom.ManhattanDistance`.
    fn manhattan(self, other: Self) -> i32;
}

impl TileExt for (i32, i32) {
    fn float_dist(self, other: Self) -> f32 {
        Vec2::new(self.0 as f32, self.1 as f32).distance(Vec2::new(other.0 as f32, other.1 as f32))
    }

    fn manhattan(self, other: Self) -> i32 {
        (self.0 - other.0).abs() + (self.1 - other.1).abs()
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

    #[test]
    fn angle_and_slerp() {
        let right = Vec2::new(1.0, 0.0);
        let up = Vec2::new(0.0, 1.0);
        assert!((right.angle_to(up) - 90.0).abs() < 1e-4);
        assert_eq!(Vec2::ZERO.angle_to(up), 0.0);
        let half = right.slerp(up, 0.5);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((half.x - s).abs() < 1e-6 && (half.y - s).abs() < 1e-6);
        // Clockwise targets turn clockwise.
        let down_half = right.slerp(Vec2::new(0.0, -1.0), 0.5);
        assert!((down_half.x - s).abs() < 1e-6 && (down_half.y + s).abs() < 1e-6);
        assert_eq!(right.slerp(up, 0.0), right);
    }
}
