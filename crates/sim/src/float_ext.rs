//! Unity / `RWCustom.Custom` scalar helpers on `f32`.
//!
//! Mirrors: RWCustom/Custom.cs

pub trait FloatExt: Sized {
    /// `Mathf.Lerp`: `self` at `t = 0`, `to` at `t = 1`, `t` clamped to `[0, 1]`.
    fn lerp(self, to: f32, t: f32) -> f32;

    /// `Mathf.InverseLerp`: where `self` lies between `a` and `b`, clamped to `[0, 1]`.
    fn inverse_lerp(self, a: f32, b: f32) -> f32;

    /// `Custom.LerpMap`: maps `self` from `from_a..to_a` onto `from_b..to_b`, clamped.
    fn lerp_map(self, from_a: f32, to_a: f32, from_b: f32, to_b: f32) -> f32 {
        self.lerp_map_pow(from_a, to_a, from_b, to_b, 1.0)
    }

    /// `Custom.LerpMap` with an exponent applied to the clamped fraction.
    fn lerp_map_pow(self, from_a: f32, to_a: f32, from_b: f32, to_b: f32, exponent: f32) -> f32;

    /// C#'s `(int)` cast as the game's x86-64 runtime does it (`cvttss2si`): truncation,
    /// with NaN and out-of-range values giving `int.MinValue` (INFERRED from the
    /// instruction; Rust's `as` saturates instead).
    fn to_int_cs(self) -> i32;

    /// `Mathf.Pow`: `(float)Math.Pow(self, p)`, in double precision.
    fn mathf_pow(self, p: f32) -> f32;

    /// `Mathf.Sign`: 1 for zero, -1 below it.
    fn unity_sign(self) -> f32;

    /// `Custom.InRange`: between `a` and `b` inclusive, in either order.
    fn in_range(self, a: f32, b: f32) -> bool;

    /// `Custom.LerpAndTick`: lerp toward `to`, then step a further `tick` toward it
    /// without passing it.
    fn lerp_and_tick(self, to: f32, lerp: f32, tick: f32) -> f32 {
        let v = self.lerp(to, lerp);
        if v < to {
            (v + tick).min(to)
        } else {
            (v - tick).max(to)
        }
    }
}

impl FloatExt for f32 {
    fn lerp(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t.clamp(0.0, 1.0)
    }

    fn inverse_lerp(self, a: f32, b: f32) -> f32 {
        if a == b {
            0.0
        } else {
            ((self - a) / (b - a)).clamp(0.0, 1.0)
        }
    }

    fn mathf_pow(self, p: f32) -> f32 {
        f64::from(self).powf(f64::from(p)) as f32
    }

    fn unity_sign(self) -> f32 {
        if self >= 0.0 { 1.0 } else { -1.0 }
    }

    fn in_range(self, a: f32, b: f32) -> bool {
        if a < b {
            self >= a && self <= b
        } else {
            self >= b && self <= a
        }
    }

    fn to_int_cs(self) -> i32 {
        if self.is_nan() || self >= 2_147_483_648.0 || self < -2_147_483_648.0 {
            i32::MIN
        } else {
            self as i32
        }
    }

    fn lerp_map_pow(self, from_a: f32, to_a: f32, from_b: f32, to_b: f32, exponent: f32) -> f32 {
        let t = self.inverse_lerp(from_a, to_a).mathf_pow(exponent);
        from_b + (to_b - from_b) * t
    }
}

/// `Mathf.Sin`: `(float)Math.Sin` of the double.
pub(crate) trait MathfSin {
    fn mathf_sin(self) -> f32;
}

impl MathfSin for f32 {
    fn mathf_sin(self) -> f32 {
        f64::from(self).sin() as f32
    }
}

/// `Custom.SCurve` (kept off [`FloatExt`], whose users have their own).
pub(crate) trait SCurve {
    fn s_curve(self, k: f32) -> f32;
}

impl SCurve for f32 {
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
