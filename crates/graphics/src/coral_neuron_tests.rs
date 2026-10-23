//! A `CoralNeuron`'s and a `ProjectedCircle`'s sprites against `CoralNeuron.cs` and
//! `ProjectedCircle.cs`: the colour ramp, the two meshes' vertices on a straight neuron
//! worked by hand, the bumps' discs and containers, the circle's discs, spokes and lines.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{CircleOwner, CoralNeuron, MyceliumPool, NeuronCx, ProjectedCircle, Vec2};

use crate::coral_neuron_draw::{CoralNeuronSprites, NeuronLook};
use crate::draw::{Color, DrawItem, Layer, Shader};
use crate::projected_circle_draw::{CircleSurroundings, ProjectedCircleSprites};
use crate::tests::room;

fn open_room() -> rw_assets::Room {
    let rows: Vec<String> = (0..20).map(|_| ".".repeat(30)).collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// A neuron of 100 (5 segments, 3 strand pairs, 6 bumps) along y = 200 from x = 100 in 25 px
/// steps, at rest.
fn straight_neuron() -> (CoralNeuron, MyceliumPool) {
    let r = open_room();
    let mut pool = MyceliumPool::default();
    let system = pool.create_registry();
    let mut seq = 0;
    let mut cx = NeuronCx {
        room: &r,
        system,
        pool: &mut pool,
        projector: false,
        circles: Vec::new(),
        new_vine_system: false,
        seq: &mut seq,
    };
    let ends = (Some(Vec2::new(100.0, 200.0)), Some(Vec2::new(200.0, 200.0)));
    let mut n = CoralNeuron::build(100.0, ends.0, ends.1, &mut cx, &mut UnityRandom::new(1))
        .expect("a neuron")
        .neuron;
    for (k, s) in n.geometry.segments.iter_mut().enumerate() {
        let at = Vec2::new(100.0 + 25.0 * k as f32, 200.0);
        *s = [at, at, Vec2::ZERO];
    }
    (n, pool)
}

fn near(a: Vec2, x: f32, y: f32) -> bool {
    (a.x - x).abs() < 1e-3 && (a.y - y).abs() < 1e-3
}

/// `MeshColor(f)`: at the middle `HSL(Decimal(Lerp(1.025, 0.9638889, 0.5)), 1, 0.5)`, at the
/// ends `HSL(0.9638889, 0.5, 0.15)`; worked from `Custom.HSL2RGB`.
#[test]
fn the_colour_ramp() {
    let mid = NeuronLook::mesh_color(0.5);
    assert!((mid.r - 1.0).abs() < 1e-5 && mid.g.abs() < 1e-5, "{mid:?}");
    assert!((mid.b - (1.0 - 0.966_666_7)).abs() < 1e-4, "{mid:?}");
    for f in [0.0, 1.0] {
        let end = NeuronLook::mesh_color(f);
        assert!((end.r - 0.225).abs() < 1e-5, "{end:?}");
        assert!((end.g - 0.075).abs() < 1e-5, "{end:?}");
        assert!((end.b - 0.1075).abs() < 1e-4, "{end:?}");
    }
}

/// `Rad(f)`: 0.2 at the ends, 1 in the middle, `Lerp(0.2, 1, Pow(sin(f pi), 0.5))` between.
#[test]
fn the_width_along_the_body() {
    assert_eq!(NeuronLook::rad(0.5), 1.0);
    assert!((NeuronLook::rad(0.0) - 0.2).abs() < 1e-6);
    assert!((NeuronLook::rad(0.25) - (0.2 + 0.8 * 0.840_896_4)).abs() < 1e-5);
}

/// The sprites: the colour mesh (4 vertices a segment, the ramp), the line mesh over it
/// under the `CoralNeuron` shader with alpha `1 / segments`, then a mesh a strand and three
/// discs a bump, numbered as `InitiateSprites` does; vertices of the first body segment
/// worked by hand for a straight neuron.
#[test]
fn a_neuron_is_two_meshes_strands_and_bumps() {
    let (n, pool) = straight_neuron();
    assert_eq!((n.mycelia.len(), n.bumps.len()), (6, 6));
    let list = n.draw(&pool, 1.0, false);
    assert_eq!(list.items.len(), 2 + 6 + 18);
    for (i, item) in list.items.iter().enumerate() {
        assert_eq!(item.index(), i);
        assert_eq!(item.layer(), Layer::Items);
    }
    let DrawItem::Mesh(body) = &list.items[0] else {
        panic!();
    };
    assert_eq!(body.shader, Shader::Basic);
    assert_eq!(body.vertices.len(), 20);
    assert_eq!(body.vertex_colors.len(), 20);
    assert_eq!(body.vertex_colors[0], NeuronLook::mesh_color(0.0));
    assert_eq!(body.vertex_colors[19], NeuronLook::mesh_color(1.0));
    // Segment 0: both vertices at the first point (no direction yet). Segment 1 (25 px
    // apart, `overlap` 5, width `Rad(1/4)` = 0.8727): the line runs along +x so the
    // perpendicular is (0, -1) turned from (-1, 0).
    assert!(near(body.vertices[0], 100.0, 200.0) && near(body.vertices[1], 100.0, 200.0));
    let w = NeuronLook::rad(0.25);
    assert!(
        near(body.vertices[4], 105.0, 200.0 + 1.5 * w),
        "{:?}",
        body.vertices[4]
    );
    assert!(near(body.vertices[5], 105.0, 200.0 - 1.5 * w));
    assert!(near(body.vertices[6], 120.0, 200.0 + 3.5 * w));
    assert!(near(body.vertices[7], 120.0, 200.0 - 3.5 * w));
    // The ends' last segment has the plain perpendicular of width 1.
    assert!(near(body.vertices[18], 200.0 - 0.0, 199.0) || near(body.vertices[18], 200.0, 201.0));
    let DrawItem::Mesh(lines) = &list.items[1] else {
        panic!();
    };
    assert_eq!(lines.shader, Shader::CoralNeuron);
    assert_eq!(lines.vertices, body.vertices);
    assert_eq!(lines.color, Color::WHITE.with_alpha(0.2));
    assert!(lines.vertex_colors.is_empty());
    assert_eq!(lines.uvs, body.uvs);
    // Strands: meshes 2 to 7 coloured by the segment they hang from.
    for (i, item) in list.items[2..8].iter().enumerate() {
        let DrawItem::Mesh(m) = item else {
            panic!("{item:?}");
        };
        let hung = n.geometry.segment_of_mycelium(i) as f32 / 4.0;
        assert_eq!(m.vertex_colors[0], NeuronLook::mesh_color(hung));
    }
}

/// A culled strand is not drawn but keeps its numbers; discs follow after the strands.
#[test]
fn culled_strands_leave_gaps_in_the_numbers() {
    let (n, mut pool) = straight_neuron();
    pool.get_mut(n.mycelia[1]).unwrap().culled = true;
    let list = n.draw(&pool, 1.0, false);
    assert_eq!(list.items.len(), 2 + 5 + 18);
    assert_eq!(list.items[2].index(), 2);
    assert_eq!(list.items[3].index(), 4);
    assert_eq!(list.items[7].index(), 8, "the first bump's disc");
}

/// A bump at `(0.5, 0.5)`: the point at the third segment (150, 200) moved along the
/// perpendicular by `x * 4 * Rad` = 2; the highlight up and left by `1.5 * Rad`; the ping
/// dot at the bump in `HSL(2/3, 1, 0.1 + 0.9 * ping)`, in `Water` when the room has
/// `WaterLights`.
#[test]
fn a_bump_is_three_discs() {
    let (mut n, pool) = straight_neuron();
    n.bumps = vec![Vec2::new(0.5, 0.5)];
    n.bump_pings = vec![[0.5, 1.0]];
    let air = n.draw(&pool, 1.0, false);
    let lit = n.draw(&pool, 1.0, true);
    for (list, water) in [(&air, false), (&lit, true)] {
        let s: Vec<_> = list
            .items
            .iter()
            .filter_map(|i| match i {
                DrawItem::Sprite(s) => Some(*s),
                _ => None,
            })
            .collect();
        assert_eq!(s.len(), 3);
        assert_eq!(s.iter().map(|s| s.index).collect::<Vec<_>>(), [8, 9, 10]);
        assert!(s.iter().all(|s| s.element == "deerEyeB"));
        assert!(near(s[0].pos, 150.0, 202.0), "{:?}", s[0].pos);
        assert!(near(s[1].pos, 148.5, 203.5), "{:?}", s[1].pos);
        assert!(near(s[2].pos, 150.0, 202.0));
        assert_eq!(s[0].scale, Vec2::new(1.0, 1.0));
        assert_eq!(s[1].scale, Vec2::new(0.5, 0.5));
        assert_eq!(s[1].color, Color::WHITE.with_alpha(0.3));
        assert_eq!(
            s[0].color,
            NeuronLook::mesh_color(0.5).lerp(Color::rgb(0.0, 0.0, 0.2), 0.25)
        );
        // At t = 1 the ping is its current value 0.5.
        assert_eq!(s[2].color, Color::hsl(2.0 / 3.0, 1.0, 0.1 + 0.9 * 0.5));
        assert_eq!(s[2].layer, if water { Layer::Water } else { Layer::Items });
        assert_eq!(s[0].layer, Layer::Items);
    }
}

fn circle_with(owner: u64) -> ProjectedCircle {
    let mut rng = UnityRandom::new(3);
    ProjectedCircle::new(
        5,
        CircleOwner::Neuron { order: owner },
        0,
        0.5,
        &open_room(),
        &[],
        &mut rng,
    )
}

/// The circle's two discs, spokes and lines: positions shifted against the light by
/// `depth * 15`, scale `rad / 8`, the black ring's alpha `4 / Lerp(rad, 4, Pow(blink, 0.5))`.
#[test]
fn a_circle_is_two_discs_spokes_and_lines() {
    let (mut n, _) = straight_neuron();
    n.update_order = 9;
    let mut c = circle_with(9);
    c.off_screen_connections = vec![Vec2::new(-10.0, 100.0)];
    c.connected_circles.clear();
    c.connections_blink = vec![0];
    c.last_depth_offset = 1.0;
    c.depth_offset = 1.0;
    c.rad = 120.0;
    c.last_rad = 120.0;
    c.rotation = 30.0;
    c.last_rotation = 30.0;
    let neurons = [n];
    let around = CircleSurroundings {
        neurons: &neurons,
        circles: std::slice::from_ref(&c),
        light_angle: Vec2::new(0.5, 0.25),
    };
    let mut rng = UnityRandom::new(8);
    let list = c.draw(&around, 1.0, &mut rng);
    assert_eq!(
        rng.state(),
        UnityRandom::new(8).state(),
        "no blinking line, no draw"
    );
    // 2 discs, `spokes` = 15, 1 line.
    assert_eq!(list.items.len(), 2 + 15 + 1);
    let DrawItem::Sprite(red) = list.items[0] else {
        panic!();
    };
    // Circle 0 hangs on the first segment (100, 200); depth 1 moves it by -(0.5, -0.25) * 15.
    assert!(near(red.pos, 92.5, 203.75), "{:?}", red.pos);
    assert_eq!(
        (red.element, red.shader),
        ("Futile_White", Shader::VectorCircle)
    );
    assert_eq!(red.scale, Vec2::new(15.0, 15.0));
    assert_eq!(red.color, Color::rgb(1.0 / 255.0, 0.0, 0.0));
    assert_eq!(red.layer, Layer::BackgroundShortcuts);
    let DrawItem::Sprite(ring) = list.items[1] else {
        panic!();
    };
    assert_eq!(ring.color, Color::BLACK.with_alpha(4.0 / 120.0));
    // Spoke 0: 30 degrees, at `rad - Lerp(1, 5, blink 0)` = 119 from the centre.
    let DrawItem::Sprite(spoke) = list.items[2] else {
        panic!();
    };
    let dir = Vec2::from_degrees(30.0);
    assert!(
        near(spoke.pos, 92.5 + dir.x * 119.0, 203.75 + dir.y * 119.0),
        "{:?}",
        spoke.pos
    );
    assert_eq!((spoke.rotation, spoke.element), (30.0, "pixel"));
    assert_eq!(spoke.anchor, Vec2::new(0.5, 0.0));
    assert_eq!(spoke.scale, Vec2::new(4.0, 10.0));
    // The line to the off-screen point leaves the rim and runs to it.
    let DrawItem::Sprite(line) = list.items[17] else {
        panic!();
    };
    let to = Vec2::new(-10.0, 100.0);
    let centre = Vec2::new(92.5, 203.75);
    assert!((line.scale.y - (centre.distance(to) - 120.0)).abs() < 1e-3);
    assert_eq!(line.scale.x, 2.0);
}

/// A blinking connection's width takes a cosmetic draw: 4 when it is under 0.7.
#[test]
fn a_blinking_line_draws_its_width() {
    let (mut n, _) = straight_neuron();
    n.update_order = 9;
    let mut c = circle_with(9);
    c.connected_circles.clear();
    c.off_screen_connections = vec![Vec2::new(-10.0, 100.0)];
    c.connections_blink = vec![5];
    let neurons = [n];
    let around = CircleSurroundings {
        neurons: &neurons,
        circles: std::slice::from_ref(&c),
        light_angle: Vec2::new(0.0, 0.0),
    };
    for seed in 1..20 {
        let mut rng = UnityRandom::new(seed);
        let list = c.draw(&around, 1.0, &mut rng);
        let first = UnityRandom::new(seed).value();
        let DrawItem::Sprite(line) = list.items.last().unwrap() else {
            panic!();
        };
        assert_eq!(line.scale.x, if first < 0.7 { 4.0 } else { 2.0 });
        let mut one = UnityRandom::new(seed);
        one.value();
        assert_eq!(rng.state(), one.state());
    }
}

/// A connection to another circle runs between the rims at their positions.
#[test]
fn a_connected_circle_is_reached_rim_to_rim() {
    let (mut n, _) = straight_neuron();
    n.update_order = 9;
    let mut a = circle_with(9);
    let mut b = circle_with(9);
    b.update_order = 6;
    b.index = 1;
    a.connected_circles = vec![6];
    a.off_screen_connections.clear();
    a.connections_blink = vec![0];
    a.rad = 100.0;
    a.last_rad = 100.0;
    b.rad = 50.0;
    b.last_rad = 50.0;
    let neurons = [n];
    let circles = [a.clone(), b];
    let around = CircleSurroundings {
        neurons: &neurons,
        circles: &circles,
        light_angle: Vec2::new(0.0, 0.0),
    };
    let list = a.draw(&around, 1.0, &mut UnityRandom::new(1));
    let DrawItem::Sprite(line) = list.items.last().unwrap() else {
        panic!();
    };
    // From (100, 200) to the last segment (200, 200): 100 apart, 100 + 50 of radii.
    assert_eq!(line.scale.y, 0.0, "overlapping rims leave no line");
    assert!(near(line.pos, 200.0, 200.0), "{:?}", line.pos);
    assert!((line.rotation - 90.0).abs() < 1e-3);
}
