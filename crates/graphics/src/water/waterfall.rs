//! `WaterFall`'s sprites: one `Futile_White` quad under the `WaterFall` shader over the
//! fall (its colour carries the density and the two ends' fade), then a `LizardBubble5`
//! per bubble coloured between the water colours; all in `Background`. The inverted-water
//! variant is Remix/DLC (D30). No draws.
//!
//! Mirrors: WaterFall.cs (InitiateSprites, DrawSprites, ApplyPalette, AddToContainer)

use rw_assets::RoomPalette;
use rw_sim::{FloatExt, Vec2, Water, WaterFall};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// A fall's sprites.
pub trait WaterFallSprites {
    /// `DrawSprites` at `t` between ticks after `ApplyPalette`; `water` is the room's
    /// (it sets the strike level).
    fn draw(&self, water: Option<&Water>, palette: &RoomPalette, t: f32) -> DrawList;
}

impl WaterFallSprites for WaterFall {
    fn draw(&self, water: Option<&Water>, palette: &RoomPalette, t: f32) -> DrawList {
        let mut items = Vec::new();
        if !self.flooded(water) {
            let strike = WaterFall::strike_level(water);
            let top = self.top_pos[1].lerp(self.top_pos[0], t);
            let bottom = self.bottom_pos[1].lerp(self.bottom_pos[0], t);
            let covered = ((top - bottom) / self.float_length(water)).min(1.0);
            let near_top = top.inverse_lerp(self.pos.y, strike).mathf_pow(0.2);
            let near_bottom = bottom.inverse_lerp(strike, self.pos.y).mathf_pow(0.2);
            let top_fade = near_top.lerp(1.0, 1.0 - covered);
            let bottom_fade = near_bottom.lerp(1.0, 1.0 - covered);
            items.push(DrawItem::Sprite(Sprite {
                index: 0,
                element: "Futile_White",
                pos: Vec2::new(self.pos.x, bottom),
                rotation: 0.0,
                scale: Vec2::new(self.width as f32 * 20.0 / 16.0, (bottom - top) / 16.0),
                anchor: Vec2::new(0.0, 1.0),
                color: Color::rgb(
                    self.last_visual_dens.lerp(self.visual_density, t),
                    1.0 / 100.0_f32.lerp(2.0, top_fade),
                    1.0 / 100.0_f32.lerp(2.0, bottom_fade),
                ),
                shader: Shader::WaterFall,
                layer: Layer::Background,
            }));
        }
        let color = Color::from(palette.water_color1).lerp(Color::from(palette.water_color2), 0.3);
        items.extend(self.bubbles.iter().enumerate().map(|(i, b)| {
            let scale = b[3].x.mathf_pow(0.5);
            DrawItem::Sprite(Sprite {
                index: 1 + i,
                element: "LizardBubble5",
                pos: b[1].lerp(b[0], t),
                rotation: 0.0,
                scale: Vec2::new(scale, scale),
                anchor: Vec2::new(0.5, 0.5),
                color,
                shader: Shader::Basic,
                layer: Layer::Background,
            })
        }));
        DrawList { items }
    }
}

#[cfg(test)]
#[path = "tests/waterfall.rs"]
mod tests;
