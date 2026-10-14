//! Corridors. Expected values are derived by hand from `rainworld-re/notes/corridors.md`
//! (derivation next to each assertion). Tile `(x, y)` has its middle at
//! `(20 x + 10, 20 y + 10)`.

use rw_assets::Room;

use super::poles::Grid;
use super::{DRAG, G, JUMP, assert_close, assert_vec};
use crate::player::{Animation, BodyMode, HEAD, HIPS, Input, Player};
use crate::{ConnectionKind, Vec2};

const LEFT: Input = Input::new(-1, 0, false);
const RIGHT: Input = Input::new(1, 0, false);
const UP: Input = Input::new(0, 1, false);
const DOWN: Input = Input::new(0, -1, false);

/// Damping times air drag: the factor between one tick's end velocity and the next.
const A: f32 = 0.75 * DRAG;
/// Climb push of a chunk plus the head's extra tenth, shared by both chunks:
/// `1.2 * 1.1 / 2`.
const PUSH_MEAN: f32 = 0.66;

/// Steady corridor speed in px/tick: the mean end velocity solves `m = A m + 0.66`,
/// and physics moves by `0.999 m` (about 2.6295; the notes' 2.64 omits the drag).
fn steady_speed() -> f32 {
    DRAG * PUSH_MEAN / (1.0 - A)
}

/// `w` x `h`, all Solid; carve with `row` and `set`.
fn solid_room(w: usize, h: usize) -> Grid {
    let mut g = Grid::new(w, h);
    for y in 2..h {
        g = g.row(y, 0..=w - 1, b'#');
    }
    g
}

/// A one-tile tunnel in row 5 from column 3 to 26, closed at both ends.
fn tunnel_room() -> Room {
    solid_room(30, 12).row(5, 3..=26, b'.').room()
}

/// A one-tile shaft in column 5 from row 2 (on the Solid floor) to row 27.
fn shaft_room() -> Room {
    solid_room(12, 30).set(5, 2..=27, b'.').room()
}

/// A tunnel along the floor in row 2 (columns 1-19, Solid above) opening into a room
/// (columns 20-28, rows 2-12).
fn mouth_room() -> Room {
    let mut g = solid_room(30, 14).row(2, 1..=19, b'.');
    for y in 2..=12 {
        g = g.row(y, 20..=28, b'.');
    }
    g.room()
}

impl Player {
    /// In `CorridorClimb` at rest: both chunks keep the end-of-tick `vel.y = 0.9` that the
    /// next physics step's gravity cancels exactly.
    fn in_corridor_at(head: Vec2, hips: Vec2) -> Self {
        let mut p = Self::new();
        p.body.chunks[HEAD].reset_at(head);
        p.body.chunks[HIPS].reset_at(hips);
        for c in &mut p.body.chunks {
            c.vel.y = G;
        }
        p.body_mode = BodyMode::CorridorClimb;
        p.go_into_corridor_climb = 3;
        p
    }

    fn mean_vel(&self) -> Vec2 {
        (self.body.chunks[HEAD].vel + self.body.chunks[HIPS].vel) / 2.0
    }

    fn mean_pos(&self) -> Vec2 {
        (self.body.chunks[HEAD].pos + self.body.chunks[HIPS].pos) / 2.0
    }

    fn in_corridor_now(&self, room: &Room) -> bool {
        self.in_corridor(room, self.corridor_pairs(room))
    }
}

// ---- detection --------------------------------------------------------------------

