//! `Room.Loaded`'s `RoomBorderPushBack` and its push on the player and what it holds.
//! Expected values from `Room.cs` (`Loaded`) and `RoomBorderPushBack.cs` (`Update`,
//! `PushAtCreature`). The test room has one camera at (0, 0): the left bound is 117, the
//! right bound 1283.

use rw_assets::{RoomEffect, RoomEffectType};

use crate::creature::{GrabRequest, Shareability};
use crate::game::lizard_perception::tests::{HALL, game};
use crate::item::Entity;
use crate::{HEAD, HIPS, RainWorldGame};

impl RainWorldGame {
    fn with_border_push_back(&mut self) -> u64 {
        self.world_mut()
            .realized_mut(0)
            .expect("room")
            .settings
            .effects = vec![RoomEffect {
            kind: RoomEffectType::BorderPushBack.value().to_owned(),
            amount: 0.5,
        }];
        self.add_border_push_backs(0);
        self.world().realized(0).unwrap().border_push_backs[0].update_order
    }

    fn set_push_power(&mut self, power: f32) {
        self.world_mut().realized_mut(0).unwrap().border_push_backs[0].push_power[0] = power;
    }

    fn push_power(&self) -> f32 {
        self.world().realized(0).unwrap().border_push_backs[0].push_power[0]
    }

    fn player_vels(&self) -> Vec<f32> {
        self.player
            .as_ref()
            .unwrap()
            .body
            .chunks
            .iter()
            .map(|c| c.vel.x)
            .collect()
    }
}

/// `Room.Loaded` adds one object per `BorderPushBack` effect, whatever its amount, with
/// one `pushPower` for the one player.
#[test]
fn loaded_makes_one_per_effect() {
    let mut g = game(&HALL, (3, 1));
    let before = g.world().update_seq();
    let order = g.with_border_push_back();
    assert_eq!(order, before + 1);
    let b = &g.world().realized(0).unwrap().border_push_backs[0];
    assert_eq!(b.margin, -117.0);
    assert_eq!((b.leftmost_camera_pos, b.rightmost_camera_pos), (0.0, 0.0));
    assert_eq!(b.push_power, vec![0.0]);
}

/// A slugcat left of `leftmost - margin` (x 117) is pushed by `3 * InverseLerp(0.3, 1,
/// pushPower)` on every chunk, and `pushPower` climbs by `LerpAndTick(p, 1, 0.01,
/// 1/600)`; at full power the push is the full 3.
#[test]
fn a_player_outside_is_pushed_in_by_the_power() {
    let mut g = game(&HALL, (3, 1));
    let order = g.with_border_push_back();
    let x = g.player.as_ref().unwrap().body.chunks[HEAD].pos.x;
    assert!(x < 117.0, "{x}");
    let v0 = g.player_vels();
    // No power yet: the push is 0.
    g.run_border_push_back(0, order);
    assert_eq!(g.player_vels(), v0);
    let want = 0.0f32 + (1.0 - 0.0) * 0.01 + 1.0 / 600.0;
    assert!((g.push_power() - want).abs() < 1e-7);
    // Power 0.65 gives `3 * (0.65 - 0.3) / 0.7` = 1.5.
    g.set_push_power(0.65);
    g.run_border_push_back(0, order);
    let v1 = g.player_vels();
    for (a, b) in v0.iter().zip(&v1) {
        assert!((b - a - 1.5).abs() < 1e-5, "{a} {b}");
    }
    // At full power: 3, and it stays 1.
    g.set_push_power(1.0);
    g.run_border_push_back(0, order);
    for (a, b) in v1.iter().zip(g.player_vels()) {
        assert!((b - a - 3.0).abs() < 1e-5, "{a} {b}");
    }
    assert_eq!(g.push_power(), 1.0);
}

/// Inside the camera span nothing is pushed and the power fades.
#[test]
fn a_player_inside_is_not_pushed() {
    let mut g = game(&HALL, (10, 1));
    let order = g.with_border_push_back();
    let x = g.player.as_ref().unwrap().body.chunks[HEAD].pos.x;
    assert!((117.0..1283.0).contains(&x), "{x}");
    g.set_push_power(0.5);
    let v0 = g.player_vels();
    g.run_border_push_back(0, order);
    assert_eq!(g.player_vels(), v0);
    assert!((g.push_power() - (0.5 * 0.99 - 1.0 / 80.0)).abs() < 1e-6);
}

/// What the slugcat holds is pushed with it (`PushAtCreature`'s grasps loop), and a
/// slugcat held by another creature is not tested at all.
#[test]
fn what_it_holds_is_pushed_and_a_held_player_is_not() {
    let mut g = game(&HALL, (3, 1));
    let l = g.still_lizard((4, 1), 1.0, 1);
    let order = g.with_border_push_back();
    let request = GrabRequest {
        grabber: g.player_id,
        grabbed: Entity::Creature(l),
        grasp_used: 0,
        chunk_grabbed: 0,
        shareability: Shareability::NonExclusive,
        dominance: 0.0,
        override_equally_dominant: true,
        pacifying: false,
    };
    assert!(g.creature_grab(request));
    let held: Vec<f32> = g
        .world
        .lizard_ref(l)
        .body
        .chunks
        .iter()
        .map(|c| c.vel.x)
        .collect();
    let v0 = g.player_vels();
    g.set_push_power(1.0);
    g.run_border_push_back(0, order);
    for (a, b) in v0.iter().zip(g.player_vels()) {
        assert!((b - a - 3.0).abs() < 1e-5);
    }
    let after: Vec<f32> = g
        .world
        .lizard_ref(l)
        .body
        .chunks
        .iter()
        .map(|c| c.vel.x)
        .collect();
    for (a, b) in held.iter().zip(&after) {
        assert!((b - a - 3.0).abs() < 1e-5, "{a} {b}");
    }
    // Held by a lizard (`grabbedBy.Count > 0`): not pushed, and the power fades.
    let l2 = g.still_lizard((8, 1), -1.0, 1);
    let request = GrabRequest {
        grabber: l2,
        grabbed: Entity::Creature(g.player_id),
        grasp_used: 0,
        chunk_grabbed: HIPS,
        shareability: Shareability::CanOnlyShareWithNonExclusive,
        dominance: 0.5,
        override_equally_dominant: true,
        pacifying: true,
    };
    assert!(g.creature_grab(request));
    let v1 = g.player_vels();
    g.set_push_power(1.0);
    g.run_border_push_back(0, order);
    assert_eq!(g.player_vels(), v1);
    assert!(g.push_power() < 1.0);
}
