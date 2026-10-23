//! `CustomDecal`'s grid division, mesh vertices and per-vertex noise draws. Expected values
//! follow `CustomDecal.GetIdealGridDiv` and `UpdateVerts`, replayed on seeded streams.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::CustomDecalData;
use rw_random::UnityRandom;

use super::CustomDecal;
use crate::{FloatExt, Vec2};

fn data(handles: [[f32; 2]; 3], noise: f32, vertices: [[f32; 2]; 4]) -> CustomDecalData {
    CustomDecalData {
        handles,
        panel: [0.0; 2],
        from_depth: 0.25,
        to_depth: 0.75,
        noise,
        vertices,
        image_name: "rust1".to_owned(),
    }
}

fn square(side: f32) -> [Vec2; 4] {
    [
        Vec2::new(0.0, 0.0),
        Vec2::new(0.0, side),
        Vec2::new(side, side),
        Vec2::new(side, 0.0),
    ]
}

/// The longest side over 150, rounded half to even (`Mathf.RoundToInt`), within 1 to 20.
#[test]
fn grid_division() {
    for (side, want) in [
        (40.0, 1),
        (74.0, 1),
        (76.0, 1),
        (150.0, 1),
        (224.0, 1),
        (226.0, 2),
        (300.0, 2),
        (374.0, 2),
        (375.0, 2),
        (376.0, 3),
        (525.0, 4),
        (100_000.0, 20),
    ] {
        assert_eq!(CustomDecal::ideal_grid_div(&square(side)), want, "{side}");
    }
}

/// The closing side `0-3` counts: the other sides are 10, 10 and 206 (a division of 1),
/// side `0-3` is 226 (2).
#[test]
fn grid_division_reads_the_closing_side() {
    let quad = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(20.0, 0.0),
        Vec2::new(226.0, 0.0),
    ];
    assert_eq!(CustomDecal::ideal_grid_div(&quad), 2);
}

/// An axis-aligned 300 px square, no noise: a 3 x 3 vertex grid every 150 px with
/// `(from, to, v[.][1], v[.][0])` as the colour. Index `j * 3 + i`.
#[test]
fn flat_quad_vertices_and_colours() {
    let d = data(
        [[0.0, 300.0], [300.0, 300.0], [300.0, 0.0]],
        0.0,
        [[0.5, 0.0], [0.5, 0.0], [0.5, 0.0], [0.5, 0.0]],
    );
    let decal = CustomDecal::new(7, Vec2::new(0.0, 0.0), d);
    assert_eq!(decal.update_order, 7);
    assert_eq!(decal.grid_div, 2);
    assert_eq!(decal.verts.len(), 9);
    for j in 0..3 {
        for i in 0..3 {
            // The vertex is the crossing of the line from `Lerp(q0, q1, j/2)` to
            // `Lerp(q3, q2, j/2)` with the line from `Lerp(q0, q3, i/2)` to
            // `Lerp(q1, q2, i/2)`: q0 -> q1 runs along y, q0 -> q3 along x.
            let want = Vec2::new(150.0 * i as f32, 150.0 * j as f32);
            assert_eq!(decal.verts[j * 3 + i], want, "({i}, {j})");
            assert_eq!(decal.colors[j * 3 + i], [0.25, 0.75, 0.0, 0.5]);
        }
    }
}

/// The corners' colour values blend bilinearly over the grid (`v[3]`, `v[2]` along `i`,
/// `v[0]`, `v[1]` along `i`, then along `j`), `f` from column 1 and `f2` from column 0.
#[test]
fn colour_values_interpolate_over_the_grid() {
    let d = data(
        [[0.0, 100.0], [100.0, 100.0], [100.0, 0.0]],
        0.0,
        [[0.1, 0.2], [0.3, 0.4], [0.5, 0.6], [0.7, 0.8]],
    );
    let decal = CustomDecal::new(0, Vec2::ZERO, d);
    assert_eq!(decal.grid_div, 1);
    // Index j * 2 + i: j = 0 is the blend of v[3] (i = 0) and v[2] (i = 1), j = 1 that
    // of v[0] and v[1].
    let want_f = [0.8, 0.6, 0.2, 0.4];
    let want_f2 = [0.7, 0.5, 0.1, 0.3];
    for k in 0..4 {
        // `Mathf.Lerp` in single precision: 0.8 + (0.2 - 0.8) is 0.19999999.
        assert!((decal.colors[k][2] - want_f[k]).abs() < 1e-6, "f {k}");
        assert!((decal.colors[k][3] - want_f2[k]).abs() < 1e-6, "f2 {k}");
    }
}

