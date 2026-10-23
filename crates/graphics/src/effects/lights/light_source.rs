//! `LightSource.DrawSprites`: the light's disc over the level and, in a room with
//! water, its glow under the surface; its alpha is the `Alpha` property's (a placed
//! light fades with the sun). The game's sprite 0 is a 64-segment
//! `CircularSprite`; a quad draws the same, as the shader is 0 outside the inscribed
//! circle. No draws.
//!
//! Mirrors: LightSource.cs (DrawSprites)

use rw_sim::{FloatExt, LightSource, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// What `DrawSprites` reads of the camera and its room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSourceView {
    pub cam_pos: Vec2,
    /// `Room.Darkness(pos)`: the camera's palette darkness.
    pub darkness: f32,
    /// `darkenLightsFactor`: the room's `DarkenLights` amount.
    pub darken_lights: f32,
    /// `room.water`: the room has a water level.
    pub water: bool,
    /// The room's `WaterLights` amount is above 0 (`AddToContainer` makes it
    /// submersible).
    pub water_lights: bool,
    /// The rain cycle's `ShaderLight`, for the light's `Alpha`.
    pub shader_light: f32,
}

pub trait LightSourceSprites {
    fn draw(&self, view: &LightSourceView, t: f32) -> DrawList;
}

impl LightSourceSprites for LightSource {
    fn draw(&self, view: &LightSourceView, t: f32) -> DrawList {
        // No blinking (`blinkType` None): only the room's darkening dims it.
        let blink = 1.0 - view.darken_lights;
        let pos = self.last_pos.lerp(self.pos, t);
        let x = view.snap(pos.x, view.cam_pos.x);
        let rad = self.last_rad.lerp(self.rad, t);
        // `sprite.alpha` (with `colorAlpha`), then Futile's vertex alpha `color.a *
        // alpha`, where `color.a` is the setter's 1 / `colorAlpha` (D24).
        let alpha = self.last_alpha.lerp(self.alpha_now(view.shader_light), t)
            * 1.0.lerp(view.darkness, self.affected_by_palette_darkness)
            * self.color_alpha
            * blink
            * self.color_a;
        let [r, g, b] = self.color;
        let color = Color::rgb(r, g, b);
        let submersible = self.submersible || view.water_lights;
        let mut items = vec![DrawItem::Sprite(Sprite {
            index: 0,
            element: self.element.name(),
            pos: Vec2::new(x, view.snap(pos.y, view.cam_pos.y)),
            rotation: 0.0,
            scale: Vec2::new(rad / 8.0, rad / 8.0),
            anchor: Vec2::new(0.5, 0.5),
            color: color.with_alpha(alpha),
            shader: if self.flat {
                Shader::FlatLight
            } else {
                Shader::LightSource
            },
            layer: if submersible {
                Layer::Water
            } else {
                Layer::ForegroundLights
            },
        })];
        if view.water {
            let surface = self.water_surface_level;
            let under = pos
                .y
                .inverse_lerp(surface - self.rad * 0.25, surface + self.rad * 0.25);
            let fade = (1.0 - under).mathf_pow(0.5);
            let size = rad * 0.5 * fade;
            let y = pos.y.min(pos.y.lerp(surface - size * 0.5, 0.5));
            items.push(DrawItem::Sprite(Sprite {
                index: 1,
                element: "Futile_White",
                pos: Vec2::new(x, view.snap(y, view.cam_pos.y)),
                rotation: 0.0,
                scale: Vec2::new(size / 8.0, size / 8.0),
                anchor: Vec2::new(0.5, 0.5),
                color: color.with_alpha(alpha * fade),
                shader: Shader::UnderWaterLight,
                layer: Layer::Water,
            }));
        }
        DrawList { items }
    }
}

impl LightSourceView {
    /// `Mathf.Floor(p - camPos) + 0.5`, back in room pixels.
    fn snap(&self, p: f32, cam: f32) -> f32 {
        (p - cam).floor() + 0.5 + cam
    }
}

#[cfg(test)]
#[path = "tests/light_source.rs"]
mod tests;
