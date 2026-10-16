//! `Player.Die` → `RainWorldGame.GameOver` (56840) and `karmaFlowerGrowPos` (86975),
//! `PlaceKarmaFlowerOnDeathSpot` (54041), `PlaceKarmaFlower` (84018).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, Room, RoomSettings};
use rw_random::UnityRandom;

use super::GameOver;
use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::{room, room_with_water};
use crate::world::World;
use crate::{HEAD, HIPS, Input, RainWorldGame, Sounds, Start, Vec2, WorldCoordinate};

/// A 10 x 8 room: walls, a floor at row 1, air above.
fn floor_room() -> Room {
    let mut rows = vec!["##########"];
    rows.extend(["#........#"; 6]);
    rows.push("##########");
    room(&rows)
}

fn game(room: Room, campaign: &'static SlugcatCampaign, story: StoryStart) -> RainWorldGame {
    let data = WorldData::from_room(&room);
    let rooms = Arc::new(FixedRoom(LoadedRoom {
        room,
        settings: RoomSettings::ANCESTOR.clone(),
    }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let start = Start {
        room: 0,
        tile: Some((4, 1)),
    };
    let rng = UnityRandom::new(5);
    RainWorldGame::new_story(
        world,
        campaign,
        story,
        start,
        rng,
        Sounds::default(),
        &mut (),
    )
}

fn settled(campaign: &'static SlugcatCampaign) -> RainWorldGame {
    let mut g = game(floor_room(), campaign, StoryStart::new_game(campaign));
    for _ in 0..40 {
        g.tick(Input::NONE, &mut ());
    }
    g
}

#[test]
fn place_karma_flower_by_campaign() {
    // `KarmaIsReinforced || Yellow`, the Hunter only out of cycles (19; 24 with the extra
    // cycles).
    let survivor = &SlugcatCampaign::SURVIVOR;
    assert!(!settled(survivor).place_karma_flower());
    assert!(settled(&SlugcatCampaign::MONK).place_karma_flower());
    let reinforced = StoryStart {
        karma_reinforced: true,
        ..StoryStart::new_game(survivor)
    };
    assert!(game(floor_room(), survivor, reinforced).place_karma_flower());
    let hunter = &SlugcatCampaign::HUNTER;
    for (cycle, extra, out) in [(18, false, false), (19, false, true), (23, true, false)] {
        let story = StoryStart {
            cycle_number: cycle,
            red_extra_cycles: extra,
            ..StoryStart::new_game(hunter)
        };
        let g = game(floor_room(), hunter, story);
        assert_eq!(g.red_is_out_of_cycles(), out, "cycle {cycle}");
        assert_eq!(g.place_karma_flower(), out, "cycle {cycle}");
    }
}

#[test]
fn the_grow_pos_follows_the_hips_on_the_floor() {
    // Only a player that places flowers tracks it: `room.GetWorldCoordinate(hips)`,
    // node -1.
    assert_eq!(
        settled(&SlugcatCampaign::SURVIVOR).karma_flower_grow_pos(),
        None
    );
    let g = settled(&SlugcatCampaign::MONK);
    let (x, y) = g.player().unwrap().body.chunks[HIPS].pos.tile();
    assert_eq!(y, 1, "standing on the floor");
    assert_eq!(
        g.karma_flower_grow_pos(),
        Some(WorldCoordinate::new(0, x, y, -1))
    );
}

#[test]
fn no_grow_pos_in_deep_water() {
    // `!room.GetTile(hips).DeepWater`: the water line at row 5 covers the floor.
    let mut rows = vec!["##########"];
    rows.extend(["#........#"; 6]);
    rows.push("##########");
    let monk = &SlugcatCampaign::MONK;
    let mut g = game(room_with_water(&rows, 5), monk, StoryStart::new_game(monk));
    for _ in 0..40 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(g.karma_flower_grow_pos(), None);
}

#[test]
fn the_first_death_calls_game_over_once() {
    let mut g = settled(&SlugcatCampaign::MONK);
    let grow = g.karma_flower_grow_pos();
    assert!(grow.is_some());
    let head = g.player().unwrap().body.chunks[HEAD].pos;
    // `GameOver`, `PlaceKarmaFlowerOnDeathSpot` and `InitGameOverMode` draw nothing;
    // `Die`'s sound is drawn with the player's next sounds.
    let before = g.rng.state();
    g.kill_player();
    assert_eq!(g.rng.state(), before);
    assert_eq!(
        g.take_game_over(),
        Some(GameOver {
            food_in_stomach: 0,
            death_room: 0,
            death_pos: head,
            karma_flower: Some(grow),
        })
    );
    // Dying again (`if (!dead)`) asks nothing more; the dead body keeps simulating.
    g.kill_player();
    for _ in 0..10 {
        g.tick(Input::new(1, 0, true), &mut ());
    }
    assert_eq!(g.take_game_over(), None);
    assert_eq!(g.karma_flower_grow_pos(), grow);
    assert!(g.player().unwrap().is_dead());
}

#[test]
fn a_falling_death_restricts_the_position_and_skips_the_flower() {
    // `RestrictInRect(mainBodyChunk.pos, RoomRect.Grow(50))`; the Survivor places no
    // flower (`karma_flower` None). A fall out of the room's bottom kills there.
    let survivor = &SlugcatCampaign::SURVIVOR;
    let mut g = game(
        room(&[".........."; 8]),
        survivor,
        StoryStart::new_game(survivor),
    );
    let mut over = None;
    for _ in 0..400 {
        g.tick(Input::NONE, &mut ());
        over = over.or(g.take_game_over());
    }
    let over = over.expect("game over");
    assert_eq!(over.karma_flower, None);
    assert_eq!(over.death_pos.y, -50.0);
    assert!((0.0..=200.0).contains(&over.death_pos.x));
    assert!(g.player().is_none(), "destroyed");
}

#[test]
fn a_death_inside_the_room_keeps_its_position() {
    // A death inside the room keeps its position.
    let mut g = settled(&SlugcatCampaign::SURVIVOR);
    let head = g.player().unwrap().body.chunks[HEAD].pos;
    g.kill_player();
    assert_eq!(g.take_game_over().map(|o| o.death_pos), Some(head));
    assert_ne!(head, Vec2::ZERO);
}

#[test]
fn the_hunter_out_of_cycles_goes_to_reds_game_over() {
    // `if (GetStorySession.RedIsOutOfCycles) { GoToRedsGameOver(); return; }`: no HUD
    // game over; the app fades the music (`FadeOutAllSongs(20)`).
    let hunter = &SlugcatCampaign::HUNTER;
    let story = StoryStart {
        cycle_number: 19,
        ..StoryStart::new_game(hunter)
    };
    let mut g = game(floor_room(), hunter, story);
    for _ in 0..40 {
        g.tick(Input::NONE, &mut ());
    }
    g.kill_player();
    assert_eq!(g.take_game_over(), None);
    assert!(g.take_reds_game_over());
    assert!(!g.take_reds_game_over());
}
