//! The dev rain timer control: the held timer stops only the timer, a set timer is read
//! back, and unused it changes nothing.

use crate::Input;
use crate::creature::lizard::perception_turn::tests::{HALL, game};

#[test]
fn unused_the_timer_ticks_once_a_tick() {
    let mut g = game(&HALL, (3, 1));
    let before = g.world().rain_cycle.timer;
    for _ in 0..10 {
        g.tick(Input::NONE, &mut ());
    }
    assert_eq!(g.world().rain_cycle.timer, before + 10);
}

#[test]
fn held_stops_the_timer_and_nothing_else() {
    let mut g = game(&HALL, (3, 1));
    g.dev_set_rain_paused(true);
    let cycle = g.world().rain_cycle;
    for _ in 0..10 {
        g.tick(Input::NONE, &mut ());
    }
    let now = &g.world().rain_cycle;
    assert_eq!(now.timer, cycle.timer);
    assert_eq!(now.day_night_counter, cycle.day_night_counter);
    assert!(now.water_cycle.time_in_stage > cycle.water_cycle.time_in_stage);
    g.dev_set_rain_paused(false);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.world().rain_cycle.timer, cycle.timer + 1);
}

#[test]
fn set_timer_is_read_back_and_counts_the_night() {
    let mut g = game(&HALL, (3, 1));
    let cycle = g.world().rain_cycle;
    g.dev_set_rain_timer(cycle.cycle_length + 1200);
    let c = &g.world().rain_cycle;
    assert_eq!(c.timer, cycle.cycle_length + 1200);
    assert_eq!(c.time_until_rain(), -1200);
    assert_eq!(c.day_night_counter, c.timer - c.sun_down_start_time + 1);
    g.dev_set_rain_timer(-5);
    assert_eq!(g.world().rain_cycle.timer, 0);
    assert_eq!(g.world().rain_cycle.day_night_counter, 0);
}

#[test]
fn setting_the_timer_keeps_the_games_pause_and_speed_waits_for_it() {
    let mut g = game(&HALL, (3, 1));
    g.world_mut().rain_cycle.pause = 3;
    g.dev_set_rain_timer(100);
    assert_eq!(g.world().rain_cycle.pause, 3);
    g.dev_set_rain_speed(5);
    g.tick(Input::NONE, &mut ());
    assert_eq!(
        (g.world().rain_cycle.timer, g.world().rain_cycle.pause),
        (100, 2)
    );
    g.dev_clear_rain_pause();
    assert_eq!(g.world().rain_cycle.pause, 0);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.world().rain_cycle.timer, 105);
}

#[test]
fn speed_adds_ticks_and_the_cycle_length_is_set() {
    let mut g = game(&HALL, (3, 1));
    g.dev_set_rain_timer(100);
    g.dev_set_rain_speed(5);
    g.tick(Input::NONE, &mut ());
    assert_eq!(g.world().rain_cycle.timer, 105);
    g.dev_set_rain_speed(99);
    assert_eq!(g.dev_rain_control().speed, 20);
    g.dev_set_rain_cycle_length(2000);
    assert_eq!(g.world().rain_cycle.cycle_length, 2000);
    assert!(g.world().rain_cycle.base_cycle_length > 2000);
}

/// The dev reset (`restart`/`stop`) gives the cycle a fresh global rain.
#[test]
fn the_dev_reset_starts_the_global_rain_over() {
    let mut g = game(&HALL, (3, 1));
    let length = g.world().rain_cycle.cycle_length;
    g.dev_set_rain_timer(length - 1);
    for _ in 0..30 {
        g.tick(Input::NONE, &mut ());
    }
    assert!(g.world().rain_cycle.death_rain_has_hit);
    g.dev_set_rain_timer(0);
    g.dev_reset_global_rain();
    let c = &g.world().rain_cycle;
    assert!(!c.death_rain_has_hit);
    assert_eq!(c.global_rain, crate::world::GlobalRain::default());
}
