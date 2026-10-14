//! Unity / `RWCustom.Custom` scalar helpers on `f32`.

pub(crate) trait FloatExt: Sized {
    /// `Mathf.InverseLerp`: where `self` lies between `a` and `b`, clamped to `[0, 1]`.
    fn inverse_lerp(self, a: f32, b: f32) -> f32;

    /// `Custom.LerpMap`: maps `self` from `from_a..to_a` onto `from_b..to_b`, clamped.
    fn lerp_map(self, from_a: f32, to_a: f32, from_b: f32, to_b: f32) -> f32 {
        self.lerp_map_pow(from_a, to_a, from_b, to_b, 1.0)
    }

    /// `Custom.LerpMap` with an exponent applied to the clamped fraction.
    fn lerp_map_pow(self, from_a: f32, to_a: f32, from_b: f32, to_b: f32, exponent: f32) -> f32;
}

impl FloatExt for f32 {
    fn inverse_lerp(self, a: f32, b: f32) -> f32 {
        if a == b {
            0.0
        } else {
            ((self - a) / (b - a)).clamp(0.0, 1.0)
        }
    }

    fn lerp_map_pow(self, from_a: f32, to_a: f32, from_b: f32, to_b: f32, exponent: f32) -> f32 {
        let t = self.inverse_lerp(from_a, to_a).powf(exponent);
        from_b + (to_b - from_b) * t
    }
}
