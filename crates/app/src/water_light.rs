//! The room camera's `WaterLight`: in a room with water, one sprite with the
//! `WaterLight` shader in `Foreground` (added once, just after the level graphic), from
//! the surface's left end to its right end and from the still water level up, drawing
//! faint caustics where the room's `RainMask_` texture is blue.
//!
//! Mirrors: WaterLight.cs, RoomCamera.cs (ChangeRoom: waterLight, DrawUpdate: waterLight)

use rw_assets::{Room, RoomSettings};
use rw_graphics::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use rw_render::SpriteAtlas;
use rw_sim::{Vec2, Water};

/// The camera's `WaterLight` in its room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterLight {
    /// `roomSettings.WaterReflectionAlpha`.
    pub reflection_alpha: f32,
    /// The `RainMask_` element's height (`TileHeight` texels): `NewRoom` swaps the
    /// element in and keeps the constructor's `scaleY`.
    mask_height: f32,
}

impl WaterLight {
    /// The constructor's `scaleY`.
    pub const SCALE_Y: f32 = 18.75;
    /// `Futile_White`'s size, which the port's sprite keeps (scaled to the mask's).
    const WHITE_SIZE: f32 = 16.0;

    /// `ChangeRoom`: a `WaterLight` for a room with water (`newRoom.water`), none
    /// otherwise. `room.gameOverRoom`, which would clean it out, is never set.
    #[must_use]
    pub fn new(room: &Room, water: Option<&Water>, settings: &RoomSettings) -> Option<Self> {
        water.map(|_| Self {
            reflection_alpha: settings.water_reflection_alpha,
            mask_height: room.height() as f32,
        })
    }

    /// `DrawUpdate`'s alpha: `(1 - PaletteDarkness()) * WaterReflectionAlpha`.
    #[must_use]
    pub fn alpha(&self, palette_darkness: f32) -> f32 {
        (1.0 - palette_darkness) * self.reflection_alpha
    }

    /// The sprite: anchored at its bottom-left on the surface's left end
    /// (`SurfaceLeftAndRightBoundries().x`) at `fWaterLevel`, as wide as the surface
    /// (`scaleX` over the mask's width) and `TileHeight * 18.75` px tall, white at
    /// [`Self::alpha`]. The shader reads only the quad's uv and the colour's alpha.
    #[must_use]
    pub fn draw_list(&self, water: &Water, palette_darkness: f32) -> DrawList {
        let (left, right) = water.surface_bounds();
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: SpriteAtlas::FUTILE_WHITE,
                pos: Vec2::new(left, water.still_level()),
                rotation: 0.0,
                scale: Vec2::new(
                    (right - left) / Self::WHITE_SIZE,
                    self.mask_height * Self::SCALE_Y / Self::WHITE_SIZE,
                ),
                anchor: Vec2::ZERO,
                color: Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: self.alpha(palette_darkness),
                },
                shader: Shader::WaterLight,
                layer: Layer::Foreground,
            })],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 20 x 12 room of air with water at `water` (-1 for none).
    fn room(water: i32) -> Room {
        let (w, h) = (20, 12);
        let text = format!(
            "Test\n{w}*{h}|{water}|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{}\n",
            "0|".repeat(w * h)
        );
        Room::parse(&text).expect("test room parses")
    }

    #[test]
    fn only_rooms_with_water_have_one() {
        let dry = room(-1);
        assert!(WaterLight::new(&dry, None, &RoomSettings::ANCESTOR).is_none());
        let wet = room(3);
        let water = Water::new(&wet, &RoomSettings::ANCESTOR).expect("water");
        let light = WaterLight::new(&wet, Some(&water), &RoomSettings::ANCESTOR).unwrap();
        assert_eq!(light.reflection_alpha, 1.0);
    }

    /// Water row 3 in a 400 px wide room: the surface runs from -220 to 620 (43 points
    /// 20 px apart), the still level is 70; 12 tiles high, so 12 * 18.75 = 225 px.
    /// GW_C08's `WaterReflectionAlpha` 0.2934783 under a palette darkness of 0.25.
    #[test]
    fn sprite_spans_the_surface_from_the_still_level() {
        let r = room(3);
        let water = Water::new(&r, &RoomSettings::ANCESTOR).expect("water");
        let settings = RoomSettings {
            water_reflection_alpha: 0.293_478_3,
            ..RoomSettings::ANCESTOR
        };
        let light = WaterLight::new(&r, Some(&water), &settings).unwrap();
        let list = light.draw_list(&water, 0.25);
        let [DrawItem::Sprite(s)] = list.items.as_slice() else {
            panic!("one sprite");
        };
        assert_eq!(s.pos, Vec2::new(-220.0, 70.0));
        assert_eq!(s.anchor, Vec2::ZERO);
        assert_eq!(s.scale * 16.0, Vec2::new(840.0, 225.0));
        assert!((s.color.a - 0.75 * 0.293_478_3).abs() < 1e-6);
        assert_eq!((s.shader, s.layer), (Shader::WaterLight, Layer::Foreground));
    }
}
