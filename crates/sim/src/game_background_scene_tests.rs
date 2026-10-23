//! `Room.Loaded` making a `BackgroundScene`, its first turn adding the elements to the
//! update list far to near, and the `DistantLightning`s' turns in it with their shared
//! draws. Expected values follow `BackgroundScene.cs` (`Update`), `AboveCloudsView.cs`
//! and `Room.cs` (`Update` walks the list from its end).
//!
//! Mirrors: - (port-only: tests)

use rw_assets::{RoomEffect, RoomEffectType};
use rw_random::UnityRandom;

use super::RoomObject;
use crate::game::lizard_perception::tests::{HALL, game};
use crate::{ElementKind, Input, RainWorldGame, Vec2};

impl RainWorldGame {
    fn with_scene_effect(kind: RoomEffectType) -> Self {
        let mut g = game(&HALL, (3, 1));
        g.world_mut()
            .realized_mut(0)
            .expect("room")
            .settings
            .effects = vec![RoomEffect {
            kind: kind.value().to_owned(),
            amount: 1.0,
        }];
        g
    }
}

#[test]
fn the_scene_joins_the_update_list_and_its_first_turn_adds_the_elements() {
    let mut g = RainWorldGame::with_scene_effect(RoomEffectType::AboveCloudsView);
    let before = g.world().update_seq();
    g.add_room_effect_particles(0);
    let scene = &g.world().realized(0).unwrap().background_scenes[0];
    assert_eq!(scene.update_order, before + 1);
    assert!(!scene.elements_added);
    assert!(scene.elements.iter().all(|e| e.update_order == 0));
    // Only the scene is in the update list until it has had its turn.
    assert_eq!(
        g.room_background_scene_objects(0),
        [(before + 1, RoomObject::BackgroundScene(before + 1))]
    );
    let seq = g.world().update_seq();
    g.run_background_scene(0, before + 1);
    let scene = &g.world().realized(0).unwrap().background_scenes[0];
    assert!(scene.elements_added);
    assert_eq!(scene.convergence_point, Vec2::new(683.0, 512.0));
    // Each element took the next place, in `OrderByDescending(depth)` order.
    let order = scene.add_order();
    for (k, &i) in order.iter().enumerate() {
        assert_eq!(scene.elements[i].update_order, seq + 1 + k as u64);
    }
    // A second call (the next tick) changes nothing.
    let after = g.world().update_seq();
    g.run_background_scene(0, before + 1);
    assert_eq!(g.world().update_seq(), after);
}

#[test]
fn the_lightnings_update_nearest_first_with_five_draws_on_their_first_turn() {
    let mut g = RainWorldGame::with_scene_effect(RoomEffectType::AboveCloudsView);
    g.add_room_effect_particles(0);
    let order = g.world().realized(0).unwrap().background_scenes[0].update_order;
    g.run_background_scene(0, order);
    // `Room.Update` walks the list from its end: the highest place (the nearest, by
    // depth less `minusDepthForLayering`) first.
    let mut list = g.room_background_scene_objects(0);
    list.sort_by_key(|&(seq, _)| std::cmp::Reverse(seq));
    let lightnings: Vec<u64> = list
        .iter()
        .filter_map(|&(_, o)| match o {
            RoomObject::SceneElement(seq) => Some(seq),
            _ => None,
        })
        .collect();
    assert_eq!(lightnings.len(), 4);
    let depths: Vec<f32> = lightnings
        .iter()
        .map(|&seq| {
            let scene = &g.world().realized(0).unwrap().background_scenes[0];
            let e = scene
                .elements
                .iter()
                .find(|e| e.update_order == seq)
                .unwrap();
            e.depth
        })
        .collect();
    assert!(depths.windows(2).all(|w| w[0] <= w[1]), "{depths:?}");
    let before = g.rng().clone();
    for &seq in &lightnings {
        g.run_scene_element(0, seq);
    }
    // 2 (randomLevelChange) + 3 (Reset) each.
    let mut probe: UnityRandom = before;
    for _ in 0..4 * 5 {
        probe.next_u32();
    }
    assert_eq!(g.rng().state(), probe.state());
    // The first turn restored each depth.
    let scene = &g.world().realized(0).unwrap().background_scenes[0];
    for e in &scene.elements {
        if let ElementKind::DistantLightning(l) = &e.kind {
            assert!(l.restored_depth);
        }
    }
    g.tick(Input::NONE, &mut ());
}

#[test]
fn the_roof_top_view_makes_four_smoke_draws_when_loaded() {
    let mut g = RainWorldGame::with_scene_effect(RoomEffectType::RoofTopView);
    let before = g.rng().clone();
    g.add_room_effect_particles(0);
    let mut probe = before;
    for _ in 0..4 {
        probe.next_u32();
    }
    assert_eq!(g.rng().state(), probe.state());
    assert_eq!(
        g.world().realized(0).unwrap().background_scenes[0]
            .elements
            .len(),
        30
    );
}

#[test]
fn the_elements_are_drawn_in_the_order_they_were_added() {
    let mut g = RainWorldGame::with_scene_effect(RoomEffectType::AboveCloudsView);
    g.add_room_effect_particles(0);
    let order = g.world().realized(0).unwrap().background_scenes[0].update_order;
    assert!(
        g.world().realized(0).unwrap().background_scenes[0]
            .added_order()
            .is_empty()
    );
    g.run_background_scene(0, order);
    let scene = &g.world().realized(0).unwrap().background_scenes[0];
    let added = scene.added_order();
    assert_eq!(added.len(), scene.elements.len());
    let lights: Vec<&str> = added
        .iter()
        .filter_map(|&i| match &scene.elements[i].kind {
            ElementKind::DistantLightning(l) => Some(l.asset_name),
            _ => None,
        })
        .collect();
    assert_eq!(
        lights,
        ["AtC_Light3", "AtC_Light1", "AtC_Light2", "AtC_Light1"]
    );
}
