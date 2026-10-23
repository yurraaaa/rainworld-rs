//! The rainbow's quad. Expected values follow `Rainbow.DrawSprites`.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::RainbowData;
use rw_sim::{Rainbow, Vec2};

use super::RainbowSprites;
use crate::core::draw::{DrawItem, Layer, Shader};

#[test]
fn rainbow_quad_and_colours() {
    let data = RainbowData::parse("-30~40~0~0~1,0.5,0.25,0,0.5,0.15").unwrap();
    let mut r = Rainbow::new(0, Vec2::new(100.0, 200.0), &data, false);
    r.fade = 0.5;
    let list = r.draw(0.0);
    let [DrawItem::ElementMesh(e)] = &list.items[..] else {
        panic!("one element mesh");
    };
    assert_eq!(e.element, "rainbow");
    assert_eq!(
        (e.mesh.shader, e.mesh.layer),
        (Shader::Rainbow, Layer::GrabShaders)
    );
    // pos = (130, 160), rad = 100; corners at eightDirections[1, 3, 5, 7] * rad.
    assert_eq!(
        e.mesh.vertices,
        [
            Vec2::new(30.0, 60.0),
            Vec2::new(230.0, 60.0),
            Vec2::new(230.0, 260.0),
            Vec2::new(30.0, 260.0)
        ]
    );
    assert_eq!(&e.mesh.triangles[..], [[0, 1, 2], [0, 2, 3]]);
    // (fades[4], 0, 0, fade * fades[i]) in 8 bits.
    let alphas = [0.5, 0.25, 0.125, 0.0].map(|a: f32| (a * 255.0).round_ties_even() / 255.0);
    for (c, a) in e.mesh.vertex_colors.iter().zip(alphas) {
        assert_eq!((c.r, c.g, c.b, c.a), (128.0 / 255.0, 0.0, 0.0, a));
    }
    // A camera ghost mode of 0.1 halves it (`InverseLerp(0.2, 0, ghostMode)`).
    let [DrawItem::ElementMesh(g)] = &r.draw(0.1).items[..] else {
        panic!("one element mesh");
    };
    assert_eq!(
        g.mesh.vertex_colors[0].a,
        (0.25_f32 * 255.0).round_ties_even() / 255.0
    );
}
