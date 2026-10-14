//! Vertical poles and ledge grabs. Expected values are derived by hand from
//! `rainworld-re/notes/vertical-poles.md` and `ledge-grab.md` (derivation next to each
//! assertion). A pole column `x` has its middle at `20 x + 10`.

use std::ops::RangeInclusive;

use rw_assets::Room;

use super::{DRAG, G, JUMP, RIGHT, assert_close, assert_vec};
use crate::Vec2;
use crate::player::{Animation, BodyMode, HEAD, HIPS, Input, Player};
use crate::tests::room;

const UP: Input = Input::new(0, 1, false);
const DOWN: Input = Input::new(0, -1, false);
const LEFT: Input = Input::new(-1, 0, false);

/// A room drawn bottom row first; starts as Air with Solid rows 0 and 1 (floor top 40).
pub(super) struct Grid {
    rows: Vec<Vec<u8>>,
}

impl Grid {
    pub(super) fn new(w: usize, h: usize) -> Self {
        let mut rows = vec![vec![b'.'; w]; h];
        rows[0].fill(b'#');
        rows[1].fill(b'#');
        Self { rows }
    }

    pub(super) fn set(mut self, x: usize, ys: RangeInclusive<usize>, cell: u8) -> Self {
        for y in ys {
            self.rows[y][x] = cell;
        }
        self
    }

    /// Sets row `y` over the columns `xs` to `cell`.
    pub(super) fn row(mut self, y: usize, xs: RangeInclusive<usize>, cell: u8) -> Self {
        for x in xs {
            self.rows[y][x] = cell;
        }
        self
    }

    pub(super) fn room(&self) -> Room {
        let text: Vec<String> = self
            .rows
            .iter()
            .rev()
            .map(|r| String::from_utf8(r.clone()).expect("ascii"))
            .collect();
        let refs: Vec<&str> = text.iter().map(String::as_str).collect();
        room(&refs)
    }
}

impl Player {
    /// On a pole in `ClimbOnBeam`: body vertical, hips centre at `hips`, at rest.
    fn on_pole_at(hips: Vec2, flip: i32) -> Self {
        let mut p = Self::upright_at(hips);
        p.animation = Animation::ClimbOnBeam;
        p.body_mode = BodyMode::ClimbingOnBeam;
        p.flip_direction = flip;
        p.last_flip_direction = flip;
        p
    }

    /// Steady pole climb speed for the climb term `c` (see the climb-speed test).
    fn steady_climb_speed(c: f32) -> f32 {
        DRAG * c / 2.0 / (1.0 - DRAG * 0.75)
    }

    /// Middle of the two chunks.
    fn centre(&self) -> Vec2 {
        (self.body.chunks[HEAD].pos + self.body.chunks[HIPS].pos) / 2.0
    }

    /// Runs until `animation` is reached; panics after `limit` ticks.
    fn run_until(&mut self, room: &Room, input: Input, animation: Animation, limit: usize) {
        for _ in 0..limit {
            if self.animation == animation {
                return;
            }
            self.update(room, input);
        }
        assert_eq!(self.animation, animation, "not reached in {limit} ticks");
    }
}

/// 20 x 40 tiles, one pole in column 5 from row 2 to row 38.
fn tall_pole() -> Room {
    Grid::new(20, 40).set(5, 2..=38, b'|').room()
}

// ---- grabbing ----------------------------------------------------------------------

#[test]
fn airborne_grab_snaps_the_head_onto_the_pole() {
    let r = tall_pole();
    let mut p = Player::upright_at(Vec2::new(107.0, 300.0));
    p.update(&r, UP);
    // Head tile (5, 15) has the pole: ClimbOnBeam, head x := 110, head vel := 0; facing
    // from the side: 107 < 110 gives -1. The hips are not moved.
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.0);
    assert_eq!(p.body.chunks[HEAD].pos.x, 110.0);
    assert_eq!(p.body.chunks[HIPS].pos.x, 107.0);
    assert_eq!(p.flip_direction(), -1);
}