#[test]
fn corridor_pairs_count_opposite_solid_neighbours() {
    // In a tunnel each chunk has S/N, SW/NE and SE/NW solid: 3 pairs each.
    let r = tunnel_room();
    let p = Player::in_corridor_at(Vec2::new(127.0, 110.0), Vec2::new(110.0, 110.0));
    assert_eq!(p.corridor_pairs(&r), 6);
    assert!(p.in_corridor_now(&r));
    // In a shaft: W/E, SW/NE and SE/NW.
    let r = shaft_room();
    let p = Player::in_corridor_at(Vec2::new(110.0, 227.0), Vec2::new(110.0, 210.0));
    assert_eq!(p.corridor_pairs(&r), 6);

    // Single blocks at (10, 6) and (10, 4) around the head in (10, 5): only S/N, one
    // pair; the hips in (11, 5) have none. One pair that is not left/right: no corridor.
    let r = Grid::new(20, 10)
        .set(10, 6..=6, b'#')
        .set(10, 4..=4, b'#')
        .room();
    let p = Player::in_corridor_at(Vec2::new(210.0, 110.0), Vec2::new(227.0, 110.0));
    assert_eq!(p.corridor_pairs(&r), 1);
    assert!(!p.in_corridor_now(&r));
    // Blocks left and right of the head instead: still one pair, but W/E counts alone.
    let r = Grid::new(20, 10)
        .set(9, 5..=5, b'#')
        .set(11, 5..=5, b'#')
        .room();
    let p = Player::in_corridor_at(Vec2::new(210.0, 110.0), Vec2::new(210.0, 93.0));
    assert_eq!(p.corridor_pairs(&r), 1);
    assert!(p.in_corridor_now(&r));
}

#[test]
fn corridor_mode_needs_three_ticks_and_leaves_at_once() {
    let r = mouth_room();
    // Lying on the floor of the open part, head left toward the tunnel mouth.
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(520.0, 48.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(503.0, 49.0));
    p.run(&r, Input::NONE, 10);
    let mut prev = (p.body_mode, p.can_jump, Vec2::ZERO);
    let mut ticks = 0;
    while p.go_into_corridor_climb == 0 {
        let xs = Vec2::new(p.body.chunks[HEAD].pos.x, p.body.chunks[HIPS].pos.x);
        prev = (p.body_mode, p.can_jump, xs);
        p.update(&r, LEFT);
        ticks += 1;
        assert!(ticks < 100, "never entered");
    }
    // Column 20 has only SE/NW (one pair), column 19 also S/N, column 21 on none: two
    // pairs need the head in column 19 or both chunks in column 20, so the count
    // starts once the head is left of x = 400 or the hips left of x = 420.
    let (head, hips) = (p.body.chunks[HEAD].pos.x, p.body.chunks[HIPS].pos.x);
    assert!(head < 400.0 || hips < 420.0, "{head} {hips}");
    assert!(prev.2.x >= 400.0 && prev.2.y >= 420.0, "{:?}", prev.2);
    assert!(matches!(prev.0, BodyMode::Crawl | BodyMode::Stand));
    assert_eq!(prev.1, 5);
    // Ticks 1 and 2: the old mode stays and canJump is not refreshed (5 -> 4 -> 3).
    assert_eq!(p.body_mode, prev.0);
    assert_eq!(p.can_jump, 4);
    p.update(&r, LEFT);
    assert_eq!(p.go_into_corridor_climb, 2);
    assert_eq!(p.body_mode, prev.0);
    assert_eq!(p.can_jump, 3);
    p.update(&r, LEFT);
    assert_eq!(p.go_into_corridor_climb, 3);
    assert_eq!(p.body_mode, BodyMode::CorridorClimb);
    assert_eq!(p.animation, Animation::None);
    assert_eq!(p.can_jump, 2);
    p.run(&r, LEFT, 20);
    assert_eq!(p.body_mode, BodyMode::CorridorClimb);
    assert_eq!(p.time_since_in_corridor_mode, 0);

    // Back out tail first: the mode ends on the first tick with at most one pair, the
    // same line crossed the other way.
    let mut ticks = 0;
    loop {
        let before = (p.body.chunks[HEAD].pos.x, p.body.chunks[HIPS].pos.x);
        p.update(&r, RIGHT);
        let (head, hips) = (p.body.chunks[HEAD].pos.x, p.body.chunks[HIPS].pos.x);
        if p.body_mode != BodyMode::CorridorClimb {
            assert!(head >= 400.0 && hips >= 420.0, "{head} {hips}");
            assert!(before.0 < 400.0 || before.1 < 420.0, "{before:?}");
            break;
        }
        ticks += 1;
        assert!(ticks < 100, "never left");
    }
    // Classification ran the same tick (Default: the corridor mode held the body just
    // off the floor).
    assert_eq!(p.go_into_corridor_climb, 0);
    assert!(matches!(
        p.body_mode,
        BodyMode::Default | BodyMode::Crawl | BodyMode::Stand
    ));
}

