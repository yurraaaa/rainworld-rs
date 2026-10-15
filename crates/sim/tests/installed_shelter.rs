//! The shelter door and hibernation in the installed game's SU_S01 (read-only), as the
//! Survivor. Set `RW_DIR` to the game install directory; skipped if absent.
//!
//! Expected values from the decompiled `ShelterDoor` and `Player`: the door is shut while
//! `rainCycle.timer < initialWait` (80) and opens over `openUpTicks` (350); a slugcat
//! that wakes there lies curled at `sleepCounter` 99 until an input; one that came in
//! through the shortcut with `foodToHibernate` (4) pips, standing still more than 6
//! tiles from the entrance, closes the door after 21 still ticks, and the door shuts
//! after 320 increments of `closeSpeed` (1/320) with `Win(false)`.

use std::path::PathBuf;
use std::sync::Arc;

use rw_assets::world::{
    GameRooms, LineFilter, Regions, RoomSource, StoryConditions, WorldData, WorldRequest,
};
use rw_assets::{ActiveMods, StreamingAssets, tile_centre};
use rw_random::UnityRandom;
use rw_sim::campaign::SlugcatCampaign;
use rw_sim::shelter::{SessionEnd, ShelterDoor};
use rw_sim::world::World;
use rw_sim::{Input, RainWorldGame, Sounds, Start, Vec2};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Installed(StreamingAssets);

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(Self(StreamingAssets::new(sa)))
        } else {
            eprintln!("skipping: no world under {}", sa.display());
            None
        }
    }

    /// A Survivor story in `room`'s region with the player in `room`.
    fn game(&self, room: &str) -> RainWorldGame {
        let campaign = &SlugcatCampaign::SURVIVOR;
        let filter = LineFilter {
            timeline: Some(campaign.timeline),
            mods: ActiveMods::NONE,
            conditions: StoryConditions::new_story(),
        };
        let regions = Regions::load(&self.0).expect("regions");
        let request = WorldRequest::for_first_room(&self.0, room, filter);
        let data = WorldData::load(&self.0, &regions, &request).expect("world loads");
        let first = data.room_named(room).expect("room in world").index;
        let rooms = Arc::new(GameRooms {
            assets: self.0.clone(),
            region: Some(request.name.to_lowercase()),
            timeline: campaign.timeline,
        });
        // A shelter start puts the slugcat at the door's `playerSpawnPos`.
        let tile = data.room_named(room).filter(|r| r.shelter()).map(|r| {
            let loaded = rooms.load_room(&r.name).expect("shelter loads");
            ShelterDoor::player_spawn_pos(&loaded.room, &loaded.settings, false, ActiveMods::NONE)
        });
        let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
        let start = Start { room: first, tile };
        RainWorldGame::new(
            world,
            campaign,
            start,
            UnityRandom::new(1),
            Sounds::default(),
            &mut (),
        )
    }
}

fn door_fac(game: &RainWorldGame, room: i32) -> f32 {
    game.shelter_door(room).expect("door").closed_fac
}

#[test]
fn the_door_opens_with_the_cycle_and_the_slugcat_wakes_curled_up() {
    let Some(installed) = Installed::open() else {
        return;
    };
    let mut game = installed.game("SU_S01");
    let shelter = game.world().room_index("SU_S01").expect("shelter");
    let timer = game.world().rain_cycle.timer;
    assert!(timer < 80, "a new cycle starts at timer {timer}");
    assert_eq!(door_fac(&game, shelter), 1.0);
    let player = game.player().expect("player");
    assert!(player.still_in_start_shelter());
    assert_eq!(player.sleep_counter(), 100);
    game.tick(Input::default(), &mut ());
    // `sleepCounter` 100 lays the body down and waits at 99.
    assert_eq!(game.player().expect("player").sleep_counter(), 99);
    while game.world().rain_cycle.timer < 255 {
        game.tick(Input::default(), &mut ());
    }
    // InverseLerp(430, 80, 255) = 0.5.
    assert!((door_fac(&game, shelter) - 0.5).abs() < 1e-6);
    let player = game.player().expect("player");
    assert_eq!(player.sleep_counter(), 99);
    assert_eq!(player.sleep_curl_up(), 1.0);
    while game.world().rain_cycle.timer < 431 {
        game.tick(Input::default(), &mut ());
    }
    assert_eq!(door_fac(&game, shelter), 0.0);
    // An input starts the 17-tick uncurl, standing up at 0.
    game.tick(Input::new(1, 0, false), &mut ());
    assert_eq!(game.player().expect("player").sleep_counter(), 17);
    for _ in 0..17 {
        game.tick(Input::default(), &mut ());
    }
    let player = game.player().expect("player");
    assert_eq!(player.sleep_counter(), 0);
    assert!(!player.sleeping());
    // Never left the shelter: fed or not, it cannot win.
    assert!(player.still_in_start_shelter());
}

