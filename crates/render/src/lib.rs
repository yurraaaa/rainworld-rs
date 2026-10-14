//! Debug renderer: room tile geometry and body chunks, drawn through the small
//! [`Renderer`] trait so the backend can be replaced.
//!
//! The only backend is [`WgpuRenderer`] (wgpu pinned to Vulkan). Everything that does
//! not need a GPU (camera, interpolation, tessellation, tile geometry) lives in plain
//! modules and is unit-tested.
//!
//! Coordinates are room pixels: y up, origin at the room's bottom-left corner,
//! [`rw_assets::TILE_SIZE`] px per tile.

mod body;
mod camera;
mod error;
mod gpu;
mod mesh;
mod room_geometry;

pub use body::ChunkInterpolation;
pub use camera::Camera;
pub use error::RenderError;
pub use gpu::WgpuRenderer;
pub use mesh::{Color, Mesh, Vertex};
pub use room_geometry::{RoomDebugGeometry, RoomShape, SlopeGeometry};

use rw_assets::Room;
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
pub trait Renderer {
    /// Drawable size in physical pixels.
    fn size(&self) -> (u32, u32);

    /// Resizes the drawable; zero sizes are allowed (minimised) and skip presenting.
    fn resize(&mut self, width: u32, height: u32);

    /// Builds the static geometry of `room` for later [`draw_room`](Renderer::draw_room) calls.
    fn set_room(&mut self, room: &Room);

    /// Starts a frame seen through `camera`, clearing queued shapes.
    fn begin_frame(&mut self, camera: Camera);

    /// Draws the room set by [`set_room`](Renderer::set_room), under all other shapes.
    fn draw_room(&mut self, layers: RoomLayers);

    /// Filled circle.
    fn draw_circle(&mut self, centre: [f32; 2], radius: f32, color: Color);

    /// Circle outline of the given stroke width, centred on `radius`.
    fn draw_ring(&mut self, centre: [f32; 2], radius: f32, width: f32, color: Color);

    /// Line segment of the given width.
    fn draw_line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: Color);

    /// Axis-aligned filled rectangle.
    fn draw_rect(&mut self, min: [f32; 2], max: [f32; 2], color: Color);

    /// Renders and presents the frame.
    ///
    /// # Errors
    /// [`RenderError`] when the backend cannot render; recoverable surface states
    /// (outdated, timeout, occluded) are handled internally and skip the frame.
    fn present(&mut self) -> Result<(), RenderError>;

    /// Saves the next presented frame to `path` (binary PPM). Errors are reported by
    /// that frame's [`present`](Renderer::present).
    fn capture_next_frame(&mut self, path: std::path::PathBuf);

    /// Draws an object's chunks as circles at their interpolated positions (`alpha`
    /// of the way from `last_pos` to `pos`), a line for every active connection, and a
    /// small mark where a chunk touches terrain.
    fn draw_object(&mut self, obj: &PhysicalObject, alpha: f32) {
        for (i, chunk) in obj.chunks.iter().enumerate() {
            let c = chunk.interpolated_pos(alpha);
            self.draw_circle(
                c,
                chunk.rad,
                body_palette::FILL[i % body_palette::FILL.len()],
            );
            self.draw_ring(c, chunk.rad, 1.5, body_palette::EDGE);
            let contact = chunk.contact();
            if contact.x != 0 || contact.y != 0 {
                let p = [
                    c[0] + f32::from(contact.x) * chunk.rad,
                    c[1] + f32::from(contact.y) * chunk.rad,
                ];
                self.draw_circle(p, 2.5, body_palette::CONTACT);
            }
        }
        for conn in obj.connections.iter().filter(|c| c.active) {
            let a = obj.chunks[conn.chunk1].interpolated_pos(alpha);
            let b = obj.chunks[conn.chunk2].interpolated_pos(alpha);
            self.draw_line(a, b, 2.0, body_palette::ROD);
        }
    }
}

/// Colours of [`Renderer::draw_object`].
mod body_palette {
    use crate::Color;

    /// Chunk fills, cycled by chunk index (head, hips, ...).
    pub const FILL: [Color; 2] = [
        Color::rgba(0.95, 0.95, 1.0, 0.35),
        Color::rgba(0.75, 0.85, 1.0, 0.30),
    ];
    pub const EDGE: Color = Color::rgb(0.95, 0.95, 1.0);
    pub const ROD: Color = Color::rgb(1.0, 0.55, 0.25);
    pub const CONTACT: Color = Color::rgb(0.3, 1.0, 0.5);
}
