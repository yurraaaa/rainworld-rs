//! Skid, flip jump, roll, belly slide, rocket jump and the landing roll. Expected
//! values are derived by hand from `rainworld-re/notes/advanced-moves.md` (derivation
//! next to each assertion); chunks are placed directly and single blocks called where a
//! whole tick would mix in other forces.

use std::f32::consts::PI;

use rw_assets::Room;

use super::{DRAG, FLOOR, G, RIGHT, assert_close, assert_vec, flat_room, hit};
use crate::player::{Animation, BodyMode, HEAD, HIPS, Input, Player};
use crate::tests::room;
use crate::{Contact, TerrainImpact, Vec2};

const LEFT: Input = Input::new(-1, 0, false);
const DOWN_RIGHT: Input = Input::new(1, -1, false);
const DOWN: Input = Input::new(0, -1, false);

impl Player {
    /// Lying flat on the flat room's floor (top 40), head to the right, at rest.
    fn lying_on_floor() -> Self {
        let mut p = Self::new();
        p.body.chunks[HIPS].reset_at(Vec2::new(100.0, 48.0));
        p.body.chunks[HEAD].reset_at(Vec2::new(117.0, 49.0));
        p
    }

    /// Belly sliding right at `roll_counter` `n` on the flat room's floor.
    fn sliding(n: i32, input: Input) -> Self {
        let mut p = Self::lying_on_floor();
        p.animation = Animation::BellySlide;
        p.roll_direction = 1;
        p.roll_counter = n;
        p.input[0] = input;
        p
    }

    /// Ready for a landing roll: down-right held long enough, recently airborne.
    fn holding_down_right(mut self, consistent: i32) -> Self {
        self.input[0] = DOWN_RIGHT;
        self.input[1] = DOWN_RIGHT;
        self.consistent_down_diagonal = consistent;
        self.allow_roll = 15;
        self
    }
}

// ---- skid and flip jump (section 1) ---------------------------------------------------

#[test]
fn skid_starts_after_more_than_ten_ticks_running_one_way() {
    for (ticks, skids) in [(11, false), (12, true)] {
        let r = flat_room();
        let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
        // Tick 1 sets slideDirection = 1 (from 0) and zeroes initSlideCounter; ticks
        // 2..=n count it up to n - 1, which must exceed 10.
        p.run(&r, RIGHT, ticks);
        assert_eq!(p.body_mode(), BodyMode::Stand);
        assert_eq!(
            (p.slide_direction, p.init_slide_counter),
            (1, ticks as i32 - 1)
        );
        p.update(&r, LEFT);
        if skids {
            assert_eq!((p.slide_counter, p.slide_direction), (1, 1));
        } else {
            assert_eq!((p.slide_counter, p.slide_direction), (0, -1));
        }
        assert_eq!(p.init_slide_counter, 0);
    }
}

#[test]
fn skid_impulse_drifts_then_brakes_and_ends_with_n_one_half() {
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.slide_direction = 1;
    p.slide_counter = 1;
    p.input[0] = LEFT;
    let (mut head, mut hips) = (0.0_f32, 0.0_f32);
    // Counter 2..=20: n = 0.5 - sin(c / 20 * pi / 2); head += 3.5 n - (n < 0 ? 0.8 :
    // 0.5), hips += 3.5 n + 0.5 (slideDirection 1).
    for c in 2..=20 {
        p.skid();
        assert_eq!(p.slide_counter, c);
        let n = 0.5 - (c as f32 / 20.0 * PI / 2.0).sin();
        head += 3.5 * n - if n < 0.0 { 0.8 } else { 0.5 };
        hips += 3.5 * n + 0.5;
        assert_close(p.body.chunks[HEAD].vel.x, head, &format!("head c {c}"));
        assert_close(p.body.chunks[HIPS].vel.x, hips, &format!("hips c {c}"));
    }
    // Counter 21 ends the skid; the impulse is then computed with counter 0: n = 0.5.
    p.skid();
    assert_eq!(p.slide_counter, 0);
    assert_close(p.body.chunks[HEAD].vel.x, head + 1.25, "head last");
    assert_close(p.body.chunks[HIPS].vel.x, hips + 2.25, "hips last");

    // Releasing the reversed direction also ends it (and pushes once with n = 0.5).
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.slide_direction = 1;
    p.slide_counter = 5;
    p.skid();
    assert_eq!(p.slide_counter, 0);
    assert_close(p.body.chunks[HEAD].vel.x, 1.25, "head released");
}

