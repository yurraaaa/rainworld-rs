//! `Rainbow`'s sprite: a `CustomFSprite` over the `rainbow` gradient texture in
//! `GrabShaders`, a square of half-side `rad` about the rainbow's position, drawn by the
//! `Rainbow` shader, whose ring is cut from it. The vertex colour carries the inner ring
//! fraction (red) and the corner's opacity times the fade (alpha).
//!
//! Mirrors: Rainbow.cs (InitiateSprites, DrawSprites, AddToContainer), CustomFSprite.cs, RainbowNoFade.cs (InitiateSprites, DrawSprites)

use std::borrow::Cow;

use rw_sim::{FloatExt, Rainbow, Vec2};

use crate::decal::Unorm8;
use crate::draw::{Color, DrawItem, DrawList, ElementMesh, Layer, Mesh, Shader};

/// The atlas element of the gradient (`Futile.atlasManager.LoadAtlasFromTexture("rainbow",
/// Resources.Load("Atlases/Rainbow"))`).
pub const RAINBOW_ELEMENT: &str = "rainbow";

/// The rainbow's sprite.
pub trait RainbowSprites {
    /// `DrawSprites` with the camera's `ghostMode` (0: echoes are not ported).
    fn draw(&self, ghost_mode: f32) -> DrawList;
}

impl RainbowSprites for Rainbow {
    fn draw(&self, ghost_mode: f32) -> DrawList {
        let shown = self.fade * ghost_mode.inverse_lerp(0.2, 0.0);
        // `Custom.eightDirections[1 + i * 2]`: the four diagonals, not normalised.
        let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let mesh = Mesh {
            index: 0,
            vertices: corners
                .iter()
                .map(|&(x, y)| self.pos + Vec2::new(x, y) * self.rad)
                .collect(),
            // The element's uvTopLeft, TopRight, BottomRight, BottomLeft.
            uvs: vec![
                Vec2::new(0.0, 1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 0.0),
            ],
            triangles: Cow::Borrowed(&[[0, 1, 2], [0, 2, 3]]),
            color: Color::WHITE,
            vertex_colors: (0..4)
                .map(|i| Color::from([self.fades[4], 0.0, 0.0, shown * self.fades[i]].unorm8()))
                .collect(),
            shader: Shader::Rainbow,
            layer: Layer::GrabShaders,
        };
        DrawList {
            items: vec![DrawItem::ElementMesh(ElementMesh {
                element: Cow::Borrowed(RAINBOW_ELEMENT),
                mesh,
            })],
        }
    }
}
