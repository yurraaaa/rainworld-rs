use rw_sim::creature::CreatureId;
use rw_sim::item::{Entity, ObjectId};

use super::{Drawable, Move, Present, SpriteOrder};

fn creature(i: usize) -> Drawable {
    Drawable::Entity(Entity::Creature(CreatureId(i)))
}

fn object(i: usize) -> Drawable {
    Drawable::Entity(Entity::Object(ObjectId(i)))
}

fn weapon(drawable: Drawable, seq: u64, moves: u32, in_front: bool) -> Present {
    Present {
        overlap: Some((moves, in_front)),
        ..Present::added(drawable, seq)
    }
}

fn order(o: &SpriteOrder) -> Vec<Drawable> {
    o.order().collect()
}

/// `ChangeRoom` makes leasers for `drawableObjects` in order: the `AddObject`ed
/// drawables, then the modules `NowViewed` appends from `physicalObjects[0]` (flies,
/// overseers) and `[1]` (lizards, a player without a module yet), then the shortcuts.
#[test]
fn a_new_room_makes_leasers_in_drawable_objects_order() {
    let (door, rock, spear) = (Drawable::ShelterDoor, object(0), object(1));
    let (lizard, fly, overseer, player) = (creature(1), creature(2), creature(3), creature(0));
    let mut o = SpriteOrder::default();
    o.update(
        4,
        &[
            Present::module(lizard, 2, 1),
            Present::module(player, 9, 1),
            Present::added(spear, 6),
            Present::module(fly, 8, 0),
            Present::added(rock, 5),
            Present::module(overseer, 3, 0),
            Present::added(door, 1),
        ],
        &[],
        &[],
    );
    assert_eq!(
        order(&o),
        [
            door,
            rock,
            spear,
            overseer,
            fly,
            lizard,
            player,
            Drawable::Shortcuts
        ]
    );
}

/// `AddObject` in the viewed room puts each new leaser on top, in update-list order.
#[test]
fn objects_added_later_go_on_top_in_add_order() {
    let (a, b, c) = (object(0), object(1), creature(1));
    let mut o = SpriteOrder::default();
    o.update(1, &[Present::added(a, 1)], &[], &[]);
    o.update(
        1,
        &[
            Present::added(a, 1),
            Present::module(c, 8, 1),
            Present::added(b, 7),
        ],
        &[],
        &[],
    );
    assert_eq!(order(&o), [a, Drawable::Shortcuts, b, c]);
}

/// `ReleaseAllInternallyContainedSprites` re-adds from the last entry: each goes on
/// top; a later `AddObject` goes above them; a move before an `AddObject` in the same
/// tick (by the update-list counter) leaves the new leaser above.
#[test]
fn released_sprites_go_on_top_in_release_order() {
    let (player, lizard, rock, fly) = (creature(0), creature(1), object(0), creature(2));
    let mut o = SpriteOrder::default();
    let room = [
        Present::added(player, 1),
        Present::module(lizard, 2, 1),
        Present::added(rock, 3),
    ];
    o.update(1, &room, &[], &[]);
    let hold = [
        (
            3,
            Move::IntoJaws {
                obj: player,
                holder: lizard,
            },
        ),
        (
            3,
            Move::IntoJaws {
                obj: rock,
                holder: lizard,
            },
        ),
    ];
    o.update(1, &room, &hold, &[]);
    assert_eq!(o.held_by(lizard), [player, rock]);
    let mut later = room.to_vec();
    later.push(Present::added(fly, 4));
    o.update(
        1,
        &later,
        &[(3, Move::ToDefault(rock)), (3, Move::ToDefault(player))],
        &[],
    );
    assert!(o.held_by(lizard).is_empty());
    assert_eq!(order(&o), [lizard, Drawable::Shortcuts, rock, player, fly]);
}

/// The sprites follow the latest `AddObjectToInternalContainer`; a stale entry left in
/// the first holder's list (`ReleaseSpecific`'s skipped neighbour) still releases them
/// from the second holder's jaws when the first lets all go.
#[test]
fn the_latest_holder_wins_over_a_stale_entry() {
    let (a, b, fly) = (creature(1), creature(2), creature(3));
    let room = [
        Present::module(a, 1, 1),
        Present::module(b, 2, 1),
        Present::module(fly, 3, 0),
    ];
    let mut o = SpriteOrder::default();
    o.update(1, &room, &[], &[]);
    o.update(
        1,
        &room,
        &[
            (
                3,
                Move::IntoJaws {
                    obj: fly,
                    holder: a,
                },
            ),
            (
                3,
                Move::IntoJaws {
                    obj: fly,
                    holder: b,
                },
            ),
        ],
        &[],
    );
    assert!(o.held_by(a).is_empty());
    assert_eq!(o.held_by(b), [fly]);
    o.update(1, &room, &[(3, Move::ToDefault(fly))], &[]);
    assert!(o.held_by(b).is_empty());
    assert_eq!(order(&o).last(), Some(&fly));
}

/// `MoveObjectToInternalContainer` passes `null` when the holder has no leaser: the
/// sprites go on top of their defaults. A holder whose leaser goes takes what it holds
/// off the stage.
#[test]
fn a_holder_without_a_leaser_and_a_holder_that_leaves() {
    let (lizard, other, fly) = (creature(1), creature(5), creature(3));
    let room = [Present::module(lizard, 1, 1), Present::module(fly, 2, 0)];
    let mut o = SpriteOrder::default();
    o.update(1, &room, &[], &[]);
    o.update(
        1,
        &room,
        &[(
            2,
            Move::IntoJaws {
                obj: fly,
                holder: other,
            },
        )],
        &[],
    );
    assert!(o.shown(fly));
    assert_eq!(order(&o).last(), Some(&fly));
    o.update(
        1,
        &room,
        &[(
            2,
            Move::IntoJaws {
                obj: fly,
                holder: lizard,
            },
        )],
        &[],
    );
    assert!(o.shown(fly));
    o.update(1, &[Present::module(fly, 2, 0)], &[], &[]);
    assert!(!o.shown(fly));
    o.update(
        1,
        &[Present::module(fly, 2, 0)],
        &[],
        &[Move::ToDefault(fly)],
    );
    assert!(o.shown(fly));
}

/// `Weapon.ChangeOverlap` re-adds on top of `Background` or `Items` when it changes
/// `inFrontOfObjects`; a new leaser starts in `Items` (`AddToContainer(null)`).
#[test]
fn change_overlap_re_adds_the_weapon() {
    let (spear, rock) = (object(0), object(1));
    let mut o = SpriteOrder::default();
    o.update(
        1,
        &[weapon(spear, 1, 3, false), weapon(rock, 2, 0, false)],
        &[],
        &[],
    );
    assert!(!o.behind(spear), "a new leaser is in Items");
    o.update(
        1,
        &[weapon(spear, 1, 4, false), weapon(rock, 2, 0, false)],
        &[],
        &[],
    );
    assert!(o.behind(spear));
    assert_eq!(order(&o), [rock, Drawable::Shortcuts, spear]);
    o.update(
        1,
        &[weapon(spear, 1, 6, false), weapon(rock, 2, 0, false)],
        &[],
        &[],
    );
    assert!(o.behind(spear));
}