#[test]
fn flip_jump_within_nine_ticks_of_the_skid() {
    let r = flat_room();
    for (counter, flips) in [(1, true), (9, true), (10, false)] {
        let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
        p.slide_direction = 1;
        p.slide_counter = counter;
        p.body.chunks[HEAD].vel.x = 3.0;
        p.body.chunks[HIPS].vel.x = 2.0;
        p.jump(&r);
        if flips {
            // vel.y = 9 / 7; vel.x halved; head -= slideDirection * 4.
            assert_vec(p.body.chunks[HEAD].vel, 1.5 - 4.0, 9.0);
            assert_vec(p.body.chunks[HIPS].vel, 1.0, 7.0);
            assert_eq!(p.jump_boost, 5.0);
            assert_eq!(p.animation(), Animation::Flip);
            assert_eq!(p.slide_counter, 0);
        } else {
            assert_vec(p.body.chunks[HEAD].vel, 3.0, 4.0);
            assert_vec(p.body.chunks[HIPS].vel, 2.0, 3.0);
            assert_eq!(p.jump_boost, 8.0);
            assert_eq!(p.animation(), Animation::None);
        }
    }
}

#[test]
fn flip_torque_uses_slide_direction() {
    // Body upright: perp(hips - head) = perp((0, -17)) = (1, 0).
    for (sd, from_slide, k) in [
        (1, false, 0.38),
        (-1, false, -0.38),
        (1, true, 0.95),
        (0, true, 0.0),
    ] {
        let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
        p.animation = Animation::Flip;
        p.slide_direction = sd;
        p.flip_from_slide = from_slide;
        p.flip();
        assert_vec(p.body.chunks[HEAD].vel, -k, 0.0);
        assert_vec(p.body.chunks[HIPS].vel, k, 0.0);
        assert!(!p.is_standing());
        assert_eq!(p.animation(), Animation::Flip);
    }
    // Any terrain contact ends it; standing again when the head is above the hips.
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.animation = Animation::Flip;
    p.slide_direction = 1;
    p.update(&r, Input::NONE);
    assert_eq!(p.animation(), Animation::None);
    assert!(p.is_standing());
}

// ---- landing roll (section 5) ---------------------------------------------------------

#[test]
fn landing_roll_needs_a_held_diagonal_by_speed() {
    // speed > 24: consistentDownDiagonal > 1; 12 < speed <= 24: > 6.
    for (speed, consistent, rolls) in [
        (30.0, 1, false),
        (30.0, 2, true),
        (20.0, 6, false),
        (20.0, 7, true),
        (24.0, 6, false),
        (12.0, 7, false),
    ] {
        let mut p = Player::new().holding_down_right(consistent);
        p.receive_impact(hit(HIPS, FLOOR, speed, true));
        let rolled = p.animation() == Animation::Roll;
        assert_eq!(rolled, rolls, "speed {speed}, held {consistent}");
    }
}

#[test]
fn landing_roll_other_conditions() {
    let mut p = Player::new().holding_down_right(10);
    p.allow_roll = 0;
    p.receive_impact(hit(HIPS, FLOOR, 20.0, true));
    assert_eq!(p.animation(), Animation::None, "allowRoll 0");
    for dir in [Contact { x: 0, y: 1 }, Contact { x: 1, y: 0 }] {
        let mut p = Player::new().holding_down_right(10);
        p.receive_impact(hit(HIPS, dir, 20.0, true));
        assert_eq!(p.animation(), Animation::None, "{dir:?}");
    }
    let mut p = Player::new().holding_down_right(10);
    p.input[0] = RIGHT;
    p.receive_impact(hit(HIPS, FLOOR, 20.0, true));
    assert_eq!(p.animation(), Animation::None, "no down-diagonal");
    // A flip or a belly-slide rocket jump rolls at any speed.
    let mut p = Player::new().holding_down_right(10);
    p.animation = Animation::Flip;
    p.receive_impact(hit(HIPS, FLOOR, 2.0, true));
    assert_eq!(p.animation(), Animation::Roll);
    let mut p = Player::new().holding_down_right(10);
    p.animation = Animation::RocketJump;
    p.receive_impact(hit(HIPS, FLOOR, 2.0, true));
    assert_eq!(p.animation(), Animation::RocketJump, "roll-jump rocket");
}

