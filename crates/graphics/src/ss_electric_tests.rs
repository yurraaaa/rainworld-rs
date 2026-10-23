//! The superstructure electric objects' sprites: a rod's bar, a coil's flare vertices and
//! colour, a flash's draws, the fuse cells' positions and colour modes.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::SsLightRodData;
use rw_random::UnityRandom;
use rw_sim::{FloatExt, FuseSetup, SsLightRod, SuperStructureFuses, Vec2, ZapCoil, ZapFlash};

use super::{FuseSprites, LightRodSprites, ZapCoilSprites, ZapFlashSprites};
use crate::draw::{Color, DrawItem, Layer, Shader, Sprite};

fn sprite(item: &DrawItem) -> Sprite {
    match item {
        DrawItem::Sprite(s) => *s,
        other => panic!("{other:?}"),
    }
}

/// A rod is one `pixel` 4 wide and `length` long anchored at its foot on `CustomDepth` in
/// `Water`, alpha `1 - depth`, rotated by the data, and black by 90 percent of the lack of
/// power.
#[test]
fn the_rod_is_a_pixel_bar() {
    let data = SsLightRodData {
        panel_pos: [0.0, 0.0],
        depth: 0.25,
        rotation: 135.0,
        length: 110.0,
        brightness: 0.5,
    };
    let rod = SsLightRod::new(
        9,
        Vec2::new(30.0, 40.0),
        data,
        &[1, 2],
        &mut UnityRandom::new(1),
    );
    let list = rod.draw(1.0);
    let s = sprite(&list.items[0]);
    assert_eq!(
        (s.element, s.pos, s.rotation, s.scale, s.anchor),
        (
            "pixel",
            Vec2::new(30.0, 40.0),
            135.0,
            Vec2::new(4.0, 110.0),
            Vec2::new(0.5, 0.0)
        )
    );
    assert_eq!((s.shader, s.layer), (Shader::CustomDepth, Layer::Water));
    assert_eq!(s.color, Color::rgb(0.4, 1.0, 0.8).with_alpha(0.75));
    let dark = sprite(&rod.draw(0.0).items[0]);
    let want = Color::rgb(0.4, 1.0, 0.8).lerp(Color::BLACK, 0.9);
    assert_eq!(
        (dark.color.r, dark.color.g, dark.color.b),
        (want.r, want.g, want.b)
    );
}

fn vertices(list: &crate::draw::DrawList) -> Vec<Vec2> {
    match &list.items[0] {
        DrawItem::Mesh(m) => m.vertices.clone(),
        other => panic!("{other:?}"),
    }
}

/// A horizontal coil's eight vertices: the rectangle's pixels widened 30 at both ends,
/// its two edges swinging `120 * turnedOn * flicker` away; UVs run 0, 0.4, 0.6, 1.
#[test]
fn a_horizontal_coil_flares_at_its_edges() {
    let mut c = ZapCoil::new(7, [2, 3, 5, 4]);
    c.flicker = [[0.5, 0.5, 0.0, 0.0], [0.25, 0.25, 0.0, 0.0]];
    let list = c.draw(0.5, false);
    let want = [
        (10.0, 0.0),
        (10.0, 160.0),
        (40.0, 0.0),
        (40.0, 160.0),
        (120.0, 30.0),
        (120.0, 130.0),
        (150.0, 30.0),
        (150.0, 130.0),
    ]
    .map(|(x, y)| Vec2::new(x, y));
    assert_eq!(vertices(&list), want);
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("mesh");
    };
    assert_eq!((m.shader, m.layer), (Shader::FlareBomb, Layer::Foreground));
    assert_eq!(m.uvs[2], Vec2::new(0.0, 0.4));
    assert_eq!(m.uvs[4], Vec2::new(0.0, 1.0 - 0.4));
    assert_eq!(m.triangles[5], [5, 6, 7]);
    assert_eq!(m.color, Color::rgb(0.0, 0.0, 1.0).with_alpha(1.0));
    let DrawItem::Mesh(w) = &c.draw(0.5, true).items[0] else {
        panic!("mesh");
    };
    assert_eq!(w.layer, Layer::Water);
}

