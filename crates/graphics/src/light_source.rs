//! `LightSource.DrawSprites`: the light's disc over the level and, in a room with
//! water, its glow under the surface; its alpha is the `Alpha` property's (a placed
//! light fades with the sun). The game's sprite 0 is a 64-segment
//! `CircularSprite`; a quad draws the same, as the shader is 0 outside the inscribed
//! circle. No draws.
//!
//! Mirrors: LightSource.cs (DrawSprites)

use rw_sim::{FloatExt, LightSource, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

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
        let alpha = self.last_alpha.lerp(self.alpha_now(view.shader_light), t)
            * 1.0.lerp(view.darkness, self.affected_by_palette_darkness)
            * self.color_alpha
            * blink;
        let [r, g, b] = self.color;
        let color = Color::rgb(r, g, b);
        let submersible = self.submersible || view.water_lights;
        let mut items = vec![DrawItem::Sprite(Sprite {
            index: 0,
            element: "Futile_White",
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
mod tests {
    use rw_sim::{LightHardSet, LightSource, Vec2};

    use super::{LightSourceSprites, LightSourceView};
    use crate::draw::{DrawItem, Layer, Shader};

    fn view(water: bool) -> LightSourceView {
        LightSourceView {
            cam_pos: Vec2::new(10.25, 20.0),
            darkness: 0.9,
            darken_lights: 0.0,
            water,
            water_lights: false,
            shader_light: 1.0,
        }
    }

    fn disc_alpha(list: &crate::draw::DrawList) -> f32 {
        let DrawItem::Sprite(s) = &list.items[0] else {
            panic!("a sprite");
        };
        s.color.a
    }

    fn light() -> LightSource {
        let mut l = LightSource::new(1, 0, Vec2::ZERO, false, [1.0, 0.8, 0.3], None);
        l.hard_set(LightHardSet {
            pos: Vec2::new(100.7, 50.2),
            rad: 160.0,
            alpha: 0.1,
        });
        l
    }

    /// The guide light's disc: at the floored stage position plus half a pixel, scale
    /// rad / 8, alpha `alpha * Lerp(1, darkness, 1) * colorAlpha` in `ForegroundLights`.
    #[test]
    fn disc_in_foreground_lights() {
        let list = light().draw(&view(false), 0.3);
        assert_eq!(list.items.len(), 1);
        let DrawItem::Sprite(s) = &list.items[0] else {
            panic!("a sprite");
        };
        assert_eq!(s.shader, Shader::LightSource);
        assert_eq!(s.layer, Layer::ForegroundLights);
        assert_eq!(s.pos, Vec2::new(90.5 + 10.25, 30.5 + 20.0));
        assert_eq!(s.scale, Vec2::new(20.0, 20.0));
        assert!((s.color.a - 0.09).abs() < 1e-6);
        assert_eq!((s.color.r, s.color.g, s.color.b), (1.0, 0.8, 0.3));
    }

    /// `Lerp(lastAlpha, Alpha, t)`: a light fading with the sun at `ShaderLight` -0.25
    /// draws `0.1 * 0.5^1.2` at t = 1, its `lastAlpha` at t = 0.
    #[test]
    fn alpha_fades_with_the_sun() {
        let mut l = light();
        l.fade_with_sun = true;
        let mut v = view(false);
        v.shader_light = -0.25;
        let want = 0.1 * 0.5f32.powf(1.2) * 0.9;
        assert!((disc_alpha(&l.draw(&v, 1.0)) - want).abs() < 1e-6);
        assert!((disc_alpha(&l.draw(&v, 0.0)) - 0.09).abs() < 1e-6);
    }

    /// With water, the under-water glow: above the surface by more than rad / 4 it has
    /// no size or alpha; deep under it, half the rad, at the light, full alpha.
    #[test]
    fn under_water_glow() {
        let mut l = light();
        l.water_surface_level = 0.0;
        let list = l.draw(&view(true), 1.0);
        let DrawItem::Sprite(s) = &list.items[1] else {
            panic!("a sprite");
        };
        assert_eq!((s.shader, s.layer), (Shader::UnderWaterLight, Layer::Water));
        assert_eq!((s.scale.x, s.color.a), (0.0, 0.0));
        l.water_surface_level = 500.0;
        let list = l.draw(&view(true), 1.0);
        let DrawItem::Sprite(s) = &list.items[1] else {
            panic!("a sprite");
        };
        assert_eq!(s.scale, Vec2::new(10.0, 10.0));
        assert_eq!(s.pos, Vec2::new(90.5 + 10.25, 30.5 + 20.0));
        assert!((s.color.a - 0.09).abs() < 1e-6);
    }
}