#[test]
fn fast_head_faces_its_velocity_when_grabbing() {
    let r = tall_pole();
    let mut p = Player::upright_at(Vec2::new(101.0, 300.0));
    p.body.chunks[HEAD].vel.x = 6.0;
    p.body.chunks[HIPS].vel.x = 6.0;
    p.update(&r, UP);
    // After physics head x = 106.994 (< 110, side says -1) but |vel.x| = 5.994 > 5 on
    // the head's own column: facing from the velocity, +1.
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.flip_direction(), 1);
}

/// Poles in columns 4 and 6, the player standing in column 5.
fn two_poles() -> Room {
    Grid::new(12, 20)
        .set(4, 2..=18, b'|')
        .set(6, 2..=18, b'|')
        .room()
}

#[test]
fn standing_between_two_poles_grabs_the_last_candidate() {
    let r = two_poles();
    let mut p = Player::upright_at(Vec2::new(110.0, 48.0));
    p.run(&r, Input::NONE, 10);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    p.update(&r, UP);
    // Grounded, no x: candidates 0 (none), -1 (column 4: head x := 90), +1 (column 6:
    // 90 < 130 so facing -1, head x := 130). The last match wins.
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.body.chunks[HEAD].pos.x, 130.0);
    assert_eq!(p.flip_direction(), -1);
}

#[test]
fn x_input_limits_the_grab_to_the_head_column() {
    let r = two_poles();
    let mut p = Player::upright_at(Vec2::new(110.0, 48.0));
    p.run(&r, Input::NONE, 10);
    p.update(&r, Input::new(1, 1, false));
    assert_eq!(p.animation(), Animation::None);
}

#[test]
fn vertical_pole_overrides_a_same_tick_beam_grab() {
    // Both flags in tile (5, 15) only.
    let r = Grid::new(12, 30).set(5, 15..=15, b'+').room();
    let mut p = Player::upright_at(Vec2::new(107.0, 290.0));
    p.update(&r, UP);
    // Physics: both vel.y = -0.9 * 0.999; Default standing: hips -= 4. The beam grab
    // snaps head y to 310 and scales hips vel.y by 0.25; then the pole takes over:
    // ClimbOnBeam, head x := 110, head vel := 0.
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_vec(p.body.chunks[HEAD].pos, 110.0, 310.0);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.0);
    assert_close(
        p.body.chunks[HIPS].vel.y,
        (-G * DRAG - 4.0) * 0.25,
        "hips vel.y",
    );
}

#[test]
fn pole_end_above_the_head_hangs_under_it() {
    // Pole rows 16..=28; row 15 below it is open.
    let r = Grid::new(12, 30).set(5, 16..=28, b'|').room();
    let mut p = Player::upright_at(Vec2::new(107.0, 301.0));
    for c in &mut p.body.chunks {
        c.vel.y = -12.0;
    }
    p.update(&r, UP);
    // Physics: head 318 - 12.9 * 0.999 = 305.11 (row 15, pole above, none here).
    // Under-hang: head := middle (110, 310) + (0, 5), head vel := 0, hips vel
    // (0, -12.887 - 4) clamped to length 9.
    assert_eq!(p.animation(), Animation::HangUnderVerticalBeam);
    assert_vec(p.body.chunks[HEAD].pos, 110.0, 315.0);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.0);
    assert_vec(p.body.chunks[HIPS].vel, 0.0, -9.0);
}

// ---- climbing ----------------------------------------------------------------------

