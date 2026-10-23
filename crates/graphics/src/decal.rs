//! `CustomDecal`'s sprite: a grid `TriangleMesh` over the decal's image element, in
//! `Foreground`, drawn by the `Decal` shader. The vertices come from the simulation
//! (`UpdateVerts` runs once; `DrawSprites` only moves them by the camera).
//!
//! Mirrors: CustomDecal.cs (InitiateSprites, DrawSprites, AddToContainer), TriangleMesh.cs (MakeGridMesh)

use std::borrow::Cow;

use rw_sim::{CustomDecal, Vec2};

use crate::draw::{Color, DrawItem, DrawList, ElementMesh, Layer, Mesh, Shader};

/// A vertex colour as Unity stores a mesh's `Color[]`: 8 bits a channel.
pub(crate) trait Unorm8 {
    /// Each channel clamped to `[0, 1]` and rounded to a multiple of 1/255.
    fn unorm8(self) -> Self;
}

impl Unorm8 for [f32; 4] {
    fn unorm8(self) -> Self {
        self.map(|c| (c.clamp(0.0, 1.0) * 255.0).round_ties_even() / 255.0)
    }
}

/// The decal's sprite.
pub trait DecalSprites {
    /// `InitiateSprites` and `DrawSprites`.
    fn draw(&self) -> DrawList;

    /// `TriangleMesh.MakeGridMesh`'s triangles for a `div` x `div` grid.
    fn grid_triangles(div: usize) -> Vec<[u16; 3]>;
}

impl DecalSprites for CustomDecal {
    fn draw(&self) -> DrawList {
        let div = self.grid_div;
        let n = div + 1;
        let uvs = (0..n * n)
            .map(|k| Vec2::new((k % n) as f32 / div as f32, (k / n) as f32 / div as f32))
            .collect();
        let mesh = Mesh {
            index: 0,
            vertices: self.verts.clone(),
            uvs,
            triangles: Cow::Owned(Self::grid_triangles(div)),
            color: Color::WHITE,
            vertex_colors: self
                .colors
                .iter()
                .map(|&c| Color::from(c.unorm8()))
                .collect(),
            shader: Shader::Decal,
            layer: Layer::Foreground,
        };
        DrawList {
            items: vec![DrawItem::ElementMesh(ElementMesh {
                element: Cow::Owned(self.data.image_name.clone()),
                mesh,
            })],
        }
    }

    fn grid_triangles(div: usize) -> Vec<[u16; 3]> {
        let w = div + 1;
        let mut triangles = vec![[0; 3]; div * div * 2];
        for i in 0..div {
            for j in 0..div {
                let up = ((j + 1) * w + i) as u16;
                let a = (j * w + i) as u16;
                let right = (j * w + i + 1) as u16;
                let corner = ((j + 1) * w + i + 1) as u16;
                triangles[(j * div + i) * 2] = [a, right, up];
                triangles[(j * div + i) * 2 + 1] = [right, up, corner];
            }
        }
        triangles
    }
}
