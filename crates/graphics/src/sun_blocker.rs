//! `SunBlocker` (the `SunBlock` room effect): one `pixel` sprite of colour
//! `(1/255, 0, 0)` over the whole screen in the `Shadows` container. The level shader's
//! object-shadow step reads any non-black pixel behind the level as an object, so it
//! takes the sun off every pixel at depth >= 5; its mask test needs red above 1/255, so
//! nothing shows through.
//!
//! Mirrors: SunBlocker.cs

use rw_sim::Vec2;

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// The sprite of one `SunBlocker`.
pub struct SunBlocker;

impl SunBlocker {
    /// `DrawSprites`: 1500x900 `pixel` centred on the screen point (683, 384), which is
    /// the room point `cam_pos + (683, 384)` for the frame's camera position.
    #[must_use]
    pub fn draw(cam_pos: Vec2) -> DrawList {
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "pixel",
                pos: cam_pos + Vec2::new(683.0, 384.0),
                rotation: 0.0,
                scale: Vec2::new(1500.0, 900.0),
                anchor: Vec2::new(0.5, 0.5),
                color: Color {
                    r: 1.0 / 255.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                shader: Shader::Basic,
                layer: Layer::Shadows,
            })],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_the_screen_in_shadows() {
        let list = SunBlocker::draw(Vec2::new(100.0, 50.0));
        let s = list.sprite(0).expect("one sprite");
        assert_eq!(s.pos, Vec2::new(783.0, 434.0));
        assert_eq!(s.scale, Vec2::new(1500.0, 900.0));
        assert_eq!(s.color.r, 1.0 / 255.0);
        assert_eq!(s.layer, Layer::Shadows);
    }
}