#[test]
fn hold_forces_per_chunk() {
    let r = tall_pole();
    // (input, climb term): up +1, none 0, down -2.2 * (0.2 + 0.8).
    for (input, climb) in [(UP, 1.0), (Input::NONE, 0.0), (DOWN, -2.2)] {
        let mut p = Player::on_pole_at(Vec2::new(113.0, 300.0), 1);
        p.body.chunks[HEAD].vel.y = 2.0;
        p.body.chunks[HIPS].vel.y = -1.0;
        p.input[0] = input;
        p.climb_on_beam(&r);
        // head: 2 * 0.5 + climb + (1 + 0.9); hips: -1 - (1 - 0.9).
        assert_close(p.body.chunks[HEAD].vel.y, 1.0 + climb + 1.9, "head");
        assert_close(p.body.chunks[HIPS].vel.y, -1.1, "hips");
        // Net of the constant part after gravity: head +1.0, hips -1.0.
        assert_close(1.9 - G, 1.0, "head net");
        assert_close(-0.1 - G, -1.0, "hips net");
    }
    // Side offset: head x (113 + 110 + 5) / 2, hips (7 * 113 + 115) / 8.
    let mut p = Player::on_pole_at(Vec2::new(113.0, 300.0), 1);
    p.climb_on_beam(&r);
    assert_close(p.body.chunks[HEAD].pos.x, 114.0, "head x");
    assert_close(p.body.chunks[HIPS].pos.x, 113.25, "hips x");
}

#[test]
fn climb_speeds_match_the_momentum_balance() {
    // Body vertical and rigid: after the rod both chunks share the speed v. The block
    // gives head 0.5 v + c + 1.9, hips v - 0.1, then physics subtracts 0.9 from each
    // and multiplies by 0.999, and the rod averages: v' = 0.999 (0.75 v + c / 2).
    // Steady state: v = 0.999 c / 2 / (1 - 0.999 * 0.75).
    let r = tall_pole();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 150.0), 1);
    p.run(&r, UP, 40);
    let y0 = p.centre().y;
    p.run(&r, UP, 100);
    let up = (p.centre().y - y0) / 100.0;
    assert!(
        (up - Player::steady_climb_speed(1.0)).abs() < 1e-3,
        "up {up}"
    );
    assert_eq!(p.animation(), Animation::ClimbOnBeam);

    p.run(&r, DOWN, 30);
    let y0 = p.centre().y;
    p.run(&r, DOWN, 40);
    let down = (p.centre().y - y0) / 40.0;
    assert!(
        (down - Player::steady_climb_speed(-2.2)).abs() < 1e-3,
        "down {down}"
    );

    p.run(&r, Input::NONE, 60);
    let y0 = p.centre().y;
    p.run(&r, Input::NONE, 20);
    assert!((p.centre().y - y0).abs() < 1e-3, "still drifting");
    // Side offset: both chunks 5 px to the facing side of the middle.
    assert!((p.body.chunks[HEAD].pos.x - 115.0).abs() < 1e-3);
    assert!((p.body.chunks[HIPS].pos.x - 115.0).abs() < 1e-3);
    eprintln!(
        "measured steady climb {up} px/tick up, {down} down (derived {} / {})",
        Player::steady_climb_speed(1.0),
        Player::steady_climb_speed(-2.2)
    );
}

#[test]
fn holding_the_other_side_switches_sides() {
    let r = tall_pole();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.update(&r, LEFT);
    // Facing follows x first; then head x (115 + 110 - 5) / 2, hips (7 * 115 + 105) / 8.
    assert_eq!(p.flip_direction(), -1);
    assert_close(p.body.chunks[HEAD].pos.x, 110.0, "head x");
    assert_close(p.body.chunks[HIPS].pos.x, 113.75, "hips x");
    p.run(&r, LEFT, 80);
    assert!((p.body.chunks[HEAD].pos.x - 105.0).abs() < 0.01);
    assert!((p.body.chunks[HIPS].pos.x - 105.0).abs() < 0.01);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
}

#[test]
fn pressing_toward_the_facing_side_hops_to_a_neighbour_pole() {
    let r = Grid::new(12, 30)
        .set(5, 2..=28, b'|')
        .set(6, 2..=28, b'|')
        .room();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.update(&r, Input::NONE);
    p.update(&r, RIGHT);
    // x edge toward the facing side with a pole in (6, row): head x := 130 - 5,
    // facing reversed, jumpStun = 11 * -1.
    assert_eq!(p.body.chunks[HEAD].pos.x, 125.0);
    assert_eq!(p.flip_direction(), -1);
    assert_eq!(p.jump_stun, -11);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    // jumpStun forces the facing against the held key while it counts down.
    p.run(&r, RIGHT, 9);
    assert_eq!(p.flip_direction(), -1);
}

