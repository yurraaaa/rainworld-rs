//! A room's items on its first realization (`LoadFromDataString` 64211, `Room.Loaded`
//! 61149) and their draws, on small test rooms.

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{AbstractObjectType, ActiveMods, Room, RoomSettings, Timeline};
use rw_random::UnityRandom;

use super::World;
use crate::creature::{EntityId, IdIssuer};
use crate::item::AbstractObject;
use crate::tests::{room, room_with_items};

fn world_of(room: Room, settings: RoomSettings, shelter: bool) -> World {
    let mut data = WorldData::from_room(&room);
    if shelter {
        data.rooms[0].shelter_index = 0;
    }
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    World::new(data, rooms, Timeline::White, ActiveMods::NONE, true)
}

fn density(d: f32) -> RoomSettings {
    RoomSettings {
        random_item_density: d,
        ..RoomSettings::ANCESTOR
    }
}

/// The stream state after `n` draws from `seed`.
fn after_draws(seed: i32, n: usize) -> [u32; 4] {
    let mut rng = UnityRandom::new(seed);
    for _ in 0..n {
        rng.next_u32();
    }
    rng.state()
}

/// An object as (kind, ID number, tile, explosive).
type ObjectRow = (AbstractObjectType, i32, (i32, i32), Option<bool>);

fn objects(world: &World) -> Vec<ObjectRow> {
    world
        .room_objects(0)
        .map(|id| world.object(id))
        .map(|o: &AbstractObject| {
            (
                o.kind,
                o.id.number,
                o.pos.tile,
                o.spear().map(|s| s.explosive),
            )
        })
        .collect()
}

#[test]
fn random_items_make_two_draws_per_failed_attempt() {
    // An all-Solid 10 x 8 room: every `RandomTile` lands on Solid. Attempts are
    // `(int)(80 * d^2 / 5)` counted down to 0 inclusive: 1, 5 and 17.
    let solid = ["##########"; 8];
    for (d, draws) in [(0.0, 2), (0.5, 10), (1.0, 34)] {
        let mut world = world_of(room(&solid), density(d), false);
        let mut ids = IdIssuer {
            next_issued_id: 1000,
        };
        let mut rng = UnityRandom::new(3);
        world.activate_room(0, &mut ids, &mut rng).expect("loads");
        assert_eq!(rng.state(), after_draws(3, draws), "density {d}");
        assert_eq!(ids.next_issued_id, 1000);
        assert!(objects(&world).is_empty());
    }
}

#[test]
fn a_random_item_on_a_free_tile_takes_an_id_and_one_or_two_more_draws() {
    // 1 x 2: Air over Solid. `RandomTile` is (0, 0) (Solid: nothing) or (0, 1), whose
    // tiles below clamp to (0, 0); there the item takes an ID and draws spear (< 0.2),
    // and a spear draws explosive (< 0, never, on the Survivor's timeline).
    let (mut rocks, mut spears, mut misses) = (0, 0, 0);
    for seed in 0..60 {
        let mut world = world_of(room(&[".", "#"]), density(0.0), false);
        let mut ids = IdIssuer {
            next_issued_id: 1000,
        };
        let mut rng = UnityRandom::new(seed);
        world.activate_room(0, &mut ids, &mut rng).expect("loads");

        let mut expect = UnityRandom::new(seed);
        assert_eq!(expect.range_i32(0, 1), 0);
        let y = expect.range_i32(0, 2);
        let want = if y == 1 {
            if expect.value() < 0.2 {
                spears += 1;
                expect.value();
                vec![(AbstractObjectType::Spear, 1001, (0, 1), Some(false))]
            } else {
                rocks += 1;
                vec![(AbstractObjectType::Rock, 1001, (0, 1), None)]
            }
        } else {
            misses += 1;
            Vec::new()
        };
        assert_eq!(rng.state(), expect.state(), "seed {seed}");
        assert_eq!(objects(&world), want, "seed {seed}");
        assert!(
            world
                .room_objects(0)
                .all(|id| world.object(id).id.spawner == 0 && world.object(id).pos.node == -1)
        );
    }
    assert!(rocks > 0 && spears > 0 && misses > 0);
}

