//! The screen-grab objects' sprites: the mesh and its vertex colour, the disc and streaks
//! with their depth pushes, the specks' grey pixels.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::DeepProcessingData;
use rw_random::UnityRandom;
use rw_sim::{DeepProcessing, DisruptorSpeck, FloatExt, GravityDisruptor, SPECKS, Vec2};

use super::{DeepProcessingSprites, DisruptorSpeckSprites, GravityDisruptorSprites};
use crate::core::draw::{Color, DrawItem, Layer, Shader};

fn sprite(item: &DrawItem) -> &crate::core::draw::Sprite {
    match item {
        DrawItem::Sprite(s) => s,
        other => panic!("not a sprite: {other:?}"),
    }
}

/// A 300 px quad is a 2 x 2 grid (150 px a division): 9 vertices, 8 triangles, all one
/// colour `(fromDepth, toDepth, power, intensity)` in `Foreground` with the shader.
#[test]
fn the_mesh_is_a_grid_with_the_vertex_colour() {
    let data = DeepProcessingData::parse("0~300~300~300~300~0~0~0~0.25~0.9~0.4").expect("data");
    let mut d = DeepProcessing::new(1, Vec2::new(5.0, 6.0), data);
    d.power = 0.7;
    let list = d.draw();
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("a mesh");
    };
    assert_eq!(
        (m.shader, m.layer),
        (Shader::DeepProcessing, Layer::Foreground)
    );
    assert_eq!(
        (m.vertices.len(), m.triangles.len(), m.uvs.len()),
        (9, 8, 9)
    );
    assert_eq!(
        m.color,
        Color {
            r: 0.25,
            g: 0.9,
            b: 0.7,
            a: 0.4
        }
    );
    assert_eq!(m.vertices[0], Vec2::new(5.0, 6.0));
    assert_eq!(m.vertices[8], Vec2::new(305.0, 306.0));
}

fn disruptor(depth: i32) -> GravityDisruptor {
    GravityDisruptor::new(30, Vec2::new(100.0, 200.0), depth, &mut UnityRandom::new(1))
}

/// 17 sprites: the disc in `HUD` (scale 37.5, `GravityDisruptor`, white at the power
/// between ticks) and sixteen streaks in `Foreground`, 22.5 degrees apart, anchored below
/// the centre, faded by the depth: `1 - (10 depth + 2) / 30`.
#[test]
fn a_disc_and_sixteen_streaks() {
    for (depth, alpha) in [
        (0, 1.0 - 2.0 / 30.0),
        (1, 1.0 - 12.0 / 30.0),
        (2, 1.0 - 22.0 / 30.0),
    ] {
        let mut d = disruptor(depth);
        d.last_power = 0.2;
        d.power = 0.6;
        let list = d.draw(0.5, Vec2::ZERO);
        assert_eq!(list.items.len(), 17);
        let disc = sprite(&list.items[0]);
        assert_eq!(
            (disc.shader, disc.layer, disc.scale),
            (Shader::GravityDisruptor, Layer::Hud, Vec2::new(37.5, 37.5))
        );
        assert_eq!(disc.color, Color::WHITE.with_alpha(0.2f32.lerp(0.6, 0.5)));
        for (i, item) in list.items[1..].iter().enumerate() {
            let s = sprite(item);
            assert_eq!(s.index, i + 1);
            assert_eq!(
                (s.shader, s.layer),
                (Shader::CustomDepth, Layer::Foreground)
            );
            assert_eq!(s.scale, Vec2::new(0.9375, 3.125));
            assert_eq!(s.anchor, Vec2::new(0.5, -0.55));
            assert_eq!(s.rotation, i as f32 / 16.0 * 360.0);
            assert_eq!(s.color.a, alpha);
            assert_eq!(s.pos, disc.pos);
        }
    }
}

/// `ApplyDepth(placed, -3 + 10 depth)` from the depth point `camera + (700, 533.3334)`:
/// `(p - dp) / ((10 + 0.025 d) * 0.1) + dp`.
#[test]
fn the_sprites_are_pushed_by_the_depth() {
    let camera = Vec2::new(50.0, 20.0);
    let dp = camera + Vec2::new(700.0, 533.3334);
    for depth in 0..3 {
        let d = disruptor(depth);
        let list = d.draw(1.0, camera);
        let push = -3.0 + 10.0 * depth as f32;
        let expected = (Vec2::new(100.0, 200.0) - dp) / ((10.0 + 0.025 * push) * 0.1) + dp;
        assert_eq!(sprite(&list.items[0]).pos, expected);
    }
}

/// A streak is blue by its brightness between ticks times the power; a moved placed
/// object paints them in a red and green chequer.
#[test]
fn the_streaks_are_blue_or_chequered() {
    let mut d = disruptor(1);
    d.power = 0.5;
    d.last_power = 0.5;
    d.lights[4] = [0.8, 0.4, 0.0, 0.0];
    let list = d.draw(0.25, Vec2::ZERO);
    let s = sprite(&list.items[5]);
    assert_eq!(
        (s.color.r, s.color.g, s.color.b),
        (0.0, 0.0, 0.4f32.lerp(0.8, 0.25) * 0.5)
    );
    d.debug_mode = 3;
    let list = d.draw(0.25, Vec2::ZERO);
    let (even, odd) = (sprite(&list.items[1]), sprite(&list.items[2]));
    assert_eq!((even.color.r, even.color.g, even.color.b), (1.0, 0.0, 0.0));
    assert_eq!((odd.color.r, odd.color.g, odd.color.b), (0.0, 1.0, 0.0));
}

/// A speck is a `pixel` in `Foreground`, grey at the disruptor's power, between its
/// last and current position.
#[test]
fn a_speck_is_a_grey_pixel() {
    let orders = [0; SPECKS];
    let mut specks =
        GravityDisruptor::make_specks(Vec2::ZERO, &orders, 30, &mut UnityRandom::new(4));
    let s: &mut DisruptorSpeck = &mut specks[0];
    s.last_pos = Vec2::new(10.0, 20.0);
    s.pos = Vec2::new(14.0, 28.0);
    let list = s.draw(0.3, 0.25);
    let p = sprite(&list.items[0]);
    assert_eq!(
        (p.element, p.layer, p.shader),
        ("pixel", Layer::Foreground, Shader::Basic)
    );
    assert_eq!(p.pos, Vec2::new(11.0, 22.0));
    assert_eq!(p.color, Color::rgb(0.3, 0.3, 0.3));
}
