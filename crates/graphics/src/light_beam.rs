//! `LightBeam.DrawSprites`: the beam's grid mesh in `ForegroundLights` (`Water` in a
//! room with `WaterLights`) whose vertex colour is the palette-and-environment colour
//! over its largest component, its alpha the data's alpha with the sun's fade inside the
//! alpha's third. The colour is only rebuilt when the alpha or the camera position
//! changes (`UpdateColor`), as in the game. Blinking and night beams are not ported
//! (D24). No draws.
//!
//! Mirrors: LightBeam.cs (DrawSprites, UpdateColor, UpdateVerts, AddToContainer)

use std::borrow::Cow;

use rw_sim::{FloatExt, LightBeam};

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader};
use crate::quad_grid::QuadGrid;

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

#[cfg(test)]
mod tests {
    use rw_assets::LightBeamData;
    use rw_sim::{LightBeam, Vec2};

    use super::{LightBeamSprites, LightBeamView};
    use crate::draw::{DrawItem, Layer, Shader};

    fn view() -> LightBeamView {
        LightBeamView {
            camera: 0,
            shader_light: 1.0,
            darken_lights: 0.0,
            water_lights: false,
        }
    }

    fn beam(alpha: f32, color_a: f32, color_b: f32, sun: bool) -> LightBeam {
        let data = LightBeamData::parse(&format!(
            "0~40~40~40~40~0~0~0~{alpha}~{color_a}~{color_b}~{}",
            u8::from(sun)
        ))
        .expect("data");
        let mut b = LightBeam::new(0, Vec2::new(10.0, 20.0), data);
        b.palette_lit_color = [0.5, 0.25, 0.0, 1.0];
        b
    }

    /// With the sun at full strength the alpha is the data's; the third it lies in
    /// is unchanged by the sun's fade of the position inside it.
    #[test]
    fn alpha_in_thirds() {
        let b = beam(0.5, 0.0, 0.0, true);
        assert!((LightBeamSprites::frame_alpha(&b, &view()) - 0.5).abs() < 1e-6);
        // ShaderLight -0.1: InverseLerp(-0.2, 0, -0.1) = 0.5, ^1.2; base = 1 (0.333..),
        // x = InverseLerp(1/3, 2/3, 0.5) = 0.5.
        let mut v = view();
        v.shader_light = -0.1;
        let x = 0.5f32 * 0.5f32.powf(1.2);
        let want = (1.0 / 3.0) + x * (1.0 / 3.0);
        assert!((LightBeamSprites::frame_alpha(&b, &v) - want).abs() < 1e-6);
        // Full dark: down to the third's floor. The room's darkening scales it.
        v.shader_light = -1.0;
        assert!((LightBeamSprites::frame_alpha(&b, &v) - 1.0 / 3.0).abs() < 1e-6);
        v.darken_lights = 0.25;
        assert!((LightBeamSprites::frame_alpha(&b, &v) - 0.25).abs() < 1e-6);
        // Without the sun flag the shader light is ignored.
        let b = beam(0.5, 0.0, 0.0, false);
        assert!((LightBeamSprites::frame_alpha(&b, &v) - 0.375).abs() < 1e-6);
    }

    /// The colour over its largest component: the palette's (0.5, 0.25, 0) becomes
    /// (1, 0.5, 0); with `colorA` 1 it is white; the mesh is in `ForegroundLights`.
    #[test]
    fn colour_over_its_largest_component() {
        let mut s = LightBeamSprites::default();
        let b = beam(0.5, 0.0, 0.0, true);
        let list = s.draw(&b, &view());
        let DrawItem::Mesh(m) = &list.items[0] else {
            panic!("a mesh");
        };
        assert_eq!(
            (m.shader, m.layer),
            (Shader::LightBeam, Layer::ForegroundLights)
        );
        assert_eq!((m.color.r, m.color.g, m.color.b), (1.0, 0.5, 0.0));
        assert!((m.color.a - 0.5).abs() < 1e-6);
        let mut s = LightBeamSprites::default();
        let list = s.draw(&beam(0.5, 1.0, 0.0, true), &view());
        let DrawItem::Mesh(m) = &list.items[0] else {
            panic!("a mesh");
        };
        assert_eq!((m.color.r, m.color.g, m.color.b), (1.0, 1.0, 1.0));
    }

    /// `UpdateColor` runs only on a first draw, an alpha change or a new camera
    /// position: an environment colour that changes in between is not drawn until then.
    #[test]
    fn colour_is_rebuilt_only_on_alpha_or_camera_change() {
        let mut s = LightBeamSprites::default();
        let mut b = beam(0.5, 0.0, 1.0, true);
        b.environment_color = [0.0, 1.0, 0.0, 1.0];
        let first = s.draw(&b, &view());
        b.environment_color = [0.0, 0.0, 1.0, 1.0];
        let same = s.draw(&b, &view());
        assert_eq!(first, same);
        let mut v = view();
        v.camera = 1;
        let DrawItem::Mesh(m) = &s.draw(&b, &v).items[0] else {
            panic!("a mesh");
        };
        assert_eq!((m.color.g, m.color.b), (0.0, 1.0));
    }

    /// A mesh in a room with `WaterLights` is in `Water`; a 400 px quad is a 2 x 2 grid.
    #[test]
    fn water_lights_layer_and_grid() {
        let mut s = LightBeamSprites::default();
        let mut v = view();
        v.water_lights = true;
        let data = LightBeamData::parse("0~400~400~400~400~0~0~0~0.4~0~0~1").expect("data");
        let b = LightBeam::new(0, Vec2::ZERO, data);
        let DrawItem::Mesh(m) = &s.draw(&b, &v).items[0] else {
            panic!("a mesh");
        };
        assert_eq!(
            (m.layer, m.vertices.len(), m.triangles.len()),
            (Layer::Water, 9, 8)
        );
    }
}
