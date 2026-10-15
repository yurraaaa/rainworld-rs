//! The Unity `Mathf`/`Vector2` and game `Custom` helpers the graphics use, as
//! extension traits on `f32` and [`Vec2`]. `Custom.DegToVec` and `Custom.RNV` are
//! rw-sim's [`Vec2::from_degrees`] and [`Vec2::random_unit`].

use rw_sim::Vec2;
pub use rw_sim::Vec3;

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
    /// `Mathf.Sin`: `(float)Math.Sin(self)`.
    fn mathf_sin(self) -> f32;
    /// `Mathf.Cos`.
    fn mathf_cos(self) -> f32;
    /// `Mathf.LerpAngle(self, to, t)` in degrees: the short way round, `t` clamped.
    fn lerp_angle(self, to: f32, t: f32) -> f32;
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

    fn mathf_sin(self) -> f32 {
        f64::from(self).sin() as f32
    }

    fn mathf_cos(self) -> f32 {
        f64::from(self).cos() as f32
    }

    fn lerp_angle(self, to: f32, t: f32) -> f32 {
        // `Mathf.Repeat(to - self, 360)`.
        let d = to - self;
        let mut num = (d - (d / 360.0).floor() * 360.0).clamp(0.0, 360.0);
        if num > 180.0 {
            num -= 360.0;
        }
        self + num * t.clamp(0.0, 1.0)
    }
}

/// Unity `Vector3` operations rw-sim's [`Vec3`] lacks.
pub trait UnityVector3: Sized {
    /// `Vector3.Lerp`: `t` clamped to `[0, 1]`.
    fn lerp_to(self, to: Self, t: f32) -> Self;
    fn minus(self, o: Self) -> Self;
    /// `Vector3.normalized`: zero when the length is at most `1e-5`.
    fn unit(self) -> Self;
    /// `OverseerHologram.Rotate3DPos`: turned about y by `ry`, z by `rz`, then x by `rx`
    /// (in turns).
    fn rotate_3d(self, rx: f32, ry: f32, rz: f32) -> Self;
}

impl UnityVector3 for Vec3 {
    fn lerp_to(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.x + (to.x - self.x) * t,
            self.y + (to.y - self.y) * t,
            self.z + (to.z - self.z) * t,
        )
    }

    fn minus(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    fn unit(self) -> Self {
        let len = (f64::from(self.x * self.x + self.y * self.y + self.z * self.z)).sqrt() as f32;
        if len > 1e-5 {
            Self::new(self.x / len, self.y / len, self.z / len)
        } else {
            Self::new(0.0, 0.0, 0.0)
        }
    }

    fn rotate_3d(mut self, rx: f32, ry: f32, rz: f32) -> Self {
        let pi = std::f32::consts::PI;
        let (sy, cy) = ((ry * pi * 2.0).mathf_sin(), (ry * pi * 2.0).mathf_cos());
        let z = self.z * cy - self.x * sy;
        self.x = self.z * sy + self.x * cy;
        self.z = z;
        let (sz, cz) = ((rz * pi * 2.0).mathf_sin(), (rz * pi * 2.0).mathf_cos());
        let x = self.x * cz - self.y * sz;
        self.y = self.x * sz + self.y * cz;
        self.x = x;
        let (sx, cx) = ((rx * pi * 2.0).mathf_sin(), (rx * pi * 2.0).mathf_cos());
        let y = self.y * cx - self.z * sx;
        self.z = self.y * sx + self.z * cx;
        self.y = y;
        self
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

/// The game's `Custom` curves and vector helpers that are not `Vec2` methods.
pub trait Custom {
    /// `Custom.SCurve(self, k)`.
    fn s_curve(self, k: f32) -> f32;
}

impl Custom for f32 {
    fn s_curve(self, k: f32) -> f32 {
        let x = self * 2.0 - 1.0;
        if x < 0.0 {
            let x = (1.0 + x).abs();
            return k * x / (k - x + 1.0) * 0.5;
        }
        let k = -1.0 - k;
        0.5 + k * x / (k - x + 1.0) * 0.5
    }
}

impl Vec2Bezier for Vec2 {
    fn bezier(self, ca: Self, b: Self, cb: Self, f: f32) -> Self {
        let f = f.clamp(0.0, 1.0);
        let mid = ca.lerp(cb, f);
        let ca = self.lerp(ca, f);
        let cb = cb.lerp(b, f);
        let ca = ca.lerp(mid, f);
        let cb = mid.lerp(cb, f);
        ca.lerp(cb, f)
    }

    fn vec_to_deg(self) -> f32 {
        (self.x.atan2(self.y) / (std::f32::consts::PI * 2.0)) * 360.0
    }
}

/// `Custom.Bezier` and `Custom.VecToDeg`.
pub trait Vec2Bezier: Sized {
    /// `Custom.Bezier(self, cA, B, cB, f)`: the cubic from `self` to `b` with control
    /// points `ca` (at `self`) and `cb` (at `b`).
    fn bezier(self, ca: Self, b: Self, cb: Self, f: f32) -> Self;
    /// `Custom.VecToDeg`.
    fn vec_to_deg(self) -> f32;
}
