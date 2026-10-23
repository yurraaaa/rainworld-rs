use rw_assets::LightBeamData;

use super::LightBeam;
use crate::Vec2;

fn data(color_b: f32) -> LightBeamData {
    LightBeamData::parse(&format!("0~40~40~40~40~0~0~0~0.4~0~{color_b}~1")).expect("data")
}

/// The constructor's quad: the position, then the position plus each handle.
#[test]
fn quad_is_position_plus_handles() {
    let b = LightBeam::new(3, Vec2::new(100.0, 50.0), data(0.0));
    assert_eq!(
        b.quad,
        [
            Vec2::new(100.0, 50.0),
            Vec2::new(100.0, 90.0),
            Vec2::new(140.0, 90.0),
            Vec2::new(140.0, 50.0)
        ]
    );
}

/// `Update` reads the environment colour only for `colorB > 0`.
#[test]
fn environment_color_only_with_color_b() {
    let c = [0.2, 0.4, 0.6, 1.0];
    let mut b = LightBeam::new(0, Vec2::ZERO, data(0.0));
    b.update(Some(c));
    assert_eq!(b.environment_color, [0.0; 4]);
    let mut b = LightBeam::new(0, Vec2::ZERO, data(0.25));
    b.update(None);
    assert_eq!(b.environment_color, [0.0; 4]);
    b.update(Some(c));
    assert_eq!(b.environment_color, c);
}