// ---- climbing ---------------------------------------------------------------------

#[test]
fn tunnel_push_damping_and_steady_speed() {
    let r = tunnel_room();
    let mut p = Player::in_corridor_at(Vec2::new(127.0, 110.0), Vec2::new(110.0, 110.0));
    p.update(&r, RIGHT);
    // Physics leaves everything still. Damping keeps vel 0 and the gravity term gives
    // vel.y 0.9; the head (open tile ahead) gets +1.2 in x and is centred,
    // 0.9 * 0.8 - 0 * 0.2 = 0.72, and so are the hips (y not held); then the head's
    // extra 0.12.
    assert_vec(p.body.chunks[HEAD].vel, 1.32, 0.72);
    assert_vec(p.body.chunks[HIPS].vel, 0.0, 0.72);
    // The rod conserves the mean, so the mean x velocity follows m' = A m + 0.66.
    let mut m = 0.66;
    for _ in 0..30 {
        p.update(&r, RIGHT);
        m = A * m + PUSH_MEAN;
        assert_close(p.mean_vel().x, m, "mean vel.x");
    }
    let x0 = p.mean_pos().x;
    p.run(&r, RIGHT, 10);
    let speed = (p.mean_pos().x - x0) / 10.0;
    assert_close(steady_speed(), 2.629_471, "steady");
    assert!((speed - steady_speed()).abs() < 1e-3, "speed {speed}");
    assert_eq!(p.body_mode, BodyMode::CorridorClimb);
}

#[test]
fn shaft_clings_without_input_and_climbs_at_the_same_speed() {
    let r = shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 227.0), Vec2::new(110.0, 210.0));
    // End vel.y 0.9 - 0.9 gravity = 0; then 0 * 0.75 + 0.9 again: no movement at all.
    p.run(&r, Input::NONE, 20);
    assert_vec(p.body.chunks[HIPS].pos, 110.0, 210.0);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, G);
    // Up: the same momentum balance as in a tunnel, the gravity term cancelling.
    p.run(&r, UP, 40);
    let y0 = p.mean_pos().y;
    p.run(&r, UP, 10);
    let speed = (p.mean_pos().y - y0) / 10.0;
    assert!((speed - steady_speed()).abs() < 1e-3, "speed {speed}");
}

// ---- corridor jumps ---------------------------------------------------------------

const UP_JUMP: Input = Input::new(0, 1, true);

#[test]
fn shaft_shoot_success_from_the_bottom() {
    let r = shaft_room();
    // Hips in the bottom tile (5, 2) with Solid (5, 1) below: canCorridorJump = 5.
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 67.0), Vec2::new(110.0, 50.0));
    p.update(&r, UP_JUMP);
    // +15 / +10, then slide damping 0.9 (counter 22 >= 10) and gravity term
    // 0.9 * lerp(1, 0.2, 1) = 0.18: 13.68 / 9.18. Up held: the head's push with
    // slowMovementStun 2 is 1.2 * lerp(0.1, 1, 0.8) = 0.984, plus 0.0984.
    assert_close(p.body.chunks[HEAD].vel.y, 13.68 + 0.984 + 0.0984, "head");
    assert_close(p.body.chunks[HIPS].vel.y, 9.18, "hips");
    assert_eq!(p.shoot_up_counter, 30);
    assert_eq!(p.vertical_corridor_slide_counter, 22);
    assert_eq!(p.slow_movement_stun, 2);
    assert_eq!(p.can_corridor_jump, 0);
    // Holding up and jump while rising keeps the shoot-up counter and the grab block.
    p.update(&r, UP_JUMP);
    assert_eq!(p.shoot_up_counter, 29);
    assert_eq!(p.no_grab_counter, 1);
    // Released: cleared (it sees last tick's input, so one tick later).
    p.update(&r, UP);
    p.update(&r, UP);
    assert_eq!(p.shoot_up_counter, 0);
}