/// With noise, each vertex takes four draws in order: the exponents of `f` and `f2`,
/// then the targets of `f` and `f2`; `f2`'s pull uses the already pulled `f`. The seed
/// is the truncated sum of the corner coordinates: corners (10, 20), (10, 120),
/// (110, 120), (110, 20) add up to 520. Vertices are visited `i` outer, `j` inner and
/// stored at `j * 2 + i`.
#[test]
fn noise_draws_four_per_vertex_from_the_corner_sum() {
    let pos = Vec2::new(10.0, 20.0);
    let d = data(
        [[0.0, 100.0], [100.0, 100.0], [100.0, 0.0]],
        0.6,
        [[0.4, 0.5]; 4],
    );
    let decal = CustomDecal::new(0, pos, d);
    assert_eq!(decal.grid_div, 1);
    let mut r = UnityRandom::new(520);
    let noise = 0.6_f32;
    for (i, j) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        let (a, b, c, e) = (r.value(), r.value(), r.value(), r.value());
        let mut f = 0.5_f32.mathf_pow(1.0 + (-0.5_f32).lerp(0.5, a) * noise);
        let mut f2 = 0.4_f32.mathf_pow(1.0 + (-0.5_f32).lerp(0.5, b) * noise);
        f = f.lerp(c, noise * (1.0 - 2.0 * (f - 0.5).abs()).mathf_pow(2.5));
        f2 = f2.lerp(e, noise * (1.0 - 2.0 * (f - 0.5).abs()).mathf_pow(2.5));
        assert_eq!(
            decal.colors[j * 2 + i],
            [0.25, 0.75, f, f2],
            "vertex ({i}, {j})"
        );
    }
}

/// The noise draws happen even when `noise` is 0 (they leave the values unchanged), and
/// building a decal never touches another stream: two builds agree.
#[test]
fn building_is_repeatable() {
    let d = data(
        [[0.0, 700.0], [700.0, 700.0], [700.0, 0.0]],
        0.3,
        [[0.5, 0.5]; 4],
    );
    let a = CustomDecal::new(1, Vec2::new(5.0, 5.0), d.clone());
    let b = CustomDecal::new(2, Vec2::new(5.0, 5.0), d);
    assert_eq!(a.grid_div, 5);
    assert_eq!(a.verts.len(), 36);
    assert_eq!(a.verts, b.verts);
    assert_eq!(a.colors, b.colors);
}

/// A skewed quad warps the grid by line intersection, not by bilinear blending of the
/// corners: the middle vertex of a trapezoid (wide at the bottom) is the crossing of the
/// two midlines, not the corners' average.
#[test]
fn skewed_quad_uses_line_intersection() {
    // q0 = (0, 0), q1 = (0, 300), q2 = (200, 300), q3 = (300, 0): a trapezoid.
    let d = data(
        [[0.0, 300.0], [200.0, 300.0], [300.0, 0.0]],
        0.0,
        [[0.5, 0.0]; 4],
    );
    let decal = CustomDecal::new(0, Vec2::ZERO, d);
    assert_eq!(decal.grid_div, 2);
    // Vertex (i = 1, j = 1): lines (Lerp(q0,q1,.5) = (0,150)) -> (Lerp(q3,q2,.5) =
    // (250,150)) and (Lerp(q0,q3,.5) = (150,0)) -> (Lerp(q1,q2,.5) = (100,300)).
    // The first is y = 150; the second runs from (150, 0) to (100, 300), at y = 150 x = 125.
    let mid = decal.verts[3 + 1];
    assert!(
        (mid.x - 125.0).abs() < 1e-3 && (mid.y - 150.0).abs() < 1e-3,
        "{mid:?}"
    );
}
