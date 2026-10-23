use super::DripPalette;
use crate::core::draw::Color;

/// `ApplyPalette`: black, black half to white, white; a water-coloured drip's
/// `Lerp(waterColor2, waterColor1, 0.5)`, `waterColor1`, white.
#[test]
fn drip_colours_by_kind() {
    let p = DripPalette {
        black: Color::rgb(0.1, 0.1, 0.1),
        water_color1: Color::rgb(0.2, 0.4, 0.6),
        water_color2: Color::rgb(0.0, 0.2, 0.2),
        darkness: 0.0,
    };
    assert_eq!(
        p.colors(false),
        [
            Color::rgb(0.1, 0.1, 0.1),
            Color::rgb(0.55, 0.55, 0.55),
            Color::WHITE
        ]
    );
    let [mid, one, white] = p.colors(true);
    assert!((mid.r - 0.1).abs() < 1e-6 && (mid.g - 0.3).abs() < 1e-6);
    assert!((mid.b - 0.4).abs() < 1e-6);
    assert_eq!((one, white), (p.water_color1, Color::WHITE));
}
