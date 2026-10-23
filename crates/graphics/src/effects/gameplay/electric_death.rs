//! `ElectricDeath`'s sprites in the `Water` container: the full-screen web (`Futile_White`
//! scaled 87.5 x 50 on the `ElectricDeath` shader) and nine green `LightSource` discs, the
//! first of which the game never updates (a unit disc at the screen's lower-left corner),
//! and a `SparkFlash`'s three (`LightSource`, `FlatLight` and `FlareBomb` discs; the game's
//! `*BothSides` shaders are these without the Watcher). Their positions are screen pixels,
//! so they are placed at the frame's camera position plus the screen point. The draws they
//! make (two a light, six a flash) are on the cosmetic stream (D1); omitted branches and
//! the NaN first frame: D33.
//!
//! Mirrors: ElectricDeath.cs (InitiateSprites, DrawSprites, LightFlash.Draw, SparkFlash.InitiateSprites, SparkFlash.DrawSprites)

use rw_random::UnityRandom;
use rw_sim::{ElectricDeath, FloatExt, SparkFlash, Vec2};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};
use crate::core::math::Mathf;

impl Sprite {
    /// A green `Futile_White` disc in `Water`.
    fn green_disc(index: usize, pos: Vec2, scale: f32, alpha: f32, shader: Shader) -> DrawItem {
        DrawItem::Sprite(Self {
            index,
            element: "Futile_White",
            pos,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::rgb(0.0, 1.0, 0.0).with_alpha(alpha),
            shader,
            layer: Layer::Water,
        })
    }
}

pub trait ElectricDeathSprites {
    /// `DrawSprites` at `t` between ticks with the frame's `cam_pos` and `screen`
    /// (`rainWorld.screenSize`); `intensity` is `Intensity` now.
    fn draw(
        &self,
        intensity: f32,
        cam_pos: Vec2,
        screen: Vec2,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList;
}

impl ElectricDeathSprites for ElectricDeath {
    fn draw(
        &self,
        intensity: f32,
        cam_pos: Vec2,
        screen: Vec2,
        t: f32,
        rng: &mut UnityRandom,
    ) -> DrawList {
        let sin = self.last_sin.lerp(self.sin, t);
        let alpha = intensity.mathf_pow(0.5)
            * (0.5 + 0.5 * (sin * std::f32::consts::PI * 2.0).mathf_sin())
                .lerp(1.0, intensity.mathf_pow(4.0));
        let color = Color::rgb(
            self.last_color.x.lerp(self.color.x, t),
            self.last_color.y.lerp(self.color.y, t),
            self.last_color.z.lerp(self.color.z, t),
        )
        .with_alpha(alpha);
        let mut items = vec![DrawItem::Sprite(Sprite {
            index: 0,
            element: "Futile_White",
            pos: cam_pos + screen * 0.5,
            rotation: 0.0,
            scale: Vec2::new(87.5, 50.0),
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader: Shader::ElectricDeath,
            layer: Layer::Water,
        })];
        // `flashes[0]` is sprite 1, never updated or drawn: `FSprite`'s defaults.
        items.push(Sprite::green_disc(
            1,
            cam_pos,
            1.0,
            1.0,
            Shader::LightSource,
        ));
        for (i, flash) in self.flashes.iter().enumerate().skip(1) {
            let num = flash.last_life.lerp(flash.life, t);
            let sine = (num * std::f32::consts::PI).mathf_sin();
            let scale = 0.5f32.lerp(1.0, sine) * 0.8f32.lerp(1.2, rng.value()) * flash.rad / 8.0;
            let alpha = sine * 0.6f32.lerp(1.0, rng.value()) * 0.6 * intensity;
            items.push(Sprite::green_disc(
                i + 1,
                cam_pos + flash.pos,
                scale,
                alpha,
                Shader::LightSource,
            ));
        }
        DrawList { items }
    }
}

pub trait SparkFlashSprites {
    /// `DrawSprites` at `t`: three discs at the flash's position.
    fn sprites(&self, t: f32, rng: &mut UnityRandom) -> Vec<DrawItem>;
}

impl SparkFlashSprites for SparkFlash {
    fn sprites(&self, t: f32, rng: &mut UnityRandom) -> Vec<DrawItem> {
        let num = self.last_life.lerp(self.life, t);
        let sine = (num * std::f32::consts::PI).mathf_sin();
        let wide = sine.mathf_pow(0.5);
        let size = 20.0f32.lerp(120.0, self.size.mathf_pow(1.5));
        let a = wide * 0.8f32.lerp(1.2, rng.value()) * size * 4.0 / 8.0;
        let a_alpha = wide * 0.6f32.lerp(1.0, rng.value());
        let b = wide * 0.8f32.lerp(1.2, rng.value()) * size * 4.0 / 8.0;
        let b_alpha = wide * 0.6f32.lerp(1.0, rng.value()) * 0.2;
        let c = 0.5f32.lerp(1.0, sine) * 0.8f32.lerp(1.2, rng.value()) * size / 8.0;
        let c_alpha = sine * rng.value();
        vec![
            Sprite::green_disc(0, self.pos, a, a_alpha, Shader::LightSource),
            Sprite::green_disc(1, self.pos, b, b_alpha, Shader::FlatLight),
            Sprite::green_disc(2, self.pos, c, c_alpha, Shader::FlareBomb),
        ]
    }
}

#[cfg(test)]
#[path = "tests/electric_death.rs"]
mod tests;
