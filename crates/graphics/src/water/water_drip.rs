//! `WaterDrip.DrawSprites`: a thin triangle trailing the drip and a glint, in
//! the palette's black or, for a water-coloured drip, its water colours (`ApplyPalette`).
//! Mirrors: WaterDrip.cs (DrawSprites, ApplyPalette)

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::{Vec2, WaterDrip, WaterGlitter};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::core::math::Mathf;

/// What a drip reads of the room palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DripPalette {
    pub black: Color,
    pub water_color1: Color,
    pub water_color2: Color,
    pub darkness: f32,
}

impl DripPalette {
    /// `ApplyPalette`'s three colours: black, half white, white; a water-coloured drip's
    /// `Lerp(waterColor2, waterColor1, 0.5)`, `waterColor1`, white.
    #[must_use]
    pub fn colors(self, water_color: bool) -> [Color; 3] {
        if water_color {
            let mid = self.water_color2.lerp(self.water_color1, 0.5);
            [mid, self.water_color1, Color::WHITE]
        } else {
            let black = self.black;
            [black, black.lerp(Color::WHITE, 0.5), Color::WHITE]
        }
    }
}

/// The drip's sprites, drawn by the camera of its room.
pub trait WaterDripSprites {
    /// `DrawSprites` at `t` (draws on the cosmetic stream).
    fn draw(
        &self,
        glitter: WaterGlitter,
        palette: DripPalette,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList;
}

impl WaterDripSprites for WaterDrip {
    fn draw(
        &self,
        glitter: WaterGlitter,
        palette: DripPalette,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let width = if self.water_color { 1.5 } else { 1.0 };
        let head = self.last_pos.lerp(self.pos, t);
        let mut tail = self.last_last_last_pos.lerp(self.last_last_pos, t);
        if self.last_life > 0.0 && self.life <= 0.0 {
            tail = tail.lerp(head, t);
        }
        let side = (head - tail).normalized().perpendicular();
        let lightness = self.last_random_lightness.lerp_to(self.random_lightness, t);
        let num = glitter.shinyness(head, t) * lightness * palette.darkness.inverse_lerp(1.0, 0.5);
        let mut items = Vec::new();
        let colors = palette.colors(self.water_color);
        let num2 = num.inverse_lerp(0.0, 0.5);
        let color = if num2 < 0.5 {
            colors[0].lerp(colors[1], num2.inverse_lerp(0.0, 0.5))
        } else {
            colors[1].lerp(colors[2], num2.inverse_lerp(0.5, 1.0))
        };
        items.push(DrawItem::Mesh(Mesh {
            index: 0,
            vertices: vec![head + side * width, head - side * width, tail],
            uvs: Vec::new(),
            triangles: Cow::Borrowed(&[[0, 1, 2]]),
            color,
            vertex_colors: Vec::new(),
            shader: Shader::default(),
            layer: Layer::Midground,
        }));
        if num > 0.5 && rng.value() < 0.3 {
            let scale = rng.value() * 0.5 * num.inverse_lerp(0.5, 1.0) * width;
            items.push(DrawItem::Sprite(Sprite {
                index: 1,
                element: "Circle20",
                pos: head,
                rotation: 0.0,
                scale: Vec2::new(scale, scale),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::WHITE,
                shader: Shader::default(),
                layer: Layer::Midground,
            }));
        }
        DrawList { items }
    }
}

#[cfg(test)]
#[path = "tests/water_drip.rs"]
mod tests;