#[test]
fn landing_roll_sets_direction_and_steers_both_chunks() {
    let mut p = Player::new().holding_down_right(10);
    p.standing = true;
    p.body.chunks[HEAD].vel = Vec2::new(2.0, -5.0);
    p.body.chunks[HIPS].vel = Vec2::new(-1.0, -5.0);
    p.receive_impact(hit(HEAD, FLOOR, 20.0, true));
    // lerp(v, 9 * input.x, 0.7): 2 + 7 * 0.7 = 6.9; -1 + 10 * 0.7 = 6.
    assert_vec(p.body.chunks[HEAD].vel, 6.9, -5.0);
    assert_vec(p.body.chunks[HIPS].vel, 6.0, -5.0);
    assert_eq!((p.roll_direction, p.roll_counter), (1, 0));
    assert!(!p.is_standing());

    // Landing from a belly-slide rocket jump also shifts the chunks 3 px apart.
    let mut p = Player::new().holding_down_right(10);
    p.animation = Animation::RocketJump;
    p.rocket_jump_from_belly_slide = true;
    p.body.chunks[HEAD].pos = Vec2::new(0.0, 17.0);
    p.receive_impact(hit(HIPS, FLOOR, 5.0, true));
    assert_eq!(p.animation(), Animation::Roll);
    assert_vec(p.body.chunks[HEAD].pos, 0.0, 14.0);
    assert_vec(p.body.chunks[HIPS].pos, 0.0, 3.0);
    // vel.x: lerp(0, 9, 0.7) = 6.3.
    assert_vec(p.body.chunks[HEAD].vel, 6.3, -3.0);
    assert_vec(p.body.chunks[HIPS].vel, 6.3, 3.0);
}

#[test]
fn landing_roll_pre_empts_stun_and_death_on_that_impact() {
    for speed in [47.5, 70.0] {
        let mut p = Player::new().holding_down_right(2);
        p.receive_impact(hit(HIPS, FLOOR, speed, true));
        assert_eq!(p.animation(), Animation::Roll);
        assert!(!p.is_dead() && p.stun_ticks() == 0, "speed {speed}");
        // Without the held diagonal the same impact stuns (57) or kills.
        let mut p = Player::new().holding_down_right(1);
        p.receive_impact(hit(HIPS, FLOOR, speed, true));
        assert!(p.is_dead() || p.stun_ticks() == 57, "speed {speed}");
    }
    // Once rolling, a later impact (the other chunk) takes the damage chain.
    let mut p = Player::new().holding_down_right(2);
    p.receive_impact(hit(HEAD, FLOOR, 47.5, true));
    p.receive_impact(hit(HIPS, FLOOR, 47.5, true));
    assert_eq!(p.stun_ticks(), 57);
}

#[test]
fn allow_roll_and_consistent_down_diagonal_counters() {
    let r = flat_room();
    // In the air (tile 20 px under the hips is Air): allowRoll held at 15.
    let mut p = Player::upright_at(Vec2::new(100.0, 200.0));
    p.update(&r, DOWN_RIGHT);
    assert_eq!(p.allow_roll, 15);
    // +1 per tick while the same down-diagonal is held as the tick before.
    assert_eq!(p.consistent_down_diagonal, 0);
    p.update(&r, DOWN_RIGHT);
    p.update(&r, DOWN_RIGHT);
    assert_eq!(p.consistent_down_diagonal, 2);
    p.update(&r, DOWN);
    assert_eq!(p.consistent_down_diagonal, 0);
    // On the floor: only counts down.
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.allow_roll = 15;
    p.run(&r, Input::NONE, 3);
    assert_eq!(p.allow_roll, 12);
}

/// Floor rows 0 and 1 (top 40), a wall at column 5 (x 100..120).
fn corner_room() -> Room {
    room(&[
        ".....#.", ".....#.", ".....#.", ".....#.", ".....#.", "#######", "#######",
    ])
}

/// Head falling into the floor/wall corner at (10, -20), hips behind it in the air.
fn falling_into_corner(input: Input) -> Player {
    let mut p = Player::new().holding_down_right(10);
    p.input[0] = input;
    p.body.chunks[HEAD].reset_at(Vec2::new(85.0, 55.0));
    p.body.chunks[HIPS].reset_at(Vec2::new(70.0, 62.0));
    for c in &mut p.body.chunks {
        c.vel = Vec2::new(10.0, -20.0);
    }
    p
}

