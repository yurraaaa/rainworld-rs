//! `ShortcutHelper`: wrong-hole classification, neighbour links, the player's push, the
//! round-robin and an `ObjectPusher`'s turn. Expected values from `ShortcutHelper.cs`
//! (constructor, `LookForNeighbors`, `Update`, `ObjectPusher.Update`).

use rw_assets::AbstractObjectType;

use super::*;
use crate::SegmentRef;
use crate::creature::lizard::perception_turn::tests::game;
use crate::item::Entity;
use crate::tests::room;
use crate::water::world::LayerEntry;
use crate::{FloatExt, Input, SoundId, Sounds, Vec2};

/// A dead-end shortcut (a pipe stub) at (5, 1) in a floor, hole up.
const STUB: [&str; 7] = [
    "############",
    "#..........#",
    "#..........#",
    "#..........#",
    "#..........#",
    "#####E######",
    "#####x######",
];

/// A dead end at x 1 and a normal pipe from x 3 to x 5, all holes up.
const THREE: [&str; 5] = ["#######", "#.....#", "#E#E#E#", "#x#xxx#", "#######"];

fn is_wrong(kind: ShortcutType, node: Option<usize>, single: bool, gate: bool) -> bool {
    ShortcutHelper::wrong_hole(kind, node, &[-1, 7], single, gate)
}

fn built_wrong(r: &rw_assets::Room, connections: &[i32], single: bool, gate: bool) -> bool {
    ShortcutHelper::new(r, connections, single, gate, 0).pushers[0].wrong_hole
}

fn segment(layer: i32, seq: u64, segment: u32) -> LayerEntry {
    let at = SegmentRef {
        room: 0,
        stem: 0,
        segment,
    };
    (layer, seq, ChunkOwner::Segment(at))
}

