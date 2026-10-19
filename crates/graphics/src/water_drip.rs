//! `WaterDrip.DrawSprites` (42328): a thin triangle trailing the drip and a glint.
//! Mirrors: WaterDrip.cs (DrawSprites)

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::{Vec2, WaterDrip, WaterGlitter};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::math::Mathf;

/// The drip's sprites, drawn by the camera of its room.
pub trait WaterDripSprites {
    /// `DrawSprites` at `t` (draws on the cosmetic stream): `black` and `darkness` are
    /// the palette's `blackColor` and `darkness`.
    fn draw(
        &self,
        glitter: WaterGlitter,
        black: Color,
        darkness: f32,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList;
}

impl WaterDripSprites for WaterDrip {
    fn draw(
        &self,
        glitter: WaterGlitter,
        black: Color,
        darkness: f32,
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
        let num = glitter.shinyness(head, t) * lightness * darkness.inverse_lerp(1.0, 0.5);
        let mut items = Vec::new();
        // The palette's water colours are not used: no ported caller makes a water drip.
        let colors = [black, black.lerp(Color::WHITE, 0.5), Color::WHITE];
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
