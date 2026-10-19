//! The game's water drawing on the CPU: `Water.InitiateSprites` / `DrawSprites`
//! building the surface band and the deep water body as `WaterTriangleMesh`es, coloured
//! from the room palette. `water.wgsl` (`WaterSurface` / `DeepWater`) shades them.
//! GPU-free. Behaviour: `rainworld-re/notes/water-rendering.md`, base game.
//!
//! Mirrors: Water.cs, WaterTriangleMesh.cs

use bytemuck::{Pod, Zeroable};
use rw_assets::RoomPalette;
use rw_graphics::Mathf;
use rw_sim::Vec2;

use crate::WaterSurface;

/// One triangle corner: stage position (room pixels minus the camera position), `uv`
/// (`Futile_White`'s corner, both components equal) and colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct WaterVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// `WaterTriangleMesh` over `Futile_White` with triangles `(l, l+1, l+2)`: positions and
/// colours persist between frames, as the game's arrays do.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterTriangleMesh {
    vertices: Vec<Vec2>,
    /// Per-vertex colours (`customColor`), else `None` and every vertex gets `color`.
    colors: Option<Vec<[f32; 4]>>,
    color: [f32; 4],
}

/// The followed player's light in the water body (`DeepWater`'s vertex colour).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerLight {
    /// Main body chunk at its draw position, room pixels.
    pub pos: [f32; 2],
    /// `outOfBreathFac` this tick.
    pub out_of_breath: [f32; 4],
}

/// A room `LightSource` as `Water.GetSurfaceColor` reads it (as it was before this
/// frame's drawables hard-set it: `Water` draws first).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceLight {
    /// `Pos`, room pixels.
    pub pos: [f32; 2],
    pub rad: f32,
    /// `color`'s rgb.
    pub color: [f32; 3],
    /// `Alpha`.
    pub alpha: f32,
}

/// What `Water.DrawSprites` reads in one frame.
#[derive(Debug, Clone, Copy)]
pub struct WaterFrame<'a> {
    pub surface: &'a WaterSurface,
    /// `PreviousPoint(camPos.x - 30)` on this tick's front points.
    pub first_point: usize,
    /// The camera's draw position (stage origin), room pixels.
    pub cam_pos: [f32; 2],
    /// `rCam.sSize`.
    pub screen_size: [f32; 2],
    pub palette: &'a RoomPalette,
    /// `waterGlitterCycle` between ticks.
    pub glitter: f32,
    /// `None` unless the camera follows a live player in this room.
    pub light: Option<PlayerLight>,
    /// The room's light sources.
    pub lights: &'a [SurfaceLight],
    /// `Room.Darkness`: the camera's palette darkness.
    pub darkness: f32,
    pub time_stacker: f32,
}

/// The two sprites of the room's `Water` (base game: one surface, no air pockets),
/// drawn in this order into the `Water` container.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterSprites {
    points_to_render: usize,
    /// Sprite 0, `WaterSurface` shader.
    pub surface: WaterTriangleMesh,
    /// Sprite 1, `DeepWater` shader.
    pub deep: WaterTriangleMesh,
}

/// Colour helpers on RGBA arrays (Unity `Color`).
trait ColorExt {
    /// `Color.Lerp`, `t` clamped.
    fn lerp_color(self, to: Self, t: f32) -> Self;
}

impl ColorExt for [f32; 4] {
    fn lerp_color(self, to: Self, t: f32) -> Self {
        std::array::from_fn(|i| self[i].lerp_to(to[i], t))
    }
}

/// `Custom.ApplyDepthOnVector`.
pub trait ApplyDepth {
    fn apply_depth(self, depth_point: Self, d: f32) -> Self;
}

impl ApplyDepth for Vec2 {
    fn apply_depth(self, depth_point: Self, d: f32) -> Self {
        let scale = (10.0 - d * -0.025) * 0.1;
        (self - depth_point) / scale + depth_point
    }
}

impl WaterTriangleMesh {
    /// The strip of `2 * points` triangles over `2 * points + 2` vertices at (0, 0),
    /// colours white (`_alphaColor`) when `custom_color`.
    #[must_use]
    pub fn strip(points: usize, custom_color: bool) -> Self {
        let count = 2 * points + 2;
        Self {
            vertices: vec![Vec2::ZERO; count],
            colors: custom_color.then(|| vec![[1.0; 4]; count]),
            color: [1.0; 4],
        }
    }

    #[must_use]
    pub fn vertices(&self) -> &[Vec2] {
        &self.vertices
    }

    /// Colour of vertex `i` as Futile hands it to the shader.
    #[must_use]
    pub fn vertex_color(&self, i: usize) -> [f32; 4] {
        self.colors.as_ref().map_or(self.color, |c| c[i])
    }

