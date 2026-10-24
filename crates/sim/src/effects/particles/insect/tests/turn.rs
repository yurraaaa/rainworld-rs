//! The cosmetic insects in a game: when `NowViewed` rolls them (`RoomCamera.Update`'s
//! `ApplyPositionChange` -> `ChangeRoom`, at the head of the tick after the camera
//! moved) and their places in the room's update list (`Room.Update` walks `updateList`
//! from its end, so the newest object goes first).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{AbstractObjectType, ActiveMods, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::creature::lizard::perception_turn::tests::HALL;
use crate::game::RoomObject;

use crate::tests::room;
use crate::world::World;
use crate::{Input, RainWorldGame, Sounds, Start, Vec2};

/// A Survivor story's first cycle in `HALL` with the `Flies` effect at 5 (19 tries: at 1 the
/// four spots of the camera-move roll all fall on walls), and a copy
/// of the room as global room 2 (index 1 is the off-screen den), no tick run yet.
fn flies_game() -> RainWorldGame {
    let hall = room(&HALL);
    let mut data = WorldData::from_room(&hall);
    let mut second = data.rooms[0].clone();
    second.name = "HALL_B".to_owned();
    second.index = 2;
    data.rooms.push(second);
    let settings = RoomSettings {
        random_item_density: 0.0,
        effects: vec![RoomEffect {
            kind: "Flies".to_owned(),
            amount: 5.0,
        }],
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom {
        room: hall,
        settings,
    }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((3, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
    )
}

impl RainWorldGame {
    /// Room `index`'s insects' positions and last positions.
    fn insect_places(&self, index: i32) -> Vec<(Vec2, Vec2)> {
        self.world
            .realized(index)
            .and_then(|r| r.insects.as_ref())
            .map_or_else(Vec::new, |c| {
                c.insects
                    .iter()
                    .map(|i| (i.base.pos, i.base.last_pos))
                    .collect()
            })
    }

    /// Room 2 realized and loaded, and camera 0 moved there; no tick run.
    fn camera_to_second_room(&mut self) {
        self.activate_room(2).expect("loads");
        self.world.finish_loading();
        self.camera_moves_to(2, 0);
    }
}

/// The insects' first update starts with `lastPos = pos` (`CosmeticSprite.Update`), so
/// after the tick that rolled them their last positions are where they were made.
fn spawned_at(places: &[(Vec2, Vec2)]) -> Vec<Vec2> {
    places.iter().map(|&(_, last)| last).collect()
}

/// The session's first room is shown by the first tick's camera update, before
/// anything else of that tick draws: the tick rolls the same insects as `ChangeRoom`
/// called alone from the state before the tick.
#[test]
fn the_start_room_rolls_its_insects_at_the_head_of_the_first_tick() {
    let mut g = flies_game();
    assert!(g.insect_places(0).is_empty());
    g.tick(Input::NONE, &mut ());
    let ticked = g.insect_places(0);
    assert!(!ticked.is_empty());

    let mut alone = flies_game();
    alone.apply_view_change(&mut ());
    let made: Vec<Vec2> = alone.insect_places(0).iter().map(|&(pos, _)| pos).collect();
    assert_eq!(spawned_at(&ticked), made);
}

/// After the camera moves, nothing changes until the next tick's camera update; that
/// tick first clears the old room's insects (`NoLongerViewed`) and rolls the new room's
/// (`NowViewed`) from the stream as it stood at the head of the tick.
#[test]
fn a_camera_move_rolls_the_new_rooms_insects_at_the_head_of_the_next_tick() {
    let mut g = flies_game();
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g.camera_to_second_room();
    let left = g.insect_places(0).len();
    assert!(left > 0);
    assert!(g.insect_places(2).is_empty());

    let mut alone = flies_game();
    for _ in 0..20 {
        alone.tick(Input::NONE, &mut ());
    }
    alone.camera_to_second_room();
    alone.apply_view_change(&mut ());
    assert!(alone.insect_places(0).is_empty());
    let made: Vec<Vec2> = alone.insect_places(2).iter().map(|&(pos, _)| pos).collect();

    g.tick(Input::NONE, &mut ());
    assert!(g.insect_places(0).is_empty(), "NoLongerViewed cleared them");
    let ticked = g.insect_places(2);
    assert!(!made.is_empty());
    assert!(!ticked.is_empty());
    assert_eq!(spawned_at(&ticked), made);
}

/// Insects join the update list when rolled: a rock realized later updates before
/// them, they update newest first, and everything is in descending list order.
#[test]
fn insects_take_their_places_in_the_update_list_newest_first() {
    let mut g = flies_game();
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    g.dev_spawn_object(AbstractObjectType::Rock, 0, (10, 1), false)
        .expect("spawns");
    g.tick(Input::NONE, &mut ());
    let list = g.room_update_list(0);
    let seqs: Vec<u64> = list.iter().map(|&(seq, _)| seq).collect();
    assert!(seqs.windows(2).all(|w| w[0] > w[1]), "{seqs:?}");
    let first_insect = list
        .iter()
        .position(|(_, o)| matches!(o, RoomObject::Insect(_)))
        .expect("insects");
    let rock = list
        .iter()
        .position(|(_, o)| matches!(o, RoomObject::Weapon(_)))
        .expect("the rock");
    assert!(rock < first_insect, "the newer rock goes first");
    let insects: Vec<u64> = list
        .iter()
        .filter_map(|&(_, o)| match o {
            RoomObject::Insect(order) => Some(order),
            _ => None,
        })
        .collect();
    let mut newest_first = insects.clone();
    newest_first.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(insects, newest_first);
    let made = g
        .world
        .realized(0)
        .and_then(|r| r.insects.as_ref())
        .map_or(0, |c| c.insects.len());
    assert_eq!(insects.len(), made);
}
