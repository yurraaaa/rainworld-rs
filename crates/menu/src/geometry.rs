//! Unity's `Rect` and Futile's `FMatrix`, with their exact field semantics.

/// Unity's `Rect`: `x_min` is `x` and `x_max` is `x + width` even when the width is
/// negative (the game builds such rects for empty text lines).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn x_min(&self) -> f32 {
        self.x
    }

    #[must_use]
    pub fn x_max(&self) -> f32 {
        self.x + self.width
    }

    #[must_use]
    pub fn y_min(&self) -> f32 {
        self.y
    }

    #[must_use]
    pub fn y_max(&self) -> f32 {
        self.y + self.height
    }
}

/// `FMatrix`: `[x', y'] = [x * a + y * c + tx, x * b + y * d + ty]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// Futile's degrees-to-radians factor in `FNode.UpdateDepthMatrixAlpha`
    /// (`rotation * -0.01745329f`), slightly below pi / 180.
    pub const DEGREES_TO_RADIANS: f32 = 0.017_453_29;

    /// `SetScaleThenRotate` as `FNode` calls it: `rotation` in degrees, clockwise.
    #[must_use]
    pub fn node(x: f32, y: f32, scale_x: f32, scale_y: f32, rotation: f32) -> Self {
        let (sin, cos) = (rotation * -Self::DEGREES_TO_RADIANS).sin_cos();
        Self {
            a: scale_x * cos,
            b: scale_x * sin,
            c: scale_y * -sin,
            d: scale_y * cos,
            tx: x,
            ty: y,
        }
    }

    /// `ConcatAndCopyValues(self, parent)`: this node's matrix inside `parent`'s.
    #[must_use]
    pub fn then(&self, parent: &Self) -> Self {
        Self {
            a: self.a * parent.a + self.b * parent.c,
            b: self.a * parent.b + self.b * parent.d,
            c: self.c * parent.a + self.d * parent.c,
            d: self.c * parent.b + self.d * parent.d,
            tx: self.tx * parent.a + self.ty * parent.c + parent.tx,
            ty: self.tx * parent.b + self.ty * parent.d + parent.ty,
        }
    }

    /// `ApplyVector3FromLocalVector2` (without z).
    #[must_use]
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        [
            p[0] * self.a + p[1] * self.c + self.tx,
            p[0] * self.b + p[1] * self.d + self.ty,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_width_rect_keeps_x_as_min() {
        let r = Rect::new(0.0, 0.0, -15.0, -30.0);
        assert_eq!(r.x_min(), 0.0);
        assert_eq!(r.x_max(), -15.0);
        assert_eq!(r.y_max(), -30.0);
    }

    #[test]
    fn child_in_translated_scaled_parent() {
        let parent = Matrix::node(100.0, 50.0, 2.0, 2.0, 0.0);
        let child = Matrix::node(10.0, 5.0, 1.0, 1.0, 0.0);
        assert_eq!(child.then(&parent).apply([1.0, 1.0]), [122.0, 62.0]);
    }

    #[test]
    fn rotation_is_clockwise() {
        let m = Matrix::node(0.0, 0.0, 1.0, 1.0, 90.0);
        let p = m.apply([1.0, 0.0]);
        assert!(p[0].abs() < 1e-5 && (p[1] + 1.0).abs() < 1e-5, "{p:?}");
    }
}