#[test]
fn landing_roll_changes_velocities_mid_sweep() {
    let r = corner_room();
    // Physics: v = ((10, -20) - (0, 0.9)) * 0.999 = (9.99, -20.8791). Head: y 55 ->
    // 34.12 crosses the floor (row 1 under Air row 2): snap y 49, floor impact 20.8791.
    // With down-right held 10 ticks it rolls inside that call: head and hips vel.x =
    // lerp(v, 9, 0.7). The head's bounce keeps vel.x (friction factor 1), so its wall
    // sweep (column 5 from column 4) reports 9.297 instead of 9.99. The hips were not
    // integrated yet: they move with lerp(10, 9, 0.7) * 0.999 and land with the
    // floor impact the roll no longer intercepts (first contact, under 35: nothing).
    let head_vx = 9.99_f32 + (9.0 - 9.99) * 0.7;
    let vy = (-20.0 - G) * DRAG;
    let mut p = falling_into_corner(DOWN_RIGHT);
    p.update(&r, DOWN_RIGHT);
    let expected = [
        TerrainImpact {
            chunk: HEAD,
            direction: FLOOR,
            speed: -vy,
            first_contact: true,
        },
        TerrainImpact {
            chunk: HEAD,
            direction: Contact { x: 1, y: 0 },
            speed: head_vx,
            first_contact: true,
        },
        TerrainImpact {
            chunk: HIPS,
            direction: FLOOR,
            speed: -vy,
            first_contact: true,
        },
    ];
    assert_eq!(p.body.impacts().len(), 3, "{:?}", p.body.impacts());
    for (got, want) in p.body.impacts().iter().zip(&expected) {
        assert_eq!((got.chunk, got.direction), (want.chunk, want.direction));
        assert_close(got.speed, want.speed, "impact speed");
    }
    assert_eq!(p.animation(), Animation::Roll);
    assert_eq!(p.stun_ticks(), 0);
    // The rod is 10 from the roll's first MovementUpdate.
    assert_eq!(p.body.connections[0].distance, 10.0);

    // Without the diagonal the wall is hit at the unchanged 9.99.
    let mut p = falling_into_corner(RIGHT);
    p.update(&r, RIGHT);
    assert_close(p.body.impacts()[1].speed, 9.99, "wall without roll");
    assert_eq!(p.body.connections[0].distance, 17.0);
}

// ---- roll (section 2) -----------------------------------------------------------------

#[test]
fn roll_tick_on_the_ground() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.animation = Animation::Roll;
    p.roll_direction = 1;
    p.body.chunks[HEAD].vel = Vec2::new(1.0, 1.0);
    p.roll(&r);
    // vel * 0.9, head += 2 * perp = (2, 0), hips -= (2, 0), grounded: both += 1.1.
    assert_vec(p.body.chunks[HEAD].vel, 0.9 + 2.0 + 1.1, 0.9);
    assert_vec(p.body.chunks[HIPS].vel, -2.0 + 1.1, 0.0);
    assert_eq!(p.can_jump, 5);
    assert_eq!(p.stop_rolling_counter, 0);
    assert_eq!(p.animation(), Animation::Roll);
}

#[test]
fn roll_over_a_drop_counts_as_blocked_and_stops_after_seven_ticks() {
    let r = flat_room();
    // Nothing solid within two tiles below either chunk: blocked every tick.
    let mut p = Player::upright_at(Vec2::new(100.0, 200.0));
    p.animation = Animation::Roll;
    p.roll_direction = 1;
    for tick in 1..=6 {
        p.roll(&r);
        assert_eq!(p.stop_rolling_counter, tick);
        assert_eq!(p.animation(), Animation::Roll);
    }
    p.roll(&r);
    assert_eq!(p.animation(), Animation::None);
    assert_eq!(p.roll_direction, 0);
    // input.y > -1: standing.
    assert!(p.is_standing());

    // A one-tile gap: hug the ground (vel * 0.7, y - 2.5), not blocked.
    let mut p = Player::upright_at(Vec2::new(100.0, 68.0));
    p.animation = Animation::Roll;
    p.roll_direction = 1;
    p.roll(&r);
    assert_eq!(p.stop_rolling_counter, 0);
    assert_vec(p.body.chunks[HIPS].pos, 100.0, 65.5);
    // Hips: (0 * 0.9 - 2) * 0.7.
    assert_vec(p.body.chunks[HIPS].vel, -1.4, 0.0);
}

