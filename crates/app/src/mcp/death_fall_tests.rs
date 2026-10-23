//! The death fall's `get_state` entries (one per realized room, counted since the room
//! loaded) and the other-room screenshot's camera sprites, on `CC_A06` (camera room) and
//! its realized neighbour `CC_H01` of the installed game (skipped without it). Both have an
//! open bottom with `defaultHeight` -70 and the `Fog` effect (`cc_settingstemplate`).

use rw_graphics::{DrawItem, Layer, Shader};
use rw_sim::Vec2;

use super::Session;
use super::params::{InputParam, LoadRoomParams, StepParams};

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
    assert_eq!(rooms, ["CC_A06", "CC_H01"]);
    assert!(
        before
            .iter()
            .all(|f| f.counter > 0 && f.default_height == -70.0)
    );
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

/// An other-room shot draws what the camera viewing that room would: the death fall in
/// `Bloom` at the room's default height and the `Fog` effect in `Foreground`.
#[test]
fn another_rooms_shot_has_its_death_fall_and_effect() {
    let Some(mut s) = Session::cc_a06() else {
        return;
    };
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
    s.step(&StepParams {
        ticks: Some(40),
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
