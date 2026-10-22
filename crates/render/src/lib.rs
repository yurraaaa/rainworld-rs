//! Renderer: the room's level graphic (camera image through the palette) or its debug
//! tile geometry, shortcut tubes, water, body chunks and Futile atlas sprites (an
//! `rw_graphics::DrawList`), and Futile stage quads in screen pixels for the menus and
//! the process fade ([`StageBatch`]), drawn through the small [`Renderer`] trait so the
//! backend can be replaced.
//!
//! The only backend is [`WgpuRenderer`] (wgpu pinned to Vulkan). Everything that does
//! not need a GPU (camera, interpolation, tessellation, tile geometry) lives in plain
//! modules and is unit-tested.
//!
//! Coordinates are room pixels: y up, origin at the room's bottom-left corner,
//! [`rw_assets::TILE_SIZE`] px per tile.
//!
//! Mirrors: -

mod body;
mod camera;
mod error;
mod frame_image;
mod gpu;
mod level;
mod level_gpu;
mod mesh;
mod overlay;
mod overlay_gpu;
mod present;
mod present_gpu;
mod room_camera;
mod room_geometry;
mod sprite_gpu;
mod sprites;
mod stage;
mod stage_gpu;
mod status;
#[cfg(test)]
mod test_room;
mod water;
mod water_gpu;
mod water_sprites;

pub use body::ChunkInterpolation;
pub use camera::Camera;
pub use error::RenderError;
pub use frame_image::FrameImage;
pub use gpu::WgpuRenderer;
pub use level::{LevelGraphics, LevelParams, LevelView};
pub use mesh::{Color, Mesh, Vertex};
pub use overlay::{Overlay, OverlayDraw, OverlayVertex};
pub use present::{PresentFilter, PresentRect, Resolution};
pub use room_camera::{Follow, RoomCamera};
pub use room_geometry::{RoomDebugGeometry, RoomShape, SlopeGeometry};
pub use sprites::{
    Batch, DrawQueue, NodeMatrix, RoomGlobals, RoomVertex, SpriteAtlas, SpriteBatch, SpriteDraw,
    SpriteVertex, WaterShader,
};
pub use stage::{
    MapGlobals, MenuSceneGlobals, StageBatch, StageDraw, StageGlobals, StageShader, StageTextureId,
    TextureFilter, TextureSampling,
};
pub use status::{BodyStatus, BodyStyle};
pub use water::WaterSurface;
pub use water_sprites::{
    ApplyDepth, PlayerLight, SurfaceLight, WaterFrame, WaterSprites, WaterTriangleMesh, WaterVertex,
};

use rw_assets::{PngImage, Room, TILE_SIZE};
use rw_graphics::{DrawList, Layer};
use rw_sim::PhysicalObject;

/// Which room layers [`Renderer::draw_room`] draws this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomLayers {
    /// Tile geometry: solids, slopes, floors, poles, shortcut markers.
    pub tiles: bool,
    /// Thin lines on every tile boundary.
    pub grid: bool,
}

impl Default for RoomLayers {
    fn default() -> Self {
        Self {
            tiles: true,
            grid: false,
        }
    }
}

/// A frame-based debug renderer.
///
/// Per frame: [`begin_frame`](Renderer::begin_frame), any number of draw calls, then
/// [`present`](Renderer::present). Draw calls between frames are discarded by the next
/// `begin_frame`. All positions and sizes are in room pixels.
///
/// The frame is drawn at the game's screen [`Resolution`] and shown in the window
/// scaled as the game is ([`PresentRect`]).
pub trait Renderer {
    /// Frame size in pixels: the game's screen resolution, whatever the window size.
    fn size(&self) -> (u32, u32);

    /// Resizes the window's drawable (physical pixels); zero sizes are allowed
    /// (minimised) and skip presenting.
    fn resize(&mut self, width: u32, height: u32);

    /// Builds the static geometry of `room` for later [`draw_room`](Renderer::draw_room) calls.
    fn set_room(&mut self, room: &Room);

