//! A `ProjectedCircle`'s sprites against `ProjectedCircle.cs`: the circle's discs, spokes
//! and lines.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{CircleOwner, ProjectedCircle, Vec2};

use super::{CircleSurroundings, ProjectedCircleSprites};
use crate::core::draw::{Color, DrawItem, Layer, Shader};
use crate::superstructure::coral::neuron::tests::straight_neuron;
use crate::tests::{near, open_room};

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
        stems: &[],
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
        stems: &[],
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
        stems: &[],
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
