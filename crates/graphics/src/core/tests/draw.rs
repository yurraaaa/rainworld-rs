//! `MoveObjectToInternalContainer`.

use rw_sim::Vec2;

use crate::{Color, DrawItem, Layer};

fn test_sprite(index: usize, layer: Layer) -> DrawItem {
    DrawItem::Sprite(crate::Sprite {
        index,
        element: "pixel",
        pos: Vec2::ZERO,
        rotation: 0.0,
        scale: Vec2::new(1.0, 1.0),
        anchor: Vec2::new(0.5, 0.5),
        color: Color::BLACK,
        shader: crate::Shader::Basic,
        layer,
    })
}

#[test]
fn an_object_moved_into_a_container_draws_there_in_its_layer() {
    // `MoveObjectToInternalContainer`: the moved sprites become children of the
    // container (its layer), after what it already holds; sprites the object keeps in
    // other containers (the slugcat's `Foreground` ones) stay.
    let mut lizard = crate::DrawList {
        items: (0..4).map(|i| test_sprite(i, Layer::Midground)).collect(),
    };
    let container = crate::InternalContainer {
        at: 2,
        layer: Layer::Midground,
    };
    let mut player = crate::DrawList {
        items: vec![
            test_sprite(10, Layer::Midground),
            test_sprite(11, Layer::Foreground),
            test_sprite(12, Layer::Midground),
        ],
    };
    let after = lizard.take_into_container(container, &mut player, &[Layer::Midground]);
    let order: Vec<usize> = lizard.items.iter().map(DrawItem::index).collect();
    assert_eq!(order, [0, 1, 10, 12, 2, 3]);
    assert_eq!(after.at, 4);
    assert_eq!(player.items.len(), 1);
    assert_eq!(player.items[0].layer(), Layer::Foreground);
    let mut rock = crate::DrawList {
        items: vec![test_sprite(20, Layer::Items)],
    };
    lizard.take_into_container(after, &mut rock, &[Layer::Items]);
    let order: Vec<usize> = lizard.items.iter().map(DrawItem::index).collect();
    assert_eq!(order, [0, 1, 10, 12, 20, 2, 3]);
    assert_eq!(lizard.items[4].layer(), Layer::Midground);
    assert!(rock.items.is_empty());
}
