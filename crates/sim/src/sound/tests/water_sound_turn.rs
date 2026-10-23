//! The water's surface loop in play. Expected values from `Water`'s constructor and
//! `Update` (`waterSounds`), `Room.Update` (water before the update list) and
//! `Room.PlayRectSound` (the emitter joins the update list).

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, ClipNames, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::tests::room_with_water;
use crate::world::World;
use crate::{Input, RainWorldGame, SoundEvent, SoundId, SoundRect, SoundSource, Sounds, Start};

/// A 30 x 8 room, floor at y 1, water row 3.
const POOL: [&str; 8] = [
    "##############################",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#............................#",
    "##############################",
];

/// A Survivor story in `POOL` hearing the surface loop's trigger, before any tick.
fn game() -> RainWorldGame {
    let room = room_with_water(&POOL, 3);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        ..RoomSettings::ANCESTOR
    };
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let campaign = &SlugcatCampaign::SURVIVOR;
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let text = "Volume: 6\nVolume exponent: 1.8\n\
        Water_Surface_Calm_LOOP/rangeFac=0.8 : waterGurgleLOOP1/vol=0.6\n";
    let names = ClipNames::new(["watergurgleloop1".to_owned()], None);
    let sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((5, 4)),
        },
        UnityRandom::new(5),
        sounds,
    )
}

/// The viewed water room plays the loop on its band once, then keeps it every tick;
/// the emitter sits in the room's update list.
#[test]
fn the_viewed_water_room_plays_the_calm_loop() {
    let mut g = game();
    let mut plays = Vec::new();
    for _ in 0..80 {
        g.tick(Input::NONE, &mut ());
        for e in g.sounds.take_events() {
            if let SoundEvent::Play(p) = e
                && p.id == SoundId::WaterSurfaceCalmLoop
            {
                plays.push(p);
            }
        }
    }
    assert_eq!(plays.len(), 1, "{plays:?}");
    let r = g.world().realized(0).expect("realized");
    let w = r.water_sound.as_ref().expect("water sound");
    let rect = SoundRect {
        left: 0.0,
        bottom: 40.0,
        right: 600.0,
        top: 60.0,
    };
    assert_eq!(w.rect, rect);
    let emitter = w.sound_loop.emitter.expect("emitter");
    assert_eq!(
        plays[0].source,
        SoundSource::Rect {
            emitter: emitter.sound,
            rect,
        }
    );
    assert!(w.playing());
    assert!(
        g.room_update_list(0)
            .iter()
            .any(|&(seq, _)| seq == emitter.update_order)
    );
}
