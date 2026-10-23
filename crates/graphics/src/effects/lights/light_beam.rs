//! `LightBeam.DrawSprites`: the beam's grid mesh in `ForegroundLights` (`Water` in a
//! room with `WaterLights`) whose vertex colour is the palette-and-environment colour
//! over its largest component, its alpha the data's alpha with the sun's fade inside the
//! alpha's third. The colour is only rebuilt when the alpha or the camera position
//! changes (`UpdateColor`), as in the game. Blinking and night beams are not ported
//! (D24). No draws.
//!
//! `SpotLight.DrawSprites`: the quad's grid mesh in `Shadows` with black vertices, drawn
//! by the `VectorCircle` shader as a black ellipse inscribed in the quad. No draws.
//!
//! Mirrors: LightBeam.cs (DrawSprites, UpdateColor, UpdateVerts, AddToContainer), SpotLight.cs (InitiateSprites, DrawSprites, UpdateVerts, AddToContainer)

use std::borrow::Cow;

use rw_sim::{FloatExt, LightBeam, SpotLight};

use crate::core::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader};
use crate::effects::lights::quad_grid::QuadGrid;

/// What `DrawSprites` reads of the camera and its room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightBeamView {
    /// `rCam.currentCameraPosition`.
    pub camera: usize,
    /// The rain cycle's `ShaderLight`.
    pub shader_light: f32,
    /// `darkenLightsFactor`: the room's `DarkenLights` amount.
    pub darken_lights: f32,
    /// The room's `WaterLights` amount is above 0.
    pub water_lights: bool,
}

/// The beam's drawing state (`lastAlpha`, `lastCamPos`, `meshDirty` and the vertex
/// colour the last `UpdateColor` set).
#[derive(Debug, Clone, PartialEq)]
pub struct LightBeamSprites {
    pub last_alpha: f32,
    pub last_camera: Option<usize>,
    pub mesh_dirty: bool,
    pub color: Color,
}

impl Default for LightBeamSprites {
    fn default() -> Self {
        Self {
            last_alpha: 0.0,
            last_camera: None,
            mesh_dirty: true,
            color: Color::default(),
        }
    }
}

impl LightBeamSprites {
    /// `InitiateSprites` sets `meshDirty` again when the camera enters the room.
    pub fn initiate(&mut self) {
        self.mesh_dirty = true;
    }

    /// `UpdateColor`: `Lerp(Lerp(paletteLitColor, white, colorA), environmentColor,
    /// colorB)` over its largest component (the `color` setter; a black colour divides
    /// by 0 as the game's does), with alpha `a`.
    fn update_color(&mut self, beam: &LightBeam, a: f32) {
        let lit = Color::from(beam.palette_lit_color).lerp(Color::WHITE, beam.data.color_a);
        let c = lit.lerp(Color::from(beam.environment_color), beam.data.color_b);
        let mut color_alpha = 0.0f32;
        for channel in [c.r, c.g, c.b] {
            if channel > color_alpha {
                color_alpha = channel;
            }
        }
        self.color = Color {
            r: c.r / color_alpha,
            g: c.g / color_alpha,
            b: c.b / color_alpha,
            a,
        };
    }

    /// The alpha of this frame: the data's alpha in thirds, the sun's fade applied to
    /// the position inside its third, then the room's darkening.
    #[must_use]
    pub fn frame_alpha(beam: &LightBeam, view: &LightBeamView) -> f32 {
        let alpha = beam.data.alpha;
        let third = 1.0f32 / 3.0;
        let base = (alpha * 3.0).floor();
        let mut x = alpha.inverse_lerp(third * base, third * (base + 1.0));
        if beam.data.sun {
            x *= view.shader_light.inverse_lerp(-0.2, 0.0).mathf_pow(1.2);
        }
        x = (third * base).lerp(third * (base + 1.0), x);
        // `nightFade` and `BlinkFade()` are 1 (D24).
        x = (x - third * base) * 1.0 * 1.0 + third * base;
        x * (1.0 - view.darken_lights)
    }

    #[must_use]
    pub fn draw(&mut self, beam: &LightBeam, view: &LightBeamView) -> DrawList {
        if self.mesh_dirty {
            self.update_color(beam, self.last_alpha);
            self.mesh_dirty = false;
        }
        let a = Self::frame_alpha(beam, view);
        if a != self.last_alpha {
            self.update_color(beam, a);
            self.last_alpha = a;
        }
        if self.last_camera != Some(view.camera) {
            self.last_camera = Some(view.camera);
            self.update_color(beam, a);
        }
        let grid = QuadGrid::new(&beam.quad, 250.0);
        DrawList {
            items: vec![DrawItem::Mesh(Mesh {
                index: 0,
                vertices: grid.vertices,
                uvs: grid.uvs,
                triangles: Cow::Owned(grid.triangles),
                color: self.color,
                vertex_colors: Vec::new(),
                shader: Shader::LightBeam,
                layer: if view.water_lights {
                    Layer::Water
                } else {
                    Layer::ForegroundLights
                },
            })],
        }
    }
}

pub trait SpotLightSprites {
    fn draw(&self) -> DrawList;
}

impl SpotLightSprites for SpotLight {
    fn draw(&self) -> DrawList {
        let grid = QuadGrid::new(&self.quad, 150.0);
        DrawList {
            items: vec![DrawItem::Mesh(Mesh {
                index: 0,
                vertices: grid.vertices,
                uvs: grid.uvs,
                triangles: Cow::Owned(grid.triangles),
                // `new Color(0f, 0f, 0f)`: opaque black, which the shader reads as
                // "no cut-out" (`1 - a = 0`).
                color: Color::BLACK,
                vertex_colors: Vec::new(),
                shader: Shader::VectorCircle,
                layer: Layer::Shadows,
            })],
        }
    }
}

#[cfg(test)]
#[path = "tests/light_beam.rs"]
mod tests;
