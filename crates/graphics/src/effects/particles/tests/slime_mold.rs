use rw_assets::RoomPalette;

use super::SlimeMoldPalette;
use crate::core::draw::Color;

/// `SlimeMoldColorFromPalette` with no darkness and no fog amount: `HSL2RGB(0.07, 1,
/// 0.55)` 2.5 % of the way to the fog colour; the dark end 15 % from black to fog.
#[test]
fn slime_mold_palette_colours() {
    let white = [1.0, 1.0, 1.0, 1.0];
    let p = RoomPalette {
        sky_color: white,
        fog_color: white,
        black_color: [0.0, 0.0, 0.0, 1.0],
        water_color1: white,
        water_color2: white,
        water_surface_color1: white,
        water_surface_color2: white,
        water_shine_color: white,
        fog_amount: 0.0,
        darkness: 0.0,
        shortcut_colors: [white; 3],
        shortcut_symbol: white,
    };
    let fog = Color::from(white);
    assert_eq!(
        p.slime_mold_color(),
        Color::hsl(0.07, 1.0, 0.55).lerp(fog, 0.025)
    );
    assert_eq!(p.slime_mold_dark(), Color::rgb(0.15, 0.15, 0.15));
}
