use rw_sim::Vec2;

use super::QuadGrid;

fn square(side: f32) -> [Vec2; 4] {
    [
        Vec2::new(0.0, 0.0),
        Vec2::new(0.0, side),
        Vec2::new(side, side),
        Vec2::new(side, 0.0),
    ]
}

/// `Mathf.RoundToInt(longest / 250)` rounds half to even and clamps to 1..20; the
/// SpotLight divides by 150.
#[test]
fn ideal_div_rounds_and_clamps() {
    assert_eq!(QuadGrid::ideal_div(&square(100.0), 250.0), 1);
    assert_eq!(QuadGrid::ideal_div(&square(125.0), 250.0), 1);
    assert_eq!(QuadGrid::ideal_div(&square(375.0), 250.0), 2);
    assert_eq!(QuadGrid::ideal_div(&square(625.0), 250.0), 2);
    assert_eq!(QuadGrid::ideal_div(&square(700.0), 250.0), 3);
    assert_eq!(QuadGrid::ideal_div(&square(100_000.0), 250.0), 20);
    assert_eq!(QuadGrid::ideal_div(&square(300.0), 150.0), 2);
}

/// The corners are the quad's corners, the interior crossings of a square are the
/// lerped points; UVs are `(i, j) / div` at index `j * (div + 1) + i`.
#[test]
fn square_grid_vertices_and_uvs() {
    let g = QuadGrid::new(&square(500.0), 250.0);
    assert_eq!(g.div, 2);
    assert_eq!(g.vertices.len(), 9);
    // index j * 3 + i: i along quad[0]-quad[3] (x), j along quad[0]-quad[1] (y).
    assert_eq!(g.vertices[0], Vec2::new(0.0, 0.0));
    assert_eq!(g.vertices[2], Vec2::new(500.0, 0.0));
    assert_eq!(g.vertices[3 + 1], Vec2::new(250.0, 250.0));
    assert_eq!(g.vertices[6], Vec2::new(0.0, 500.0));
    assert_eq!(g.vertices[8], Vec2::new(500.0, 500.0));
    assert_eq!(g.uvs[5], Vec2::new(1.0, 0.5));
    assert_eq!(g.uvs[6], Vec2::new(0.0, 1.0));
}

/// `MakeGridMesh`: cell (i, j) holds triangles (a, right, top) and (right, top, c)
/// at `(j * div + i) * 2`.
#[test]
fn triangles_are_the_games() {
    let t = QuadGrid::triangles_of(2);
    assert_eq!(t.len(), 8);
    assert_eq!(t[0], [0, 1, 3]);
    assert_eq!(t[1], [1, 3, 4]);
    // Cell (i = 1, j = 1): a = 4, right = 5, top = 7, c = 8, at index 6.
    assert_eq!(t[6], [4, 5, 7]);
    assert_eq!(t[7], [5, 7, 8]);
}

/// A non-parallelogram quad: the crossings stay on the quad's edges at the border.
#[test]
fn skewed_quad_borders_follow_its_edges() {
    let quad = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 300.0),
        Vec2::new(200.0, 400.0),
        Vec2::new(300.0, 20.0),
    ];
    let g = QuadGrid::new(&quad, 250.0);
    assert_eq!(g.div, 2);
    let mid = quad[0].lerp(quad[1], 0.5);
    assert!(g.vertices[3].distance(mid) < 1e-3, "{:?}", g.vertices[3]);
}
