//! Nested jaw containers: `LizardGraphics.AddToContainer` adds `containers[0]` between
//! the head sprites `SpriteHeadStart + 1` and `+ 2`, and a held lizard's sprites, that
//! container included, move into its holder's container.

use rw_graphics::{Color, DrawItem, DrawList, InternalContainer, Layer, Shader, Sprite};
use rw_sim::Vec2;
use rw_sim::creature::CreatureId;
use rw_sim::item::Entity;

use super::{JawHolder, Jaws};
use crate::sprite_order::Drawable;

/// A lizard-like list: sprites `base..base + 5`, the jaw container after the second
/// (lower jaw `+0`, `+1`, then the container, then `+2..+4`).
struct Lizard;

impl Lizard {
    fn list(base: usize) -> DrawList {
        DrawList {
            items: (base..base + 5)
                .map(|index| {
                    DrawItem::Sprite(Sprite {
                        index,
                        element: "pixel",
                        pos: Vec2::ZERO,
                        rotation: 0.0,
                        scale: Vec2::new(1.0, 1.0),
                        anchor: Vec2::new(0.5, 0.5),
                        color: Color::BLACK,
                        shader: Shader::Basic,
                        layer: Layer::Midground,
                    })
                })
                .collect(),
        }
    }

    fn holder(held: Vec<Drawable>) -> Option<JawHolder> {
        Some(JawHolder {
            container: InternalContainer {
                at: 2,
                layer: Layer::Midground,
            },
            held,
        })
    }

    fn order(list: &DrawList) -> Vec<usize> {
        list.items.iter().map(DrawItem::index).collect()
    }
}

impl Lizard {
    fn id(n: usize) -> Drawable {
        Drawable::Entity(Entity::Creature(CreatureId(n)))
    }
}

#[test]
fn a_held_lizards_hold_nests_inside_it_inside_its_holder() {
    // A (0..5) holds B (10..15), B holds C (20..25); A comes first in the list.
    let (a, b, c) = (Lizard::id(1), Lizard::id(2), Lizard::id(3));
    let mut objects = vec![
        (a, Lizard::list(0)),
        (b, Lizard::list(10)),
        (c, Lizard::list(20)),
    ];
    let holders = vec![Lizard::holder(vec![b]), Lizard::holder(vec![c]), None];
    Jaws::new(holders, Lizard::id(0)).contain(&mut objects);
    assert_eq!(
        Lizard::order(&objects[0].1),
        [0, 1, 10, 11, 20, 21, 22, 23, 24, 12, 13, 14, 2, 3, 4]
    );
    assert!(objects[1].1.items.is_empty());
    assert!(objects[2].1.items.is_empty());
}

#[test]
fn a_held_player_nests_inside_its_holder_inside_the_next() {
    // A holds B, B holds the player (Midground 30, 32; Foreground 31 stays).
    let (player_id, a, b) = (Lizard::id(0), Lizard::id(1), Lizard::id(2));
    let mut player = Lizard::list(30);
    player.items.truncate(3);
    player.items[1].set_layer(Layer::Foreground);
    let mut objects = vec![
        (player_id, player),
        (a, Lizard::list(0)),
        (b, Lizard::list(10)),
    ];
    let holders = vec![
        None,
        Lizard::holder(vec![b]),
        Lizard::holder(vec![player_id]),
    ];
    Jaws::new(holders, player_id).contain(&mut objects);
    assert_eq!(
        Lizard::order(&objects[1].1),
        [0, 1, 10, 11, 30, 32, 12, 13, 14, 2, 3, 4]
    );
    assert_eq!(Lizard::order(&objects[0].1), [31]);
}

#[test]
fn a_hold_cycle_draws_nested_in_the_first_of_it() {
    // A holds B, B holds A (D20): A draws with B inside, nothing lost, no panic.
    let (a, b) = (Lizard::id(1), Lizard::id(2));
    let mut objects = vec![(a, Lizard::list(0)), (b, Lizard::list(10))];
    let holders = vec![Lizard::holder(vec![b]), Lizard::holder(vec![a])];
    Jaws::new(holders, Lizard::id(0)).contain(&mut objects);
    assert_eq!(
        Lizard::order(&objects[0].1),
        [0, 1, 10, 11, 12, 13, 14, 2, 3, 4]
    );
    assert!(objects[1].1.items.is_empty());
}