#[test]
fn shaft_shoot_fail_midway_and_slide_recovery() {
    let r = shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 227.0), Vec2::new(110.0, 210.0));
    p.update(&r, JUMP);
    // Same impulse and damping, no push: 13.68 / 9.18.
    assert_close(p.body.chunks[HEAD].vel.y, 13.68, "head");
    assert_close(p.body.chunks[HIPS].vel.y, 9.18, "hips");
    assert_eq!(p.shoot_up_counter, 0);
    assert_eq!(p.vertical_corridor_slide_counter, 34);
    assert_eq!(p.slow_movement_stun, 18);
    // Stun 18 runs out on tick 18. On tick 20 the slide counter (14) still blocks a
    // new shoot.
    p.run(&r, Input::NONE, 19);
    assert_eq!(p.slow_movement_stun, 0);
    p.update(&r, JUMP);
    assert_eq!(p.vertical_corridor_slide_counter, 14);
    assert_eq!(p.slow_movement_stun, 0);
    // The counter reaches 1 on tick 33: slowMovementStun = 15.
    p.run(&r, Input::NONE, 12);
    assert_eq!(p.vertical_corridor_slide_counter, 2);
    assert_eq!(p.slow_movement_stun, 0);
    p.update(&r, Input::NONE);
    assert_eq!(p.body_mode, BodyMode::CorridorClimb);
    assert_eq!(p.vertical_corridor_slide_counter, 1);
    assert_eq!(p.slow_movement_stun, 15);
}

#[test]
fn tunnel_slide_success_against_the_back_wall() {
    let r = tunnel_room();
    // Hips in (3, 5) against Solid (2, 5), head free ahead, both 2 px above the middle.
    let mut p = Player::in_corridor_at(Vec2::new(87.0, 112.0), Vec2::new(70.0, 112.0));
    p.update(&r, Input::new(1, 0, true));
    // +12 px, y snapped to the head tile's middle, +7 vel.x, x 0.9 slide damping: 6.3;
    // vel.y 0 * 0.9 + 0.9, centred 0.72 on both. The head's push with stun 5:
    // 1.2 * lerp(0.1, 1, 0.5) = 0.66, plus 0.066.
    assert_vec(p.body.chunks[HEAD].pos, 99.0, 110.0);
    assert_vec(p.body.chunks[HIPS].pos, 82.0, 110.0);
    assert_vec(p.body.chunks[HEAD].vel, 6.3 + 0.66 + 0.066, 0.72);
    assert_vec(p.body.chunks[HIPS].vel, 6.3, 0.72);
    assert_eq!(p.horizontal_corridor_slide_counter, 25);
    assert_eq!(p.slow_movement_stun, 5);
}

#[test]
fn tunnel_slide_fail_without_direction() {
    let r = tunnel_room();
    let mut p = Player::in_corridor_at(Vec2::new(87.0, 112.0), Vec2::new(70.0, 112.0));
    p.update(&r, JUMP);
    // No x: the fail push +6 / +4, x 0.9: 5.4 / 3.6; stun 15 leaves a 0.12 push for
    // no input. No snap.
    assert_vec(p.body.chunks[HEAD].vel, 5.4, G);
    assert_vec(p.body.chunks[HIPS].vel, 3.6, G);
    assert_vec(p.body.chunks[HEAD].pos, 87.0, 112.0);
    assert_eq!(p.horizontal_corridor_slide_counter, 15);
    assert_eq!(p.slow_movement_stun, 15);
}

