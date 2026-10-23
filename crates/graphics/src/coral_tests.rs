//! The coral's sprites: a wall strand's mesh (`Mycelium.InitiateSprites`, `UpdateColor`,
//! `DrawSprites` of a `WallMycelia` that never sets a colour), the culled ones left out, and
//! a `NeuronSpark`'s two flickering sprites with their five cosmetic draws.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{
    Cosmetic, CosmeticKind, MyceliumPool, NeuronSpark, PointOwner, Vec2, WallMycelia, WallRoots,
};

use crate::cosmetic_graphics::{CosmeticSprites, NoLizards};
use crate::draw::{Color, DrawItem, Layer, Shader};
use crate::mycelium::LongMesh;
use crate::tests::room;
use crate::wall_mycelia_draw::WallMyceliaSprites;

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

/// `NeuronSpark.DrawSprites`: a blue `FlatLight` disc and a rotated pixel in `Foreground`;
/// five draws, the pixel's glow first, then its size, the disc's alpha and its size.
#[test]
fn a_neuron_spark_draws_five() {
    let r = room(&["....", "....", "...."]);
    let mut spark = NeuronSpark::new(Vec2::new(10.0, 20.0), &mut UnityRandom::new(1));
    spark.last_pos = Vec2::new(10.0, 20.0);
    spark.pos = Vec2::new(14.0, 20.0);
    spark.last_life = 0.8;
    spark.life = 0.4;
    let cosmetic = Cosmetic {
        id: 1,
        room: 0,
        update_order: 2,
        slated: false,
        kind: CosmeticKind::NeuronSpark(spark),
    };
    let mut rng = UnityRandom::new(8);
    let mut e = UnityRandom::new(8);
    let list = cosmetic.draw(&r, 0.5, &NoLizards(Color::BLACK), &mut rng);
    let life = 0.6f32;
    let (a, b, c, d1, d2) = (e.value(), e.value(), e.value(), e.value(), e.value());
    assert_eq!(rng.state(), e.state());
    assert_eq!(list.items.len(), 2);
    let (DrawItem::Sprite(disc), DrawItem::Sprite(pixel)) = (&list.items[0], &list.items[1]) else {
        panic!("{:?}", list.items);
    };
    assert_eq!(
        (disc.element, disc.shader, disc.layer),
        ("Futile_White", Shader::FlatLight, Layer::Foreground)
    );
    assert_eq!(
        (pixel.element, pixel.rotation, pixel.layer),
        ("pixel", 45.0, Layer::Foreground)
    );
    assert_eq!(disc.pos, Vec2::new(12.0, 20.0));
    assert_eq!(pixel.pos, Vec2::new(12.0, 20.0));
    let glow = life.powf(3.0) * a;
    assert!((pixel.color.r - glow).abs() < 1e-6 && (pixel.color.b - 1.0).abs() < 1e-6);
    assert!((pixel.scale.x - 4.0 * b * life).abs() < 1e-5);
    assert!((disc.color.a - c * life).abs() < 1e-6);
    assert_eq!((disc.color.r, disc.color.g, disc.color.b), (0.0, 0.0, 1.0));
    assert!((disc.scale.x - (15.0 * d1 + 15.0 * d2.powf(3.0)) * life / 10.0).abs() < 1e-4);
}
