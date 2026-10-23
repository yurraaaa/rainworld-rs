use super::*;

/// `Vector2.ClampMagnitude`: `(x / length * max, y / length * max)` above the maximum.
#[test]
fn clamp_magnitude_divides_then_scales() {
    let v = Vec2::new(3.0, 4.0).clamp_magnitude(2.0);
    assert_eq!((v.x, v.y), (3.0 / 5.0 * 2.0, 4.0 / 5.0 * 2.0));
    assert_eq!(
        Vec2::new(3.0, 4.0).clamp_magnitude(5.0),
        Vec2::new(3.0, 4.0)
    );
    assert_eq!(Vec2::new(3.0, 4.0).clamp_length(2.0), v);
}

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