#[test]
fn jump_dispatch_in_corridor_mode_sets_6_and_5() {
    let r = shaft_room();
    // Head down mid-shaft with coyote time left: the press does nothing in the corridor
    // block (no up for a turn, head not up for a shoot); a tick later `Jump` runs.
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 193.0), Vec2::new(110.0, 210.0));
    p.can_jump = 5;
    p.update(&r, JUMP);
    assert_eq!(p.vertical_corridor_slide_counter, 0);
    p.update(&r, JUMP);
    assert_close(p.body.chunks[HEAD].vel.y, 6.0, "head");
    assert_close(p.body.chunks[HIPS].vel.y, 5.0, "hips");
    assert!(p.standing);
    assert_close(p.jump_boost, 8.0, "boost");
}

// ---- turning and dropping ---------------------------------------------------------

#[test]
fn corridor_turn_takes_40_ticks_with_a_pull_rod() {
    let r = tunnel_room();
    let mut p = Player::in_corridor_at(Vec2::new(227.0, 110.0), Vec2::new(210.0, 110.0));
    // Jump with x pointing away from the head: turn toward (-1, 0).
    p.update(&r, Input::new(-1, 0, true));
    assert_eq!(p.corridor_turn_dir, Some((-1, 0)));
    assert_eq!(p.animation, Animation::None);
    assert_eq!(p.body.connections[0].kind, ConnectionKind::Pull);
    for tick in 1..=40 {
        p.update(&r, LEFT);
        assert_eq!(p.animation, Animation::CorridorTurn, "tick {tick}");
        assert_eq!(p.corridor_turn_counter, tick);
        assert_eq!(p.slow_movement_stun, 10);
    }
    // Tick 41 pushes out (head 6, hips 5 toward -x) and ends the turn.
    p.update(&r, LEFT);
    assert_eq!(p.animation, Animation::None);
    assert_eq!(p.corridor_turn_dir, None);
    assert_eq!(p.body.connections[0].kind, ConnectionKind::Normal);
    assert_eq!(p.slow_movement_stun, 9);
    // The ends have swapped (with the port's jiggle-free turn, docs/DIVERGENCES.md D10).
    p.run(&r, LEFT, 9);
    assert!(p.body.chunks[HEAD].pos.x < p.body.chunks[HIPS].pos.x);
    assert_eq!(p.slow_movement_stun, 0);

    // In a shaft head down, jump with up turns toward (0, 1).
    let r = shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 193.0), Vec2::new(110.0, 210.0));
    p.update(&r, UP_JUMP);
    assert_eq!(p.corridor_turn_dir, Some((0, 1)));
}

/// Shaft in column 5 from row 10 to 15 (Solid row 16 above), over an open room
/// (columns 1-10, rows 2-9) with a pole in column 5 rows 3-8.
fn shaft_over_pole_room() -> Room {
    let mut g = solid_room(12, 18).set(5, 10..=15, b'.');
    for y in 2..=9 {
        g = g.row(y, 1..=10, b'.');
    }
    g.set(5, 3..=8, b'|').room()
}

#[test]
fn corridor_drop_falls_in_default_mode() {
    let r = shaft_over_pole_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 253.0), Vec2::new(110.0, 270.0));
    p.update(&r, Input::new(0, -1, true));
    assert!(p.corridor_drop);
    assert_eq!(p.can_corridor_jump, 0);
    // Next tick still wedged (counter > 2) but Default, falling.
    p.update(&r, DOWN);
    assert_eq!(p.body_mode, BodyMode::Default);
    assert!(p.go_into_corridor_climb > 2);
    assert!(p.corridor_drop);
    // Releasing down ends the drop; the mode returns a tick later.
    p.update(&r, Input::NONE);
    assert!(!p.corridor_drop);
    assert_eq!(p.body_mode, BodyMode::Default);
    p.update(&r, Input::NONE);
    assert_eq!(p.body_mode, BodyMode::CorridorClimb);
}

