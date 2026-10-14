//! The Unity `Mathf`/`Vector2` and game `Custom` helpers the graphics use, as
//! extension traits on `f32` and [`Vec2`]. `Custom.DegToVec` and `Custom.RNV` are
//! rw-sim's [`Vec2::from_degrees`] and [`Vec2::random_unit`].

use rw_sim::Vec2;

/// `Mathf` functions on `f32`.
pub trait Mathf {
    /// `Mathf.Sign`: 1 for zero and positive values, -1 otherwise (also for NaN).
    fn unity_sign(self) -> f32;
    /// `Mathf.RoundToInt`: halves round to even, as .NET `Math.Round`.
    fn round_to_int(self) -> i32;
    /// `Mathf.Lerp`: `t` clamped to `[0, 1]`.
    fn lerp_to(self, to: f32, t: f32) -> f32;
    /// `Mathf.InverseLerp(a, b, self)`: clamped to `[0, 1]`, 0 when `a == b`.
    fn inverse_lerp(self, a: f32, b: f32) -> f32;
}

impl Mathf for f32 {
    fn unity_sign(self) -> f32 {
        if self >= 0.0 { 1.0 } else { -1.0 }
    }

    fn round_to_int(self) -> i32 {
        self.round_ties_even() as i32
    }

    fn lerp_to(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t.clamp(0.0, 1.0)
    }

    fn inverse_lerp(self, a: f32, b: f32) -> f32 {
        if a == b {
            0.0
        } else {
            ((self - a) / (b - a)).clamp(0.0, 1.0)
        }
    }
}

/// The game's `Custom` vector helpers. Angles in degrees, 0 = up, positive = clockwise.
pub trait CustomVec: Sized {
    /// `Custom.AimFromOneVectorToAnother(self, to)`.
    fn aim_to(self, to: Self) -> f32;
    /// `Custom.RotateAroundOrigo`: rotate clockwise by `deg`.
    fn rotated(self, deg: f32) -> Self;
    /// `Custom.PerpendicularVector`: the normalised vector turned 90 degrees counter-clockwise.
    fn perpendicular(self) -> Self;
    /// `Custom.DistanceToLine(self, l2, l1)`: signed distance to the line through `l1` and `l2`.
    fn distance_to_line(self, l2: Self, l1: Self) -> f32;
    /// `Vector2.Lerp`: `t` clamped to `[0, 1]`.
    fn lerp_clamped(self, to: Self, t: f32) -> Self;
    /// `Custom.DistNoSqrt`: squared distance.
    fn dist_no_sqrt(self, other: Self) -> f32;
}

impl CustomVec for Vec2 {
    fn aim_to(self, to: Self) -> f32 {
        let d = to - self;
        d.x.atan2(d.y).to_degrees()
    }

    fn rotated(self, deg: f32) -> Self {
        let rad = -deg.to_radians();
        let (sin, cos) = rad.sin_cos();
        Self::new(cos * self.x - sin * self.y, sin * self.x + cos * self.y)
    }

    fn perpendicular(self) -> Self {
        let n = self.normalized();
        Self::new(-n.y, n.x)
    }

    fn distance_to_line(self, l2: Self, l1: Self) -> f32 {
        let d = l2 - l1;
        (d.y * self.x - d.x * self.y + l2.x * l1.y - l2.y * l1.x) / (d.y * d.y + d.x * d.x).sqrt()
    }

    fn lerp_clamped(self, to: Self, t: f32) -> Self {
        self.lerp(to, t.clamp(0.0, 1.0))
    }

    fn dist_no_sqrt(self, other: Self) -> f32 {
        let d = other - self;
        d.x * d.x + d.y * d.y
    }
}