#[test]
fn file_items_draw_in_file_order_and_skip_the_last_part() {
    // A rock at (1, 1), a spear at (2, 1), an unknown type 5 at (3, 1), and a last part
    // without a trailing `|`, which the loop bound leaves out. Height 4: row 1 from the
    // top is tile row 3. Then one failed random attempt (all Solid): 3 + 2 draws.
    let solid = ["###"; 4];
    for seed in 0..20 {
        let room = room_with_items(&solid, "0,1,1|1,2,1|5,3,1|0,1,4");
        let mut world = world_of(room, density(0.0), false);
        let mut ids = IdIssuer { next_issued_id: 50 };
        let mut rng = UnityRandom::new(seed);
        world.activate_room(0, &mut ids, &mut rng).expect("loads");

        let mut expect = UnityRandom::new(seed);
        let mut next = 50;
        let mut want = Vec::new();
        for (kind, chance, x) in [(0, 0.75, 0), (1, 0.6, 1), (5, 0.75, 2)] {
            if expect.value() < chance {
                next += 1;
                match kind {
                    0 => want.push((AbstractObjectType::Rock, next, (x, 3), None)),
                    1 => want.push((AbstractObjectType::Spear, next, (x, 3), Some(false))),
                    _ => {}
                }
            }
        }
        expect.next_u32();
        expect.next_u32();
        assert_eq!(rng.state(), expect.state(), "seed {seed}");
        assert_eq!(ids.next_issued_id, next, "seed {seed}");
        assert_eq!(objects(&world), want, "seed {seed}");
    }
}

#[test]
fn items_come_only_on_the_first_realization() {
    let mut world = world_of(room(&[".", "#"]), density(1.0), false);
    let mut ids = IdIssuer { next_issued_id: 0 };
    let mut rng = UnityRandom::new(9);
    world.activate_room(0, &mut ids, &mut rng).expect("loads");
    let count = objects(&world).len();
    world.abstractize(0);
    let state = rng.state();
    world.activate_room(0, &mut ids, &mut rng).expect("loads");
    assert_eq!(rng.state(), state);
    assert_eq!(objects(&world).len(), count);
}

#[test]
fn shelters_make_file_items_but_no_random_ones() {
    let room = room_with_items(&[".", "#"], "0,1,1|");
    let mut world = world_of(room, density(1.0), true);
    let mut ids = IdIssuer { next_issued_id: 0 };
    let mut rng = UnityRandom::new(1);
    world.activate_room(0, &mut ids, &mut rng).expect("loads");
    assert_eq!(rng.state(), after_draws(1, 1));
}

#[test]
fn spear_chances_by_timeline() {
    // `SpearSpawnModifier` (242612) and `SpearSpawnExplosiveRandomChance` (242558): the
    // timeline order is Spear, Artificer, Inv, Red, Gourmand, White, Yellow, ...
    assert_eq!(Timeline::White.spear_spawn_modifier(0.2), 0.2);
    assert_eq!(Timeline::Yellow.spear_spawn_modifier(0.2), 0.2);
    assert!((Timeline::Red.spear_spawn_modifier(0.2) - 0.254_61).abs() < 1e-5);
    assert_eq!(Timeline::White.spear_spawn_explosive_random_chance(), 0.0);
    assert_eq!(Timeline::Yellow.spear_spawn_explosive_random_chance(), 0.0);
    assert_eq!(Timeline::Red.spear_spawn_explosive_random_chance(), 0.008);
    assert_eq!(
        Timeline::Artificer.spear_spawn_explosive_random_chance(),
        0.012
    );
}

