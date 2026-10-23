//! The projections in one test room: the star matrix `Room.Loaded` makes, the projector and
//! scan lines `Room.NowViewed` makes, their places in the update list and their turns.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, PlacedObject, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use super::super::RoomObject;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::room_projections::ProjectionRef;
use crate::tests::room_with_water;
use crate::world::World;
use crate::{
    Input, PartKind, ProjectorFrame, RainWorldGame, Sounds, Start, SuperStructureProjector,
};

use super::super::lights::tests::WithGraphics;

/// A 20 x 10 room with a floor, the player at (2, 1), after `ticks` ticks.
fn projection_game(placed: &str, effects: &[(&str, f32)], ticks: usize) -> RainWorldGame {
    let mut rows: Vec<String> = vec![".".repeat(20); 10];
    rows[9] = "#".repeat(20);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let room = room_with_water(&refs, -1);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(placed),
        effects: effects
            .iter()
            .map(|&(kind, amount)| RoomEffect {
                kind: kind.to_owned(),
                amount,
            })
            .collect(),
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut WithGraphics,
    );
    for _ in 0..ticks {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    g
}

/// `Room.Loaded` makes the `StarMatrix` of a `ProjectedStars` whose handle (30, 40) makes
/// `Rad` 50, so `rad` 100: `Lerp(100, 500, 0.3) = 220`, `220^2 * pi / 4000` is 38 stars.
#[test]
fn projected_stars_join_the_update_list() {
    let g = projection_game("ProjectedStars><300><100><30~40", &[], 0);
    let r = g.world.realized(0).expect("room");
    let stars = &r.projections.star_matrices;
    assert_eq!(stars.len(), 1);
    assert_eq!((stars[0].rad, stars[0].stars.len()), (100.0, 38));
    assert_eq!(stars[0].pos, crate::Vec2::new(300.0, 100.0));
    let objects = g.room_projection_objects(0);
    assert_eq!(objects.len(), 1);
    assert!(matches!(
        objects[0],
        (_, RoomObject::Projection(ProjectionRef::Stars(_)))
    ));
}

/// `NowViewed` makes the projector when camera 0 comes to the room (the first tick): the
/// parts take places in the update list before it.
#[test]
fn the_projector_is_made_when_the_room_is_first_viewed() {
    let g = projection_game("", &[("SuperStructureProjector", 1.0)], 0);
    assert!(
        g.world
            .realized(0)
            .expect("room")
            .projections
            .projectors
            .is_empty()
    );
    let g = projection_game("", &[("SuperStructureProjector", 1.0)], 1);
    let r = g.world.realized(0).expect("room");
    let p = &r.projections.projectors[0];
    // 200 tiles: no scan lines; 260 glyphs (a few may have gone inactive, new ones came).
    assert!(p.scan_lines.is_empty());
    assert!(p.parts.len() >= 260);
    assert!(p.cursors.len() == 1);
    let objects = g.room_projection_objects(0);
    assert_eq!(objects.len(), p.parts.len() + 1);
}

/// The constructor makes its parts before the projector takes its own place; parts added
/// by later updates are newer than the projector.
#[test]
fn projector_parts_are_older_or_newer_than_it() {
    let mut rng = UnityRandom::new(3);
    let mut seq = 10_u64;
    let mut p = SuperStructureProjector::new((20, 10), 1.0, &mut seq, &mut rng);
    assert!(!p.parts.is_empty());
    assert!(p.parts.iter().all(|q| q.update_order < p.update_order));
    assert_eq!(p.update_order, seq);
    let made = p.parts.len();
    let frame = ProjectorFrame {
        electric_power: 1.0,
        being_viewed: true,
        camera_position: Some(0),
    };
    for _ in 0..400 {
        p.update(&frame, &mut seq, &mut rng);
    }
    assert!(p.parts.iter().any(|q| q.update_order > p.update_order));
    assert!(p.parts.iter().all(|q| q.update_order != p.update_order));
    assert!(made > 0);
}

#[test]
fn the_projector_runs_and_refills() {
    let mut g = projection_game("", &[("SuperStructureProjector", 1.0)], 1);
    let before = g.world.realized(0).unwrap().projections.projectors[0].clone();
    for _ in 0..40 {
        g.tick(Input::NONE, &mut WithGraphics);
    }
    let p = &g.world.realized(0).unwrap().projections.projectors[0];
    assert!(!p.slated);
    assert_ne!(p.grid_pos, before.grid_pos);
    // Singles count down their life and the cursor moves on.
    assert!(p.parts.iter().any(|q| matches!(
        q.kind,
        PartKind::Single(s) if s.glyph.counter > 0
    )));
    assert_eq!(p.cursors.len(), 1);
}

#[test]
fn scan_lines_are_made_and_counted() {
    let g = projection_game("", &[("ProjectedScanLines", 1.0)], 1);
    let r = g.world.realized(0).expect("room");
    // 20 x 10 tiles: Lerp(200, 20, 0.5) / 2000 * 10 = 0.55 -> none wanted.
    assert!(r.projections.scan_lines[0].lines.is_empty());
    assert_eq!(r.projections.scan_lines[0].ideal_scan_line_number(), 0);
}

#[test]
fn the_projector_destroys_itself_when_the_room_is_not_viewed() {
    let mut g = projection_game("", &[("SuperStructureProjector", 1.0)], 1);
    g.camera_room = 1;
    g.tick(Input::NONE, &mut WithGraphics);
    let r = g.world.realized(0).expect("room");
    assert!(r.projections.projectors.is_empty());
    assert!(g.room_projection_objects(0).is_empty());
}

#[test]
fn the_grid_offset_comes_from_a_drawn_single_glyph() {
    let g = projection_game("", &[("SuperStructureProjector", 1.0)], 2);
    let p = &g.world.realized(0).unwrap().projections.projectors[0];
    let want = p.grid_offset(0.25);
    if p.visible {
        assert_eq!(g.projection_grid_offset(0.25), Some(want));
    } else {
        assert_eq!(g.projection_grid_offset(0.25), None);
    }
}