/// A vertical coil swings sideways (`turnedOn` between ticks 0.5 here) and is widened 30
/// up and down; the colour's red and green follow `InverseLerp(0, 0.5, zapLit)` times it.
#[test]
fn a_vertical_coil_flares_sideways() {
    let mut c = ZapCoil::new(7, [2, 3, 3, 8]);
    c.flicker = [[0.5, 0.5, 0.0, 0.0], [0.25, 0.25, 0.0, 0.0]];
    c.last_turned_on = 0.0;
    c.turned_on = 1.0;
    c.zap_lit = 0.25;
    let list = c.draw(0.5, false);
    let want = [
        (25.0, 210.0),
        (95.0, 210.0),
        (25.0, 180.0),
        (95.0, 180.0),
        (10.0, 60.0),
        (110.0, 60.0),
        (10.0, 30.0),
        (110.0, 30.0),
    ]
    .map(|(x, y)| Vec2::new(x, y));
    assert_eq!(vertices(&list), want);
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("mesh");
    };
    assert_eq!(m.color, Color::rgb(0.25, 0.25, 1.0).with_alpha(0.5));
}

/// A flash without a light makes four draws (the flare's scale and alpha, the flat disc's
/// scale and alpha); with one, three more come first (rad, alpha, colour) and the light is
/// set from them.
#[test]
fn a_flash_draws_four_or_seven() {
    let mut f = ZapFlash::new(4, Vec2::new(30.0, 40.0), 0.6, &mut UnityRandom::new(1));
    f.last_life = 0.8;
    f.life = 0.6;
    f.last_pos = Vec2::new(20.0, 40.0);
    let num = 0.7;
    let mut rng = UnityRandom::new(3);
    let mut e = UnityRandom::new(3);
    let (list, light) = f.draw(0.5, &mut rng);
    assert!(light.is_none());
    let draws: Vec<f32> = (0..4).map(|_| e.value()).collect();
    assert_eq!(rng.state(), e.state());
    let flare = sprite(&list.items[0]);
    let flat = sprite(&list.items[1]);
    assert_eq!(flare.pos, Vec2::new(25.0, 40.0));
    assert!((flare.scale.x - 0.5f32.lerp(1.0, draws[0] * num * 0.6) * 500.0 / 16.0).abs() < 1e-3);
    assert!((flare.color.a - num * draws[1]).abs() < 1e-6);
    assert!(
        (flat.scale.x - 0.5f32.lerp(1.0, (0.5 + 0.5 * draws[2]) * num * 0.6) * 400.0 / 16.0).abs()
            < 1e-3
    );
    assert!((flat.color.a - num * draws[3]).abs() < 1e-6);
    assert_eq!(
        (flare.shader, flat.shader, flare.layer),
        (Shader::FlareBomb, Shader::FlatLight, Layer::Foreground)
    );
    assert_eq!((flare.color.r, flare.color.b), (0.0, 1.0));
    assert_eq!((flat.color.r, flat.color.b), (1.0, 1.0));
    f.light = Some(11);
    let mut rng = UnityRandom::new(3);
    let mut e = UnityRandom::new(3);
    let (_, light) = f.draw(0.5, &mut rng);
    let d: Vec<f32> = (0..7).map(|_| e.value()).collect();
    assert_eq!(rng.state(), e.state());
    let light = light.expect("light");
    assert!((light.rad - 0.25f32.lerp(1.0, d[0] * num * 0.6) * 2400.0).abs() < 1e-2);
    assert!((light.alpha - (num * d[1]).powf(0.4)).abs() < 1e-5);
    let n = (num * d[2]).powf(4.0);
    assert!((light.color[0] - n).abs() < 1e-6 && light.color[2] == 1.0);
}

fn fuses(depth: i32) -> SuperStructureFuses {
    SuperStructureFuses::new(
        3,
        FuseSetup {
            pos: Vec2::new(100.0, 40.0),
            rect: [5, 2, 6, 3],
            size: 10.0,
            depth,
            broken: 0.0,
            gravity_dependent: false,
        },
    )
}

