//! A circuit's sprites against `CoralCircuit.cs`, `CircuitConnector.cs`, `CircuitCrab.cs`
//! and `CrabLeg.cs`: a bit's `maze` sprite (the window in the vertex colour, the flip and
//! the rotation, the shader by `InPosition`), a connector's strands and four dots, a crab's
//! dots and leg strips, all worked by hand.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::world::AiMap;
use rw_sim::{CircuitCx, CircuitPlace, CoralCircuit, MyceliumPool, SystemId, Vec2};

use crate::circuit_draw::{CircuitSprites, MAZE_ELEMENT};
use crate::decal::Unorm8;
use crate::draw::{Color, DrawItem, Layer, Shader};
use crate::tests::room;

/// A circuit of the places `(330, 210)` radius 60 in a 60 x 40 tile room with a solid
/// border.
fn circuit() -> (CoralCircuit, MyceliumPool) {
    let mut rows = vec!["#".repeat(60)];
    rows.extend((0..38).map(|_| format!("#{}#", ".".repeat(58))));
    rows.push("#".repeat(60));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let map = AiMap::new(&r, 0, false, false);
    let mut pool = MyceliumPool::default();
    let system = pool.create_registry();
    let mut seq = 0;
    let places = vec![CircuitPlace {
        pos: Vec2::new(610.0, 410.0),
        rad: 60.0,
    }];
    let c = CoralCircuit::build(
        places,
        &mut CircuitCx {
            room: &r,
            ai_map: &map,
            system,
            pool: &mut pool,
            seq: &mut seq,
        },
        &mut UnityRandom::new(7),
    );
    assert_eq!(system, SystemId(1));
    (c, pool)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

/// A bit's sprite: the `maze` element at the bit's place in `Background`, `size / 128` wide
/// and tall; the vertex colour `((int)x % 128 / 128, (int)y % 128 / 128, w / 128, h / 128)`
/// of the box's lower left, 8 bits a channel; the `CoralCircuit` shader while in place and
/// the `Dead` one after.
#[test]
fn a_bit_is_a_maze_sprite_with_its_window_in_the_colour() {
    let (mut c, _) = circuit();
    c.bits[0].stuck_pos = Vec2::new(330.7, 215.2);
    c.bits[0].size = Vec2::new(12.0, 8.0);
    c.bits[0].pos = Vec2::new(330.7, 215.2);
    c.bits[0].last_pos = c.bits[0].pos;
    // A bit that has had its turns (its first `lastDRotat` is the zero vector).
    c.bits[0].last_d_rotat = Vec2::new(0.0, 1.0);
    let list = c.draw_bit(0, 0.5);
    assert_eq!(list.items.len(), 1);
    let DrawItem::Sprite(s) = &list.items[0] else {
        panic!("a sprite")
    };
    assert_eq!(
        (s.element, s.layer, s.index),
        (MAZE_ELEMENT, Layer::Background, 0)
    );
    assert_eq!(s.shader, Shader::DeadCoralCircuit);
    // Lower left (324.7, 211.2): (int) 324 % 128 = 68, 211 % 128 = 83; sizes 12 and 8.
    let [r, g, b, a] = [68.0 / 128.0, 83.0 / 128.0, 12.0 / 128.0, 8.0 / 128.0].unorm8();
    assert!(near(s.color.r, r), "{:?}", s.color);
    assert!(near(s.color.g, g));
    assert!(near(s.color.b, b));
    assert!(near(s.color.a, a));
    assert!(near(s.scale.x, 12.0 / 128.0) && near(s.scale.y, 8.0 / 128.0));
    assert_eq!(
        (s.pos, s.rotation, s.anchor),
        (c.bits[0].pos, 0.0, Vec2::new(0.5, 0.5))
    );
    c.bits[0].shader = true;
    let DrawItem::Sprite(s) = &c.draw_bit(0, 0.5).items[0] else {
        panic!("a sprite")
    };
    assert_eq!(s.shader, Shader::CoralCircuit);
}

/// The sprite is at `Lerp(lastPos, pos, t)`, turned by `VecToDeg(Slerp(lastRotat, rotat,
/// t))` and flipped by the `y` of `Slerp(lastDRotat, dRotat, t)`.
#[test]
fn a_bit_is_interpolated_turned_and_flipped() {
    let (mut c, _) = circuit();
    let bit = &mut c.bits[0];
    bit.size = Vec2::new(20.0, 10.0);
    bit.last_pos = Vec2::new(100.0, 100.0);
    bit.pos = Vec2::new(110.0, 120.0);
    bit.last_rotat = Vec2::new(0.0, 1.0);
    bit.rotat = Vec2::new(1.0, 0.0);
    bit.last_d_rotat = Vec2::new(0.0, 1.0);
    bit.d_rotat = Vec2::new(0.0, -1.0);
    let DrawItem::Sprite(s) = &c.draw_bit(0, 0.25).items[0] else {
        panic!("a sprite")
    };
    assert_eq!(s.pos, Vec2::new(102.5, 105.0));
    // A quarter of the way from up to right: 22.5 degrees clockwise.
    assert!((s.rotation - 22.5).abs() < 1e-2, "{}", s.rotation);
    // A quarter of the way from (0, 1) to (0, -1) is at 45 degrees: y = cos(45) = 0.7071.
    assert!(
        (s.scale.x - 20.0 / 128.0 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4,
        "{}",
        s.scale.x
    );
    assert!(near(s.scale.y, 10.0 / 128.0));
}

/// A connector: its strands that are not culled first (meshes 0..n, `Midground`, from the
/// dot), then the four `Circle20` dots after them: scales 0.8, 0.4, 0.3, 0.15 times the flip
/// `Lerp(0.2, 1, |y|) * Sign(y)`, the second offset `(-1 - 2|y|, 1 + 2|y|)`, the others
/// `(-1, 1)`, the last one coloured `(0, 0, value)` from the cosmetic stream.
#[test]
fn a_connector_draws_strands_then_four_dots() {
    let (mut c, pool) = circuit();
    let n = c.connectors[0].connections.len();
    let bit = c.connectors[0].bit;
    c.bits[bit].last_d_rotat = Vec2::new(0.0, 1.0);
    c.bits[bit].d_rotat = Vec2::new(0.0, 1.0);
    let dot = c.connectors[0].dot_pos(&c.bits, 1.0);
    let mut rng = UnityRandom::new(5);
    let mut mirror = rng.clone();
    let list = c.draw_connector(0, &pool, 1.0, &mut rng);
    let glow = mirror.value();
    assert_eq!(rng.state(), mirror.state());
    let meshes: Vec<_> = list
        .items
        .iter()
        .filter_map(|i| match i {
            DrawItem::Mesh(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(meshes.len(), n);
    for (i, m) in meshes.iter().enumerate() {
        assert_eq!(
            (m.index, m.layer, m.shader),
            (i, Layer::Midground, Shader::Basic)
        );
        assert!(near_vec(m.vertices[0], dot, 5.0), "starts at the dot");
    }
    let dots: Vec<_> = list
        .items
        .iter()
        .filter_map(|i| match i {
            DrawItem::Sprite(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(dots.len(), 4);
    for (j, d) in dots.iter().enumerate() {
        assert_eq!((d.index, d.element), (n + j, "Circle20"));
        assert_eq!(d.layer, Layer::Midground);
    }
    // At rest the flip is Lerp(0.2, 1, 1) = 1.
    let (offsets, scales) = (
        [(0.0, 0.0), (-3.0, 3.0), (-1.0, 1.0), (-1.0, 1.0)],
        [0.8, 0.4, 0.3, 0.15],
    );
    for j in 0..4 {
        assert!(near(dots[j].pos.x, dot.x + offsets[j].0), "{j}");
        assert!(near(dots[j].pos.y, dot.y + offsets[j].1), "{j}");
        assert!(near(dots[j].scale.x, scales[j]) && near(dots[j].scale.y, scales[j]));
    }
    assert!(near_color(dots[3].color, Color::rgb(0.0, 0.0, glow)));
    assert!(near_color(dots[0].color, Color::hsl(0.963_888_9, 1.0, 0.1)));
    assert!(near_color(
        dots[1].color,
        Color::hsl(0.963_888_9, 1.0, 0.25)
    ));
    // Edge on (`y` 0): the flip is 0.2 (`Sign(0)` 1) and the second dot is offset (-1, 1).
    c.bits[bit].last_d_rotat = Vec2::new(1.0, 0.0);
    c.bits[bit].d_rotat = Vec2::new(1.0, 0.0);
    let list = c.draw_connector(0, &pool, 1.0, &mut UnityRandom::new(5));
    let edge: Vec<_> = list
        .items
        .iter()
        .filter_map(|i| match i {
            DrawItem::Sprite(s) => Some(s),
            _ => None,
        })
        .collect();
    assert!(near(edge[0].scale.x, 0.8 * 0.2) && near(edge[0].scale.y, 0.8));
}

fn near_vec(a: Vec2, b: Vec2, tolerance: f32) -> bool {
    a.distance(b) < tolerance
}

fn near_color(a: Color, b: Color) -> bool {
    near(a.r, b.r) && near(a.g, b.g) && near(a.b, b.b) && near(a.a, b.a)
}

/// A culled strand is not drawn.
#[test]
fn a_culled_connector_strand_is_left_out() {
    let (c, mut pool) = circuit();
    let id = c.connectors[0].connections[0];
    pool.get_mut(id).expect("strand").culled = true;
    let list = c.draw_connector(0, &pool, 1.0, &mut UnityRandom::new(5));
    let meshes = list
        .items
        .iter()
        .filter(|i| matches!(i, DrawItem::Mesh(_)))
        .count();
    assert_eq!(meshes, c.connectors[0].connections.len() - 1);
}

/// A crab: a skin-coloured `Circle20` (0.2) at its body, a white one (0.12) offset
/// `(-1, 1)`, then a strip per leg (index `i + 2`, three points, one colour) from the body.
#[test]
fn a_crab_draws_two_dots_and_a_strip_per_leg() {
    let (c, _) = circuit();
    let body = c.crabs[0].body_pos(&c.bits, 1.0);
    let list = c.draw_crab(0, 1.0);
    let legs = c.crabs[0].legs.len();
    assert_eq!(list.items.len(), 2 + legs);
    let DrawItem::Sprite(a) = &list.items[0] else {
        panic!("a sprite")
    };
    let DrawItem::Sprite(b) = &list.items[1] else {
        panic!("a sprite")
    };
    assert_eq!((a.pos, b.pos), (body, body + Vec2::new(-1.0, 1.0)));
    assert!(near(a.scale.x, 0.2) && near(b.scale.x, 0.12));
    assert!(near_color(a.color, Color::hsl(0.025, 0.5, 0.85)));
    assert_eq!(b.color, Color::WHITE);
    for (i, item) in list.items[2..].iter().enumerate() {
        let DrawItem::Mesh(m) = item else {
            panic!("a mesh")
        };
        assert_eq!((m.index, m.layer), (i + 2, Layer::Midground));
        assert_eq!(m.vertices.len(), 12);
        assert!(m.vertex_colors.is_empty());
        assert!(near_color(m.color, Color::hsl(0.025, 0.5, 0.85)));
        // The first points lie along the strip from the body: `body - dir * overlap -
        // perpendicular * 0.5` with `dir` the way to the point, so within a pixel of it.
        assert!(near_vec(m.vertices[0], body, 6.0));
    }
}