#[test]
fn x_press_moves_onto_a_crossing_beam() {
    // Pole in column 5, beam row 15 from column 0 to 5 (both flags at (5, 15)).
    let mut g = Grid::new(12, 30).set(5, 2..=28, b'|');
    for x in 0..5 {
        g = g.set(x, 15..=15, b'=');
    }
    let r = g.set(5, 15..=15, b'+').room();
    let mut p = Player::on_pole_at(Vec2::new(105.0, 293.0), -1);
    p.update(&r, Input::NONE);
    p.update(&r, LEFT);
    // x edge toward the facing side (-1 == flip == last flip) with the head in a beam
    // tile and no solid below: HangFromBeam.
    assert_eq!(p.animation(), Animation::HangFromBeam);

    // Hips in the beam tile, head above it: StandOnBeam.
    let mut p = Player::on_pole_at(Vec2::new(105.0, 312.0), -1);
    p.update(&r, Input::NONE);
    p.update(&r, LEFT);
    assert_eq!(p.animation(), Animation::StandOnBeam);
}

#[test]
fn hang_from_beam_up_press_switches_to_the_pole() {
    let r = Grid::new(12, 30)
        .set(5, 2..=28, b'|')
        .set(4, 15..=15, b'=')
        .set(5, 15..=15, b'+')
        .room();
    let mut p = Player::upright_at(Vec2::new(103.0, 300.0));
    p.animation = Animation::HangFromBeam;
    p.update(&r, Input::NONE);
    p.update(&r, UP);
    // Up edge in a tile with a pole: ClimbOnBeam, facing -1 (head x < 110).
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.flip_direction(), -1);
}

#[test]
fn down_with_feet_on_the_floor_stands_up() {
    let r = tall_pole();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 100.0), 1);
    p.run_until(&r, DOWN, Animation::StandUp, 60);
    assert!(p.body.chunks[HIPS].contact().y < 0);
}

// ---- jumping off a pole --------------------------------------------------------------

#[test]
fn pole_jump_velocities() {
    let r = tall_pole();

    // x held: velocities set from the facing.
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.input[0] = Input::new(1, 0, true);
    p.jump(&r);
    assert_eq!(p.animation(), Animation::None);
    assert_vec(p.body.chunks[HEAD].vel, 6.0, 8.0);
    assert_vec(p.body.chunks[HIPS].vel, 5.0, 7.0);

    // No x, no up: drop, head vel.y 2 and vel.x 2 * facing.
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.input[0] = JUMP;
    p.jump(&r);
    assert_eq!(p.animation(), Animation::None);
    assert_vec(p.body.chunks[HEAD].vel, 2.0, 2.0);
    assert_vec(p.body.chunks[HIPS].vel, 0.0, 0.0);

    // Down held: no x push.
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.input[0] = Input::new(0, -1, true);
    p.jump(&r);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 2.0);

    // Up held: the slide-up hop, still on the pole.
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.input[0] = Input::new(0, 1, true);
    p.jump(&r);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.slide_up_pole, 17);
    assert_vec(p.body.chunks[HIPS].pos, 115.0, 304.5);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 2.0);
    // Not again while slowed.
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.slow_movement_stun = 1;
    p.input[0] = Input::new(0, 1, true);
    p.jump(&r);
    assert_eq!(p.slide_up_pole, 0);
    assert_vec(p.body.chunks[HIPS].pos, 115.0, 300.0);
}

#[test]
fn pole_jump_fires_one_tick_after_the_press() {
    let r = tall_pole();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.update(&r, RIGHT);
    let jump_right = Input::new(1, 0, true);
    p.update(&r, jump_right);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    p.update(&r, jump_right);
    // Dispatch is the last thing in the tick: the set velocities are untouched.
    assert_eq!(p.animation(), Animation::None);
    assert_vec(p.body.chunks[HEAD].vel, 6.0, 8.0);
    assert_vec(p.body.chunks[HIPS].vel, 5.0, 7.0);
}