#[test]
fn roll_ends_upright_after_15_ticks_without_diagonal() {
    let r = flat_room();
    for (counter, input, ends) in [
        (15, Input::NONE, false),
        (16, Input::NONE, true),
        (16, DOWN_RIGHT, false),
        (31, DOWN_RIGHT, true),
        (3, LEFT, true),
    ] {
        let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
        p.animation = Animation::Roll;
        p.roll_direction = 1;
        p.roll_counter = counter;
        p.input[0] = input;
        p.roll(&r);
        assert_eq!(
            p.animation() == Animation::None,
            ends,
            "{counter} {input:?}"
        );
    }
    // Head below the hips: only rollCounter > 60 ends it.
    let mut p = Player::lying_on_floor();
    p.body.chunks[HEAD].pos.y = 40.0;
    p.animation = Animation::Roll;
    p.roll_direction = 1;
    p.roll_counter = 60;
    p.roll(&r);
    assert_eq!(p.animation(), Animation::Roll);
    p.roll_counter = 61;
    p.roll(&r);
    assert_eq!(p.animation(), Animation::None);
}

#[test]
fn roll_jump_angle_and_speed_grow_over_25_ticks() {
    let r = flat_room();
    // DegToVec(lerp(60, 35, t)) * lerp(9.5, 13.1, t), t = rollCounter / 25 clamped:
    // 0 -> (8.2272, 4.75), 25 -> (7.5139, 10.7309).
    for (counter, deg, speed) in [
        (0, 60.0_f32, 9.5_f32),
        (10, 50.0, 10.94),
        (25, 35.0, 13.1),
        (40, 35.0, 13.1),
    ] {
        let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
        p.animation = Animation::Roll;
        p.roll_direction = 1;
        p.roll_counter = counter;
        p.jump(&r);
        let (sin, cos) = deg.to_radians().sin_cos();
        for c in &p.body.chunks {
            assert_vec(c.vel, sin * speed, cos * speed);
        }
        // Hips (5, 5) up and forward, head (5, 5) beyond them.
        assert_vec(p.body.chunks[HIPS].pos, 105.0, 53.0);
        assert_vec(p.body.chunks[HEAD].pos, 110.0, 58.0);
        assert_eq!(p.animation(), Animation::RocketJump);
        assert_eq!(p.roll_direction, 0);
        assert!(!p.rocket_jump_from_belly_slide);
    }
}

#[test]
fn rocket_jump_thrusts_along_the_head_velocity() {
    let mut p = Player::upright_at(Vec2::new(100.0, 200.0));
    p.animation = Animation::RocketJump;
    p.standing = true;
    p.body.chunks[HEAD].vel = Vec2::new(3.0, 4.0);
    p.body.chunks[HIPS].vel = Vec2::new(2.0, 2.0);
    p.rocket_jump();
    // u = (0.6, 0.8); head + u + (0, 0.1); hips * 0.99 - u + (0, 0.1).
    assert_vec(p.body.chunks[HEAD].vel, 3.6, 4.9);
    assert_vec(p.body.chunks[HIPS].vel, 1.98 - 0.6, 1.98 - 0.8 + 0.1);
    assert!(!p.is_standing());
    assert_eq!(p.body_mode(), BodyMode::Default);
    assert_eq!(p.animation(), Animation::RocketJump);
}

// ---- belly slide (section 3) ----------------------------------------------------------

#[test]
fn belly_slide_starts_from_down_on_fours_with_the_facing_diagonal() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.run(&r, Input::NONE, 10);
    // Down-right: standing off, Stand mode lowers to DownOnFours; the jump press
    // fires one tick later while still lowering.
    p.update(&r, Input::new(1, -1, true));
    assert_eq!(p.animation(), Animation::DownOnFours);
    p.update(&r, Input::new(1, -1, true));
    assert_eq!(p.animation(), Animation::BellySlide);
    assert_eq!((p.roll_direction, p.roll_counter), (1, 0));
    assert!(!p.is_standing());

    // Plain down: an ordinary crouch jump.
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.run(&r, Input::NONE, 10);
    p.update(&r, Input::new(0, -1, true));
    assert_eq!(p.animation(), Animation::DownOnFours);
    p.update(&r, Input::new(0, -1, true));
    assert_eq!(p.animation(), Animation::None);
    assert_eq!(p.jump_boost, 6.0);
}

