//! `WallMycelia`'s sprites: one long thin mesh per strand in `Items`, from its root
//! through its points, black fading in from nothing (the object never sets a colour, so
//! the strand's `color` is the default struct) with a blue tip.
//!
//! Mirrors: CoralBrain/WallMycelia.cs (InitiateSprites, DrawSprites, AddToContainer), CoralBrain/Mycelium.cs (DrawSprites, UpdateColor)

use rw_sim::{MyceliumPool, WallMycelia};

use crate::draw::{Color, DrawItem, DrawList, Layer};
use crate::mycelium::MyceliumSprites;

/// A `WallMycelia`'s draw.
pub trait WallMyceliaSprites {
    /// `DrawSprites` at `t` between ticks: the strands that are not culled.
    fn draw(&self, pool: &MyceliumPool, t: f32) -> DrawList;
}

impl WallMyceliaSprites for WallMycelia {
    fn draw(&self, pool: &MyceliumPool, t: f32) -> DrawList {
        let items = self
            .mycelia
            .iter()
            .enumerate()
            .filter_map(|(i, &id)| Some((i, pool.get(id)?)))
            .filter(|(_, m)| !m.culled)
            .map(|(i, m)| {
                let from = self.roots.root_positions[i];
                DrawItem::Mesh(m.mesh(i, from, t, Color::default(), 0.0, Layer::Items))
            })
            .collect();
        DrawList { items }
    }
}
