use rw_assets::RoomPalette;
use rw_sim::{FallSpec, Vec2, WaterFall};

use super::WaterFallSprites;
use crate::core::draw::{Color, Layer, Shader};

fn palette() -> RoomPalette {
    let c = [0.0, 0.0, 0.0, 1.0];
    RoomPalette {
        sky_color: c,
        fog_color: c,
        black_color: c,
        water_color1: [0.0, 0.0, 1.0, 1.0],
        water_color2: [1.0, 0.0, 0.0, 1.0],
        water_surface_color1: c,
        water_surface_color2: c,
        water_shine_color: c,
        fog_amount: 0.0,
        darkness: 0.0,
        shortcut_colors: [c; 3],
        shortcut_symbol: c,
    }
}

fn fall() -> WaterFall {
    let room = crate::tests::room(&["....", "....", "....", "####"]);
    let spec = FallSpec {
        tile: (1, 3),
        flow: 0.5,
        width: 2,
    };
    WaterFall::new(1, &room, spec, false)
}

/// The quad is anchored at its top-left, 20 px a tile wide, with a negative `scaleY`
/// from the bottom up to the top; its colour is the density and the ends' fade
/// `1 / Lerp(100, 2, f)`. With no water the strike is -200 and the fall (top 85, bottom
/// -200) covers all of its length, so the fades are those of its ends at the start and
/// strike.
#[test]
fn quad_spans_the_fall() {
    let mut f = fall();
    f.visual_density = 0.4;
    f.last_visual_dens = 0.4;
    let list = f.draw(None, &palette(), 1.0);
    let q = *list.sprite(0).expect("quad");
    assert_eq!(
        (q.shader, q.layer, q.element),
        (Shader::WaterFall, Layer::Background, "Futile_White")
    );
    assert_eq!(q.anchor, Vec2::new(0.0, 1.0));
    assert_eq!(q.pos, Vec2::new(20.0, -200.0));
    assert_eq!(q.scale, Vec2::new(2.5, (-200.0 - 85.0) / 16.0));
    // top == start: f = 0^0.2 = 0 then Lerp(0, 1, 1 - 1) = 0 -> 1/100; bottom == strike
    // (f2 = 0).
    assert_eq!(q.color, Color::rgb(0.4, 0.01, 0.01));
    assert!(f.bubbles.is_empty() && list.items.len() == 1);
}

/// Bubbles follow with the index after the quad, between the water colours (30 %), sized
/// `Pow(x, 0.5)` and between ticks interpolated.
#[test]
fn bubbles_follow_the_quad() {
    let mut f = fall();
    f.bubbles = vec![
        [
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 0.0),
            Vec2::ZERO,
            Vec2::new(0.25, 5.0)
        ];
        2
    ];
    let list = f.draw(None, &palette(), 0.5);
    let b = *list.sprite(2).expect("bubble");
    assert_eq!(
        (b.element, b.pos, b.scale),
        ("LizardBubble5", Vec2::new(5.0, 5.0), Vec2::new(0.5, 0.5))
    );
    assert_eq!(b.color, Color::rgb(0.3, 0.0, 0.7));
    assert_eq!(list.items.len(), 3);
}
