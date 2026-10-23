//! The sprites of a `StarMatrix` in `BackgroundShortcuts`: per star a black `pixel` square
//! of 5 px, a black `pixel` line to each connected star that is switched on, and its rings
//! (`Futile_White` under the `VectorCircle` shader, black, of alpha `2 / (8 + 5 j)` so the
//! ring is 2 px wide).
//!
//! Mirrors: StarMatrix.cs (Star: InitiateSprites, DrawSprites; InitiateSprites, DrawSprites, AddToContainer)

use rw_sim::{StarMatrix, Vec2};

use crate::draw::{Color, DrawItem, DrawList, Layer, Shader, Sprite};

/// A star matrix's sprites.
pub trait StarMatrixSprites {
    /// `DrawSprites` at `t` between ticks.
    fn draw(&self, t: f32) -> DrawList;
}

impl StarMatrixSprites for StarMatrix {
    fn draw(&self, t: f32) -> DrawList {
        let at: Vec<Vec2> = (0..self.stars.len())
            .map(|i| self.on_plane_pos(i, t) * self.rad + self.pos)
            .collect();
        let mut items = Vec::new();
        for (i, star) in self.stars.iter().enumerate() {
            let here = at[i];
            let first = star.first_sprite;
            items.push(DrawItem::Sprite(Sprite {
                index: first,
                element: "pixel",
                pos: here,
                rotation: 0.0,
                scale: Vec2::new(5.0, 5.0),
                anchor: Vec2::new(0.5, 0.5),
                color: Color::BLACK,
                shader: Shader::Basic,
                layer: Layer::BackgroundShortcuts,
            }));
            for (j, &other) in star.connections.iter().enumerate() {
                if !star.connections_on[j] {
                    continue;
                }
                items.push(DrawItem::Sprite(Sprite {
                    index: first + 1 + j,
                    element: "pixel",
                    pos: here,
                    rotation: (at[other] - here).to_degrees(),
                    scale: Vec2::new(1.5, here.distance(at[other])),
                    anchor: Vec2::new(0.5, 0.0),
                    color: Color::BLACK,
                    shader: Shader::Basic,
                    layer: Layer::BackgroundShortcuts,
                }));
            }
            for k in 0..star.circles {
                let size = 8.0 + 5.0 * k as f32;
                items.push(DrawItem::Sprite(Sprite {
                    index: first + 1 + star.connections.len() + k,
                    element: "Futile_White",
                    pos: here,
                    rotation: 0.0,
                    scale: Vec2::new(size / 8.0, size / 8.0),
                    anchor: Vec2::new(0.5, 0.5),
                    color: Color::BLACK.with_alpha(2.0 / size),
                    shader: Shader::VectorCircle,
                    layer: Layer::BackgroundShortcuts,
                }));
            }
        }
        DrawList { items }
    }
}
