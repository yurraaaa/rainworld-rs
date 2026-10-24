//! `Player.SleepUpdate` and the shelter block of `Player.Update`.

use rw_random::UnityRandom;

use super::flat_room;
use crate::{HEAD, HIPS, Input, Player, ShelterCheck, Vec2};

const DOWN: Input = Input::new(0, -1, false);

/// Fed enough, far from the entrance, in an intact story shelter.
const FED: ShelterCheck = ShelterCheck {
    in_shelter: true,
    food_in_room: 4,
    food_to_hibernate: 4,
    max_food: 7,
    malnourished: false,
    rain_over: false,
    far_from_entrance: true,
    inside_range: true,
};

impl Player {
    /// Standing on the flat room's floor with `input` as this tick's.
    fn in_shelter_holding(input: Input) -> Self {
        let mut p = Self::upright_at(Vec2::new(100.0, 48.0));
        p.input[0] = input;
        p
    }

    /// The shelter block after `readyForWin = false` (its `touchedNoInputCounter` left as
    /// set).
    fn shelter_tick(&mut self, room: &rw_assets::Room, cx: &ShelterCheck) -> bool {
        self.ready_for_win = false;
        self.shelter_check(room, cx)
    }
}

#[test]
fn waking_in_a_shelter_lies_down_with_one_draw() {
    let room = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.start_asleep();
    assert!(p.sleeping());
    let mut rng = UnityRandom::new(3);
    p.sleep_update(&room, Input::default(), (5, 2), &mut rng);
    let mut expected = UnityRandom::new(3);
    let side = if expected.value() < 0.5 { -1.0 } else { 1.0 };
    assert_eq!(rng, expected);
    assert_eq!(p.sleep_counter(), 99);
    assert!(!p.standing);
    // Tile (5, 2)'s middle (110, 50), the chunks half the rod (17) apart, on the floor
    // at -10 + rad.
    let head = p.body.chunks[HEAD].pos;
    let hips = p.body.chunks[HIPS].pos;
    assert_eq!(
        head,
        Vec2::new(110.0 - 8.5 * side, 40.0 + p.body.chunks[HEAD].rad)
    );
    assert_eq!(
        hips,
        Vec2::new(110.0 + 8.5 * side, 40.0 + p.body.chunks[HIPS].rad)
    );
}

#[test]
fn an_input_starts_the_uncurl_but_the_map_button_does_not() {
    let room = flat_room();
    let mut rng = UnityRandom::new(3);
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.start_asleep();
    p.sleep_update(&room, Input::default(), (5, 2), &mut rng);
    let map_and_right = Input {
        map: true,
        ..Input::new(1, 0, false)
    };
    p.sleep_update(&room, map_and_right, (5, 2), &mut rng);
    assert_eq!(p.sleep_counter(), 99);
    p.sleep_update(&room, Input::new(1, 0, false), (5, 2), &mut rng);
    assert_eq!(p.sleep_counter(), 17);
    p.sleep_update(&room, Input::default(), (5, 2), &mut rng);
    // 16 ticks left: InverseLerp(10, 17, 16).
    assert_eq!(p.sleep_counter(), 16);
    assert!((p.sleep_curl_up() - 6.0 / 7.0).abs() < 1e-6);
}

#[test]
fn sleeping_zeroes_the_counter_after_the_door_closes() {
    // `Close` from the starving branch sets -24; SleepUpdate counts down, lying on all
    // fours at -6 (from standing) and curling up from -14 to -24.
    let room = flat_room();
    let mut rng = UnityRandom::new(3);
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.sleep_counter = -1;
    for _ in 0..5 {
        p.sleep_update(&room, Input::default(), (5, 2), &mut rng);
    }
    assert_eq!(p.sleep_counter(), -6);
    assert!(!p.standing);
    for _ in 0..13 {
        p.sleep_update(&room, Input::default(), (5, 2), &mut rng);
    }
    assert_eq!(p.sleep_counter(), -19);
    assert!((p.sleep_curl_up() - 0.5).abs() < 1e-6);
}

#[test]
fn the_start_shelter_is_not_a_place_to_win() {
    let room = flat_room();
    let mut p = Player::in_shelter_holding(Input::default());
    assert!(p.still_in_start_shelter());
    assert!(!p.shelter_tick(&room, &FED));
    assert!(!p.ready_for_win());
    // Unless the rain came.
    let rain = ShelterCheck {
        rain_over: true,
        ..FED
    };
    p.touched_no_input_counter = 21;
    assert!(p.shelter_tick(&room, &rain));
    assert!(p.ready_for_win());
}

#[test]
fn a_fed_still_slugcat_far_from_the_entrance_closes_the_door() {
    let room = flat_room();
    let mut p = Player::in_shelter_holding(Input::default());
    p.still_in_start_shelter = false;
    p.touched_no_input_counter = 20;
    assert!(!p.shelter_tick(&room, &FED));
    assert!(p.ready_for_win());
    p.touched_no_input_counter = 21;
    assert!(p.shelter_tick(&room, &FED));
    let near = ShelterCheck {
        far_from_entrance: false,
        ..FED
    };
    assert!(!p.shelter_tick(&room, &near));
    // Short of food, no closing.
    let hungry = ShelterCheck {
        food_in_room: 3,
        ..FED
    };
    assert!(!p.shelter_tick(&room, &hungry));
    assert!(!p.ready_for_win());
    // Malnourished, it takes `maxFood`.
    let malnourished = ShelterCheck {
        malnourished: true,
        food_in_room: 6,
        ..FED
    };
    assert!(!p.shelter_tick(&room, &malnourished));
    let malnourished = ShelterCheck {
        food_in_room: 7,
        ..malnourished
    };
    assert!(p.shelter_tick(&room, &malnourished));
}

#[test]
fn a_starving_slugcat_holding_down_forces_sleep_after_260_ticks() {
    let room = flat_room();
    let mut p = Player::in_shelter_holding(DOWN);
    p.still_in_start_shelter = false;
    let starving = ShelterCheck {
        food_in_room: 1,
        ..FED
    };
    for _ in 0..260 {
        assert!(!p.shelter_tick(&room, &starving));
    }
    assert_eq!(p.force_sleep_counter(), 260);
    assert!(p.shelter_tick(&room, &starving));
    assert_eq!(p.force_sleep_counter(), 261);
    assert_eq!(p.sleep_counter(), -24);
    // Letting go resets it; with no food at all it never counts.
    let mut p = Player::in_shelter_holding(DOWN);
    p.still_in_start_shelter = false;
    let empty = ShelterCheck {
        food_in_room: 0,
        ..FED
    };
    p.shelter_tick(&room, &empty);
    assert_eq!(p.force_sleep_counter(), 0);
    p.shelter_tick(&room, &starving);
    assert_eq!(p.force_sleep_counter(), 1);
    p.input[0] = Input::default();
    p.shelter_tick(&room, &starving);
    assert_eq!(p.force_sleep_counter(), 0);
}

#[test]
fn the_force_sleep_counter_curls_the_slugcat_up() {
    // `sleepCurlUp` eases to InverseLerp(10, 210, forceSleepCounter) by LerpAndTick.
    let room = flat_room();
    let mut rng = UnityRandom::new(3);
    let mut p = Player::in_shelter_holding(DOWN);
    p.force_sleep_counter = 110;
    p.sleep_update(&room, DOWN, (5, 2), &mut rng);
    // Lerp(0, 0.5, 0.04) = 0.02, then 0.05 more towards 0.5.
    assert!((p.sleep_curl_up() - 0.07).abs() < 1e-6);
}