#[test]
fn a_fed_slugcat_coming_in_hibernates_and_wins() {
    let Some(installed) = Installed::open() else {
        return;
    };
    let mut game = installed.game("SU_A22");
    for _ in 0..450 {
        game.tick(Input::default(), &mut ());
    }
    let world = game.world();
    let src = world.room_index("SU_A22").expect("SU_A22");
    let shelter = world.room_index("SU_S01").expect("SU_S01");
    let node = world.abstract_room(src).expect("data").exit_index(shelter);
    let room = &world.realized(src).expect("realized").room;
    let entrance = room
        .shortcut_map()
        .leading_to_node(usize::try_from(node).expect("node"))
        .expect("the exit's shortcut")
        .start;
    let hole = room.shortcut_hole_direction(entrance.0, entrance.1);
    let input = Input::new(-hole.0 as i8, -hole.1 as i8, false);
    game.set_player(
        Vec2::from(tile_centre(entrance.0, entrance.1)),
        Vec2::ZERO,
        &mut (),
    );
    for _ in 0..600 {
        game.tick(input, &mut ());
        let p = game.player().expect("player");
        if game.player_pos().room == shelter && !p.in_shortcut() {
            break;
        }
    }
    assert_eq!(game.player_pos().room, shelter);
    let p = game.player().expect("player");
    assert!(!p.still_in_start_shelter());
    assert_eq!(p.force_sleep_counter(), 0);
    let door = game.shelter_door(shelter).expect("door");
    assert_eq!(door.closed_fac, 0.0);
    assert!(!door.is_closing());

    // To the spawn position, far from the entrance, with the hibernation food.
    let spawn = door.player_spawn_pos;
    game.set_food_in_stomach(4);
    game.set_player(
        Vec2::from(tile_centre(spawn.0, spawn.1)),
        Vec2::ZERO,
        &mut (),
    );
    let mut closing_at = None;
    let mut end = None;
    for t in 1..=800 {
        game.tick(Input::default(), &mut ());
        let door = game.shelter_door(shelter).expect("door");
        if closing_at.is_none() && door.is_closing() {
            closing_at = Some(t);
            let p = game.player().expect("player");
            assert!(p.ready_for_win());
            assert_eq!(p.touched_no_input_counter(), 21);
        }
        if let Some(e) = game.take_session_end() {
            end = Some((t, e));
            break;
        }
    }
    let closing_at = closing_at.expect("the door closed");
    let (ended_at, end) = end.expect("the cycle ended");
    assert_eq!(
        end,
        SessionEnd::Win {
            malnourished: false
        }
    );
    // The door updates after the player (oldest in the update list, run newest first),
    // so `Close` adds its first 0.003125 the same tick: 320 increments, the last at
    // `closing_at + 319`.
    assert_eq!(ended_at - closing_at, 319);
    assert_eq!(door_fac(&game, shelter), 1.0);
    assert!(game.player().expect("player").sleeping());
    assert_eq!(ShelterDoor::CLOSE_SPEED, 1.0 / 320.0);
}
