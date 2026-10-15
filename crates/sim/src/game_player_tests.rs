//! The food the player starts a cycle with (`LoadInitCondition`, 56526) and the
//! session's `characterStats` (54006).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room;
use crate::world::World;
use crate::{RainWorldGame, Sounds, Start};

/// A 10 x 8 room: walls, a floor at row 1, air above.
fn floor_room() -> Room {
    let mut rows = vec!["##########"];
    rows.extend(["#........#"; 6]);
    rows.push("##########");
    room(&rows)
}

pub(crate) fn story_game(campaign: &'static SlugcatCampaign, story: StoryStart) -> RainWorldGame {
    let room = floor_room();
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
    RainWorldGame::new_story(
        world,
        campaign,
        story,
        start,
        UnityRandom::new(5),
        Sounds::default(),
        &mut (),
    )
}

#[test]
fn saved_food_fills_the_stomach_only_above_zero() {
    // `while (food > 0) foodInStomach++`: a malnourished `Win` leaves food 1 - 4 = -3.
    let survivor = &SlugcatCampaign::SURVIVOR;
    for (saved, stomach) in [(-3, 0), (0, 0), (3, 3)] {
        let story = StoryStart {
            food: saved,
            malnourished: true,
            ..StoryStart::new_game(survivor)
        };
        assert_eq!(story_game(survivor, story).food_in_stomach(), stomach);
    }
}

#[test]
fn character_stats_follow_the_saved_malnourishment() {
    // `new SlugcatStats(name, saveState.malnourished)`: foodToHibernate = maxFood.
    let hunter = &SlugcatCampaign::HUNTER;
    let fed = story_game(hunter, StoryStart::new_game(hunter));
    assert_eq!(fed.character_stats().food_to_hibernate, 6);
    let starving = StoryStart {
        malnourished: true,
        ..StoryStart::new_game(hunter)
    };
    let stats = story_game(hunter, starving).character_stats();
    assert_eq!((stats.max_food, stats.food_to_hibernate), (9, 9));
    assert!(stats.malnourished);
}

#[test]
fn a_full_stomach_cures_malnourishment() {
    // `AddFood`: at `maxFood` a malnourished slugcat calls `SetMalnourished(false)`;
    // `saveState.malnourished` stays until the cycle ends.
    let survivor = &SlugcatCampaign::SURVIVOR;
    let story = StoryStart {
        malnourished: true,
        ..StoryStart::new_game(survivor)
    };
    let mut g = story_game(survivor, story);
    for _ in 0..2 {
        g.tick(crate::Input::NONE, &mut ());
    }
    let p = g.player().expect("player");
    assert!(p.malnourished());
    assert!((p.total_mass() - 0.63).abs() < 1e-6);
    g.set_food_in_stomach(6);
    g.add_food(1);
    assert_eq!(g.food_in_stomach(), 7);
    let p = g.player().expect("player");
    assert!(!p.malnourished());
    assert!((p.total_mass() - 0.7).abs() < 1e-6);
    assert_eq!(p.stats().runspeed_fac, 1.0);
    assert_eq!(g.character_stats().food_to_hibernate, 4);
    assert!(g.story().malnourished);
}

#[test]
fn short_of_a_full_stomach_stays_malnourished() {
    let survivor = &SlugcatCampaign::SURVIVOR;
    let story = StoryStart {
        malnourished: true,
        ..StoryStart::new_game(survivor)
    };
    let mut g = story_game(survivor, story);
    g.tick(crate::Input::NONE, &mut ());
    g.add_food(6);
    assert!(g.player().expect("player").malnourished());
    assert_eq!(g.character_stats().food_to_hibernate, 7);
}
