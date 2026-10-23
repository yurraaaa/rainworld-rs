//! `SpotLight.DrawSprites`: the quad's grid mesh in `Shadows` with black vertices, drawn
//! by the `VectorCircle` shader as a black ellipse inscribed in the quad. No draws.
//!
//! Mirrors: SpotLight.cs (InitiateSprites, DrawSprites, UpdateVerts, AddToContainer)

use std::borrow::Cow;

use rw_sim::SpotLight;

use crate::draw::{Color, DrawItem, DrawList, Layer, Mesh, Shader};
use crate::quad_grid::QuadGrid;

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
mod tests {
    use rw_assets::QuadData;
    use rw_sim::{SpotLight, Vec2};

    use super::SpotLightSprites;
    use crate::draw::{Color, DrawItem, Layer, Shader};

    /// A 300 px quad is a 2 x 2 grid (150 px per division), black, in `Shadows`.
    #[test]
    fn black_grid_in_shadows() {
        let data = QuadData::parse("0~300~300~300~300~0").expect("data");
        let s = SpotLight::new(0, Vec2::new(5.0, 6.0), &data);
        let list = s.draw();
        let DrawItem::Mesh(m) = &list.items[0] else {
            panic!("a mesh");
        };
        assert_eq!((m.shader, m.layer), (Shader::VectorCircle, Layer::Shadows));
        assert_eq!((m.vertices.len(), m.triangles.len()), (9, 8));
        assert_eq!(m.color, Color::BLACK);
        assert_eq!(m.vertices[0], Vec2::new(5.0, 6.0));
        assert_eq!(m.vertices[8], Vec2::new(305.0, 306.0));
    }
}