    /// Starts a frame seen through `camera`, clearing queued shapes.
    fn begin_frame(&mut self, camera: Camera);

    /// Draws the room set by [`set_room`](Renderer::set_room), under all other shapes.
    fn draw_room(&mut self, layers: RoomLayers);

    /// Uploads the level graphic of one camera, replacing the previous one.
    fn set_level(&mut self, level: &LevelGraphics<'_>);

    /// Replaces the uploaded level's palette and its `_fogAmount` (the game's `ApplyFade`
    /// and `ApplyPalette` when the dark or fade blend changed); nothing without a level.
    fn set_level_palette(&mut self, palette: &rw_assets::PaletteTexture);

    /// Draws the level set by [`set_level`](Renderer::set_level) over the shapes and
    /// sprites queued so far this frame (they show only through sky and depth >= 5, see
    /// the `level` module), then the room layers and later draws. The frame is clipped
    /// to the game's window `view.origin .. view.origin + view.size`.
    fn draw_level(&mut self, view: LevelView);

    /// Uploads the sprite atlas (frame data and one RGBA image per page, in page order)
    /// for [`draw_sprites`](Renderer::draw_sprites), replacing the previous one.
    fn set_atlas(&mut self, atlas: &SpriteAtlas, images: &[&PngImage]);

    /// Queues the items of `list` in `layer`, in order, as Futile draws them, each
    /// with its game shader (`rw_graphics::Shader`), at stage positions
    /// `pos - cam_pos`; `cam_pos` is the game's `floor(camera) - 0.02` (the level view
    /// origin), and the same for every call in a frame. Call it with
    /// [`Layer::Midground`] then [`Layer::Items`] before
    /// [`draw_level`](Renderer::draw_level) and [`Layer::Foreground`] then
    /// [`Layer::ForegroundLights`] after, as the game's containers order them, and
    /// [`Layer::Bloom`] after the water. The shaders' `_spriteRect`, `_screenSize`,
    /// `_RAIN` and `_NoiseTex` come from the frame's level, `_NoiseTex2` from the stage
    /// globals.
    /// Nothing is drawn before an atlas is set.
    fn draw_sprites(&mut self, list: &DrawList, layer: Layer, cam_pos: [f32; 2]);

    /// Uploads a stage texture under `id`, replacing any texture of that id.
    fn set_stage_texture(
        &mut self,
        id: StageTextureId,
        image: &PngImage,
        sampling: TextureSampling,
    );

    /// Frees a stage texture.
    fn remove_stage_texture(&mut self, id: StageTextureId);

    /// Sets the stage shaders' globals for the next frames.
    fn set_stage_globals(&mut self, globals: StageGlobals);

    /// Queues stage quads after everything queued so far, in screen pixels of a screen
    /// of `screen_size` (y up, origin bottom-left) fitted to the frame. Draws whose
    /// texture is not uploaded are skipped.
    fn draw_stage(&mut self, batch: &StageBatch, screen_size: [f32; 2]);

    /// The colour the frame is cleared to when no level is drawn (Futile's camera
    /// clears to black; the debug view uses a dark blue). Reset by `begin_frame`.
    fn set_background(&mut self, color: Color);

    /// Filled circle.
    fn draw_circle(&mut self, centre: [f32; 2], radius: f32, color: Color);

    /// Circle outline of the given stroke width, centred on `radius`.
    fn draw_ring(&mut self, centre: [f32; 2], radius: f32, width: f32, color: Color);

    /// Line segment of the given width.
    fn draw_line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: Color);

    /// Axis-aligned filled rectangle.
    fn draw_rect(&mut self, min: [f32; 2], max: [f32; 2], color: Color);

    /// Arbitrary triangles, drawn in call order with the other shapes.
    fn draw_mesh(&mut self, mesh: &Mesh);

    /// Renders and presents the frame.
    ///
    /// # Errors
    /// [`RenderError`] when the backend cannot render; recoverable surface states
    /// (outdated, timeout, occluded) are handled internally and skip the frame.
    fn present(&mut self) -> Result<(), RenderError>;