    /// The sprite colour (used for every vertex without custom colours).
    #[must_use]
    pub fn color(&self) -> [f32; 4] {
        self.color
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.vertices.len() - 2
    }

    fn move_vertex(&mut self, i: usize, pos: Vec2) {
        self.vertices[i] = pos;
    }

    fn set_vertex_color(&mut self, i: usize, color: [f32; 4]) {
        if let Some(colors) = &mut self.colors {
            colors[i] = color;
        }
    }

    /// The triangles as Unity's mesh gets them (`PopulateRenderLayer`): corners
    /// `(l, l+1, l+2)`, uvs (0,0),(1,1),(0,0) for even `l` and (1,1),(0,0),(1,1) for odd.
    pub fn push_triangles(&self, out: &mut Vec<WaterVertex>) {
        for l in 0..self.triangle_count() {
            let even = l % 2 == 0;
            for (corner, i) in (l..l + 3).enumerate() {
                let uv = if (corner == 1) == even { 1.0 } else { 0.0 };
                out.push(WaterVertex {
                    pos: self.vertices[i].to_array(),
                    uv: [uv, uv],
                    color: self.vertex_color(i),
                });
            }
        }
    }
}

impl PlayerLight {
    /// `DeepWater`'s vertex colour: the player's screen position as fractions of the
    /// screen and the breath factor `b` (1 unless out of breath).
    #[must_use]
    pub fn color(&self, cam_pos: [f32; 2], screen: [f32; 2], t: f32) -> [f32; 4] {
        let f = self.out_of_breath;
        let short = f[1].lerp_to(f[0], t);
        let phase = f[3].lerp_to(f[2], t);
        let b = 1.0_f32.lerp_to(0.75 + 0.25 * (phase * 2.0).sin(), short);
        let x = self.pos[0] - cam_pos[0];
        let y = self.pos[1] - cam_pos[1];
        [
            x.inverse_lerp(0.0, screen[0]),
            y.inverse_lerp(0.0, screen[1]),
            b,
            1.0,
        ]
    }
}

impl SurfaceLight {
    /// One light's turn in `GetSurfaceColor`'s loop: `Custom.Screen(color, b)` (opaque),
    /// `b` its colour by alpha, nearness above the point (and within 0.7 rad below
    /// it), closeness to the surface normal's line, darkness and a rightward tangent.
    #[must_use]
    pub fn screen_on(
        &self,
        color: [f32; 4],
        point: Vec2,
        tangent: Vec2,
        darkness: f32,
    ) -> [f32; 4] {
        let pos = Vec2::from(self.pos);
        let mut near = pos.y.inverse_lerp(point.y + 500.0, point.y);
        if pos.y < point.y {
            near *= pos.y.inverse_lerp(point.y - self.rad * 0.7, point.y);
        }
        let normal = point + tangent.perpendicular() + Vec2::new(0.0, 1.0 - near);
        let line = pos.distance_to_line(point, normal).abs();
        let f = self.alpha
            * near
            * line.inverse_lerp(10.0 + 160.0 * near, 10.0)
            * darkness
            * tangent.x.inverse_lerp(0.1, 0.3);
        let b = self.color.map(|c| c * f);
        [
            1.0 - (1.0 - color[0]) * (1.0 - b[0]),
            1.0 - (1.0 - color[1]) * (1.0 - b[1]),
            1.0 - (1.0 - color[2]) * (1.0 - b[2]),
            1.0,
        ]
    }
}

impl WaterFrame<'_> {
    /// `Room.WaterShinyness`.
    #[must_use]
    pub fn shinyness(&self, pos: Vec2) -> f32 {
        let phase = (-self.glitter + (pos.x - pos.y) / 800.0) * std::f32::consts::TAU;
        (0.5 + phase.sin() * 0.5).powf(7.0)
    }

    /// `Water.GetSurfaceColor`: the front edge's colour from the shine and the surface
    /// slope, in the dark with each light source screened on.
    #[must_use]
    pub fn surface_color(&self, point: Vec2, tangent: Vec2) -> [f32; 4] {
        let s = self.shinyness(point);
        let slope = tangent.dot(Vec2::from_degrees(60.0));
        let s = s.powf(0.1) * slope.inverse_lerp(0.9 - s * 0.1, 0.98 - s * 0.05);
        let p = self.palette;
        let mut color = p.water_surface_color1.lerp_color(p.water_shine_color, s);
        if self.darkness > 0.0 {
            for light in self.lights {
                color = light.screen_on(color, point, tangent, self.darkness);
            }
        }
        color
    }

    /// `ApplyDepthOnVector`'s vanishing point: centre, two thirds up the screen.
    fn depth_point(&self) -> Vec2 {
        Vec2::new(self.screen_size[0] / 2.0, self.screen_size[1] * (2.0 / 3.0))
    }

    fn front(&self, i: usize) -> Vec2 {
        Vec2::from(self.surface.front(i))
    }

    fn back(&self, i: usize) -> Vec2 {
        Vec2::from(self.surface.back(i))
    }

    fn stage(&self, room: Vec2) -> Vec2 {
        room - Vec2::from(self.cam_pos)
    }
}