#[test]
fn belly_slide_forces() {
    let r = flat_room();
    // No contact on either chunk (placed directly): both vel.x * 0.5 at the end.
    for n in 0..8 {
        let mut p = Player::sliding(n, DOWN_RIGHT);
        p.belly_slide(&r);
        // The hips kick runs for rollCounter 0..=5 (6 ticks), then solid under the
        // hips pulls them down by 0.5.
        let (hips_vx, hips_vy) = if n < 6 { (-9.1, 2.7) } else { (0.0, -0.5) };
        assert_vec(p.body.chunks[HIPS].vel, hips_vx * 0.5, hips_vy);
        // Head: 18.1 * sin(n / 15 * pi), then * 0.5; solid under the head: vy - 2.3.
        let push = 18.1 * (n as f32 / 15.0 * PI).sin();
        assert_vec(p.body.chunks[HEAD].vel, push * 0.5, -2.3);
        assert_eq!(p.animation(), Animation::BellySlide, "n {n}");
        assert_eq!(p.body_mode(), BodyMode::Default);
    }
}

#[test]
fn belly_slide_lasts_17_ticks_and_slows_movement() {
    let r = flat_room();
    let mut p = Player::sliding(0, DOWN_RIGHT);
    // The block sees rollCounter 0..=16 on successive ticks; > 15 ends it.
    for n in 0..16 {
        p.roll_counter = n;
        p.belly_slide(&r);
        assert_eq!(p.animation(), Animation::BellySlide, "n {n}");
    }
    p.roll_counter = 16;
    p.belly_slide(&r);
    assert_eq!(p.animation(), Animation::None);
    assert_eq!(p.roll_direction, 0);
    assert!(!p.is_standing());
    assert_eq!(p.slow_movement_stun, 40);
    assert_eq!(p.body.chunks[HEAD].vel.y, 0.0);
}

#[test]
fn belly_slide_exits() {
    let r = flat_room();
    // Jump edge with 0 < rollCounter < 12.
    for (n, ends) in [(0, false), (1, true), (11, true), (12, false)] {
        let mut p = Player::sliding(n, Input::new(1, -1, true));
        p.belly_slide(&r);
        assert_eq!(p.animation() == Animation::None, ends, "jump at {n}");
    }
    // Letting go of the direction: more than 6 ticks, after rollCounter 8.
    let mut p = Player::sliding(9, Input::NONE);
    p.exit_belly_slide_counter = 6;
    p.belly_slide(&r);
    assert_eq!(p.animation(), Animation::None);
    // Up held with headroom: stands, slowed for 20 ticks; fast chunks halved.
    let mut p = Player::sliding(16, Input::new(0, 1, false));
    p.body.chunks[HEAD].vel.x = 30.0;
    p.belly_slide(&r);
    assert!(p.is_standing());
    assert_eq!(p.slow_movement_stun, 20);
    // (30 + 18.1 * sin(16 pi / 15)) * 0.5 = 13.118 > 8, so halved again.
    let v = (30.0 + 18.1 * (16.0 / 15.0 * PI).sin()) * 0.5 * 0.5;
    assert_vec(p.body.chunks[HEAD].vel, v, 0.0);
    // Reversing after rollCounter 5 arms the whiplash jump.
    let mut p = Player::sliding(6, LEFT);
    p.belly_slide(&r);
    assert!(p.whiplash_jump);
    let mut p = Player::sliding(5, LEFT);
    p.belly_slide(&r);
    assert!(!p.whiplash_jump);
}

#[test]
fn belly_slide_drops_through_a_one_tile_hole() {
    // Floor with a hole at column 6 (x 120..140).
    let r = room(&[
        "..........",
        "..........",
        "..........",
        "..........",
        "######.###",
        "######.###",
    ]);
    let mut p = Player::sliding(9, DOWN);
    p.body.chunks[HIPS].reset_at(Vec2::new(112.0, 48.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(129.0, 48.0));
    p.body.connections[0].distance = 10.0;
    p.belly_slide(&r);
    // Head to its tile middle (130, 50) - 20; hips halfway to head + (0, rod 10).
    assert_vec(p.body.chunks[HEAD].pos, 130.0, 30.0);
    assert_vec(p.body.chunks[HIPS].pos, 121.0, 44.0);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, -11.0);
    assert_vec(p.body.chunks[HIPS].vel, 0.0, -11.0);
    assert_eq!(p.animation(), Animation::None);
    assert!(p.body.chunks.iter().all(|c| c.go_through_floors));
    assert_eq!(p.roll_direction, 0);
}