#[test]
fn slide_up_hop_lifts_then_slows() {
    let r = tall_pole();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 300.0), 1);
    p.slide_up_pole = 17;
    p.climb_on_beam(&r);
    // 17 -> 16: > 14 so both pos.y += 2; head += LerpMap(16, 17, 0, 3, -1.2, 0.45),
    // hips += LerpMap(16, 17, 0, 1.5, -1.4, 0.45); t = (1 / 17)^0.45.
    let t = (1.0_f32 / 17.0).powf(0.45);
    assert_eq!(p.slide_up_pole, 16);
    assert_close(p.body.chunks[HIPS].pos.y, 302.0, "hips y");
    assert_close(p.body.chunks[HEAD].vel.y, 1.9 + 3.0 - 4.2 * t, "head");
    assert_close(p.body.chunks[HIPS].vel.y, -0.1 + 1.5 - 2.9 * t, "hips");
    for _ in 0..15 {
        p.climb_on_beam(&r);
    }
    // Lifted only on the ticks that leave 16 and 15.
    assert_eq!(p.slide_up_pole, 1);
    assert_close(p.body.chunks[HIPS].pos.y, 304.0, "hips y");
    assert_eq!(p.slow_movement_stun, 0);
    p.climb_on_beam(&r);
    assert_eq!(p.slide_up_pole, 0);
    assert_eq!(p.slow_movement_stun, 16);
    // Slowed climbing: up adds LerpMap(16, 0, 10, 1, 0.2) = 0.2.
    p.input[0] = UP;
    p.body.chunks[HEAD].vel.y = 0.0;
    p.climb_on_beam(&r);
    assert_close(p.body.chunks[HEAD].vel.y, 0.2 + 1.9, "slowed climb");
    // Leaving the pole clears the slide.
    p.slide_up_pole = 5;
    p.animation = Animation::None;
    p.update_animation(&r);
    assert_eq!(p.slide_up_pole, 0);
}

// ---- pole top ----------------------------------------------------------------------

/// Pole in column 5 from row 2 to row 15; open above it.
fn short_pole() -> Room {
    Grid::new(12, 30).set(5, 2..=15, b'|').room()
}

#[test]
fn climbing_off_the_top_stands_on_the_tip() {
    let r = short_pole();
    let mut p = Player::on_pole_at(Vec2::new(115.0, 200.0), 1);
    // Head leaves the top tile (row 15) with the pole below it: GetUpToBeamTip.
    p.run_until(&r, UP, Animation::GetUpToBeamTip, 100);
    assert_eq!(p.tile_pos(HEAD), (5, 16));
    // Pulled until the hips are within 6 of the middle of (5, 16).
    p.run_until(&r, UP, Animation::BeamTip, 100);
    assert!(p.body.chunks[HIPS].pos.distance(Vec2::new(110.0, 330.0)) < 6.0);
    p.run(&r, Input::NONE, 40);
    assert_eq!(p.animation(), Animation::BeamTip);
    assert_eq!(p.body_mode(), BodyMode::ClimbingOnBeam);
    assert_eq!(p.can_jump, 5);
    // The hips are halved toward the tile middle every tick against gravity (they settle
    // a little below it) while the head balances above them.
    assert!(p.body.chunks[HIPS].pos.distance(Vec2::new(110.0, 330.0)) < 1.0);
    assert!(p.body.chunks[HEAD].pos.y > 340.0);

    // A down edge steps off.
    let mut q = p.clone();
    q.update(&r, DOWN);
    assert_eq!(q.animation(), Animation::None);

    // An up edge on the tip: the block's own up exit is undone by the trigger in the
    // same tick, leaving hips vel 0 and canJump 0.
    let mut q = p.clone();
    q.update(&r, UP);
    assert_eq!(q.animation(), Animation::BeamTip);
    assert_eq!(q.can_jump, 0);
    assert_vec(q.body.chunks[HIPS].vel, 0.0, 0.0);

    // Jump: an ordinary standing jump, one tick after the press.
    let mut q = p.clone();
    q.update(&r, JUMP);
    q.update(&r, JUMP);
    assert_eq!(q.animation(), Animation::None);
    assert_close(q.body.chunks[HEAD].vel.y, 4.0, "head");
    assert_close(q.body.chunks[HIPS].vel.y, 3.0, "hips");
    assert_eq!(q.jump_boost, 8.0);
}

