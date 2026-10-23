//! The decal's grid mesh. Expected values follow `TriangleMesh.MakeGridMesh` and
//! `CustomDecal.InitiateSprites`.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::CustomDecalData;
use rw_sim::{CustomDecal, Vec2};

use crate::DecalSprites;
use crate::core::draw::{DrawItem, Layer, Shader};
use crate::effects::particles::decal::Unorm8;

fn decal(side: f32) -> CustomDecal {
    let data = CustomDecalData::parse(&format!(
        "0~{side}~{side}~{side}~{side}~0~0~0~0.2~0.9~0~Rust1"
    ))
    .unwrap();
    CustomDecal::new(0, Vec2::new(10.0, 20.0), data)
}

/// A one-cell grid: vertices 0..3 (index `j * 2 + i`), triangles `(a, right, up)` and
/// `(right, up, corner)`.
#[test]
fn single_cell_grid() {
    assert_eq!(CustomDecal::grid_triangles(1), [[0, 1, 2], [1, 2, 3]]);
}

/// A 2 x 2 grid over a 3 x 3 vertex lattice, cell `(i, j)` at triangles `(j * 2 + i) * 2`.
#[test]
fn two_by_two_grid() {
    assert_eq!(
        CustomDecal::grid_triangles(2),
        [
            [0, 1, 3],
            [1, 3, 4],
            [1, 2, 4],
            [2, 4, 5],
            [3, 4, 6],
            [4, 6, 7],
            [4, 5, 7],
            [5, 7, 8],
        ]
    );
}

#[test]
fn decal_mesh_carries_the_simulations_vertices() {
    let d = decal(300.0);
    let list = d.draw();
    let [DrawItem::ElementMesh(e)] = &list.items[..] else {
        panic!("one element mesh");
    };
    assert_eq!(e.element, "rust1");
    assert_eq!(
        (e.mesh.shader, e.mesh.layer),
        (Shader::Decal, Layer::Foreground)
    );
    assert_eq!(e.mesh.vertices, d.verts);
    assert_eq!(e.mesh.triangles.len(), 8);
    // MakeGridMesh: `uvBottomLeft + size * (k / div, l / div)` for vertex `l * 3 + k`.
    assert_eq!(e.mesh.uvs[0], Vec2::new(0.0, 0.0));
    assert_eq!(e.mesh.uvs[1], Vec2::new(0.5, 0.0));
    assert_eq!(e.mesh.uvs[3], Vec2::new(0.0, 0.5));
    assert_eq!(e.mesh.uvs[8], Vec2::new(1.0, 1.0));
    // Colours (from, to, f, f2), 8 bits a channel: 0.2 -> 51/255, 0.9 -> 230/255,
    // f = 0 (the default second column), f2 = 0.5 -> 128/255.
    let c = e.mesh.vertex_colors[4];
    assert_eq!(
        (c.r, c.g, c.b, c.a),
        (51.0 / 255.0, 230.0 / 255.0, 0.0, 128.0 / 255.0)
    );
}

#[test]
fn colours_are_stored_in_eight_bits() {
    assert_eq!(
        [0.5_f32, 0.25, 1.5, -1.0].unorm8(),
        [128.0 / 255.0, 64.0 / 255.0, 1.0, 0.0]
    );
}
