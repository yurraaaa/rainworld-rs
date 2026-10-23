//! The coral's sprites: a wall strand's mesh (`Mycelium.InitiateSprites`, `UpdateColor`,
//! `DrawSprites` of a `WallMycelia` that never sets a colour) and the culled ones left out.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{MyceliumPool, PointOwner, Vec2, WallMycelia, WallRoots};

use crate::core::draw::{Color, DrawItem, Layer, Shader};
use crate::superstructure::coral::mycelium::LongMesh;
use crate::superstructure::coral::mycelium::WallMyceliaSprites;

fn wall_of_one(length: f32) -> (WallMycelia, MyceliumPool) {
    let mut pool = MyceliumPool::default();
    let owner = pool.new_owner();
    let root = Vec2::new(100.0, 100.0);
    let id = pool.add(
        None,
        (owner, &PointOwner(root)),
        0,
        length,
        root,
        &mut UnityRandom::new(3),
    );
    let wall = WallMycelia {
        update_order: 4,
        owner,
        places: Vec::new(),
        roots: WallRoots {
            root_positions: vec![root],
            directions: vec![Vec2::new(0.0, 1.0)],
        },
        random_movements: vec![Vec2::ZERO],
        mycelia: vec![id],
    };
    (wall, pool)
}

/// `TriangleMesh.MakeLongMesh(segments, false, true)`: four vertices a segment, four
/// triangles each and two for the end; `UpdateColor(default, 0, ...)` shades from the
/// default colour (transparent black) to the dark blue-green, the last two vertices blue.
#[test]
fn a_wall_strand_is_a_mesh_from_nothing_to_blue() {
    let (wall, pool) = wall_of_one(60.0);
    let list = wall.draw(&pool, 1.0);
    assert_eq!(list.items.len(), 1);
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("{:?}", list.items[0]);
    };
    let segments = pool.get(wall.mycelia[0]).unwrap().points.len();
    assert_eq!(segments, 4);
    assert_eq!(
        (m.index, m.layer, m.shader),
        (0, Layer::Items, Shader::Basic)
    );
    assert_eq!(m.vertices.len(), segments * 4);
    assert_eq!(m.uvs, LongMesh::uvs(segments));
    assert_eq!(m.triangles.len(), (segments - 1) * 4 + 2);
    assert_eq!(m.vertex_colors.len(), segments * 4);
    assert_eq!(m.vertex_colors[0], Color::default());
    let dark = Color::hsl(22.0 / 45.0, 0.5, 0.2);
    let last = (segments * 4 - 1) as f32;
    for i in 1..segments * 4 - 2 {
        let t = i as f32 / last;
        assert_eq!(m.vertex_colors[i], Color::default().lerp(dark, t));
    }
    assert_eq!(m.vertex_colors[segments * 4 - 2], Color::rgb(0.0, 0.0, 1.0));
    assert_eq!(m.vertex_colors[segments * 4 - 1], Color::rgb(0.0, 0.0, 1.0));
    // The strip starts at the root: the first vertices straddle it by half a pixel.
    let root = Vec2::new(100.0, 100.0);
    assert!(m.vertices[0].distance(root) < 2.0);
}

/// A culled strand is not visible (`isVisible = !culled`).
#[test]
fn a_culled_strand_is_not_drawn() {
    let (wall, mut pool) = wall_of_one(60.0);
    pool.get_mut(wall.mycelia[0]).unwrap().culled = true;
    assert!(wall.draw(&pool, 1.0).items.is_empty());
}
