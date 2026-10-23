//! A `StarMatrix`'s sprites. Expected values follow `StarMatrix.cs` (`InitiateSprites`,
//! `DrawSprites`).
//!
//! Mirrors: - (port-only: tests)

use rw_random::UnityRandom;
use rw_sim::{StarMatrix, Vec2};

use super::StarMatrixSprites;
use crate::core::draw::{Layer, Shader};
use crate::superstructure::projections::projector::tests::sprites;

/// Sprite indices run on star after star: the star, its lit connections, its rings.
#[test]
fn a_star_matrix_draws_squares_lines_and_rings() {
    let mut rng = UnityRandom::new(11);
    let mut m = StarMatrix::new(0, Vec2::new(500.0, 300.0), 150.0, &mut rng);
    let list = m.draw(0.0);
    let s = sprites(&list.items);
    let lit: usize = m
        .stars
        .iter()
        .map(|st| 1 + st.connections_on.iter().filter(|&&on| on).count() + st.circles)
        .sum();
    assert_eq!(s.len(), lit);
    let star = &m.stars[0];
    // Each star's square sits at its plane position times `rad`, about `pos`.
    let at = m.on_plane_pos(0, 0.0) * m.rad + m.pos;
    let square = s.iter().find(|x| x.index == star.first_sprite).unwrap();
    assert_eq!(square.pos, at);
    assert_eq!(
        (square.scale, square.element),
        (Vec2::new(5.0, 5.0), "pixel")
    );
    assert!(
        s.iter()
            .all(|x| x.layer == Layer::BackgroundShortcuts && x.color.r == 0.0)
    );
    // A star with a connection and a ring.
    let (i, st) = m
        .stars
        .iter()
        .enumerate()
        .find(|(_, st)| !st.connections.is_empty() && st.circles > 0)
        .expect("a connected star with rings");
    let (first_sprite, rings, connections) = (
        st.first_sprite,
        st.connections.len(),
        st.connections.clone(),
    );
    let here = m.on_plane_pos(i, 0.0) * m.rad + m.pos;
    let other = m.on_plane_pos(connections[0], 0.0) * m.rad + m.pos;
    // Switched off, it is not drawn; on, it points at the other star and spans the gap.
    m.stars[i].connections_on[0] = false;
    let off = sprites(&m.draw(0.0).items);
    assert!(off.iter().all(|x| x.index != first_sprite + 1));
    m.stars[i].connections_on[0] = true;
    let on = sprites(&m.draw(0.0).items);
    let line = on.iter().find(|x| x.index == first_sprite + 1).unwrap();
    assert_eq!(line.rotation, (other - here).to_degrees());
    assert_eq!(line.scale, Vec2::new(1.5, here.distance(other)));
    assert_eq!(line.anchor, Vec2::new(0.5, 0.0));
    // Rings: Futile_White scaled (8 + 5j) / 8 of alpha 2 / (8 + 5j) under VectorCircle.
    let ring = on
        .iter()
        .find(|x| x.index == first_sprite + 1 + rings)
        .unwrap();
    assert_eq!(ring.element, "Futile_White");
    assert_eq!(ring.shader, Shader::VectorCircle);
    assert_eq!(ring.scale, Vec2::new(1.0, 1.0));
    assert_eq!(ring.color.a, 0.25);
}