#[test]
fn crawling_head_first_out_of_a_shaft_catches_the_pole_below() {
    let r = shaft_over_pole_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 273.0), Vec2::new(110.0, 290.0));
    let mut seen_tile = false;
    let mut ticks = 0;
    while p.animation != Animation::ClimbOnBeam {
        p.update(&r, DOWN);
        // With the head in (5, 9), under the shaft's walls, and no beam there, the tile
        // below with the pole is remembered.
        if p.body_mode == BodyMode::CorridorClimb && p.tile_pos(HEAD) == (5, 9) {
            assert_eq!(p.drop_grab_tile, Some((5, 8)));
            seen_tile = true;
        }
        ticks += 1;
        assert!(ticks < 100, "never caught: {:?}", p.body.chunks[HEAD].pos);
    }
    assert!(seen_tile);
    // Caught in Default mode: head snapped to the middle of (5, 8).
    assert_eq!(p.body_mode, BodyMode::Default);
    assert_vec(p.body.chunks[HEAD].pos, 110.0, 170.0);
    assert_eq!(p.drop_grab_tile, None);
    p.run(&r, Input::NONE, 20);
    assert_eq!(p.animation, Animation::ClimbOnBeam);
    assert_eq!(p.body_mode, BodyMode::ClimbingOnBeam);
}

// ---- exits to beams ---------------------------------------------------------------

/// A pole in column 5 from row 2 to 20; a one-tile shaft around it up to row 10, an
/// open room (columns 1-10, rows 11-20) above.
fn pole_shaft_room() -> Room {
    let mut g = solid_room(12, 22).set(5, 2..=10, b'.');
    for y in 11..=20 {
        g = g.row(y, 1..=10, b'.');
    }
    g.set(5, 2..=20, b'|').room()
}

#[test]
fn pole_in_a_shaft_is_climbed_in_corridor_mode_then_grabbed_above() {
    let r = pole_shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 67.0), Vec2::new(110.0, 50.0));
    p.run(&r, UP, 30);
    // Walls on both sides of the head: no pole exit, corridor speed (not the pole's 2.0).
    assert_eq!(p.body_mode, BodyMode::CorridorClimb);
    let y0 = p.mean_pos().y;
    p.run(&r, UP, 5);
    let speed = (p.mean_pos().y - y0) / 5.0;
    assert!((speed - steady_speed()).abs() < 1e-2, "speed {speed}");
    let mut ticks = 0;
    loop {
        p.update(&r, UP);
        let (head, hips) = (p.tile_pos(HEAD).1, p.tile_pos(HIPS).1);
        if head <= 10 {
            assert_eq!(p.body_mode, BodyMode::CorridorClimb);
        } else if hips <= 10 {
            // Head above the walls: the exit fires every tick, the still-wedged hips
            // put the next tick back into corridor mode before the animation runs.
            assert_eq!(p.animation, Animation::ClimbOnBeam);
        } else {
            break;
        }
        ticks += 1;
        assert!(ticks < 100);
    }
    p.run(&r, UP, 10);
    assert_eq!(p.animation, Animation::ClimbOnBeam);
    assert_eq!(p.body_mode, BodyMode::ClimbingOnBeam);
}

#[test]
fn tunnel_onto_a_horizontal_beam() {
    // Tunnel in row 10 (columns 2-9) into a room (columns 10-25, rows 2-20); a horizontal
    // beam in row 10 from column 9 (inside the tunnel's last tile) to 20.
    let mut g = solid_room(28, 22).row(10, 2..=9, b'.');
    for y in 2..=20 {
        g = g.row(y, 10..=25, b'.');
    }
    let r = g.row(10, 9..=20, b'=').room();
    let mut p = Player::in_corridor_at(Vec2::new(87.0, 210.0), Vec2::new(70.0, 210.0));
    let mut ticks = 0;
    while p.animation != Animation::HangFromBeam {
        p.update(&r, RIGHT);
        ticks += 1;
        assert!(ticks < 100, "{:?}", p.body.chunks[HEAD].pos);
    }
    // Head in the beam tile (9, 10) with the hips walled above and below and no floor
    // ahead below the head.
    p.run(&r, RIGHT, 20);
    assert_eq!(p.animation, Animation::HangFromBeam);
    assert_eq!(p.body.chunks[HEAD].pos.y, 210.0);
}
