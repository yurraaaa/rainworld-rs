//! The superstructure's screen-grab objects' sprites: a `DeepProcessing`'s grid mesh over its
//! quad (`Foreground`, the `DeepProcessing` shader, the vertex colour `(fromDepth, toDepth,
//! power, intensity)`) and a `GravityDisruptor`'s distorting disc (`HUD`, the
//! `GravityDisruptor` shader, which grabs the frame) with its sixteen streaks (`Foreground`,
//! `CustomDepth`, pushed by the camera's depth) and the pixels of its `DisruptorSpeck`s
//! (`Foreground`). None of them draws random numbers.
//!
//! Mirrors: DeepProcessing.cs (InitiateSprites, DrawSprites, UpdateVerts), GravityDisruptor.cs (InitiateSprites, DrawSprites, DisruptorSpeck)

use std::borrow::Cow;

use rw_sim::{DeepProcessing, DisruptorSpeck, FloatExt, GravityDisruptor, STREAKS, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader, Sprite};
use crate::math::CustomVec;
use crate::quad_grid::QuadGrid;

/// A `DeepProcessing`'s mesh.
pub trait DeepProcessingSprites {
    /// `DrawSprites`: the grid over the quad, every vertex coloured `(fromDepth, toDepth,
    /// power, intensity)`.
    fn draw(&self) -> DrawList;
}

impl DeepProcessingSprites for DeepProcessing {
    fn draw(&self) -> DrawList {
        let grid = QuadGrid::new(&self.quad, 150.0);
        DrawList {
            items: vec![DrawItem::Mesh(Mesh {
                index: 0,
                vertices: grid.vertices,
                uvs: grid.uvs,
                triangles: Cow::Owned(grid.triangles),
                color: Color {
                    r: self.data.from_depth,
                    g: self.data.to_depth,
                    b: self.power,
                    a: self.data.intensity,
                },
                vertex_colors: Vec::new(),
                shader: Shader::DeepProcessing,
                layer: Layer::Foreground,
            })],
        }
    }
}

/// A disruptor's disc and streaks.
pub trait GravityDisruptorSprites {
    /// `DrawSprites` at `t` between ticks; `camera` is `CamPos(currentCameraPosition)`,
    /// which `ApplyDepth` pushes the sprites from.
    fn draw(&self, t: f32, camera: Vec2) -> DrawList;
}

impl GravityDisruptorSprites for GravityDisruptor {
    fn draw(&self, t: f32, camera: Vec2) -> DrawList {
        let power = self.last_power.lerp(self.power, t);
        let depth_point = camera + Vec2::new(700.0, 533.3334);
        let at = self
            .placed_pos
            .apply_depth(depth_point, -3.0 + 10.0 * self.depth as f32);
        let alpha = 1.0 - (10.0 * self.depth as f32 + 2.0) / 30.0;
        let mut items = vec![DrawItem::Sprite(Sprite {
            index: 0,
            element: "Futile_White",
            pos: at,
            rotation: 0.0,
            scale: Vec2::new(37.5, 37.5),
            anchor: Vec2::new(0.5, 0.5),
            color: Color::WHITE.with_alpha(power),
            shader: Shader::GravityDisruptor,
            layer: Layer::Hud,
        })];
        for i in 0..STREAKS {
            let color = if self.debug_mode > 0 {
                Color::rgb(
                    if i % 2 == 0 { 1.0 } else { 0.0 },
                    if i % 2 == 1 { 1.0 } else { 0.0 },
                    0.0,
                )
            } else {
                Color::rgb(
                    0.0,
                    0.0,
                    self.lights[i][1].lerp(self.lights[i][0], t) * power,
                )
            };
            items.push(DrawItem::Sprite(Sprite {
                index: i + 1,
                element: "Futile_White",
                pos: at,
                rotation: i as f32 / 16.0 * 360.0,
                scale: Vec2::new(0.9375, 3.125),
                anchor: Vec2::new(0.5, -0.55),
                color: color.with_alpha(alpha),
                shader: Shader::CustomDepth,
                layer: Layer::Foreground,
            }));
        }
        DrawList { items }
    }
}

/// A `DisruptorSpeck`'s pixel.
pub trait DisruptorSpeckSprites {
    /// `DrawSprites` at `t`: a grey pixel as bright as the disruptor's `power`.
    fn draw(&self, power: f32, t: f32) -> DrawList;
}

impl DisruptorSpeckSprites for DisruptorSpeck {
    fn draw(&self, power: f32, t: f32) -> DrawList {
        DrawList {
            items: vec![DrawItem::Sprite(Sprite {
                index: 0,
                element: "pixel",
                pos: self.last_pos.lerp(self.pos, t),
                rotation: 0.0,
                scale: Vec2::new(1.0, 1.0),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::rgb(power, power, power),
                shader: Shader::Basic,
                layer: Layer::Foreground,
            })],
        }
    }
}

#[cfg(test)]
#[path = "ss_grab_tests.rs"]
mod tests;
