//! The death fall's `get_state` entries (one per realized room, counted since the room
//! loaded) and the other-room screenshot's camera sprites, on `CC_A06` (camera room) and
//! its realized neighbours `CC_C12` (first, the realizer's pick with seed 1; it was `CC_C09` before the global rain drew) and `CC_H01` of the installed game (skipped
//! without it). `CC_A06` and `CC_H01` have an open bottom (`defaultHeight` -70 once drawn)
//! and the `Fog` effect (`cc_settingstemplate`).

use rw_graphics::{DrawItem, Layer, Shader};
use rw_sim::Vec2;

use crate::mcp::Session;
use crate::mcp::params::{InputParam, LoadRoomParams, StepParams, TeleportPlayerParams};

impl Session {
    fn cc_a06() -> Option<Self> {
        let mut s = Self::installed()?;
        s.load_room(&LoadRoomParams {
            name: "CC_A06".to_owned(),
            seed: Some(1),
            spawn_tile: None,
            slugcat: None,
        })
        .expect("room loads");
        s.step(&StepParams {
            ticks: Some(40),
            input: Some(InputParam::default()),
        })
        .expect("steps");
        Some(s)
    }
}

/// Every realized room has its own death fall, counting from its own load whether or not
/// the camera is in it: after 10 more ticks both counters are 10 higher.
#[test]
fn every_realized_room_counts_its_own_death_fall() {
    let Some(mut s) = Session::cc_a06() else {
        return;
    };
    let before = s.state().expect("state").death_fall;
    let rooms: Vec<_> = before.iter().map(|f| f.room.as_str()).collect();
    assert_eq!(rooms, ["CC_A06", "CC_C12"]);
    assert!(before.iter().all(|f| f.counter > 0));
    // Only the room a camera has drawn has run `InitiateSprites`.
    let defaults: Vec<_> = before.iter().map(|f| f.default_height).collect();
    assert_eq!(defaults, [-70.0, 0.0]);
    s.step(&StepParams {
        ticks: Some(10),
        input: Some(InputParam::default()),
    })
    .expect("steps");
    let after = s.state().expect("state").death_fall;
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(a.counter, b.counter + 10, "{}", a.room);
    }
}

impl Session {
    /// Steps one tick at a time until the player is out of the shortcut in `room`;
    /// returns the ticks stepped.
    fn travel_to(&mut self, room: &str) -> u32 {
        for ticks in 1..=200 {
            let state = self
                .step(&StepParams {
                    ticks: Some(1),
                    input: Some(InputParam::default()),
                })
                .expect("steps");
            if state.room == room && state.player.as_ref().is_some_and(|p| !p.in_shortcut) {
                return ticks;
            }
        }
        panic!("no arrival in {room}");
    }

    fn fall_of(&mut self, room: &str) -> (u64, i32) {
        let f = self
            .state()
            .expect("state")
            .death_fall
            .into_iter()
            .find(|f| f.room == room)
            .expect("death fall");
        (f.id, f.counter)
    }
}

/// The camera leaves CC_A06 for CC_C09 and comes back, the room staying realized: its
/// death fall is the same object and its counter went up by every tick stepped.
#[test]
fn the_counter_survives_the_camera_leaving_and_returning() {
    let Some(mut s) = Session::cc_a06() else {
        return;
    };
    let (id, start) = s.fall_of("CC_A06");
    let mut ticks = 0;
    for (to, wait) in [("CC_C09", 30), ("CC_A06", 0)] {
        s.teleport_player(&TeleportPlayerParams {
            room: to.to_owned(),
            node: None,
        })
        .expect("teleports");
        ticks += s.travel_to(to);
        let state = s.state().expect("state");
        assert!(
            state.realized_rooms.iter().any(|r| r.room == "CC_A06"),
            "CC_A06 stays realized"
        );
        s.step(&StepParams {
            ticks: Some(wait),
            input: Some(InputParam::default()),
        })
        .expect("steps");
        ticks += wait;
    }
    let (id_after, counter) = s.fall_of("CC_A06");
    assert_eq!(id_after, id);
    assert_eq!(counter, start + i32::try_from(ticks).unwrap());
}

/// An other-room shot draws what the camera viewing that room would: the death fall in
/// `Bloom` at the room's default height and the `Fog` effect in `Foreground`.
#[test]
fn another_rooms_shot_has_its_death_fall_and_effect() {
    let Some(mut s) = Session::cc_a06() else {
        return;
    };
    // The realizer brings CC_H01 in last of the neighbours (about 200 ticks with seed 1).
    for _ in 0..400 {
        let state = s.state().expect("state");
        if state.realized_rooms.iter().any(|r| r.room == "CC_H01") {
            break;
        }
        s.step(&StepParams {
            ticks: Some(1),
            input: Some(InputParam::default()),
        })
        .expect("steps");
    }
    let room = s.room_named(Some("CC_H01")).expect("room");
    let shot = s.room_shot(room, 0, true).expect("shot");
    let game = s.scene_mut().expect("game").game();
    let lists = shot.camera_lists(game, 1366.0, Vec2::ZERO, None);
    let fall = lists.death_fall.expect("death fall");
    let [DrawItem::Sprite(sprite)] = fall.items.as_slice() else {
        panic!("one sprite");
    };
    assert_eq!(
        (sprite.shader, sprite.layer),
        (Shader::DeathFall, Layer::Bloom)
    );
    assert_eq!(sprite.pos, Vec2::new(683.0, -70.0));
    let (layer, effect) = lists.effect.expect("Fog effect");
    assert_eq!(layer, Layer::Foreground);
    assert_eq!(effect.items.len(), 1);
    assert!(lists.water_light.is_none(), "no water in CC_H01");
}

/// An other-room shot has the `WaterLight` sprite exactly where the room has water.
#[test]
fn another_rooms_shot_has_a_water_light_where_there_is_water() {
    let Some(mut s) = Session::installed() else {
        return;
    };
    s.load_room(&LoadRoomParams {
        name: "GW_A07".to_owned(),
        seed: Some(1),
        spawn_tile: None,
        slugcat: None,
    })
    .expect("room loads");
    // The realizer brings GW_A06 in after GW_A10 (about 80 ticks with seed 1).
    s.step(&StepParams {
        ticks: Some(90),
        input: Some(InputParam::default()),
    })
    .expect("steps");
    let other = s.room_named(Some("GW_A06")).expect("room");
    let shot = s.room_shot(other, 0, true).expect("shot");
    let game = s.scene_mut().expect("game").game();
    let water = game.sim().world().realized(other).unwrap().water.is_some();
    let lists = shot.camera_lists(game, 1366.0, Vec2::ZERO, None);
    assert!(water, "GW_A06 has water");
    let light = lists.water_light.expect("water light");
    let [DrawItem::Sprite(sprite)] = light.items.as_slice() else {
        panic!("one sprite");
    };
    assert_eq!(
        (sprite.shader, sprite.layer),
        (Shader::WaterLight, Layer::Foreground)
    );
    let still = game
        .sim()
        .world()
        .realized(other)
        .and_then(|r| r.water.as_ref())
        .map(rw_sim::Water::still_level)
        .unwrap();
    assert_eq!(sprite.pos.y, still);
}