/// A grid is one `Futile_White` per cell, `size / 16` big, on `CustomDepth` in
/// `Foreground`, alpha by the depth (59/60, 19/30, 0.3), pushed by `ApplyDepth` at
/// `Min(-4 + 10 depth, 12)` about the camera plus (700, 533.3334); nothing while culled.
#[test]
fn the_cells_are_pushed_by_depth() {
    let camera = Vec2::new(50.0, 20.0);
    for (depth, alpha, push) in [
        (0, 59.0 / 60.0, -4.0),
        (1, 19.0 / 30.0, 6.0),
        (2, 0.3, 12.0),
    ] {
        let f = fuses(depth);
        let list = f.draw(0.0, camera);
        assert_eq!(list.items.len(), 2 * 2);
        let s = sprite(&list.items[1]);
        assert_eq!(s.index, 1);
        assert_eq!(
            (s.shader, s.layer),
            (Shader::CustomDepth, Layer::Foreground)
        );
        assert_eq!(s.scale, Vec2::new(10.0 / 16.0, 10.0 / 16.0));
        assert_eq!(s.color.a, alpha);
        // Cell (0, 1): (100 + 5, 40 + 5 + 10).
        let d = (10.0 - push * -0.025) * 0.1;
        let dp = camera + Vec2::new(700.0, 533.3334);
        let want = (Vec2::new(105.0, 55.0) - dp) / d + dp;
        assert_eq!(s.pos, want);
    }
    let mut f = fuses(1);
    f.culled = true;
    assert!(f.draw(0.0, camera).items.is_empty());
}

/// The colours: blue `Pow(Lerp(last, now, t), 2.5 - lightness) * Lerp(0.3, 1, lightness)`
/// by default; mode 5 blinks red by its timer over 8; a malfunction rectangle's bright
/// cells go red; the debug mode shows a chequer.
#[test]
fn the_cell_colours_follow_the_modes() {
    let camera = Vec2::ZERO;
    let mut f = fuses(1);
    f.last_lightness = 0.5;
    f.lightness = 0.5;
    let c = f.cell_mut(0, 0);
    c[0] = 0.6;
    c[1] = 0.2;
    let blue = sprite(&f.draw(0.5, camera).items[0]).color;
    let level = 0.4f32.mathf_pow(2.0) * 0.3f32.lerp(1.0, 0.5);
    assert_eq!((blue.r, blue.g), (0.0, 0.0));
    assert!((blue.b - level).abs() < 1e-6, "{}", blue.b);
    // Mode 5, timer 5: 5 % 8 > 3 lit; timer 2 dark.
    let c = f.cell_mut(0, 1);
    c[3] = 5.0;
    c[4] = 5.0;
    f.power_flicker = 0.8;
    let lit = sprite(&f.draw(0.5, camera).items[1]).color;
    assert_eq!((lit.r, lit.g, lit.b), (0.8, 0.0, 0.0));
    f.cell_mut(0, 1)[4] = 2.0;
    let dark = sprite(&f.draw(0.5, camera).items[1]).color;
    assert_eq!(dark.r, 0.0);
    // A malfunction rectangle over cell (1, 0) with a bright cell.
    f.smoothed_malfunction = 0.5;
    f.malfunction_rect = [1, 0, 1, 0];
    f.cell_mut(1, 0)[0] = 0.9;
    let red = sprite(&f.draw(0.5, camera).items[2]).color;
    let want = 1.0f32.lerp(0.25, 0.5f32.mathf_pow(0.2)) * 0.8;
    assert!((red.r - want).abs() < 1e-6 && red.b == 0.0);
    // Debug mode: (j even, k even, j odd and k odd).
    f.debug_mode = 3;
    let items = f.draw(0.5, camera).items;
    let rgb: Vec<(f32, f32, f32)> = [0, 1, 3]
        .map(|i| sprite(&items[i]).color)
        .map(|c| (c.r, c.g, c.b))
        .to_vec();
    assert_eq!(rgb, [(1.0, 1.0, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, 1.0)]);
}
