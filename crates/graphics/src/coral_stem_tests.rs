//! A `StemSegment`'s sprites against `StemSegment.cs`: the sprite numbering, the colours, the
//! bridge disc between two segments worked by hand, the bridge of the last segment left at
//! the screen's corner, the dots' glow and their draws.
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::creature::IdIssuer;
use rw_sim::world::AiMap;
use rw_sim::{CoralStem, MyceliumPool, NeuronCx, StemCx, Vec2};

use crate::coral_stem_draw::{StemFrame, StemLook, StemSegmentSprites};
use crate::draw::{Color, DrawItem, Layer, Shader};
use crate::tests::room;

fn open_room() -> rw_assets::Room {
    let rows: Vec<String> = (0..40).map(|_| ".".repeat(60)).collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

fn stem(rad: f32) -> (CoralStem, MyceliumPool) {
    let r = open_room();
    let map = AiMap::new(&r, 0, false, false);
    let mut pool = MyceliumPool::default();
    let system = pool.create_registry();
    let mut ids = IdIssuer {
        next_issued_id: 100,
    };
    let mut seq = 0;
    let mut cx = StemCx {
        neuron: NeuronCx {
            room: &r,
            system,
            pool: &mut pool,
            projector: false,
            circles: Vec::new(),
            new_vine_system: false,
            seq: &mut seq,
        },
        ai_map: &map,
        ids: &mut ids,
        index: 0,
    };
    let built = CoralStem::build(
        Vec2::new(610.0, 410.0),
        rad,
        false,
        &mut cx,
        &mut UnityRandom::new(1),
    );
    (built.stem, pool)
}

fn frame(water_lights: bool) -> StemFrame {
    StemFrame {
        t: 1.0,
        darkness: 0.0,
        water_lights,
        cam_pos: Vec2::new(33.0, 44.0),
    }
}

fn near(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

/// The colours of `BaseColor` and `HighLightColor` for a size 0 segment in the light:
/// `Lerp(HSL(0.025, 0.4, 0.05), (0, 0, 0.1), Pow(InverseLerp(0.45, -0.05, 0), 0.9) * 0.5)`
/// and `Lerp(HSL(0.025, 0.5, 0.15), (0, 0, 0.15), Pow(0.9, 0.9) * 0.4)`; a size over 0.45
/// is not tinted toward the dark blue; a dark room dims `l` by up to `0.5` and `0.65`.
#[test]
fn the_colours() {
    let k = 0.9f32.powf(0.9);
    let base = StemLook::base_color(0.0, 0.0);
    let want = Color::hsl(0.025, 0.4, 0.05).lerp(Color::rgb(0.0, 0.0, 0.1), k * 0.5);
    assert_eq!(base, want);
    let high = StemLook::highlight_color(0.0, 0.0);
    assert_eq!(
        high,
        Color::hsl(0.025, 0.5, 0.15).lerp(Color::rgb(0.0, 0.0, 0.15), k * 0.4)
    );
    // Size 1: `Lerp(0.4, 0.1, 1)`, `Lerp(0.05, 0.7, 1)`, no tint.
    close(StemLook::base_color(1.0, 0.0), Color::hsl(0.025, 0.1, 0.7));
    close(
        StemLook::highlight_color(1.0, 1.0),
        Color::hsl(0.025, 0.1, 0.85 - 0.65),
    );
    close(StemLook::base_color(1.0, 1.0), Color::hsl(0.025, 0.1, 0.2));
}

fn close(a: Color, b: Color) {
    let d = (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs() + (a.a - b.a).abs();
    assert!(d < 1e-5, "{a:?} vs {b:?}");
}

/// A size 0 stem is one segment of 20 strands and one dot: `InitiateSprites` numbers the
/// strands 0 to 19, the bridge `Circle20` 20, the two meshes 21 and 22 (`meshSegs` 5: 20
/// vertices) and the dot's three discs 23 to 25. The last segment's bridge is never moved:
/// it stays at the screen's origin (the camera's position), `scaleX = rad / 20`; the dot's
/// glow draws 1 or 2 values.
#[test]
fn a_size_zero_segment_is_strands_a_bridge_two_meshes_and_a_dot() {
    let (stem, pool) = stem(0.0);
    let mut rng = UnityRandom::new(3);
    let list = stem.draw_segment(0, &pool, &frame(false), &mut rng);
    let g = &stem.segments[0];
    assert_eq!(list.items.len(), 20 + 1 + 2 + 3);
    for (i, item) in list.items[..20].iter().enumerate() {
        let DrawItem::Mesh(m) = item else {
            panic!("a strand mesh")
        };
        assert_eq!(
            (m.index, m.layer, m.shader),
            (i, Layer::Items, Shader::Basic)
        );
        assert_eq!(m.vertex_colors.len(), m.vertices.len());
    }
    let DrawItem::Sprite(bridge) = &list.items[20] else {
        panic!("the bridge")
    };
    assert_eq!((bridge.index, bridge.element), (20, "Circle20"));
    assert_eq!(bridge.pos, Vec2::new(33.0, 44.0));
    assert_eq!(bridge.scale, Vec2::new(g.body.chunks[1].rad / 20.0, 1.0));
    assert_eq!(bridge.color, Color::hsl(0.025, 1.0, 0.1));
    for (i, color) in [
        (21, StemLook::base_color(0.0, 0.0)),
        (22, StemLook::highlight_color(0.0, 0.0)),
    ] {
        let DrawItem::Mesh(m) = &list.items[i] else {
            panic!("a body mesh")
        };
        assert_eq!((m.index, m.vertices.len(), m.color), (i, 20, color));
    }
    let discs: Vec<_> = list.items[23..]
        .iter()
        .map(|i| match i {
            DrawItem::Sprite(s) => s,
            _ => panic!("a disc"),
        })
        .collect();
    assert_eq!(
        discs.iter().map(|s| s.index).collect::<Vec<_>>(),
        [23, 24, 25]
    );
    // `Lerp(1.2, 7, 0)`: 1.2 / 20, the glow 0.9 of it.
    assert!((discs[0].scale.x - 1.2 / 20.0).abs() < 1e-6);
    assert!((discs[2].scale.x - 1.2 / 20.0 * 0.9).abs() < 1e-6);
    // The first two are the base and highlight a pixel off the dot, opposite ways.
    assert!(near(discs[0].pos - discs[1].pos, Vec2::new(-2.0, 2.0)));
    // The glow: `value < 0.5` then `HSL(2/3, 1, 0.1 + 0.4 * value)`, else `(0, 0, 0.1)`.
    let mut reference = UnityRandom::new(3);
    let blue = reference.value() < 0.5;
    let glow = if blue {
        Color::hsl(2.0 / 3.0, 1.0, 0.1 + 0.4 * reference.value())
    } else {
        Color::rgb(0.0, 0.0, 0.1)
    };
    assert_eq!(discs[2].color, glow);
    assert_eq!(rng.state(), reference.state());
}

/// In a room with `WaterLights` the glow of each dot is in `Water`, the rest in `Items`.
#[test]
fn the_glow_goes_to_the_water_container() {
    let (stem, pool) = stem(0.0);
    let list = stem.draw_segment(0, &pool, &frame(true), &mut UnityRandom::new(3));
    let layers: Vec<Layer> = list.items[23..].iter().map(DrawItem::layer).collect();
    assert_eq!(layers, [Layer::Items, Layer::Items, Layer::Water]);
}

/// The bridge of a segment that has a next one: `Lerp(chunk 1, next chunk 0, 0.35)` is where
/// the body ends, and the disc stands half way from there to the next segment's start,
/// `scaleY = distance / 20`, turned `AimFromOneVectorToAnother`.
#[test]
fn a_bridge_joins_a_segment_to_the_next() {
    let (mut stem, pool) = stem(200.0);
    assert!(stem.segments.len() > 2);
    let (a, b) = (Vec2::new(300.0, 300.0), Vec2::new(340.0, 340.0));
    stem.segments[0].body.chunks[1].pos = a;
    stem.segments[0].body.chunks[1].last_pos = a;
    stem.segments[1].body.chunks[0].pos = b;
    stem.segments[1].body.chunks[0].last_pos = b;
    let m = stem.segments[0].mycelia.len();
    let list = stem.draw_segment(0, &pool, &frame(false), &mut UnityRandom::new(1));
    let DrawItem::Sprite(bridge) = &list.items[m] else {
        panic!("the bridge")
    };
    let end = a + (b - a) * 0.35;
    let at = end + (b - end) * 0.5;
    assert!(near(bridge.pos, at), "{:?} vs {at:?}", bridge.pos);
    assert!((bridge.scale.y - (b - end).length() / 20.0).abs() < 1e-5);
    // Up and to the right: 45 degrees clockwise from up.
    assert!((bridge.rotation - 45.0).abs() < 1e-3, "{}", bridge.rotation);
}
