//! The player's side of being held (`Player.cs` `Update`'s `dangerGrasp` block and
//! `WALLSTUCK`, `Grabbed`, `Stun`, `CanBeGrabbed`; `Creature.cs` `Update`). Expected
//! values from those members.

use rw_random::UnityRandom;

use super::super::grabbed::CANT_BE_GRABBED_AFTER_SPIT_OUT;
use super::{HEAD, HIPS, Player};
use crate::creature::{CreatureId, Grasp, Shareability};
use crate::item::Entity;
use crate::tests::room;
use crate::{Sounds, Vec2};

/// A green lizard's bite grasp on the player's hips.
fn lizard_grasp() -> Grasp {
    Grasp {
        serial: 7,
        grabber: CreatureId(3),
        grabbed: Entity::Creature(CreatureId(0)),
        grasp_used: 0,
        chunk_grabbed: HIPS,
        shareability: Shareability::CanOnlyShareWithNonExclusive,
        dominance: 0.4,
        pacifying: true,
    }
}

fn held_player() -> Player {
    let mut p = Player::new();
    p.grabbed(lizard_grasp(), true);
    p.set_held(true, true);
    p
}

#[test]
fn a_lizard_grasp_is_dangerous_and_sets_the_raw_flag() {
    // `Player.Grabbed` (base `PhysicalObject.Grabbed`: `GoThroughFloors = true`): a Lizard's grasp is the danger grasp with
    // its time at 0; another grabber's only the first.
    let p = held_player();
    assert_eq!(p.danger_grasp().grasp, Some(lizard_grasp()));
    assert_eq!(p.danger_grasp().time, 0);
    assert!(p.body.chunks[0].raw_go_through_floors());
    let mut q = Player::new();
    q.grabbed(lizard_grasp(), false);
    assert_eq!(q.danger_grasp().grasp, None);
}

#[test]
fn struggles_on_fresh_presses_until_thirty_then_the_hands_drop_and_sixty_is_game_over() {
    let mut p = held_player();
    // Tick 1: a throw press struggles; held on tick 2 it does not.
    let s = p.danger_grasp_update(false, true, false);
    assert!(s.throw_to_get_free && !s.pickup);
    assert_eq!(p.danger_grasp().time, 1);
    let s = p.danger_grasp_update(false, true, true);
    assert!(!s.throw_to_get_free && s.pickup);
    assert!(p.danger_grasp().last_throw_button && p.danger_grasp().pickup_button);
    // Stunned 30 or more: no struggle, but the buttons are still read.
    p.stun = 30;
    let s = p.danger_grasp_update(false, false, false);
    assert_eq!(s, crate::DangerGraspStep::default());
    let s = p.danger_grasp_update(false, true, false);
    assert!(!s.throw_to_get_free);
    p.stun = 29;
    p.danger_grasp_update(false, false, false);
    let s = p.danger_grasp_update(false, true, false);
    assert!(s.throw_to_get_free);
    // Ticks 7..29 nothing; 30 drops the hands; 31..59 nothing (presses ignored); 60 asks
    // for the game over; past it nothing.
    let mut log = Vec::new();
    for _ in 7..=61 {
        log.push(p.danger_grasp_update(false, true, false));
    }
    assert!(log[..23].iter().all(|s| !s.lose_all_grasps && !s.game_over));
    assert!(log[23].lose_all_grasps && !log[23].throw_to_get_free);
    assert!(
        log[24..53]
            .iter()
            .all(|s| *s == crate::DangerGraspStep::default())
    );
    assert!(log[53].game_over);
    assert_eq!(log[54], crate::DangerGraspStep::default());
    assert_eq!(p.danger_grasp().time, 61);
}

#[test]
fn a_dead_player_does_not_struggle() {
    let mut p = held_player();
    p.dead = true;
    let s = p.danger_grasp_update(false, true, true);
    assert!(!s.throw_to_get_free && !s.pickup);
}

#[test]
fn a_discontinued_grasp_is_forgotten() {
    let mut p = held_player();
    p.danger_grasp_update(false, false, false);
    p.danger_grasp_update(true, false, false);
    assert_eq!(p.danger_grasp().grasp, None);
    assert_eq!(p.danger_grasp().time, 0);
}

