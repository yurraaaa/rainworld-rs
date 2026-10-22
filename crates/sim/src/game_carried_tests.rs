//! A green lizard carrying the held player through a shortcut, and the held body's
//! physics. Expected values from `Creature.cs` (`Update`'s shortcut entry,
//! `SuckedIntoShortCut`, `SpitOutOfShortCut`), `Player.cs` (`SpitOutOfShortCut`, `Die`),
//! `AbstractCreature.cs` (`FollowedByCamera`, `IsEnteringDen`), `BodyChunk.cs`
//! (`HardSetPosition`, `Update`'s room-range clamp) and `PhysicalObject.cs` (`Update`'s
//! `grabbedBy` pruning).

use rw_random::UnityRandom;

use crate::creature::{CreatureId, GrabRequest, Shareability, WorldCoordinate};
use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::Entity;
use crate::{HEAD, HIPS, Input, RainWorldGame, Vec2};

/// Two halves split by a wall, joined by a shortcut under the floor between the
/// entrances at (2, 2) and (21, 2).
const SPLIT: [&str; 9] = [
    "########################",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "#...........#..........#",
    "##E##################E##",
    "##xxxxxxxxxxxxxxxxxxxx##",
    "########################",
];

/// A room with a den under the floor at (4, 2).
const DEN: [&str; 7] = [
    "##############",
    "#............#",
    "#............#",
    "#............#",
    "#............#",
    "####E#########",
    "####D#########",
];

impl RainWorldGame {
    /// Lizard `l` takes the player in its jaws as `Lizard.Bite` does (no damage roll).
    fn jaws_hold_player(&mut self, l: CreatureId) {
        let request = GrabRequest {
            grabber: l,
            grabbed: Entity::Creature(self.player_id),
            grasp_used: 0,
            chunk_grabbed: HIPS,
            shareability: Shareability::CanOnlyShareWithNonExclusive,
            dominance: 0.5,
            override_equally_dominant: true,
            pacifying: true,
        };
        assert!(self.creature_grab(request));
    }
}

#[test]
fn a_lizard_carries_the_held_player_through_a_shortcut() {
    // `Creature.Update` pulls every connected object to the entrance; `SuckedIntoShortCut`
    // puts the player in the lizard's vessel (`inShortcut`, out of the room); at the far
    // entrance the base `SpitOutOfShortCut` spits it out with the lizard, still held.
    let mut g = game(&SPLIT, (4, 3));
    let l = g.still_lizard((7, 3), -1.0, 40);
    g.jaws_hold_player(l);
    assert!(g.followed_by_camera(l));
    g.dev_set_lizard_destination(l, WorldCoordinate::new(0, 17, 3, -1), Some(0.5));
    let mut carried = false;
    for _ in 0..1200 {
        g.tick(Input::NONE, &mut ());
        let p = g.player_ref();
        if p.carried_by() == Some(l) {
            carried = true;
            assert!(p.in_shortcut());
            assert!(g.world.lizard_ref(l).in_shortcut);
            let (v, in_lobby) = g.creature_vessel(l).expect("the lizard's vessel");
            assert!(!in_lobby);
            assert_eq!(g.player_vessel_tile(0), Some(v.tiles().0));
            assert!(!g.player_in_camera_room() || g.player_ref().in_shortcut());
        }
        if carried && g.world.lizard_ref(l).room == Some(0) {
            let p = g.player_ref();
            assert_eq!(p.carried_by(), None);
            assert!(!p.in_shortcut());
            let grasp = g.world.creature(l).grasps[0].expect("still held");
            assert_eq!(grasp.grabbed, Entity::Creature(g.player_id));
            assert_eq!(p.danger_grasp().grasp, Some(grasp));
            // `cantBeGrabbedCounter = 30`, counted down once by the player's update.
            assert_eq!(p.danger_grasp().cant_be_grabbed_counter, 29);
            let head = p.head().pos;
            assert!(head.x > 12.0 * 20.0, "came out on the far side: {head:?}");
            return;
        }
    }
    panic!(
        "carried {carried}, lizard room {:?}",
        g.world.lizard_ref(l).room
    );
}

