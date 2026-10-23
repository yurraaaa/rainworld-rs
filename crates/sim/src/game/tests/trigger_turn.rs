//! Room triggers in a game: `Room.Loaded`'s cycle and slugcat filter (61310),
//! `ActiveTriggerChecker.FireEvent`'s draw and `multiUse` rule (53510), and
//! `RainCycle.Update`'s `RainRequestStopSong` (55474).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, EventTrigger, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room;
use crate::trigger::{MusicRequest, TriggerObjectKind};
use crate::world::World;
use crate::{Input, RainWorldGame, Sounds, Start};

/// The player starts on tile (4, 1): its main chunk is near (90, 30).
const SPOT: &str = "Spot<tA>event<tB>MusicEvent<eA>songName<eB>NA_01 - Proxima<eA>rooms<eB>6<tA>multiUse<tB>0<tA>fireChance<tB>1<tA>pos<tB>90<tB>40<tA>handlePos<tB>0<tB>80";

struct Setup;

impl Setup {
    /// A 10 x 8 room with `triggers`, the Survivor at cycle `cycle`, loaded by its loading
    /// steps alone (no tick run).
    fn game(triggers: &str, cycle: i32) -> RainWorldGame {
        let mut rows = vec!["##########"];
        rows.extend(["#........#"; 6]);
        rows.push("##########");
        let room: Room = room(&rows);
        let data = WorldData::from_room(&room);
        let settings = RoomSettings {
            triggers: EventTrigger::parse_list(triggers).expect("triggers parse"),
            ..RoomSettings::ANCESTOR.clone()
        };
        let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
        let campaign = &SlugcatCampaign::SURVIVOR;
        let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
        let story = StoryStart {
            cycle_number: cycle,
            ..StoryStart::new_game(campaign)
        };
        let start = Start {
            room: 0,
            tile: Some((4, 1)),
        };
        let mut game = RainWorldGame::new_story(
            world,
            campaign,
            story,
            start,
            UnityRandom::new(5),
            Sounds::default(),
            &mut (),
        );
        game.load_room_now(0, &mut ());
        if let Some(p) = game.player.as_mut() {
            p.leave_start_shelter();
        }
        game
    }

    fn checkers(game: &RainWorldGame) -> usize {
        game.trigger_objects
            .iter()
            .filter(|o| matches!(o.kind, TriggerObjectKind::Checker(_)))
            .count()
    }

    fn ticks(game: &mut RainWorldGame, n: usize) -> Vec<MusicRequest> {
        let mut all = Vec::new();
        for _ in 0..n {
            game.tick(Input::default(), &mut ());
            all.extend(game.take_music_requests());
        }
        all
    }
}

/// At cycle 2 a `fromCycle 3` trigger and one excluding the Survivor are left out; a
/// `toCycle 2` one is in.
#[test]
fn loaded_checkers_follow_the_save() {
    let list = [
        SPOT.to_owned(),
        SPOT.replace("<tA>multiUse", "<tA>fromCycle<tB>3<tA>multiUse"),
        SPOT.replace("<tA>multiUse", "<tA>toCycle<tB>2<tA>multiUse"),
        SPOT.replace("<tA>multiUse", "<tA>slugcats<tB>White<tA>multiUse"),
    ]
    .join(", ");
    let game = Setup::game(&list, 2);
    assert_eq!(Setup::checkers(&game), 2);
}

/// Ten ticks of `wait`, then the song request on the eleventh; a single-use checker
/// is gone after firing.
#[test]
fn spot_trigger_requests_its_song_once() {
    let mut game = Setup::game(SPOT, 0);
    assert!(Setup::ticks(&mut game, 10).is_empty());
    let requests = Setup::ticks(&mut game, 1);
    assert!(
        matches!(&requests[..], [MusicRequest::Song(m)] if m.song_name == "NA_01 - Proxima" && m.rooms_range == 6)
    );
    assert_eq!(Setup::checkers(&game), 0);
    assert!(Setup::ticks(&mut game, 5).is_empty());
}

/// A chance below 1 draws one `Random.value` when the event fires; a failed roll still
/// destroys a single-use checker, while a multi-use one rolls again next tick.
#[test]
fn failed_chance_destroys_single_use_checkers() {
    let never = SPOT.replace("fireChance<tB>1", "fireChance<tB>0.0000001");
    let mut a = Setup::game(SPOT, 0);
    let mut b = Setup::game(&never, 0);
    Setup::ticks(&mut a, 10);
    Setup::ticks(&mut b, 10);
    assert_eq!(a.rng(), b.rng());
    assert_eq!(Setup::ticks(&mut a, 1).len(), 1);
    assert!(Setup::ticks(&mut b, 1).is_empty());
    let mut one_more = a.rng().clone();
    one_more.value();
    assert_eq!(b.rng(), &one_more);
    assert_eq!(Setup::checkers(&b), 0);

    // Multi-use: the failing checker stays and draws on both firing ticks.
    let mut c = Setup::game(&never.replace("multiUse<tB>0", "multiUse<tB>1"), 0);
    let mut d = Setup::game(&SPOT.replace("multiUse<tB>0", "multiUse<tB>1"), 0);
    assert!(Setup::ticks(&mut c, 12).is_empty());
    assert_eq!(Setup::ticks(&mut d, 12).len(), 2);
    assert_eq!(Setup::checkers(&c), 1);
    let mut two_more = d.rng().clone();
    two_more.value();
    two_more.value();
    assert_eq!(c.rng(), &two_more);
}

/// Inside 2400 ticks of the rain, in a room with rain danger, the rain cycle asks the
/// music player to stop every tick; earlier it does not.
#[test]
fn rain_asks_songs_to_stop() {
    let mut game = Setup::game("", 0);
    assert!(Setup::ticks(&mut game, 1).is_empty());
    let length = game.world.rain_cycle.cycle_length;
    game.world.rain_cycle.timer = length - 2400;
    assert!(Setup::ticks(&mut game, 1).contains(&MusicRequest::RainStop));
    assert!(!game.world.music_allowed());
}

/// `PickUpObjectInstruction` adds a `PickupObjectInstruction`, which destroys itself on
/// its first update when no free rock or spear is in the room.
#[test]
fn pickup_instruction_without_items_goes() {
    let pickup = SPOT.replace(
        "event<tB>MusicEvent<eA>songName<eB>NA_01 - Proxima<eA>rooms<eB>6",
        "event<tB>PickUpObjectInstruction",
    );
    let mut game = Setup::game(&pickup, 0);
    Setup::ticks(&mut game, 11);
    assert!(matches!(
        &game.trigger_objects[..],
        [o] if matches!(o.kind, TriggerObjectKind::PickupInstruction(_))
    ));
    Setup::ticks(&mut game, 1);
    assert!(game.trigger_objects.is_empty());
}