#[test]
fn a_pacifying_hold_keeps_the_stun_at_ten() {
    // `Creature.Update`: the stun counts down first, then under 10 a pacifying grasp
    // puts it back to 10.
    let mut p = held_player();
    p.creature_update(None, &mut UnityRandom::new(0));
    assert_eq!(p.stun, 10);
    p.stun = 40;
    p.creature_update(None, &mut UnityRandom::new(0));
    assert_eq!(p.stun, 39);
    p.set_held(true, false);
    p.stun = 5;
    p.creature_update(None, &mut UnityRandom::new(0));
    assert_eq!(p.stun, 4);
}

#[test]
fn a_stun_under_a_danger_grasp_keeps_what_the_hands_hold() {
    // `Player.Stun`: past `Random.Range(5, 12)` it un-stands but `LoseAllGrasps` only
    // without a danger grasp; the two ranges are drawn either way.
    let mut rng = UnityRandom::new(3);
    let mut p = held_player();
    p.stun(45, &mut rng, &mut Sounds::default());
    assert!(!p.take_release_grasps());
    assert_eq!(p.stun, 45);
    let mut rng = UnityRandom::new(3);
    let mut q = Player::new();
    q.stun(45, &mut rng, &mut Sounds::default());
    assert!(q.take_release_grasps());
}

#[test]
fn held_near_the_bottom_every_grasp_lets_go() {
    // `Creature.Update`: the head under the out-of-room limit (-500) plus 10 while held.
    let mut p = held_player();
    for c in &mut p.body.chunks {
        c.reset_at(Vec2::new(100.0, -489.5));
    }
    p.creature_update(None, &mut UnityRandom::new(0));
    assert!(!p.take_release_grabbed_by());
    for c in &mut p.body.chunks {
        c.reset_at(Vec2::new(100.0, -490.5));
    }
    p.creature_update(None, &mut UnityRandom::new(0));
    assert!(p.take_release_grabbed_by());
    p.set_held(false, false);
    p.creature_update(None, &mut UnityRandom::new(0));
    assert!(!p.take_release_grabbed_by());
}

#[test]
fn out_of_a_shortcut_it_cannot_be_grabbed_for_thirty_ticks() {
    let mut p = Player::new();
    assert!(p.can_be_grabbed());
    p.set_cant_be_grabbed_after_spit_out();
    assert_eq!(CANT_BE_GRABBED_AFTER_SPIT_OUT, 30);
    for _ in 0..29 {
        p.danger_grasp_update(false, false, false);
        assert!(!p.can_be_grabbed());
    }
    p.danger_grasp_update(false, false, false);
    assert!(p.can_be_grabbed());
}

/// A slab one tile thick (row 2) between two corridors.
const SLAB: [&str; 6] = [
    "#######", "#.....#", "#.....#", "#######", "#.....#", "#######",
];

/// A player pressed against both faces of [`SLAB`] by one physics step, `held` or not,
/// after `WALLSTUCK`.
fn wallstuck(held: bool) -> Player {
    let r = room(&SLAB);
    let mut p = Player::new();
    p.body.chunks[HEAD].reset_at(Vec2::new(70.0, 69.5));
    p.body.chunks[HEAD].vel = Vec2::new(0.0, -2.0);
    p.body.chunks[HIPS].reset_at(Vec2::new(70.0, 31.5));
    p.body.chunks[HIPS].vel = Vec2::new(0.0, 2.0);
    p.update_physics(
        &r,
        None,
        &mut UnityRandom::new(1),
        &mut Sounds::default(),
        &mut (),
    );
    assert_eq!(p.body.chunks[HEAD].contact().y, -1);
    assert_eq!(p.body.chunks[HIPS].contact().y, 1);
    p.set_held(held, false);
    p.unstick_from_wall(&r);
    p
}

#[test]
fn wallstuck_pulls_the_hips_through_to_the_head_unless_held() {
    // Head resting on the slab, hips pressed under it: opposite floor and ceiling
    // contacts, the middle Solid, no line of sight: the hips go 2 px from the head.
    let p = wallstuck(false);
    let head = p.body.chunks[HEAD].pos;
    let hips = p.body.chunks[HIPS].pos;
    assert!(
        (hips.distance(head) - 2.0).abs() < 1e-4,
        "{head:?} {hips:?}"
    );
    assert_eq!(p.body.chunks[HIPS].last_pos, hips);
    let p = wallstuck(true);
    let (head, hips) = (p.body.chunks[HEAD].pos, p.body.chunks[HIPS].pos);
    assert!(hips.distance(head) > 10.0, "{head:?} {hips:?}");
}