#[test]
fn up_press_above_a_pole_top_lands_on_the_tip() {
    let r = short_pole();
    let mut p = Player::upright_at(Vec2::new(110.0, 333.0));
    p.body.chunks[HIPS].vel = Vec2::new(1.0, -2.0);
    p.update(&r, UP);
    // Hips in (5, 16), no pole there, pole in the tile 20 px below: BeamTip, hips
    // vel := 0. The body mode this tick is still Default (the animation pass already
    // ran), and standing it adds -4 to the hips.
    assert_eq!(p.animation(), Animation::BeamTip);
    assert_vec(p.body.chunks[HIPS].vel, 0.0, -4.0);
    // Without the edge nothing happens.
    let mut p = Player::upright_at(Vec2::new(110.0, 333.0));
    p.input[0] = UP;
    p.update(&r, UP);
    assert_eq!(p.animation(), Animation::None);
}

// ---- hanging under a pole ----------------------------------------------------------

/// Pole in column 5 from row 16 to row 28; open below it.
fn hanging_pole() -> Room {
    Grid::new(12, 30).set(5, 16..=28, b'|').room()
}

impl Player {
    /// Hanging under the pole of [`hanging_pole`], head at (110, 315).
    fn hanging_under() -> Self {
        let mut p = Self::upright_at(Vec2::new(110.0, 298.0));
        p.animation = Animation::HangUnderVerticalBeam;
        p.body_mode = BodyMode::ClimbingOnBeam;
        p
    }
}

#[test]
fn hang_under_jumps() {
    let r = hanging_pole();
    // No x: head y + 16, head vel.y = 10, standing.
    let mut p = Player::hanging_under();
    p.input[0] = JUMP;
    p.hang_under_vertical_beam(&r);
    assert_eq!(p.animation(), Animation::None);
    assert!(p.standing);
    assert_vec(p.body.chunks[HEAD].pos, 110.0, 331.0);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 10.0);
    // With x: the hang's swing adds x to the hips first, then the jump adds hips
    // (2x, 4) and head (3x, 6) on top of the zeroed head.
    let mut p = Player::hanging_under();
    p.input[0] = Input::new(1, 0, true);
    p.hang_under_vertical_beam(&r);
    assert_vec(p.body.chunks[HIPS].vel, 3.0, 4.0);
    assert_vec(p.body.chunks[HEAD].vel, 3.0, 6.0);
}

#[test]
fn hang_under_exits() {
    let r = hanging_pole();
    let mut p = Player::hanging_under();
    p.body.chunks[HIPS].vel = Vec2::new(10.5, 0.0);
    p.hang_under_vertical_beam(&r);
    assert_eq!(p.animation(), Animation::None);
    assert!(p.standing);

    let mut p = Player::hanging_under();
    p.input[0] = DOWN;
    p.hang_under_vertical_beam(&r);
    assert_eq!(p.animation(), Animation::None);

    // Holding up climbs into the pole tile.
    let mut p = Player::hanging_under();
    p.run_until(&r, UP, Animation::ClimbOnBeam, 20);
    assert!(r.tile(5, p.tile_pos(HEAD).1).vertical_pole());
}

// ---- ledge grab --------------------------------------------------------------------

/// A wall from column 10 rightward, Solid up to row 9 (top at y = 200); open left of it
/// down to the floor.
fn ledge_grid() -> Grid {
    let mut g = Grid::new(20, 20);
    for x in 10..20 {
        g = g.set(x, 2..=9, b'#');
    }
    g
}

impl Player {
    /// Moving right into the wall below its top, head at (189, `head_y`).
    fn at_wall(head_y: f32) -> Self {
        let mut p = Self::upright_at(Vec2::new(189.0, head_y - 17.0));
        for c in &mut p.body.chunks {
            c.vel.x = 3.0;
        }
        p
    }
}