#[test]
fn move_object_changes_rooms_and_keeps_the_tile_for_an_undefined_one() {
    let mut world = world_of(room(&[".", "#"]), density(0.0), false);
    let id = world.add_object(AbstractObject::new_rock(
        EntityId::new(-1, 7),
        crate::creature::WorldCoordinate::new(0, 0, 1, -1),
    ));
    assert_eq!(world.room_entities(0).len(), 1);
    world.move_object(id, crate::creature::WorldCoordinate::new(0, -1, -1, 2));
    assert_eq!(world.object(id).pos.tile, (0, 1));
    assert_eq!(world.object(id).pos.node, 2);
    world.move_object(id, crate::creature::WorldCoordinate::new(1, -1, -1, 0));
    assert!(world.room_entities(0).is_empty());
    assert_eq!(world.room_entities(1).len(), 1);
}

/// A 4 x 2 room with no Solid tile: the one random-item attempt never finds a place.
fn open_world() -> World {
    world_of(room(&["....", "...."]), density(0.0), false)
}

fn add_rock(world: &mut World, n: i32, tile: (i32, i32)) -> crate::item::ObjectId {
    world.add_object(AbstractObject::new_rock(
        EntityId::new(0, n),
        crate::creature::WorldCoordinate::new(0, tile.0, tile.1, -1),
    ))
}

#[test]
fn shortcuts_ready_realizes_the_room_objects_in_entity_order() {
    // `ShortCutsReady` (61622): `RealizeInRoom` per object, in `entities` order. The
    // rock makes 2 draws (rotation, spin), the spear 4 (rotation, needle, spin's two).
    let mut world = open_world();
    let rock = add_rock(&mut world, 1, (1, 1));
    let spear = world.add_object(AbstractObject::new_spear(
        EntityId::new(0, 2),
        crate::creature::WorldCoordinate::new(0, 2, 1, -1),
        false,
    ));
    let mut ids = IdIssuer { next_issued_id: 0 };
    let mut rng = UnityRandom::new(4);
    world.activate_room(0, &mut ids, &mut rng).expect("loads");
    world.finish_loading();
    world.realize_room_objects(0, &mut rng);
    assert_eq!(rng.state(), after_draws(4, 2 + 2 + 4));
    let (r, s) = (
        world.realized_weapon(rock, 0).expect("rock"),
        world.realized_weapon(spear, 0).expect("spear"),
    );
    assert!(r.update_order < s.update_order);
    assert_eq!(world.room_weapons(0).len(), 2);
    // `AbstractRoom.Abstractize` drops them.
    world.abstractize(0);
    assert!(world.object(rock).realized.is_none() && world.object(spear).realized.is_none());
    assert_eq!(world.object(rock).time_spent_here, 0);
}

#[test]
fn the_collision_pass_pushes_two_items_apart_by_mass() {
    // Two rocks at one spot: distance 0, `DirVec` (0, 1); equal masses share the 10 px
    // overlap, the first moving down 5 and the second up 5 (pos and vel). Their x
    // stays equal, so each gets DegToVec(value * 360) * 0.0001 more velocity (2 draws).
    let mut world = open_world();
    let a = add_rock(&mut world, 1, (1, 1));
    let b = add_rock(&mut world, 2, (1, 1));
    let mut ids = IdIssuer { next_issued_id: 0 };
    let mut rng = UnityRandom::new(8);
    world.activate_room(0, &mut ids, &mut rng).expect("loads");
    world.finish_loading();
    world.realize_room_objects(0, &mut rng);
    let mut expect = rng.clone();
    world.collide_weapons(0, &mut rng);
    let na = crate::Vec2::random_unit(&mut expect) * 0.0001;
    let nb = crate::Vec2::random_unit(&mut expect) * 0.0001;
    assert_eq!(rng.state(), expect.state());
    let (ca, cb) = (
        world.realized_weapon(a, 0).expect("a").chunk().clone(),
        world.realized_weapon(b, 0).expect("b").chunk().clone(),
    );
    assert_eq!(ca.pos, crate::Vec2::new(30.0, 25.0));
    assert_eq!(cb.pos, crate::Vec2::new(30.0, 35.0));
    assert_eq!(ca.vel, crate::Vec2::new(0.0, -5.0) + na);
    assert_eq!(cb.vel, crate::Vec2::new(0.0, 5.0) + nb);
}
