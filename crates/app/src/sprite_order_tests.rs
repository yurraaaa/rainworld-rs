use rw_sim::creature::CreatureId;
use rw_sim::item::{Entity, ObjectId};

use super::{Drawable, Move, Overlap, Present, SpriteOrder};

/// Drawables and presence records for the tests.
struct Fixture;

impl Fixture {
    fn creature(i: usize) -> Drawable {
        Drawable::Entity(Entity::Creature(CreatureId(i)))
    }

    fn object(i: usize) -> Drawable {
        Drawable::Entity(Entity::Object(ObjectId(i)))
    }

    /// A weapon added at `seq` whose last of `moves` `ChangeOverlap`s was at `at`.
    fn weapon(drawable: Drawable, seq: u64, moves: u32, in_front: bool, at: u64) -> Present {
        Present {
            overlap: Some(Overlap {
                moves,
                in_front,
                seq: at,
            }),
            ..Present::added(drawable, seq)
        }
    }

    fn order(o: &SpriteOrder) -> Vec<Drawable> {
        o.order().collect()
    }
}

/// `ChangeRoom` makes leasers for `drawableObjects` in order: the death fall (alone in
/// `Bloom`, so first), the `AddObject`ed drawables, then the modules `NowViewed` appends from `physicalObjects[0]` (flies,
/// overseers) and `[1]` (lizards, a player without a module yet), then the shortcuts;
/// `ApplyPositionChange` ends with `ApplyPalette`, which puts the full-screen effect on
/// top.
#[test]
fn a_new_room_makes_leasers_in_drawable_objects_order() {
    let (door, rock, spear) = (
        Drawable::ShelterDoor,
        Fixture::object(0),
        Fixture::object(1),
    );
    let (lizard, fly, overseer, player) = (
        Fixture::creature(1),
        Fixture::creature(2),
        Fixture::creature(3),
        Fixture::creature(0),
    );
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
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            door,
            rock,
            spear,
            overseer,
            fly,
            lizard,
            player,
            Drawable::Shortcuts,
            Drawable::FullScreenEffect,
        ]
    );
}

/// `AddObject` in the viewed room puts each new leaser on top, in update-list order.
#[test]
fn objects_added_later_go_on_top_in_add_order() {
    let (a, b, c) = (Fixture::object(0), Fixture::object(1), Fixture::creature(1));
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
    assert_eq!(
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            a,
            Drawable::Shortcuts,
            Drawable::FullScreenEffect,
            b,
            c
        ]
    );
}

/// `ReleaseAllInternallyContainedSprites` re-adds from the last entry: each goes on
/// top; a later `AddObject` goes above them; a move before an `AddObject` in the same
/// tick (by the update-list counter) leaves the new leaser above.
#[test]
fn released_sprites_go_on_top_in_release_order() {
    let (player, lizard, rock, fly) = (
        Fixture::creature(0),
        Fixture::creature(1),
        Fixture::object(0),
        Fixture::creature(2),
    );
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
    assert_eq!(
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            lizard,
            Drawable::Shortcuts,
            Drawable::FullScreenEffect,
            rock,
            player,
            fly
        ]
    );
}

/// The sprites follow the latest `AddObjectToInternalContainer`; a stale entry left in
/// the first holder's list (`ReleaseSpecific`'s skipped neighbour) still releases them
/// from the second holder's jaws when the first lets all go.
#[test]
fn the_latest_holder_wins_over_a_stale_entry() {
    let (a, b, fly) = (
        Fixture::creature(1),
        Fixture::creature(2),
        Fixture::creature(3),
    );
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
    assert_eq!(Fixture::order(&o).last(), Some(&fly));
}

/// `MoveObjectToInternalContainer` passes `null` when the holder has no leaser: the
/// sprites go on top of their defaults. A holder whose leaser goes takes what it holds
/// off the stage.
#[test]
fn a_holder_without_a_leaser_and_a_holder_that_leaves() {
    let (lizard, other, fly) = (
        Fixture::creature(1),
        Fixture::creature(5),
        Fixture::creature(3),
    );
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
    assert_eq!(Fixture::order(&o).last(), Some(&fly));
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
/// `inFrontOfObjects`; a leaser `ChangeRoom` makes starts in `Items`
/// (`AddToContainer(null)`), whatever the weapon's last move.
#[test]
fn change_overlap_re_adds_the_weapon() {
    let (spear, rock) = (Fixture::object(0), Fixture::object(1));
    let mut o = SpriteOrder::default();
    let rock_now = Fixture::weapon(rock, 2, 0, false, 0);
    o.update(
        1,
        &[Fixture::weapon(spear, 1, 3, false, 2), rock_now],
        &[],
        &[],
    );
    assert!(!o.behind(spear), "a new room's leaser is in Items");
    o.update(
        1,
        &[Fixture::weapon(spear, 1, 4, false, 5), rock_now],
        &[],
        &[],
    );
    assert!(o.behind(spear));
    assert_eq!(
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            rock,
            Drawable::Shortcuts,
            Drawable::FullScreenEffect,
            spear
        ]
    );
    o.update(
        1,
        &[Fixture::weapon(spear, 1, 6, false, 5), rock_now],
        &[],
        &[],
    );
    assert!(o.behind(spear));
}

/// A spear back from a shortcut (`AddObject` at 7; `NewRoom` set `inFrontOfObjects` to
/// -1) that the slugcat's update moves behind (`SpearOnBack`, `ChangeOverlap(false)`)
/// in the same tick goes to `Background`, above an object added before the move and
/// under one added after it. One whose last move came before its `AddObject` stays in
/// `Items`.
#[test]
fn change_overlap_in_the_tick_of_the_add_object() {
    let (spear, rock, early, late) = (
        Fixture::object(0),
        Fixture::object(1),
        Fixture::object(2),
        Fixture::object(3),
    );
    let mut o = SpriteOrder::default();
    let rock_now = Fixture::weapon(rock, 2, 0, false, 0);
    o.update(1, &[rock_now], &[], &[]);
    let tick = [
        rock_now,
        Fixture::weapon(spear, 7, 2, false, 8),
        Present::added(early, 8),
        Present::added(late, 9),
    ];
    o.update(1, &tick, &[], &[]);
    assert!(o.behind(spear));
    assert_eq!(
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            rock,
            Drawable::Shortcuts,
            Drawable::FullScreenEffect,
            early,
            spear,
            late
        ]
    );
    let other = Fixture::object(4);
    let mut next = tick.to_vec();
    next.push(Fixture::weapon(other, 12, 5, false, 11));
    o.update(1, &next, &[], &[]);
    assert!(!o.behind(other));
}

/// Every `ApplyPalette` re-adds the full-screen effect on top; leasers made after it
/// go above it.
#[test]
fn apply_palette_raises_the_full_screen_effect() {
    let (a, b) = (Fixture::object(0), Fixture::object(1));
    let mut o = SpriteOrder::default();
    o.update(1, &[Present::added(a, 1)], &[], &[]);
    o.update(1, &[Present::added(a, 1), Present::added(b, 2)], &[], &[]);
    assert_eq!(
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            a,
            Drawable::Shortcuts,
            Drawable::FullScreenEffect,
            b
        ]
    );
    o.raise_full_screen_effect();
    assert_eq!(
        Fixture::order(&o),
        [
            Drawable::WaterLight,
            Drawable::DeathFall,
            a,
            Drawable::Shortcuts,
            b,
            Drawable::FullScreenEffect
        ]
    );
}