    /// Uploads the debug overlay's texture: `width * height` RGBA pixels, rows top to
    /// bottom. Until then the overlay is drawn untextured.
    fn set_overlay_texture(&mut self, width: u32, height: u32, rgba: &[u8]);

    /// Draws `overlay` over this frame when it is presented: over the window at its
    /// physical size, or over the frame without a window. Cleared by `begin_frame`.
    fn draw_overlay(&mut self, overlay: Overlay);

    /// Saves the next frame, at [`size`](Renderer::size), to `path` (binary PPM). Errors are reported by
    /// that frame's [`present`](Renderer::present).
    fn capture_next_frame(&mut self, path: std::path::PathBuf);

    /// Draws an object's chunks as circles at their interpolated positions (`alpha`
    /// of the way from `last_pos` to `pos`), a line for every active connection, and a
    /// small mark where a chunk touches terrain, styled by `status` at `time` seconds.
    fn draw_object(&mut self, obj: &PhysicalObject, alpha: f32, status: BodyStatus, time: f32) {
        self.draw_object_styled(obj, alpha, status.style(time));
    }

    /// [`Renderer::draw_object`] with a given style (the debug view's other bodies).
    fn draw_object_styled(&mut self, obj: &PhysicalObject, alpha: f32, style: BodyStyle) {
        for (i, chunk) in obj.chunks.iter().enumerate() {
            let c = chunk.interpolated_pos(alpha);
            self.draw_circle(c, chunk.rad, style.fill[i % style.fill.len()]);
            self.draw_ring(c, chunk.rad, style.edge_width, style.edge);
            if let Some(cross) = style.cross {
                let d = chunk.rad * std::f32::consts::FRAC_1_SQRT_2;
                self.draw_line([c[0] - d, c[1] - d], [c[0] + d, c[1] + d], 2.0, cross);
                self.draw_line([c[0] - d, c[1] + d], [c[0] + d, c[1] - d], 2.0, cross);
            }
            let contact = chunk.contact();
            if contact.x != 0 || contact.y != 0 {
                let p = [
                    c[0] + f32::from(contact.x) * chunk.rad,
                    c[1] + f32::from(contact.y) * chunk.rad,
                ];
                self.draw_circle(p, 2.5, style.contact);
            }
        }
        for conn in obj.connections.iter().filter(|c| c.active) {
            let a = obj.chunks[conn.chunk1].interpolated_pos(alpha);
            let b = obj.chunks[conn.chunk2].interpolated_pos(alpha);
            self.draw_line(a, b, 2.0, style.rod);
        }
    }

    /// The game's water (`Water`'s two sprites in the `Water` container), shaded by
    /// the `WaterSurface` and `DeepWater` shaders with `_waterDepth` = `depth`. Drawn
    /// only in a frame that draws the level, after it (they read the level image and
    /// what was drawn behind the level).
    fn draw_water_sprites(&mut self, sprites: &WaterSprites, depth: f32);

    /// Debug view: translucent water from `surface` down to
    /// [`WaterSurface::DEPTH_BELOW_ROOM`] under the room. Draw it after the bodies.
    fn draw_water(&mut self, surface: &WaterSurface) {
        self.draw_mesh(&surface.mesh(-WaterSurface::DEPTH_BELOW_ROOM));
    }

    /// Lit marker on the shortcut tile a creature is travelling through, if any.
    fn draw_shortcut_vessel(&mut self, tile: Option<(i32, i32)>) {
        let Some((x, y)) = tile else { return };
        let min = [x as f32 * TILE_SIZE, y as f32 * TILE_SIZE];
        let max = [min[0] + TILE_SIZE, min[1] + TILE_SIZE];
        let mid = [min[0] + TILE_SIZE / 2.0, min[1] + TILE_SIZE / 2.0];
        self.draw_rect(min, max, Color::rgba(1.0, 1.0, 1.0, 0.85));
        self.draw_ring(mid, TILE_SIZE * 0.75, 2.0, Color::rgb(1.0, 0.95, 0.5));
    }
}