#[test]
fn a_carried_player_spat_out_draws_once_per_chunk_and_lands_at_the_hole() {
    // The base loop's `HardSetPosition(MiddleOfTile(pos) + RNV())` per chunk (one draw
    // each), then `Player.SpitOutOfShortCut(false)`: head and hips 2.5 px either side of
    // the entrance middle along the hole, moving `2 * hole`, `cantBeGrabbedCounter` 30,
    // `shortcutDelay` 20; `AddObject` puts it last in the update list.
    let mut g = game(&SPLIT, (4, 3));
    let l = g.still_lizard((7, 3), -1.0, 10);
    g.jaws_hold_player(l);
    g.player.as_mut().expect("player").carried_into_shortcut(l);
    let room = g.world.realized(0).expect("room").room.clone();
    let mut expect = g.rng.clone();
    Vec2::random_unit(&mut expect);
    Vec2::random_unit(&mut expect);
    let seq = g.player_seq;
    g.carried_player_spat_out(0, (21, 2), &room, &mut ());
    assert_eq!(g.rng.state(), expect.state());
    assert!(g.player_seq > seq);
    let p = g.player_ref();
    assert_eq!(p.carried_by(), None);
    assert_eq!(p.danger_grasp().cant_be_grabbed_counter, 30);
    assert_eq!(p.shortcut_delay(), 20);
    let (hx, hy) = room.shortcut_hole_direction(21, 2);
    let hole = Vec2::new(hx as f32, hy as f32);
    let middle = Vec2::middle_of_tile((21, 2));
    assert_eq!(p.body.chunks[HEAD].pos, middle + hole * 2.5);
    assert_eq!(p.body.chunks[HIPS].pos, middle - hole * 2.5);
    assert_eq!(p.body.chunks[HIPS].last_last_pos, middle - hole * 2.5);
    assert_eq!(p.body.chunks[HEAD].vel, hole * 2.0);
}

#[test]
fn only_a_creature_carrying_the_player_is_followed_by_the_camera() {
    let mut g = game(&HALL, (4, 1));
    let l = g.still_lizard((7, 1), -1.0, 10);
    assert!(g.followed_by_camera(g.player_id));
    assert!(!g.followed_by_camera(l));
    g.jaws_hold_player(l);
    assert!(g.followed_by_camera(l));
}

#[test]
fn the_player_dragged_into_a_den_dies() {
    // `OpportunityToEnterDen` with prey to stow: `IsEnteringDen` moves what is stuck into
    // the den, kills the held creature (`Die()` of the realized player: `GameOver`), and
    // the lizard waits 200 ticks there.
    let mut g = game(&DEN, (6, 2));
    let l = g.still_lizard((8, 2), -1.0, 10);
    g.jaws_hold_player(l);
    g.player.as_mut().expect("player").carried_into_shortcut(l);
    let mut pos = g.world.creature(l).pos;
    pos.node = 0;
    g.world.creature_mut(l).pos = pos;
    g.world
        .opportunity_to_enter_den(l, &mut UnityRandom::new(1));
    assert!(g.world.creature(l).in_den);
    assert!(g.world.creature(g.player_id).in_den);
    g.player_killed_in_den(0);
    assert!(g.player_ref().is_dead());
    assert!(g.game_over.is_some());
}

#[test]
fn hard_set_position_moves_a_pending_set_pos_too() {
    // A move from outside the chunk's update left `setPos`; `HardSetPosition` sets it
    // to the new position, so the next update does not jump back.
    let mut c = crate::BodyChunk::new(Vec2::new(10.0, 10.0), 5.0, 1.0);
    c.move_from_outside_my_update(true, false, Vec2::new(50.0, 50.0));
    assert_eq!(c.set_pos, Some(Vec2::new(50.0, 50.0)));
    c.hard_set_position(Vec2::new(80.0, 20.0));
    assert_eq!(c.set_pos, Some(Vec2::new(80.0, 20.0)));
    let mut free = crate::BodyChunk::new(Vec2::new(10.0, 10.0), 5.0, 1.0);
    free.hard_set_position(Vec2::new(80.0, 20.0));
    assert_eq!(free.set_pos, None);
}

#[test]
fn a_held_body_is_not_clamped_to_the_room_range() {
    // `BodyChunk.Update` keeps a chunk within `restrictInRoomRange` of the room only
    // while `grabbedBy` is empty.
    let mut g = game(&HALL, (4, 1));
    let l = g.still_lizard((7, 1), -1.0, 10);
    let p = g.player.as_mut().expect("player");
    p.set_held(false, false);
    assert!(!p.body.held);
    g.jaws_hold_player(l);
    let held = !g.player_grabbed_by().is_empty();
    let p = g.player.as_mut().expect("player");
    p.set_held(held, true);
    assert!(p.body.held);
    let room = crate::tests::room(&HALL);
    let far = -2000.0;
    for c in &mut p.body.chunks {
        c.hard_set_position(Vec2::new(far, 30.0));
        c.vel = Vec2::ZERO;
        c.collide_with_terrain = false;
    }
    p.body.update(&room);
    assert!(p.body.chunks[HEAD].pos.x < -1200.0);
    p.set_held(false, false);
    p.body.update(&room);
    assert_eq!(p.body.chunks[HEAD].pos.x, -1200.0);
}

#[test]
fn stale_holds_on_the_player_are_pruned() {
    // `PhysicalObject.Update` drops `grabbedBy` entries whose grabber no longer holds
    // them by that grasp.
    let mut g = game(&HALL, (4, 1));
    let l = g.still_lizard((7, 1), -1.0, 10);
    g.jaws_hold_player(l);
    assert_eq!(g.player_grabbed_by().len(), 1);
    g.world.creature_mut(l).grasps[0] = None;
    g.world.clean_grabbed_by(Entity::Creature(g.player_id));
    assert!(g.player_grabbed_by().is_empty());
}