impl WaterSprites {
    /// Body bottom edge (`num2`) without `cosmeticLowerBorder`, stage px.
    pub const BOTTOM: f32 = -10.0;
    /// x of the body's fixed end vertices.
    pub const LEFT: f32 = -10.0;
    pub const RIGHT: f32 = 1400.0;
    /// Horizontal skew of the body's bottom edge per px of height.
    pub const SKEW: f32 = 0.3;

    /// `InitiateSprites` for a surface of `total_points` points on a screen
    /// `screen_width` px wide.
    #[must_use]
    pub fn new(total_points: usize, screen_width: f32) -> Self {
        let points = Self::points_to_render(total_points, screen_width);
        let surface = WaterTriangleMesh::strip(points, true);
        let mut deep = WaterTriangleMesh::strip(points, false);
        deep.color = [0.0, 0.0, 0.0, 1.0];
        Self {
            points_to_render: points,
            surface,
            deep,
        }
    }

    /// `pointsToRender`: enough 20 px steps to cover the screen and 60 px more, plus 2.
    #[must_use]
    pub fn points_to_render(total_points: usize, screen_width: f32) -> usize {
        (((screen_width + 60.0) / 20.0) as usize + 2).min(total_points)
    }

    /// Whether these sprites were made for this surface and screen.
    #[must_use]
    pub fn fits(&self, total_points: usize, screen_width: f32) -> bool {
        self.points_to_render == Self::points_to_render(total_points, screen_width)
    }

    /// `DrawSprites` (base game: no cosmetic offsets, not inverted, one surface).
    pub fn draw(&mut self, frame: &WaterFrame<'_>) {
        let total = frame.surface.len();
        if total < 2 {
            return;
        }
        let depth_point = frame.depth_point();
        let level = frame.surface.level() - frame.cam_pos[1];
        let first = frame.first_point.min(total - 1);
        let end = (first + self.points_to_render).min(total - 1);
        for i in first..end {
            let n = (i - first) * 2;
            let v = frame.stage(frame.front(i)).apply_depth(depth_point, -10.0);
            let mut v2 = frame.stage(frame.back(i)).apply_depth(depth_point, 30.0);
            let v3 = frame
                .stage(frame.front(i + 1))
                .apply_depth(depth_point, -10.0);
            if i == first {
                v2.x -= 100.0;
            } else if i == end - 1 {
                v2.x += 100.0;
            }
            self.surface.move_vertex(n, v);
            self.surface.move_vertex(n + 1, v2);
            self.surface.move_vertex(n + 2, v3);
            self.surface.move_vertex(n + 3, v3);
            let before = frame.front(i.saturating_sub(1));
            let tangent = (frame.front(i + 1) - before).normalized();
            let p = frame.palette;
            self.surface
                .set_vertex_color(n, frame.surface_color(frame.front(i), tangent));
            let shine = frame.shinyness(frame.front(i)) * (1.0 - p.fog_amount);
            self.surface.set_vertex_color(
                n + 1,
                p.water_surface_color2
                    .lerp_color(p.water_shine_color, shine),
            );
            self.deep
                .move_vertex(n, Vec2::new(v.x + v.y * Self::SKEW, Self::BOTTOM));
            self.deep.move_vertex(n + 1, v);
            self.deep
                .move_vertex(n + 2, Vec2::new(v3.x + v3.y * Self::SKEW, Self::BOTTOM));
            self.deep.move_vertex(n + 3, v3);
        }
        let last = self.deep.vertices.len() - 1;
        self.deep
            .move_vertex(0, Vec2::new(Self::LEFT, Self::BOTTOM));
        self.deep.move_vertex(1, Vec2::new(Self::LEFT, level));
        self.deep
            .move_vertex(last - 1, Vec2::new(Self::RIGHT, level));
        self.deep
            .move_vertex(last, Vec2::new(Self::RIGHT, Self::BOTTOM));
        self.deep.color = frame.light.map_or([0.0, 0.0, 0.0, 1.0], |l| {
            l.color(frame.cam_pos, frame.screen_size, frame.time_stacker)
        });
        self.surface
            .set_vertex_color(last - 1, frame.palette.water_surface_color1);
    }

    /// Both sprites' triangles: the surface band first, then the body. Returns the
    /// split (vertices before it are the band).
    pub fn push_triangles(&self, out: &mut Vec<WaterVertex>) -> usize {
        self.surface.push_triangles(out);
        let split = out.len();
        self.deep.push_triangles(out);
        split
    }
}

#[cfg(test)]
mod tests;