#[test]
fn ledge_grab_triggers_on_a_wall_top_at_head_height() {
    let r = ledge_grid().room();
    let mut p = Player::at_wall(196.0);
    p.update(&r, RIGHT);
    // Head hits the wall (contact x = 1 = input), y 195.1 > 190 (middle of row 9),
    // WallClimb; (8, 9) open, (9, 7) open, (10, 10) open.
    assert_eq!(p.animation(), Animation::LedgeGrab);
    assert_eq!(p.body_mode(), BodyMode::Default);
}

#[test]
fn ledge_grab_negatives() {
    // Head below the tile middle.
    let mut p = Player::at_wall(189.0);
    p.update(&ledge_grid().room(), RIGHT);
    assert_eq!(p.body_mode(), BodyMode::WallClimb);
    assert_eq!(p.animation(), Animation::None);
    // Wall continues above the head.
    let mut p = Player::at_wall(196.0);
    p.update(&ledge_grid().set(10, 10..=10, b'#').room(), RIGHT);
    assert_eq!(p.animation(), Animation::None);
    // Solid two tiles under the head.
    let mut p = Player::at_wall(196.0);
    p.update(&ledge_grid().set(9, 2..=7, b'#').room(), RIGHT);
    assert_eq!(p.animation(), Animation::None);
    // A one-way floor there counts as solid too.
    let mut p = Player::at_wall(196.0);
    p.update(&ledge_grid().set(9, 7..=7, b'-').room(), RIGHT);
    assert_eq!(p.animation(), Animation::None);
    // Solid behind the head.
    let mut p = Player::at_wall(196.0);
    p.update(&ledge_grid().set(8, 9..=9, b'#').room(), RIGHT);
    assert_eq!(p.animation(), Animation::None);
    // No x input.
    let mut p = Player::at_wall(196.0);
    p.update(&ledge_grid().room(), Input::NONE);
    assert_eq!(p.animation(), Animation::None);
}

impl Player {
    /// In `LedgeGrab` facing the wall of [`ledge_grid`], counter 2.
    fn grabbing_ledge() -> Self {
        let mut p = Self::new();
        p.body.chunks[HEAD].reset_at(Vec2::new(191.0, 195.0));
        p.body.chunks[HEAD].vel = Vec2::new(2.0, -1.0);
        p.body.chunks[HIPS].reset_at(Vec2::new(189.0, 178.0));
        p.animation = Animation::LedgeGrab;
        p.ledge_grab_counter = 2;
        p
    }
}

#[test]
fn ledge_grab_block_pulls_the_head_up_and_toward_the_wall() {
    let r = ledge_grid().room();
    let mut p = Player::grabbing_ledge();
    p.input[0] = RIGHT;
    p.ledge_grab(&r);
    // head vel * 0.5; head := (pos + (190, 190) + (2, 10)) / 2, lastPos too; hips vel.x
    // += 1, then holding toward the wall: counter 3, hips vel += (-0.5, -0.5).
    assert_vec(p.body.chunks[HEAD].vel, 1.0, -0.5);
    assert_vec(p.body.chunks[HEAD].pos, 191.5, 197.5);
    assert_eq!(p.body.chunks[HEAD].last_pos, p.body.chunks[HEAD].pos);
    assert_vec(p.body.chunks[HIPS].vel, 0.5, -0.5);
    assert_eq!(p.ledge_grab_counter, 3);
    assert_eq!(p.can_jump, 1);
    assert!(p.standing);
    assert_eq!(p.animation(), Animation::LedgeGrab);

    // Down edge: let go 10 px lower, counter 0, the edge consumed in the history.
    let mut p = Player::grabbing_ledge();
    p.input[0] = DOWN;
    p.ledge_grab(&r);
    assert_vec(p.body.chunks[HEAD].pos, 191.5, 187.5);
    assert_eq!(p.animation(), Animation::None);
    assert_eq!(p.ledge_grab_counter, 0);
    assert_eq!(p.input[1].y, -1);
    assert!(p.standing);

    // Away edge (reachable only while jumpStun holds the facing): kick up by 10.
    let mut p = Player::grabbing_ledge();
    p.input[0] = LEFT;
    p.ledge_grab(&r);
    assert_close(p.body.chunks[HEAD].vel.y, 9.5, "kick");
    assert_eq!(p.animation(), Animation::None);

    // Geometry gone (facing away from the wall): ends, counter 0.
    let mut p = Player::grabbing_ledge();
    p.flip_direction = -1;
    p.ledge_grab(&r);
    assert_eq!(p.animation(), Animation::None);
    assert_eq!(p.ledge_grab_counter, 0);
}

