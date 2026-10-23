//! Unity's `Vector3`, as far as the port needs it.
//!
//! Mirrors: -

use crate::Vec2;

/// Unity's `Vector3` (`Vector3.Slerp` is also what the game's `Vector2` slerps use).
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

    /// Each component divided by `d` (Unity's `operator /`).
    fn div(self, d: f32) -> Self {
        Self::new(self.x / d, self.y / d, self.z / d)
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
        let a = self.div(la);
        if dot > 1.0 - 1e-5 {
            return self.lerp_native(to, t);
        }
        let (axis, angle) = if dot < -1.0 + 1e-5 {
            (a.ortho_normal(), std::f32::consts::PI * t)
        } else {
            let c = self.cross(to);
            (c.div(c.length()), (f64::from(dot).acos() as f32) * t)
        };
        a.rotate_about(axis, angle).scale(length)
    }

    /// Rotates by `angle` radians about the unit `axis` (Unity's axis-angle matrix).
    fn rotate_about(self, axis: Self, angle: f32) -> Self {
        let (s, c) = (f64::from(angle).sin() as f32, f64::from(angle).cos() as f32);
        let one_c = 1.0 - c;
        let Self {
            x: vx,
            y: vy,
            z: vz,
        } = axis;
        let (xx, yy, zz) = (vx * vx, vy * vy, vz * vz);
        let (xy, yz, zx) = (vx * vy, vy * vz, vz * vx);
        let (xs, ys, zs) = (vx * s, vy * s, vz * s);
        let m = [
            [one_c * xx + c, one_c * xy - zs, one_c * zx + ys],
            [one_c * xy + zs, one_c * yy + c, one_c * yz - xs],
            [one_c * zx - ys, one_c * yz + xs, one_c * zz + c],
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

impl std::ops::Sub for Vec3 {
    type Output = Self;

    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Vec3 {
    /// `Vector3.Distance`.
    #[must_use]
    pub fn distance(self, o: Self) -> f32 {
        (self - o).length()
    }

    /// `Vector3.Lerp` (`t` clamped).
    #[must_use]
    pub fn lerp(self, to: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.x + (to.x - self.x) * t,
            self.y + (to.y - self.y) * t,
            self.z + (to.z - self.z) * t,
        )
    }

    /// `Vector3.MoveTowards`.
    #[must_use]
    pub fn move_towards(self, target: Self, max_delta: f32) -> Self {
        let d = target - self;
        let magnitude = d.length();
        if magnitude <= max_delta || magnitude == 0.0 {
            return target;
        }
        self + d.div(magnitude).scale(max_delta)
    }
}
