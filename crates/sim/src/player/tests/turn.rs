//! The food the player starts a cycle with (`LoadInitCondition`, 56526) and the
//! session's `characterStats` (54006).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, ClipNames, Room, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room;
use crate::world::World;
use crate::{EmitterId, Input, RainWorldGame, SoundEvent, Sounds, Start};

/// A 10 x 8 room: walls, a floor at row 1, air above.
fn floor_room() -> Room {
    let mut rows = vec!["##########"];
    rows.extend(["#........#"; 6]);
    rows.push("##########");
    room(&rows)
}

pub(crate) fn story_game(campaign: &'static SlugcatCampaign, story: StoryStart) -> RainWorldGame {
    story_game_in(floor_room(), (4, 1), campaign, story)
}

fn story_game_in(
    room: Room,
    tile: (i32, i32),
    campaign: &'static SlugcatCampaign,
    story: StoryStart,
) -> RainWorldGame {
    let data = WorldData::from_room(&room);
    let rooms = Arc::new(FixedRoom(LoadedRoom {
        room,
        settings: RoomSettings::ANCESTOR.clone(),
    }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let start = Start {
        room: 0,
        tile: Some(tile),
    };
    RainWorldGame::new_story(
        world,
        campaign,
        story,
        start,
        UnityRandom::new(5),
        Sounds::default(),
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
    g.tick_until_ready(&mut ());
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
    g.tick_until_ready(&mut ());
    g.add_food(6);
    assert!(g.player().expect("player").malnourished());
    assert_eq!(g.character_stats().food_to_hibernate, 7);
}

impl RainWorldGame {
    /// One tick; the emitters it kept alive and the ones it stopped.
    fn tick_emitter_events(&mut self) -> (Vec<EmitterId>, Vec<EmitterId>) {
        self.sounds.take_events();
        self.tick(Input::NONE, &mut ());
        let (mut kept, mut stopped) = (Vec::new(), Vec::new());
        for e in self.sounds.take_events() {
            match e {
                SoundEvent::Emitter { id, .. } => kept.push(id),
                SoundEvent::Stop(id) => stopped.push(id),
                _ => {}
            }
        }
        (kept, stopped)
    }
}

/// `Player.Update` lets `slideLoop` go (`alive = false`) when the stun ends and the
/// wanted loop turns to none. The emitter joined the room after the player, so
/// `Room.Update`'s backward walk ran it first that tick; at its next update it finds no
/// upkeep and `Destroy`s itself, and the microphone cuts the sound.
#[test]
fn a_let_go_slide_loop_dies_at_its_next_update() {
    let survivor = &SlugcatCampaign::SURVIVOR;
    let mut g = story_game(survivor, StoryStart::new_game(survivor));
    let text = "Volume: 6\nVolume exponent: 1.8\nUI_Slugcat_Stunned_LOOP : clip\n";
    let names = ClipNames::new(["clip".to_owned()], None);
    g.sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    g.tick_until_ready(&mut ());
    for _ in 0..20 {
        g.tick(Input::NONE, &mut ());
    }
    let p = g.player.as_mut().expect("player");
    p.stun(30, &mut g.rng, &mut g.sounds);
    let mut held = None;
    for _ in 0..60 {
        let (kept, stopped) = g.tick_emitter_events();
        assert_eq!(stopped, vec![]);
        match (held, &kept[..]) {
            (_, [id]) => held = Some(*id),
            (Some(_), []) => break,
            (None, []) => {}
            _ => panic!("{kept:?}"),
        }
    }
    let held = held.expect("the stunned loop played");
    let p = g.player.as_ref().expect("player");
    assert!(!p.is_stunned());
    let e: Vec<_> = p.slide_loop_emitters().collect();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].sound, held);
    assert!(!e[0].alive && !e[0].slated, "let go, not yet destroyed");
    assert_eq!(
        g.tick_emitter_events(),
        (vec![], vec![held]),
        "its next update"
    );
    let p = g.player.as_ref().expect("player");
    assert_eq!(p.slide_loop_emitters().count(), 0);
    assert_eq!(g.tick_emitter_events(), (vec![], vec![]));
}

/// `Creature.Update` runs `SuckedIntoShortCut` inside `Player.Update`, after the slide
/// loop's update kept the emitter alive that tick; `RemoveObject` nulls the player's
/// `room`. The emitter joined after the player, so the next tick's backward walk updates
/// it first and its `chunk.owner.room != room` check destroys it: the tick after entry.
#[test]
fn the_slide_loop_emitter_dies_the_tick_after_the_player_enters_a_pipe() {
    let survivor = &SlugcatCampaign::SURVIVOR;
    let pipe = room(&[
        "############",
        "############",
        "#......ExE.#",
        "############",
    ]);
    let mut g = story_game_in(pipe, (2, 1), survivor, StoryStart::new_game(survivor));
    let text = "Volume: 6\nVolume exponent: 1.8\nSlugcat_Slide_In_Narrow_Corridor_LOOP : clip\n";
    let names = ClipNames::new(["clip".to_owned()], None);
    g.sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    g.tick_until_ready(&mut ());
    for _ in 0..30 {
        g.tick(Input::NONE, &mut ());
    }
    let mut loop_id = None;
    for _ in 0..80 {
        g.player.as_mut().unwrap().set_horizontal_corridor_slide(25);
        g.sounds.take_events();
        g.tick(Input::new(1, 0, false), &mut ());
        let kept = g
            .sounds
            .take_events()
            .into_iter()
            .find(|e| matches!(e, SoundEvent::Emitter { .. }));
        if let Some(SoundEvent::Emitter { id, .. }) = kept {
            loop_id = Some(id);
        }
        if g.player.as_ref().unwrap().in_shortcut() {
            break;
        }
    }
    let loop_id = loop_id.expect("the slide loop played");
    assert!(
        g.player.as_ref().unwrap().in_shortcut(),
        "the tick it went in"
    );
    assert_eq!(g.player.as_ref().unwrap().slide_loop_emitters().count(), 1);
    g.tick(Input::NONE, &mut ());
    let stops: Vec<_> = g
        .sounds
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, SoundEvent::Stop(_)))
        .collect();
    assert_eq!(stops, vec![SoundEvent::Stop(loop_id)], "the tick after");
    assert_eq!(g.player.as_ref().unwrap().slide_loop_emitters().count(), 0);
}