#[test]
fn ledge_grab_jumps() {
    let r = ledge_grid().room();
    let mut grabbed = Player::at_wall(196.0);
    grabbed.update(&r, RIGHT);
    assert_eq!(grabbed.animation(), Animation::LedgeGrab);

    // x toward the wall: WallJump(-x) in its hop variant (head on the wall, open above
    // it), airborne x0.7: vel.y 5.6 / 4.9, pos.y + 7, x untouched, still grabbing.
    let mut p = grabbed.clone();
    let before = [p.body.chunks[HEAD].clone(), p.body.chunks[HIPS].clone()];
    p.input[0] = Input::new(1, 0, true);
    p.jump(&r);
    for (i, b) in [HEAD, HIPS].into_iter().zip(before) {
        let c = &p.body.chunks[i];
        assert_close(c.vel.y, if i == HEAD { 5.6 } else { 4.9 }, "vel.y");
        assert_eq!(c.vel.x, b.vel.x);
        assert_close(c.pos.y, b.pos.y + 7.0, "pos.y");
    }
    assert_eq!(p.jump_boost, 0.0);
    assert_eq!(p.animation(), Animation::LedgeGrab);

    // No x: nothing at all.
    let mut p = grabbed.clone();
    p.input[0] = JUMP;
    p.jump(&r);
    assert_eq!(p.body, grabbed.body);
}

#[test]
fn holding_toward_the_wall_climbs_over_the_top() {
    let r = ledge_grid().room();
    let mut p = Player::at_wall(196.0);
    p.update(&r, RIGHT);
    assert_eq!(p.animation(), Animation::LedgeGrab);
    // The grab hauls the head over the top (4 ticks), then ground classification finds
    // the head on the wall top with the hips against the wall: LedgeCrawl, then StandUp.
    let mut ticks = 0;
    let mut crawled = false;
    while !(p.body_mode() == BodyMode::Stand && p.body.chunks[HIPS].pos.y > 200.0) {
        p.update(&r, RIGHT);
        crawled |= p.animation() == Animation::LedgeCrawl;
        ticks += 1;
        assert!(ticks < 80, "never got up: {:?}", p.chunks());
    }
    // On top of the wall: both chunks over column 10+, above its top.
    assert!(p.body.chunks[HIPS].pos.x > 200.0);
    assert!(p.body.chunks[HEAD].pos.y > p.body.chunks[HIPS].pos.y);
    assert!(crawled);
    // The counter is only reset by the grab's own exits; this one ended by LedgeCrawl.
    assert_eq!(p.ledge_grab_counter, 4);
}

#[test]
fn ledge_jump_without_wall_contact_kicks_off_the_wall() {
    let r = ledge_grid().room();
    let mut p = Player::at_wall(196.0);
    p.update(&r, RIGHT);
    let jump_right = Input::new(1, 0, true);
    p.update(&r, jump_right);
    p.update(&r, jump_right);
    // The press is dispatched on this tick (canJump = 1 from the grab block). The block
    // pulls the head toward x 190 + counter, short of touching the wall (head x 191),
    // and this tick's physics did not push it in: no contact, so WallJump's ledge flag
    // is false; no footing below either: the wall kick (-6, 8) / (-5, 7), jumpStun -8.
    assert_eq!(p.body.chunks[HEAD].contact().x, 0);
    assert_vec(p.body.chunks[HEAD].vel, -6.0, 8.0);
    assert_vec(p.body.chunks[HIPS].vel, -5.0, 7.0);
    assert_eq!(p.jump_stun, -8);
}
