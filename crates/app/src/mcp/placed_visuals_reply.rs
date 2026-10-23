//! What `get_state` reports of the realized rooms' `CustomDecal`s and `Rainbow`s.
//!
//! Mirrors: - (port-only: MCP server)

use rw_sim::world::World;
use rw_sim::{CustomDecal, Rainbow};
use serde::Serialize;

/// One `CustomDecal`.
#[derive(Debug, Clone, Serialize)]
pub struct DecalState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    /// The image's file name (lower case).
    pub image: String,
    /// `quad`: the position and the three handles' positions.
    pub quad: [[f32; 2]; 4],
    pub grid_div: usize,
    pub from_depth: f32,
    pub to_depth: f32,
    pub noise: f32,
    pub vertices: usize,
    /// The first and the last vertex's position and colour `(from, to, f, f2)`.
    pub first: ([f32; 2], [f32; 4]),
    pub last: ([f32; 2], [f32; 4]),
}

/// One `Rainbow` (or `RainbowNoFade`).
#[derive(Debug, Clone, Serialize)]
pub struct RainbowState {
    pub room: String,
    /// Its place in the update list.
    pub id: u64,
    pub pos: [f32; 2],
    pub rad: f32,
    pub fade: f32,
    /// The corners' vertex alphas: `fade * fades[i]`.
    pub alpha: [f32; 4],
    pub inner_ring: f32,
    pub always_show: bool,
}

impl DecalState {
    /// Every realized room's decals.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| r.decals.iter().map(move |d| Self::of(room.clone(), d)))
            .collect()
    }

    fn vertex(d: &CustomDecal, i: usize) -> ([f32; 2], [f32; 4]) {
        (d.verts[i].to_array(), d.colors[i])
    }

    fn of(room: String, d: &CustomDecal) -> Self {
        Self {
            room,
            id: d.update_order,
            image: d.data.image_name.clone(),
            quad: d.quad.map(|q| q.to_array()),
            grid_div: d.grid_div,
            from_depth: d.data.from_depth,
            to_depth: d.data.to_depth,
            noise: d.data.noise,
            vertices: d.verts.len(),
            first: Self::vertex(d, 0),
            last: Self::vertex(d, d.verts.len() - 1),
        }
    }
}

impl RainbowState {
    /// Every realized room's rainbows.
    #[must_use]
    pub fn all(world: &World) -> Vec<Self> {
        world
            .active_rooms()
            .into_iter()
            .filter_map(|i| Some((RoomName::of(world, i), world.realized(i)?)))
            .flat_map(|(room, r)| r.rainbows.iter().map(move |b| Self::of(room.clone(), b)))
            .collect()
    }

    fn of(room: String, b: &Rainbow) -> Self {
        Self {
            room,
            id: b.update_order,
            pos: b.pos.to_array(),
            rad: b.rad,
            fade: b.fade,
            alpha: [0, 1, 2, 3].map(|i| b.fade * b.fades[i]),
            inner_ring: b.fades[4],
            always_show: b.always_show,
        }
    }
}

/// A room's name for the replies.
struct RoomName;

impl RoomName {
    fn of(world: &World, index: i32) -> String {
        world
            .abstract_room(index)
            .map_or_else(|| format!("#{index}"), |r| r.name.clone())
    }
}
