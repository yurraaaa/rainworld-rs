//! `ShortcutGraphics.NewRoom`'s wait for `shortCutsReady` and `Draw`'s blink draws, on a
//! small test room with two pipe entrances.

use rw_assets::RoomSettings;
use rw_graphics::{Color, ShortcutPalette};
use rw_random::UnityRandom;
use rw_sim::{Input, Vec2};

use crate::game::Game;
use crate::game::tests::pipe_room;

/// What the tests ask of a game showing the test room.
trait ShortcutTestGame {
    /// The room before its first tick, shown by camera 0.
    fn waiting_room() -> Game;
    fn is_ready(&self) -> bool;
    fn is_waiting(&self) -> bool;
    /// One `ShortcutGraphics.Draw`; whether it made sprites.
    fn draw_once(&mut self) -> bool;
}

impl ShortcutTestGame for Game {
    fn waiting_room() -> Game {
        let mut g = Game::new_unticked(pipe_room(), &RoomSettings::ANCESTOR, 1, Some((2, 1)));
        g.new_shortcut_room(0);
        g
    }

    fn is_ready(&self) -> bool {
        self.sim()
            .world()
            .realized(0)
            .is_some_and(|r| r.shortcuts_ready())
    }

    fn is_waiting(&self) -> bool {
        self.shortcut_wait().is_some_and(|(_, waiting)| waiting)
    }

    fn draw_once(&mut self) -> bool {
        let palette = ShortcutPalette {
            colors: [
                Color::rgb(0.1, 0.0, 0.0),
                Color::rgb(0.5, 0.0, 0.0),
                Color::rgb(0.9, 0.0, 0.0),
            ],
            symbol: Color::rgb(0.0, 0.0, 1.0),
        };
        self.shortcut_draw_list(palette, Vec2::ZERO).is_some()
    }
}

/// `Draw` polls while `waitingForRoomToGenerateShortcuts`: no sprites and no draws until
/// the room is `shortCutsReady`; the poll that sees it runs `GenerateSprites` and returns,
/// the next one draws.
#[test]
fn draw_waits_until_the_room_is_shortcuts_ready() {
    let mut g = Game::waiting_room();
    assert!(!g.is_ready());
    assert!(g.is_waiting());
    assert!(g.shortcut_graphics().is_none());
    while !g.is_ready() {
        let rng = g.cosmetic_rng().clone();
        assert!(!g.draw_once());
        assert!(g.is_waiting());
        assert_eq!(g.cosmetic_rng(), &rng);
        g.tick(Input::NONE);
    }
    let rng = g.cosmetic_rng().clone();
    assert!(!g.draw_once(), "the poll that generates draws nothing");
    assert!(!g.is_waiting());
    assert!(g.shortcut_graphics().is_some());
    assert!(g.draw_once());
    assert_eq!(g.cosmetic_rng(), &rng, "no pipe blinks: no draws");
}

/// A room that is `shortCutsReady` when `NewRoom` runs generates at once.
#[test]
fn new_room_of_a_ready_room_does_not_wait() {
    let mut g = Game::waiting_room();
    while !g.is_ready() {
        g.tick(Input::NONE);
    }
    g.new_shortcut_room(0);
    assert!(!g.is_waiting());
    assert!(g.shortcut_graphics().is_some());
}

/// A blinking pipe makes `Draw` take three `Random.value` (the flicker's spread and
/// chance, then the dropout chance) and a fourth when the dropout happens, and an entrance
/// flashing (`shortcutsBlinking[k, 3] < 0`) one for its glow's scale.
#[test]
fn blinking_draws_in_the_games_order() {
    let mut g = Game::waiting_room();
    while !g.is_ready() {
        g.tick(Input::NONE);
    }
    g.draw_once();
    g.sim_mut()
        .world_mut()
        .realized_mut(0)
        .and_then(|r| r.shortcuts_blinking.as_mut())
        .expect("blinking rows")
        .blink(None, Some(0), 1.0);
    let before = g.cosmetic_rng().clone();
    assert!(g.draw_once());
    let mut want: UnityRandom = before.clone();
    want.value();
    want.value();
    let dropout = want.value() < 1.0 / 30.0;
    let mut want = before;
    let draws = 3 + usize::from(dropout) + 1;
    for _ in 0..draws {
        want.value();
    }
    assert_eq!(g.cosmetic_rng(), &want, "{draws} draws");
}

/// A flashing entrance on a pipe that is not lit draws once and skips the pipe's three.
#[test]
fn a_flashing_entrance_alone_draws_once() {
    let mut g = Game::waiting_room();
    while !g.is_ready() {
        g.tick(Input::NONE);
    }
    g.draw_once();
    g.sim_mut()
        .world_mut()
        .realized_mut(0)
        .and_then(|r| r.shortcuts_blinking.as_mut())
        .expect("blinking rows")
        .blink(Some(0), None, 1.0);
    let mut want = g.cosmetic_rng().clone();
    want.value();
    assert!(g.draw_once());
    assert_eq!(g.cosmetic_rng(), &want);
}

/// The camera's room abstracted and realized again is a new room: the graphics made
/// for the old one are dropped and wait for the new one to be `shortCutsReady`, instead
/// of drawing the old entrances against the new room's unmapped shortcuts.
#[test]
fn a_room_realized_again_remakes_the_graphics() {
    let mut g = Game::waiting_room();
    while !g.is_ready() {
        g.tick(Input::NONE);
    }
    g.draw_once();
    assert!(g.draw_once());
    let world = g.sim_mut().world_mut();
    world.abstractize(0, &mut UnityRandom::new(0));
    world.activate_room(0).unwrap();
    assert!(!g.is_ready());
    assert!(!g.draw_once());
    assert!(g.is_waiting());
    while !g.is_ready() {
        g.tick(Input::NONE);
    }
    assert!(!g.draw_once(), "the poll that generates draws nothing");
    assert!(g.draw_once());
}
