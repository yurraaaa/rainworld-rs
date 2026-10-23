//! The superstructure's electric objects' sprites: an `SSLightRod`'s bar (`Water`,
//! `CustomDepth`), a `ZapCoil`'s eight-vertex flare (`FlareBomb`, `Foreground` or `Water` in a
//! `WaterLights` room), a `ZapFlash`'s two discs (`FlareBomb` and `FlatLight`, `Foreground`)
//! with the light it hard-sets, and the cell lights of `SuperStructureFuses` (`Foreground`,
//! `CustomDepth`, pushed by the camera's depth). A flash makes seven draws a frame on the
//! cosmetic stream (D1), three of them only once its light exists.
//!
//! Mirrors: SSLightRod.cs (InitiateSprites, DrawSprites), ZapCoil.cs (InitiateSprites, DrawSprites, ZapFlash), SuperStructureFuses.cs (InitiateSprites, DrawSprites)

use std::borrow::Cow;

use rw_random::UnityRandom;
use rw_sim::{FloatExt, ROD_COLOR, SsLightRod, SuperStructureFuses, Vec2, ZapCoil, ZapFlash};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::math::CustomVec;

/// What `ZapFlash.DrawSprites` hard-sets on the flash's light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZapFlashLight {
    pub rad: f32,
    pub alpha: f32,
    pub color: [f32; 3],
}

/// The triangles of a coil's mesh: `Triangle(i, i + 1, i + 2)` for six strips.
const COIL_TRIANGLES: [[u16; 3]; 6] = [
    [0, 1, 2],
    [1, 2, 3],
    [2, 3, 4],
    [3, 4, 5],
    [4, 5, 6],
    [5, 6, 7],
];

/// A rod's bar.
pub trait LightRodSprites {
    /// `DrawSprites`: the bar from the object along its rotation, black toward no power.
    fn draw(&self, electric_power: f32) -> DrawList;
}

impl LightRodSprites for SsLightRod {
    fn draw(&self, electric_power: f32) -> DrawList {
        let [r, g, b] = ROD_COLOR;
        let off = (1.0 - electric_power) * 0.9;
        let color = Color::rgb(r, g, b).lerp(Color::BLACK, off);
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "pixel",
                pos: self.pos,
                rotation: self.data.rotation,
                scale: Vec2::new(4.0, self.data.length),
                anchor: Vec2::new(0.5, 0.0),
                color: color.with_alpha(1.0 - self.data.depth),
                shader: Shader::CustomDepth,
                layer: Layer::Water,
            })],
        }
    }
}

/// A coil's flare.
pub trait ZapCoilSprites {
    /// `DrawSprites` at `t` between ticks; `water_lights` is the room's `WaterLights`
    /// effect above 0 (the container the sprite was added to).
    fn draw(&self, t: f32, water_lights: bool) -> DrawList;
}

impl ZapCoilSprites for ZapCoil {
    fn draw(&self, t: f32, water_lights: bool) -> DrawList {
        let num = self.last_turned_on.lerp(self.turned_on, t);
        let [left, bottom, right, top] = self.rect;
        let mut v = Vec2::new(left as f32 * 20.0, bottom as f32 * 20.0);
        let mut v2 = Vec2::new((right + 1) as f32 * 20.0, (top + 1) as f32 * 20.0);
        let mut v3 = Vec2::new(left as f32 * 20.0, (top + 1) as f32 * 20.0);
        let mut v4 = Vec2::new((right + 1) as f32 * 20.0, bottom as f32 * 20.0);
        let reach = 120.0 * num;
        let pad = 30.0;
        let a = self.flicker[0][1].lerp(self.flicker[0][0], t);
        let b = self.flicker[1][1].lerp(self.flicker[1][0], t);
        let vertices = if self.horizontal {
            v.x -= pad;
            v3.x -= pad;
            v2.x += pad;
            v4.x += pad;
            v.y -= reach * a;
            v4.y -= reach * b;
            v3.y += reach * a;
            v2.y += reach * b;
            vec![
                v,
                v3,
                v + Vec2::new(pad, 0.0),
                v3 + Vec2::new(pad, 0.0),
                v4 + Vec2::new(-pad, 0.0),
                v2 + Vec2::new(-pad, 0.0),
                v4,
                v2,
            ]
        } else {
            v.x -= reach * a;
            v3.x -= reach * b;
            v2.x += reach * b;
            v4.x += reach * a;
            v.y -= pad;
            v4.y -= pad;
            v3.y += pad;
            v2.y += pad;
            vec![
                v3,
                v2,
                v3 + Vec2::new(0.0, -pad),
                v2 + Vec2::new(0.0, -pad),
                v + Vec2::new(0.0, pad),
                v4 + Vec2::new(0.0, pad),
                v,
                v4,
            ]
        };
        let edge = 0.4;
        let lit = self.zap_lit.inverse_lerp(0.0, 0.5) * num;
        DrawList {
            items: vec![DrawItem::Mesh(Mesh {
                index: 0,
                vertices,
                uvs: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(0.0, edge),
                    Vec2::new(1.0, edge),
                    Vec2::new(0.0, 1.0 - edge),
                    Vec2::new(1.0, 1.0 - edge),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(1.0, 1.0),
                ],
                triangles: Cow::Borrowed(&COIL_TRIANGLES),
                color: Color::rgb(lit, lit, 1.0).with_alpha(num),
                vertex_colors: Vec::new(),
                shader: Shader::FlareBomb,
                layer: if water_lights {
                    Layer::Water
                } else {
                    Layer::Foreground
                },
            })],
        }
    }
}