fn picked(objects: &[LayerEntry], check_obj: i32) -> Vec<u32> {
    ShortcutHelper::checked_objects(objects, check_obj)
        .into_iter()
        .map(|o| match o {
            ChunkOwner::Segment(s) => s.segment,
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn only_normal_tubes_and_live_room_exits_are_right_holes() {
    use ShortcutType as T;
    assert!(!is_wrong(T::Normal, None, false, false));
    assert!(is_wrong(T::CreatureHole, Some(0), false, false));
    assert!(is_wrong(T::NpcTransportation, None, false, false));
    assert!(is_wrong(T::RegionTransportation, Some(0), false, false));
    assert!(is_wrong(T::DeadEnd, None, false, false));
    // A room exit leading nowhere (`connections[destNode] == -1`) outside gates and
    // single-room worlds; a connected one, or a node beyond the array, is fine.
    assert!(is_wrong(T::RoomExit, Some(0), false, false));
    assert!(!is_wrong(T::RoomExit, Some(1), false, false));
    assert!(!is_wrong(T::RoomExit, Some(5), false, false));
    assert!(!is_wrong(T::RoomExit, Some(0), true, false));
    assert!(!is_wrong(T::RoomExit, Some(0), false, true));
}

#[test]
fn a_room_exit_to_nowhere_is_a_wrong_hole_in_the_built_helper() {
    let r = room(&["#####", "#...#", "#E###", "#X###"]);
    assert_eq!(r.shortcut_map().shortcuts()[0].kind, ShortcutType::RoomExit);
    assert!(built_wrong(&r, &[-1], false, false));
    assert!(!built_wrong(&r, &[3], false, false));
    assert!(!built_wrong(&r, &[-1], true, false));
    assert!(!built_wrong(&r, &[-1], false, true));
}

#[test]
fn a_wrong_hole_lists_right_holes_two_tiles_away_facing_the_same_way() {
    let r = room(&THREE);
    let kinds: Vec<_> = r
        .shortcut_map()
        .shortcuts()
        .iter()
        .map(|s| s.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            ShortcutType::DeadEnd,
            ShortcutType::Normal,
            ShortcutType::Normal
        ]
    );
    let h = ShortcutHelper::new(&r, &[], true, false, 0);
    let wrong: Vec<bool> = h.pushers.iter().map(|p| p.wrong_hole).collect();
    assert_eq!(wrong, [true, false, false]);
    assert!(h.pushers.iter().all(|p| p.shortcut_dir == (0, 1)));
    assert_eq!(h.pushers[0].push_pos, Vec2::middle_of_tile((1, 3)));
    // x 3 is 2 tiles from x 1; x 5 is 4.
    assert_eq!(h.pushers[0].valid_neighbors, [1]);
    assert!(h.pushers[1].valid_neighbors.is_empty());
    assert!(h.pushers[2].valid_neighbors.is_empty());
}

#[test]
fn the_round_robin_takes_check_obj_mod_count_in_each_layer() {
    // Layer 0 has three (coral segments among them, D41), layer 1 two, layer 3 one.
    let objects = [
        segment(0, 1, 0),
        segment(0, 2, 1),
        segment(0, 3, 2),
        segment(1, 4, 10),
        segment(1, 5, 11),
        segment(3, 6, 20),
    ];
    assert_eq!(picked(&objects, 1), [1, 11, 20]);
    assert_eq!(picked(&objects, 2), [2, 10, 20]);
    assert_eq!(picked(&objects, 5), [2, 11, 20]);
    assert_eq!(picked(&objects, 6), [0, 10, 20]);
    assert!(ShortcutHelper::checked_objects(&[], 3).is_empty());
}

#[test]
fn the_helper_joins_the_room_when_its_shortcuts_are_ready() {
    let g = game(&STUB, (3, 2));
    let h = g
        .world()
        .realized(0)
        .and_then(|r| r.shortcut_helper.as_ref());
    let h = h.expect("added by ShortCutsReady");
    assert_eq!(h.pushers.len(), 1);
    assert!(h.pushers[0].wrong_hole);
    assert!(!h.pushers[0].floor);
    assert_eq!(h.pushers[0].shortcut_pos, (5, 1));
}

/// Chunks inside a wrong hole's box are moved out to `pushPos + (10 + rad)` along the
/// hole direction, their velocity taking the same step.
#[test]
fn the_player_over_a_wrong_hole_is_shoved_out_of_its_box() {
    let mut g = game(&STUB, (3, 2));
    let mut pushers = g
        .world()
        .realized(0)
        .unwrap()
        .shortcut_helper
        .as_ref()
        .unwrap()
        .pushers
        .clone();
    let push_pos = pushers[0].push_pos;
    assert_eq!(push_pos, Vec2::new(110.0, 50.0));
    let p = g.player.as_mut().unwrap();
    p.body.chunks[0].pos = Vec2::new(110.0, 45.0);
    p.body.chunks[1].pos = Vec2::new(110.0, 60.0);
    for c in &mut p.body.chunks {
        c.vel = Vec2::ZERO;
    }
    p.pushed_by_shortcuts(&mut pushers, 1.0);
    let (head, hips) = (&p.body.chunks[0], &p.body.chunks[1]);
    assert_eq!(head.pos, Vec2::new(110.0, 50.0 + 10.0 + head.rad));
    assert_eq!(head.vel, Vec2::new(0.0, 50.0 + 10.0 + head.rad - 45.0));
    assert_eq!(hips.pos, Vec2::new(110.0, 50.0 + 10.0 + hips.rad));
    assert_eq!(hips.vel, Vec2::new(0.0, 50.0 + 10.0 + hips.rad - 60.0));
}

/// `PushPlayerTowardsValidNeighbor`: swell 2, then (hole facing up) y velocity
/// `+ dir.y * fac`, x velocity `* (0.5 - 0.3 fac)` plus the clamped step to the
/// neighbour's x, which the position takes too.
#[test]
fn the_push_toward_a_valid_neighbour_steers_the_player_over_it() {
    let r = room(&THREE);
    let mut g = game(&STUB, (3, 2));
    let mut pushers = ShortcutHelper::new(&r, &[], true, false, 0).pushers;
    let target = pushers[1].push_pos;
    let p = g.player.as_mut().unwrap();
    p.body.chunks[0].pos = target + Vec2::new(20.0, 10.0);
    p.body.chunks[1].pos = target + Vec2::new(20.0, 30.0);
    for c in &mut p.body.chunks {
        c.vel = Vec2::new(2.0, 0.0);
    }
    p.push_towards_valid_neighbor(&mut pushers, 0, 0.5, 1.0);
    assert_eq!(pushers[0].swell, 2.0);
    for (c, from) in p
        .body
        .chunks
        .iter()
        .zip([Vec2::new(20.0, 10.0), Vec2::new(20.0, 30.0)])
    {
        assert_eq!(c.vel, Vec2::new(2.0 * (0.5 - 0.3 * 0.5) + (-1.2), 0.5));
        assert_eq!(c.pos, target + from + Vec2::new(-1.2, 0.0));
    }
    // Out of reach (40 px): nothing changes.
    let p = g.player.as_mut().unwrap();
    p.body.chunks[0].pos = target + Vec2::new(60.0, 0.0);
    p.body.chunks[1].pos = target + Vec2::new(60.0, 20.0);
    let before = p.body.chunks.clone();
    p.push_towards_valid_neighbor(&mut pushers, 0, 0.5, 1.0);
    assert_eq!(p.body.chunks, before);
}

#[test]
fn a_swell_eases_toward_one_while_pushed_and_back_to_zero() {
    let r = room(&STUB);
    let mut pusher = ShortcutPusher::new(&r, true, (5, 1), (0, 1));
    pusher.swell = 2.0;
    pusher.settle_swell();
    assert_eq!(pusher.swell, 2.0.lerp_and_tick(0.0, 0.02, 1.0 / 30.0));
    pusher.swell_up = true;
    pusher.settle_swell();
    assert!(!pusher.swell_up);
    assert_eq!(
        pusher.swell,
        (2.0.lerp_and_tick(0.0, 0.02, 1.0 / 30.0)).lerp_and_tick(1.0, 0.02, 1.0 / 30.0)
    );
}

fn rock_in_the_stub() -> (crate::RainWorldGame, crate::item::ObjectId) {
    let mut g = game(&STUB, (3, 2));
    let id = g
        .dev_spawn_object(AbstractObjectType::Rock, 0, (5, 1), false)
        .expect("spawns");
    (g, id)
}

#[test]
fn a_rock_in_an_entrance_is_found_by_the_round_robin_and_pushed_out() {
    let (mut g, rock) = rock_in_the_stub();
    let owner = ChunkOwner::Entity(Entity::Object(rock));
    assert!(g.pops_out_of_dead_shortcuts(owner));
    for _ in 0..3 {
        g.tick(Input::NONE, &mut ());
    }
    let h = g
        .world()
        .realized(0)
        .unwrap()
        .shortcut_helper
        .as_ref()
        .unwrap();
    let pusher = h.push_out_objects.iter().find(|p| p.owner == owner);
    let pusher = pusher.expect("a pusher for the rock");
    assert!(pusher.wait_counter > 0, "it was shoved at once");
}

/// One `ObjectPusher.Update` on a rock at the entrance's middle with `counter` 0: the
/// side choice, the angle jitter, then the sound's draws; the chunk goes half-way to
/// `pushPos` and 10 px on, its y velocity zeroed and the push added.
#[test]
fn an_object_pusher_draws_twice_and_shoves_the_rock() {
    let (mut g, rock) = rock_in_the_stub();
    let owner = ChunkOwner::Entity(Entity::Object(rock));
    let r = g.world().realized(0).unwrap();
    let room = std::sync::Arc::clone(&r.room);
    let pushers = r.shortcut_helper.as_ref().unwrap().pushers.clone();
    let c = g.world().surface_chunks(Entity::Object(rock)).unwrap()[0].clone();
    assert!(
        c.pos.dist_less(Vec2::middle_of_tile((5, 1)), 1.0),
        "{:?}",
        c.pos
    );
    let mut e = g.rng.clone();
    let v1 = e.value();
    let v2 = e.value();
    Sounds::default().play_at(
        SoundId::MediumNpcTickAlongInShortcut,
        c.pos,
        1.0,
        1.0,
        &mut e,
    );
    let mut side = if v1 < 0.5 { -1.0 } else { 1.0 };
    // Air on that side above the rock, so the side flips.
    assert!(
        !room
            .tile_at((c.pos + Vec2::new(-20.0 * side, 20.0)).to_array())
            .is_solid()
    );
    side = -side;
    let counter = 1.0_f32;
    let aim = 0.0 + 30.0 * v2 * side * counter.lerp_map(20.0, 120.0, 0.3, 1.0);
    let speed = counter.lerp_map_pow(0.0, 180.0, 2.5, 15.0, 0.8);

    let mut op = ObjectPusher::new(owner);
    g.update_object_pusher(0, &room, &pushers, &mut op);

    assert_eq!(g.rng, e, "side, jitter, then the sound");
    assert_eq!(
        (op.counter, op.wait_counter, op.has_been_out_counter),
        (1, 40, 0)
    );
    let after = &g.world().surface_chunks(Entity::Object(rock)).unwrap()[0];
    assert_eq!(
        after.pos,
        c.pos.lerp(pushers[0].push_pos, 0.5) + Vec2::new(0.0, 10.0)
    );
    assert_eq!(
        after.vel,
        Vec2::new(c.vel.x, 0.0) + Vec2::from_degrees(aim) * speed
    );
    // The next 40 ticks only count down: no draws.
    let before = g.rng.clone();
    for k in 0..40 {
        g.update_object_pusher(0, &room, &pushers, &mut op);
        assert_eq!(op.wait_counter, 39 - k);
    }
    assert_eq!(g.rng, before);
}

#[test]
fn a_given_up_pusher_does_nothing() {
    let (mut g, rock) = rock_in_the_stub();
    let room = std::sync::Arc::clone(&g.world().realized(0).unwrap().room);
    let pushers = g
        .world()
        .realized(0)
        .unwrap()
        .shortcut_helper
        .as_ref()
        .unwrap()
        .pushers
        .clone();
    let mut op = ObjectPusher::new(ChunkOwner::Entity(Entity::Object(rock)));
    op.given_up = true;
    let before = g.rng.clone();
    g.update_object_pusher(0, &room, &pushers, &mut op);
    assert_eq!((op.counter, g.rng.clone()), (0, before));
}

fn waiting_rock() -> (
    crate::RainWorldGame,
    ChunkOwner,
    std::sync::Arc<rw_assets::Room>,
    Vec<ShortcutPusher>,
) {
    let (g, rock) = rock_in_the_stub();
    let r = g.world().realized(0).unwrap();
    let room = std::sync::Arc::clone(&r.room);
    let pushers = r.shortcut_helper.as_ref().unwrap().pushers.clone();
    (g, ChunkOwner::Entity(Entity::Object(rock)), room, pushers)
}

/// `waitCounter > 0` skips only the push loop: the sidestep block still runs (here the
/// object is in its recorded shaft column, so it is nudged toward `searchTowardsSide` by
/// `LerpMap(counter, 20, 180, 0, 0.1)` with `counter` 100 + 1), with no draws.
#[test]
fn the_sidestep_runs_while_the_pusher_waits() {
    let (mut g, owner, room, pushers) = waiting_rock();
    let before = g.electric_chunk(owner, 0).unwrap();
    let mut op = ObjectPusher::new(owner);
    op.wait_counter = 5;
    op.counter = 100;
    op.search_towards_side = 1;
    op.search_towards_side_x_tile = before.pos.tile().0;
    let rng = g.rng.clone();
    g.update_object_pusher(0, &room, &pushers, &mut op);
    let after = g.electric_chunk(owner, 0).unwrap();
    let push = 101.0_f32.lerp_map(20.0, 180.0, 0.0, 0.1);
    assert_eq!(after.vel.x, before.vel.x + push);
    assert_eq!((op.wait_counter, op.has_been_out_counter), (4, 0));
    assert_eq!(g.rng, rng);
}

/// The room check runs while waiting too: an object that left the room is slated.
#[test]
fn an_object_in_another_room_is_slated_while_it_waits() {
    let (mut g, owner, room, pushers) = waiting_rock();
    let mut op = ObjectPusher::new(owner);
    op.wait_counter = 10;
    let rng = g.rng.clone();
    g.update_object_pusher(1, &room, &pushers, &mut op);
    assert!(op.slated_for_deletion);
    assert_eq!(op.wait_counter, 9);
    assert_eq!(g.rng, rng);
}
