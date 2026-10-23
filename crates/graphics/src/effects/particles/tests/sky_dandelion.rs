use rw_random::UnityRandom;
use rw_sim::{DandelionStalk, SkyDandelion, Vec2};

use super::{DandelionSprites, StalkSprites};
use crate::core::draw::{Color, DrawItem, Layer, Shader};
use crate::tests::room;

/// A play-layer dandelion is one `Items` sprite in `Lerp(fog, white, 0.5)`; a
/// farther one is in `Foreground` under `CustomDepth` with alpha 0 and scaled
/// `1 / (0.6 * depth + 1)`; one below 0 adds the unscaled red shadow sprite pushed
/// along the light angle by `15 * InverseLerp(0, -0.5, depth)`.
#[test]
fn dandelion_layers_shaders_and_shadow() {
    let fog = Color::rgb(0.2, 0.4, 0.6);
    let angle = Vec2::new(0.5, 0.8);
    let mut d = SkyDandelion::new(Vec2::new(100.0, 100.0), true, &mut UnityRandom::new(1));
    let list = d.draw(0.5, fog, angle);
    let s = list.sprite(0).expect("body");
    assert_eq!((s.layer, s.shader), (Layer::Items, Shader::Basic));
    assert_eq!(s.color, fog.lerp(Color::WHITE, 0.5));
    assert_eq!(list.items.len(), 1);

    d.depth = 5.0;
    let list = d.draw(0.5, fog, angle);
    let s = list.sprite(0).expect("body");
    assert_eq!(
        (s.layer, s.shader),
        (Layer::Foreground, Shader::CustomDepth)
    );
    assert_eq!((s.color.a, s.scale.x), (0.0, 1.0 / 4.0));

    d.depth = -0.25;
    let list = d.draw(0.5, fog, angle);
    let shadow = list.sprite(1).expect("shadow");
    assert_eq!(shadow.layer, Layer::Shadows);
    assert_eq!(shadow.scale, Vec2::new(1.0, 1.0));
    assert_eq!(shadow.pos, Vec2::new(100.0 + 0.5 * 7.5, 100.0 - 0.8 * 7.5));
    assert_eq!(shadow.color, Color::rgb(1.0 / 255.0, 0.0, 0.0));
    d.slated = true;
    assert!(d.draw(0.5, fog, angle).items.is_empty());
}

/// The stalk is one `Midground` long mesh of four vertices a segment, shaded from
/// the root colour to white, whose last vertices sit by the dandelion.
#[test]
fn stalk_mesh_runs_from_root_to_dandelion() {
    let rows = ["..........."; 12];
    let room = room(&rows);
    let mut d = SkyDandelion::new(Vec2::new(110.0, 100.0), true, &mut UnityRandom::new(1));
    d.update_order = 3;
    let mut stalk = DandelionStalk::new(&d, &room, 0, &mut UnityRandom::new(1));
    stalk.dandelion = Some(3);
    let root = Color::rgb(0.1, 0.2, 0.3);
    let list = stalk.draw(0.5, Some(d.pos), root);
    let DrawItem::Mesh(m) = &list.items[0] else {
        panic!("mesh");
    };
    assert_eq!(m.layer, Layer::Midground);
    assert_eq!(m.vertices.len(), stalk.segs * 4);
    assert_eq!(m.vertex_colors[0], root);
    assert_eq!(*m.vertex_colors.last().expect("colours"), Color::WHITE);
    assert!(m.vertices[m.vertices.len() - 1].distance(d.pos) < 5.0);
}