impl Sprite {
    /// A `Futile_White` disc of a `ZapFlash` in `Foreground`.
    fn zap_disc(index: usize, pos: Vec2, scale: f32, color: Color, shader: Shader) -> DrawItem {
        DrawItem::Sprite(Self {
            index,
            element: "Futile_White",
            pos,
            rotation: 0.0,
            scale: Vec2::new(scale, scale),
            anchor: Vec2::new(0.5, 0.5),
            color,
            shader,
            layer: Layer::Foreground,
        })
    }
}

/// A flash's discs.
pub trait ZapFlashSprites {
    /// `DrawSprites` at `t`: the two discs and, once the flash has its light, what the draw
    /// hard-sets on it (its three draws come first).
    fn draw(&self, t: f32, rng: &mut UnityRandom) -> (DrawList, Option<ZapFlashLight>);
}

impl ZapFlashSprites for ZapFlash {
    fn draw(&self, t: f32, rng: &mut UnityRandom) -> (DrawList, Option<ZapFlashLight>) {
        let num = self.last_life.lerp(self.life, t);
        let size = self.size;
        let pos = self.last_pos.lerp(self.pos, t);
        let light = self.light.map(|_| {
            let rad = 0.25f32.lerp(1.0, rng.value() * num * size) * 2400.0;
            let alpha = (num * rng.value()).mathf_pow(0.4);
            let n = (num * rng.value()).mathf_pow(4.0);
            ZapFlashLight {
                rad,
                alpha,
                color: [n, n, 1.0],
            }
        });
        let flare_scale = 0.5f32.lerp(1.0, rng.value() * num * size) * 500.0 / 16.0;
        let flare_alpha = num * rng.value();
        let flat_scale = 0.5f32.lerp(1.0, (0.5 + 0.5 * rng.value()) * num * size) * 400.0 / 16.0;
        let flat_alpha = num * rng.value();
        let items = vec![
            Sprite::zap_disc(
                0,
                pos,
                flare_scale,
                Color::rgb(0.0, 0.0, 1.0).with_alpha(flare_alpha),
                Shader::FlareBomb,
            ),
            Sprite::zap_disc(
                1,
                pos,
                flat_scale,
                Color::rgb(1.0, 1.0, 1.0).with_alpha(flat_alpha),
                Shader::FlatLight,
            ),
        ];
        (DrawList { items }, light)
    }
}

/// A fuse grid's cell lights.
pub trait FuseSprites {
    /// `DrawSprites` at `t` between ticks; `camera` is `CamPos(currentCameraPosition)`,
    /// which `ApplyDepth` pushes the cells away from. Nothing while culled.
    fn draw(&self, t: f32, camera: Vec2) -> DrawList;
}

impl FuseSprites for SuperStructureFuses {
    fn draw(&self, t: f32, camera: Vec2) -> DrawList {
        if self.culled {
            return DrawList::default();
        }
        let alpha = match self.depth {
            0 => 59.0 / 60.0,
            1 => 19.0 / 30.0,
            _ => 0.3,
        };
        let lightness = self.last_lightness.lerp(self.lightness, t);
        let depth_point = camera + Vec2::new(700.0, 533.3334);
        let push = (-4.0 + 10.0 * self.depth as f32).min(12.0);
        let malfunction = 1.0f32.lerp(0.25, self.smoothed_malfunction.mathf_pow(0.2));
        let mut items = Vec::with_capacity(self.width * self.height);
        for j in 0..self.width {
            for k in 0..self.height {
                let cell = self.cell(j, k);
                let at = (self.pos
                    + Vec2::new(5.0 + self.size * j as f32, 5.0 + self.size * k as f32))
                .apply_depth(depth_point, push);
                let color = if self.debug_mode > 0 {
                    Color::rgb(
                        if j % 2 == 0 { 1.0 } else { 0.0 },
                        if k % 2 == 0 { 1.0 } else { 0.0 },
                        if j % 2 == 1 && k % 2 == 1 { 1.0 } else { 0.0 },
                    )
                } else if self.smoothed_malfunction > 0.2
                    && cell[0] > 0.5
                    && self.in_malfunction_rect(j, k)
                {
                    Color::rgb(malfunction * self.power_flicker, 0.0, 0.0)
                } else if cell[3] == 5.0 {
                    let blink = if (cell[4] as i32) % 8 > 3 { 1.0 } else { 0.0 };
                    Color::rgb(blink * malfunction * self.power_flicker, 0.0, 0.0)
                } else {
                    let level = cell[1].lerp(cell[0], t).max(0.0).mathf_pow(2.5 - lightness)
                        * 0.3f32.lerp(1.0, lightness)
                        * 1.0f32.lerp(0.5, self.smoothed_malfunction.mathf_pow(0.2))
                        * self.power_flicker;
                    Color::rgb(0.0, 0.0, level)
                };
                items.push(DrawItem::Sprite(Sprite {
                    index: j * self.height + k,
                    element: "Futile_White",
                    pos: at,
                    rotation: 0.0,
                    scale: Vec2::new(self.size / 16.0, self.size / 16.0),
                    anchor: Vec2::new(0.5, 0.5),
                    color: color.with_alpha(alpha),
                    shader: Shader::CustomDepth,
                    layer: Layer::Foreground,
                }));
            }
        }
        DrawList { items }
    }
}

#[cfg(test)]
#[path = "ss_electric_tests.rs"]
mod tests;