#[test]
fn belly_slide_rocket_jump() {
    let r = flat_room();
    let mut p = Player::sliding(12, DOWN_RIGHT);
    p.jump(&r);
    // Hips (5, 5) up and forward, head (5, 5) beyond; both vel (9 dir, 8.5).
    assert_vec(p.body.chunks[HIPS].pos, 105.0, 53.0);
    assert_vec(p.body.chunks[HEAD].pos, 110.0, 58.0);
    for c in &p.body.chunks {
        assert_vec(c.vel, 9.0, 8.5);
    }
    assert_eq!(p.animation(), Animation::RocketJump);
    assert!(p.rocket_jump_from_belly_slide);
    assert_eq!(p.roll_direction, 0);
}

#[test]
fn belly_slide_whiplash_jump() {
    // Open floor: three 15 px steps back are free, so n = 3.
    let r = flat_room();
    for (whiplash, input) in [(false, LEFT), (true, RIGHT)] {
        let mut p = Player::sliding(12, input);
        p.whiplash_jump = whiplash;
        p.jump_boost = 3.0;
        p.jump(&r);
        // Head += (-(15 n + 8), 14), hips += (-(15 n + 2), 0).
        assert_vec(p.body.chunks[HEAD].pos, 117.0 - 53.0, 49.0 + 14.0);
        assert_vec(p.body.chunks[HIPS].pos, 100.0 - 47.0, 48.0);
        assert_vec(p.body.chunks[HEAD].vel, -7.0, 10.0);
        assert_vec(p.body.chunks[HIPS].vel, -7.0, 11.0);
        assert_eq!(p.animation(), Animation::Flip);
        assert!(p.is_standing() && p.flip_from_slide && !p.whiplash_jump);
        assert_eq!((p.roll_direction, p.jump_boost), (-1, 0.0));
    }
    // A wall two steps back (column 4, x 80..100): only one step is free.
    let r = room(&[
        "....#.....",
        "....#.....",
        "....#.....",
        "##########",
        "##########",
    ]);
    let mut p = Player::sliding(12, LEFT);
    p.body.chunks[HIPS].reset_at(Vec2::new(108.0, 48.0));
    p.jump(&r);
    assert_vec(p.body.chunks[HEAD].pos, 117.0 - 23.0, 63.0);
}

#[test]
fn rod_is_10_while_roll_direction_is_set() {
    let r = flat_room();
    let mut p = Player::sliding(0, DOWN_RIGHT);
    p.update(&r, DOWN_RIGHT);
    assert_eq!(p.body.connections[0].distance, 10.0);
    assert_eq!(p.roll_counter, 1);
    // Out of Default mode with no rolling animation: the direction is dropped (rod
    // still 10 that tick), and the rod is 17 again the tick after.
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.roll_direction = 1;
    p.update(&r, Input::NONE);
    assert_eq!(p.body.connections[0].distance, 10.0);
    assert_eq!(p.roll_direction, 0);
    p.update(&r, Input::NONE);
    assert_eq!(p.body.connections[0].distance, 17.0);
}

#[test]
fn reversing_and_jumping_during_the_skid_flips() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.run(&r, RIGHT, 12);
    p.update(&r, LEFT);
    assert_eq!(p.slide_counter, 1);
    // Press on skid tick 2; the buffered jump fires the next tick (counter 3, in 1..9)
    // as the last step of the tick, so the set velocities are what remains.
    p.update(&r, Input::new(-1, 0, true));
    p.update(&r, Input::new(-1, 0, true));
    assert_eq!(p.animation(), Animation::Flip);
    assert_eq!(p.slide_counter, 0);
    assert_eq!(p.body.chunks[HEAD].vel.y, 9.0);
    assert_eq!(p.body.chunks[HIPS].vel.y, 7.0);
    assert_eq!(p.jump_boost, 5.0);
}
