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
    /// `Mathf.Sin`: `(float)Math.Sin(self)`.
    fn mathf_sin(self) -> f32;
    /// `Mathf.Cos`.
    fn mathf_cos(self) -> f32;
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

/// Unity's `Vector3`, for the overseer's projected shapes and its look direction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const FORWARD: Self = Self::new(0.0, 0.0, 1.0);

    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The implicit `Vector2` to `Vector3` conversion (`z` = 0).
    #[must_use]
    pub const fn from_xy(v: Vec2) -> Self {
        Self::new(v.x, v.y, 0.0)
    }

    /// The implicit conversion back to `Vector2` (drops `z`).
    #[must_use]
    pub const fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    #[must_use]
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    #[must_use]
    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    #[must_use]
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[must_use]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Unity's native `Lerp` form: `to * t + self * (1 - t)`.
    fn lerp_native(self, to: Self, t: f32) -> Self {
        to.scale(t) + self.scale(1.0 - t)
    }

    /// `OrthoNormalVectorFast` of a unit vector.
    fn ortho_normal(self) -> Self {
        if self.z.abs() > std::f32::consts::FRAC_1_SQRT_2 {
            let k = 1.0 / (self.y * self.y + self.z * self.z).sqrt();
            Self::new(0.0, -self.z * k, self.y * k)
        } else {
            let k = 1.0 / (self.x * self.x + self.y * self.y).sqrt();
            Self::new(-self.y * k, self.x * k, 0.0)
        }
    }

    /// `Vector3.Slerp` (native; INFERRED reconstruction): `t` clamped; near-zero or
    /// near-parallel vectors lerp; otherwise the direction turns about the normalized
    /// cross product (an orthonormal axis when opposite) and the length is lerped.
    #[must_use]
    pub fn slerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let (la, lb) = (self.length(), to.length());
        if la < 1e-5 || lb < 1e-5 {
            return self.lerp_native(to, t);
        }
        let length = lb * t + la * (1.0 - t);
        let dot = self.dot(to) / (la * lb);
        let a = self.scale(1.0 / la);
        if dot > 1.0 - 1e-5 {
            return self.lerp_native(to, t);
        }
        let (axis, angle) = if dot < -1.0 + 1e-5 {
            (a.ortho_normal(), std::f32::consts::PI * t)
        } else {
            let c = self.cross(to);
            (
                c.scale(1.0 / c.length()),
                (f64::from(dot).acos() as f32) * t,
            )
        };
        a.rotate_about(axis, angle).scale(length)
    }

    /// Rotates by `angle` radians about the unit `axis` (Unity's axis-angle matrix).
    fn rotate_about(self, axis: Self, angle: f32) -> Self {
        let (s, c) = (angle.mathf_sin(), angle.mathf_cos());
        let one_c = 1.0 - c;
        let Self {
            x: vx,
            y: vy,
            z: vz,
        } = axis;
        let (xs, ys, zs) = (vx * s, vy * s, vz * s);
        let m = [
            [
                one_c * vx * vx + c,
                one_c * vx * vy - zs,
                one_c * vz * vx + ys,
            ],
            [
                one_c * vx * vy + zs,
                one_c * vy * vy + c,
                one_c * vy * vz - xs,
            ],
            [
                one_c * vz * vx - ys,
                one_c * vy * vz + xs,
                one_c * vz * vz + c,
            ],
        ];
        Self::new(
            m[0][0] * self.x + m[0][1] * self.y + m[0][2] * self.z,
            m[1][0] * self.x + m[1][1] * self.y + m[1][2] * self.z,
            m[2][0] * self.x + m[2][1] * self.y + m[2][2] * self.z,
        )
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
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
