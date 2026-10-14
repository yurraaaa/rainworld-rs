//! Player unit tests. Expected values are derived by hand from the formulas in
//! `rainworld-re/notes/slugcat-movement.md`, `horizontal-beams.md` and
//! `slope-collision.md` (derivations next to each assertion). Physics constants:
//! gravity 0.9, air friction 0.999, surface friction 0.5.

use rw_assets::Room;

use super::{Animation, BodyMode, HEAD, HIPS, INPUT_HISTORY, Input, Player};
use crate::tests::{room, room_with_water};
use crate::{Contact, TerrainImpact, Vec2};

const EPS: f32 = 1e-4;
const G: f32 = 0.9;
const DRAG: f32 = 0.999;
/// `surfaceFriction^1.5` with friction 0.5.
const GROUND_BLEND: f32 = 0.353_553_38;

const RIGHT: Input = Input::new(1, 0, false);
const JUMP: Input = Input::new(0, 0, true);

impl Player {
    /// A standing player, body vertical, hips centre at `hips`, at rest.
    fn upright_at(hips: Vec2) -> Self {
        let mut p = Self::new();
        p.body.chunks[HIPS].reset_at(hips);
        p.body.chunks[HEAD].reset_at(hips + Vec2::new(0.0, 17.0));
        p.standing = true;
        p
    }

    /// A player lying flat, head `17` px to the right of the hips, at rest.
    fn lying_at(hips: Vec2) -> Self {
        let mut p = Self::new();
        p.body.chunks[HIPS].reset_at(hips);
        p.body.chunks[HEAD].reset_at(hips + Vec2::new(17.0, 0.0));
        p
    }

    fn vel_sum_y(&self) -> f32 {
        self.body.chunks[HEAD].vel.y + self.body.chunks[HIPS].vel.y
    }

    fn run(&mut self, room: &Room, input: Input, ticks: usize) {
        for _ in 0..ticks {
            self.update(room, input);
        }
    }
}

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: expected {expected}, got {actual}"
    );
}

/// 30 x 20 tiles, Solid rows y = 0 and 1: ground top at 40.
fn flat_room() -> Room {
    let mut rows = vec!["..............................".to_string(); 18];
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

// ---- input ------------------------------------------------------------------------

#[test]
fn input_history_keeps_ten_newest_first() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    for i in 0..12 {
        let x = if i % 2 == 0 { 1 } else { -1 };
        p.update(&r, Input::new(x, 0, i == 11));
    }
    let h = p.input_history();
    assert_eq!(h.len(), INPUT_HISTORY);
    assert_eq!(h[0], Input::new(-1, 0, true));
    assert_eq!(h[1], Input::new(1, 0, false));
    assert_eq!(h[9], Input::new(1, 0, false)); // tick index 2
    // Out-of-range axes are reduced to their sign.
    p.update(
        &r,
        Input {
            x: 5,
            y: -3,
            jump: false,
        },
    );
    assert_eq!(p.input_history()[0], Input::new(1, -1, false));
    assert_eq!(Input::new(-1, -1, false).down_diagonal(), -1);
    assert_eq!(Input::new(1, 0, false).down_diagonal(), 0);
}

// ---- running and ground friction --------------------------------------------------

#[test]
fn run_accelerates_by_1_2_per_tick_up_to_stand_caps() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    // Body vertical, both chunks get the same x push, so the rod (vertical) adds no
    // x velocity. Per tick: physics v *= 0.999 (a flat landing keeps vel.x since
    // clamp(2 * 0.5, 0, 1) = 1), then v += min(2.4 * 0.5, cap - v).
    let mut expected = [0.0_f32; 2];
    let caps = [4.2_f32, 4.0];
    for tick in 1..=6 {
        p.update(&r, RIGHT);
        assert_eq!(p.body_mode(), BodyMode::Stand, "tick {tick}");
        for i in 0..2 {
            let v = expected[i] * DRAG;
            expected[i] = v + (1.2_f32).min(caps[i] - v);
            if tick <= 4 {
                assert_close(
                    p.body.chunks[i].vel.x,
                    expected[i],
                    &format!("tick {tick} chunk {i}"),
                );
            }
        }
    }
    // Ticks 1-3: 1.2, 2.3988, 3.596_401; tick 4 reaches the caps exactly.
    assert_eq!(p.dynamic_run_speed, [4.2, 4.0]);
}

#[test]
fn releasing_direction_applies_ground_friction_and_stops() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.run(&r, RIGHT, 3);
    // After three ticks both chunks move at 3.596_401 (see the run test).
    let v3 = (1.2_f32 * DRAG + 1.2) * DRAG + 1.2;
    let x_release = p.body.chunks[HIPS].pos.x;
    p.update(&r, Input::NONE);
    // No input on the ground: v += (0 - v) * 0.5^1.5 after the physics drag.
    let v4 = v3 * DRAG * (1.0 - GROUND_BLEND);
    for i in 0..2 {
        assert_close(p.body.chunks[i].vel.x, v4, &format!("chunk {i}"));
    }
    assert!(
        p.feet_stuck_pos.is_some(),
        "standing still on flat ground pins the feet"
    );
    p.run(&r, Input::NONE, 60);
    for c in p.chunks() {
        assert!(c.vel.x.abs() < 1e-3, "still moving: {c:?}");
    }
    // Without the pin the geometric decay alone would travel v4 / 0.354 = 6.6 px.
    let travel = p.body.chunks[HIPS].pos.x - x_release;
    assert!(
        (0.0..10.0).contains(&travel),
        "travel after release {travel}"
    );
}

// ---- jumping ----------------------------------------------------------------------

#[test]
fn standing_jump_fires_one_tick_after_press_and_boost_decays() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    // The press edge is read before the new input, so the buffer starts next tick.
    p.update(&r, JUMP);
    assert_eq!(p.want_to_jump, 0);
    assert!(p.body.chunks[HEAD].vel.y < 4.0);
    p.update(&r, JUMP);
    // Standing jump sets vel.y = 4 / 3 and jumpBoost = 8.
    assert_eq!(p.body.chunks[HEAD].vel.y, 4.0);
    assert_eq!(p.body.chunks[HIPS].vel.y, 3.0);
    assert_eq!(p.jump_boost, 8.0);
    assert_eq!(p.animation(), Animation::None);

    // Held: each airborne tick jumpBoost -= 1.5, both chunks += (jumpBoost + 1) * 0.3.
    // The rod and the standing lift (+4 / -4) cancel in the sum, so
    // sum' = (sum - 2 * 0.9) * 0.999 + 2 * add.
    let boosts = [6.5_f32, 5.0, 3.5, 2.0, 0.5, -1.0, 0.0, 0.0];
    let adds = [2.25_f32, 1.8, 1.35, 0.9, 0.45, 0.0, 0.0, 0.0];
    let mut sum = 7.0_f32;
    for (k, (&boost, &add)) in boosts.iter().zip(&adds).enumerate() {
        p.update(&r, JUMP);
        sum = (sum - 2.0 * G) * DRAG + 2.0 * add;
        assert_eq!(p.body_mode(), BodyMode::Default, "airborne tick {k}");
        assert_close(
            p.jump_boost,
            boost,
            &format!("boost after airborne tick {k}"),
        );
        assert!(
            (p.vel_sum_y() - sum).abs() < 1e-3,
            "tick {k}: {} vs {sum}",
            p.vel_sum_y()
        );
    }
}

#[test]
fn tapped_jump_gets_no_boost() {
    let r = flat_room();
    let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
    p.update(&r, JUMP);
    p.update(&r, Input::NONE); // the jump fires this tick from the buffer
    assert_eq!(p.jump_boost, 8.0);
    p.update(&r, Input::NONE);
    // Released: jumpBoost = 0 and no add.
    assert_eq!(p.jump_boost, 0.0);
    assert!((p.vel_sum_y() - (7.0 - 2.0 * G) * DRAG).abs() < 1e-3);
}

#[test]
fn coyote_time_allows_jumps_four_ticks_after_leaving_ground() {
    let r = room(&[".........."; 30]);
    // canJump = 5 on the last grounded tick, -1 per tick before MovementUpdate. A press
    // at tick k is seen at k + 1, where canJump = 5 - (k + 1): jumps for k <= 3.
    for (press_tick, jumps) in [(1, true), (3, true), (4, false)] {
        let mut p = Player::lying_at(Vec2::new(60.0, 400.0));
        p.can_jump = 5;
        for tick in 1..=press_tick + 1 {
            let jump = tick >= press_tick;
            p.update(&r, Input::new(0, 0, jump));
        }
        // A crouched jump (not standing) sets jumpBoost = 6.
        assert_eq!(p.jump_boost == 6.0, jumps, "press at tick {press_tick}");
    }
}

#[test]
fn jump_buffer_lasts_five_ticks_before_landing() {
    let r = flat_room();
    let y0 = 120.0_f32;
    // Lying flat, both chunks fall alike (the rod is horizontal). The head (r 9)
    // lands on the first tick its sweep reaches row 1: y - 9 - 0.01 < 40.
    let mut y = y0;
    let mut v = 0.0_f32;
    let mut landing = 0;
    for tick in 1.. {
        v = (v - G) * DRAG;
        y += v;
        if y - 9.0 - 0.01 < 40.0 {
            landing = tick;
            break;
        }
    }
    // Buffer: set to 5 on the tick after the press, -1 per tick: jumps if pressed
    // at landing - 5 or later.
    for (press_tick, jumps) in [(landing - 5, true), (landing - 6, false)] {
        let mut p = Player::lying_at(Vec2::new(60.0, y0));
        for tick in 1..=landing {
            // x held keeps a crawl landing from cancelling the buffer.
            p.update(&r, Input::new(1, 0, tick == press_tick));
            if tick < landing {
                assert_eq!(p.body.chunks[HEAD].contact().y, 0, "tick {tick}");
            }
        }
        assert_eq!(
            p.body.chunks[HEAD].contact().y,
            -1,
            "landed on tick {landing}"
        );
        assert_eq!(
            p.jump_boost == 6.0,
            jumps,
            "press at {press_tick}, landing {landing}"
        );
    }
}

#[test]
fn crouched_still_jump_fires_on_release() {
    let r = flat_room();
    let mut p = Player::lying_at(Vec2::new(60.0, 48.0));
    p.run(&r, Input::NONE, 20);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    p.run(&r, JUMP, 4);
    // Crawling with no direction: the press buffer is cleared, the charge counts.
    assert_eq!(p.jump_boost, 0.0);
    assert_eq!(p.super_launch_jump, 4);
    p.update(&r, Input::NONE);
    // Release sets wantToJump = 1: crouch jump, jumpBoost 6.
    assert_eq!(p.jump_boost, 6.0);
}

#[test]
fn wall_jump_from_wall_climb() {
    // Wall column x = 8 (160..180), open air elsewhere.
    let r = room(&["........#."; 40]);
    let mut p = Player::upright_at(Vec2::new(150.0, 500.0));
    p.run(&r, RIGHT, 2);
    // Tick 2: the head (151.2 + 9 >= 160) hits the wall: contact x = 1 = input.
    assert_eq!(p.body_mode(), BodyMode::WallClimb);
    assert_eq!(p.can_wall_jump, -15);
    p.update(&r, Input::new(1, 0, true));
    p.update(&r, Input::new(1, 0, true));
    // No footing: head (6 * dir, 8), hips (5 * dir, 7), dir = sign(canWallJump) = -1.
    assert_eq!(p.body.chunks[HEAD].vel, Vec2::new(-6.0, 8.0));
    assert_eq!(p.body.chunks[HIPS].vel, Vec2::new(-5.0, 7.0));
    assert_eq!(p.jump_stun, -8);
    assert_eq!(p.can_wall_jump, 0);
}

// ---- standing and crawling --------------------------------------------------------

#[test]
fn up_press_stands_up_from_crawl() {
    let r = flat_room();
    let mut p = Player::lying_at(Vec2::new(60.0, 48.0));
    p.run(&r, Input::NONE, 20);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    assert!(!p.is_standing());
    p.update(&r, Input::new(0, 1, false));
    assert!(p.is_standing());
    p.run(&r, Input::NONE, 60);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    assert_eq!(p.animation(), Animation::None);
    let (head, hips) = (p.head(), p.hips());
    assert!(head.pos.y - hips.pos.y > 16.5, "{head:?} {hips:?}");
    assert!((head.pos.x - hips.pos.x).abs() < 1.0);
    // Down press lowers to all fours again.
    p.update(&r, Input::new(0, -1, false));
    assert!(!p.is_standing());
    p.run(&r, Input::NONE, 40);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
}

// ---- one-way floors ---------------------------------------------------------------

/// Floor tiles across row 5 (top 120), solid ground rows 0-1.
fn one_way_room() -> Room {
    let mut rows = vec!["..............."; 14];
    rows.push("---------------");
    rows.extend(["..............."; 3]);
    rows.extend(["###############"; 2]);
    room(&rows)
}

#[test]
fn one_way_floor_holds_until_down_is_held() {
    let r = one_way_room();
    let mut p = Player::lying_at(Vec2::new(100.0, 128.0));
    p.run(&r, Input::NONE, 40);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    assert_eq!(p.body.chunks[HIPS].pos.y, 128.0, "rests on the floor tiles");
    let down = Input::new(0, -1, false);
    p.update(&r, down);
    // Crawl + down sets GoThroughFloors after this tick's physics.
    assert!(p.body.chunks.iter().all(|c| c.go_through_floors));
    let rest: Vec<f32> = p.chunks().iter().map(|c| c.pos.y).collect();
    p.update(&r, down);
    for (c, y) in p.chunks().iter().zip(rest) {
        assert_eq!(c.contact().y, 0, "falls through: {c:?}");
        assert!(c.pos.y < y);
    }
    // Releasing down while the body still overlaps the floor row lands it back
    // there (the floor is solid again once GoThroughFloors resets): keep holding.
    p.run(&r, down, 10);
    assert!(
        p.chunks().iter().all(|c| c.pos.y + c.rad < 100.0),
        "below the floor row"
    );
    p.run(&r, Input::NONE, 40);
    // Landed on the solid ground (top 40).
    assert_eq!(p.body.chunks[HIPS].pos.y, 48.0);
}

// ---- slopes -----------------------------------------------------------------------

/// Slope tile (5, 2) with Solid right and below: UpLeft, downhill -x, surface y =
/// x - 100 + 40 inside the tile. Solid rows 0-1 and the block right of it.
fn ramp_room() -> Room {
    let mut rows = vec!["............"; 10];
    rows.push("...../######");
    rows.extend(["############"; 2]);
    room(&rows)
}

/// Upright on the slope at x = 110: surface 50, hips rest at 50 + 2 * 8 = 66.
fn on_ramp() -> Player {
    Player::upright_at(Vec2::new(110.0, 66.0))
}

#[test]
fn stand_on_slope_scales_caps_and_pulls_hips_down() {
    let r = ramp_room();
    let mut p = on_ramp();
    p.update(&r, Input::NONE);
    let hips = &p.body.chunks[HIPS];
    assert_eq!(hips.on_slope(), -1);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    // Caps x0.75: 4.2 * 0.75, 4 * 0.75.
    assert_eq!(p.dynamic_run_speed, [4.2 * 0.75, 4.0 * 0.75]);
    // Physics: hips lifted back to 66, head fell 0.8991, the rod splits the 0.8991
    // shortfall; then Stand-on-slope moves the hips 2 px down.
    assert_close(hips.pos.y, 66.0 - 0.8991 / 2.0 - 2.0, "hips y");
}

#[test]
fn slope_jump_kicks_downhill() {
    let r = ramp_room();
    // No x input: head += onSlope * 1.8, hips += onSlope * 1.2 (onSlope = -1).
    let mut p = on_ramp();
    p.update(&r, JUMP);
    p.update(&r, JUMP);
    assert_eq!(p.jump_boost, 8.0);
    assert_close(p.body.chunks[HEAD].vel.x, -1.8, "head");
    assert_close(p.body.chunks[HIPS].vel.x, -1.2, "hips");

    // Jumping uphill (x = -onSlope = +1): the run force first adds 1.2 to both, then
    // only the hips get onSlope * 8.
    let mut p = on_ramp();
    p.update(&r, JUMP);
    p.update(&r, Input::new(1, 0, true));
    assert_close(p.body.chunks[HEAD].vel.x, 1.2, "head");
    assert_close(p.body.chunks[HIPS].vel.x, 1.2 - 8.0, "hips");
}

// ---- horizontal beams -------------------------------------------------------------

#[test]
fn pole_hop_off_a_beam_while_passing_through() {
    // Beam row y = 10 (beam line 210).
    let mut rows = vec!["............"; 30];
    rows[29 - 10] = "============";
    let r = room(&rows);
    let mut p = Player::upright_at(Vec2::new(110.0, 210.0));
    p.update(&r, Input::new(1, 0, true));
    p.update(&r, Input::new(1, 0, true));
    // Hop: vel.y 4.5 / 3.5 set before physics, then the held boost: jumpBoost 6 -> 4.5,
    // both += 5.5 * 0.3. Sum: (8 - 1.8) * 0.999 + 3.3.
    assert_eq!(p.pole_skip_penalty, 6);
    assert_eq!(p.want_to_jump, 0);
    assert_close(p.jump_boost, 4.5, "boost");
    assert!((p.vel_sum_y() - ((8.0 - 2.0 * G) * DRAG + 3.3)).abs() < 1e-3);
}

#[test]
fn beam_shuffle_settles_at_momentum_balance_speed() {
    // A long beam on row 20 (beam line 410).
    let mut rows = vec![".".repeat(200); 40];
    rows[39 - 20] = "=".repeat(200);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let mut p = Player::upright_at(Vec2::new(200.0, 480.0));
    p.run(&r, Input::new(0, 1, false), 20);
    assert_eq!(p.animation(), Animation::HangFromBeam);
    assert_eq!(p.body.chunks[HEAD].pos.y, 410.0);
    p.run(&r, Input::NONE, 60);
    // Per tick the head gets v' = 0.2 v + 1.2 and the hips the swing
    // -0.5 * (0.5 + 0.5 sin(2 pi frame / 20)), -0.25 on average over the 20-frame
    // cycle. At a common speed u the momentum balance per tick is
    // (1.2 - 0.8 u) - 0.25 = 0: u = 1.1875 px/tick (air drag 0.999 neglected).
    p.run(&r, RIGHT, 200);
    let x0 = p.body.chunks[HEAD].pos.x;
    p.run(&r, RIGHT, 200);
    let speed = (p.body.chunks[HEAD].pos.x - x0) / 200.0;
    assert!((speed - 1.1875).abs() < 0.01, "shuffle speed {speed}");
    assert_eq!(p.body.chunks[HEAD].pos.y, 410.0);
    assert_eq!(p.animation(), Animation::HangFromBeam);
}

// ---- falls: impacts, stun, death (falls.md) -------------------------------------------

const FLOOR: Contact = Contact { x: 0, y: -1 };
const CEILING: Contact = Contact { x: 0, y: 1 };
const WALLS: [Contact; 2] = [Contact { x: 1, y: 0 }, Contact { x: -1, y: 0 }];

fn hit(chunk: usize, direction: Contact, speed: f32, first_contact: bool) -> TerrainImpact {
    TerrainImpact {
        chunk,
        direction,
        speed,
        first_contact,
    }
}

#[test]
fn fall_stun_ticks_follow_the_lerp_map() {
    // int(40 + 100 * ((speed - 35) / 25)^2.5).
    assert_eq!(Player::fall_stun_ticks(35.0), 40);
    // t = 0.5: 0.5^2.5 = 0.1767767 => 57.68 => 57.
    assert_eq!(Player::fall_stun_ticks(47.5), 57);
    assert_eq!(Player::fall_stun_ticks(60.0), 140);
    // Clamped above 60 (walls and ceilings).
    assert_eq!(Player::fall_stun_ticks(80.0), 140);
}

#[test]
fn impact_stuns_above_35_on_every_side() {
    for dir in [FLOOR, CEILING, WALLS[0], WALLS[1]] {
        let mut p = Player::upright_at(Vec2::new(100.0, 48.0));
        // Not above 35: nothing.
        p.terrain_impact(hit(HIPS, dir, 35.0, true));
        assert_eq!(p.stun_ticks(), 0, "{dir:?}");
        assert!(p.is_standing() && p.is_conscious());
        p.terrain_impact(hit(HIPS, dir, 47.5, true));
        assert_eq!(p.stun_ticks(), 57, "{dir:?}");
        // Stun > 8 un-stands.
        assert!(!p.is_standing() && p.is_stunned() && !p.is_conscious());
        // 40: t = 0.2, 0.2^2.5 = 0.0178885 => 41; the longer stun is kept.
        p.terrain_impact(hit(HEAD, dir, 40.0, true));
        assert_eq!(p.stun_ticks(), 57, "{dir:?}");
        assert!(!p.is_dead());
    }
}

#[test]
fn death_above_60_on_floors_only() {
    let mut p = Player::new();
    p.terrain_impact(hit(HIPS, FLOOR, 60.0, true));
    assert!(!p.is_dead());
    assert_eq!(p.stun_ticks(), 140);

    let mut p = Player::new();
    p.terrain_impact(hit(HIPS, FLOOR, 60.5, true));
    assert!(p.is_dead() && !p.is_conscious());
    assert_eq!(p.stun_ticks(), 0, "death takes the place of stun");

    for dir in [CEILING, WALLS[0], WALLS[1]] {
        let mut p = Player::new();
        p.terrain_impact(hit(HEAD, dir, 75.0, true));
        assert!(!p.is_dead(), "{dir:?}");
        assert_eq!(p.stun_ticks(), 140, "{dir:?}");
    }
}

#[test]
fn impact_without_first_contact_does_nothing() {
    let mut p = Player::new();
    p.terrain_impact(hit(HIPS, FLOOR, 100.0, false));
    assert!(!p.is_dead());
    assert_eq!(p.stun_ticks(), 0);
}

#[test]
fn input_is_neutral_on_every_stun_tick() {
    let r = flat_room();
    let mut p = Player::lying_at(Vec2::new(100.0, 48.0));
    p.run(&r, Input::NONE, 20);
    // Stun 2: checkInput reads nothing while stun != 0 (before the countdown).
    p.stun(2);
    p.update(&r, RIGHT);
    assert_eq!(p.input_history()[0], Input::NONE);
    assert_eq!(p.stun_ticks(), 1);
    p.update(&r, RIGHT);
    assert_eq!(p.input_history()[0], Input::NONE);
    assert_eq!(p.stun_ticks(), 0);
    p.update(&r, RIGHT);
    assert_eq!(p.input_history()[0], RIGHT);

    p.die();
    p.update(&r, RIGHT);
    assert_eq!(p.input_history()[0], Input::NONE);
}

#[test]
fn movement_is_skipped_until_stun_reaches_zero() {
    let r = flat_room();
    let mut p = Player::lying_at(Vec2::new(100.0, 48.0));
    p.run(&r, Input::NONE, 20);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    p.stun(40);
    // Ticks 1..39: Stunned mode stays (MovementUpdate would reclassify it).
    p.run(&r, Input::NONE, 39);
    assert_eq!(p.stun_ticks(), 1);
    assert_eq!(p.body_mode(), BodyMode::Stunned);
    assert_eq!(p.animation(), Animation::None);
    // Tick 40 counts 1 -> 0 before physics, so MovementUpdate runs again.
    p.update(&r, Input::NONE);
    assert_eq!(p.stun_ticks(), 0);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
}

#[test]
fn friction_is_0_3_while_not_conscious() {
    let r = flat_room();
    let mut p = Player::lying_at(Vec2::new(100.0, 48.0));
    p.update(&r, Input::NONE);
    assert_eq!(p.body.params.surface_friction, 0.5);
    // Stunned is stun >= 10, read before the countdown.
    p.stun(10);
    p.update(&r, Input::NONE);
    assert_eq!(p.body.params.surface_friction, 0.3);
    // Stun 9: conscious again, though still without input or movement.
    p.update(&r, Input::NONE);
    assert_eq!(p.body.params.surface_friction, 0.5);
    assert_eq!(p.body_mode(), BodyMode::Stunned);

    let mut p = Player::lying_at(Vec2::new(100.0, 48.0));
    p.die();
    p.update(&r, Input::NONE);
    assert_eq!(p.body.params.surface_friction, 0.3);
    assert_eq!(p.body_mode(), BodyMode::Dead);
    assert_eq!(p.animation(), Animation::Dead);
}

/// An all-Air `w` x `h` room: the tiles below it repeat the Air bottom row.
fn open_room(w: usize, h: usize, water_level: i32) -> Room {
    let row = ".".repeat(w);
    room_with_water(&vec![row.as_str(); h], water_level)
}

impl Player {
    /// Steps the player down through the room bottom; checks that it dies exactly on
    /// the first tick that starts with the head below `limit`. Returns the ticks taken.
    fn fall_to_death(&mut self, room: &Room, limit: f32) -> usize {
        for tick in 1..400 {
            let start_y = self.body.chunks[HEAD].pos.y;
            self.update(room, Input::NONE);
            assert_eq!(
                self.is_dead(),
                start_y < limit,
                "tick {tick}: head was at {start_y}"
            );
            if self.is_dead() {
                return tick;
            }
        }
        panic!("never fell out: {:?}", self.head());
    }
}

#[test]
fn falling_below_minus_500_kills() {
    // Creature.Update: limit max(-1200 + 1, -500) = -500, checked before physics.
    let mut p = Player::lying_at(Vec2::new(100.0, -480.0));
    assert!(p.fall_to_death(&open_room(20, 10, -1), -500.0) > 1);
    // defaultWaterLevel < -10 does not catch the body either.
    let mut p = Player::lying_at(Vec2::new(100.0, -480.0));
    p.fall_to_death(&open_room(20, 10, -11), -500.0);
}

#[test]
fn wall_climb_raises_the_out_of_room_limit_to_minus_250() {
    let r = open_room(20, 10, -1);
    let mut p = Player::lying_at(Vec2::new(100.0, -251.0));
    p.update(&r, Input::NONE);
    assert!(!p.is_dead());
    let mut p = Player::lying_at(Vec2::new(100.0, -251.0));
    p.body_mode = BodyMode::WallClimb;
    p.update(&r, Input::NONE);
    assert!(p.is_dead());
}

#[test]
fn water_room_never_kills_below_the_bottom() {
    let r = open_room(20, 10, 3);
    let mut p = Player::lying_at(Vec2::new(100.0, -480.0));
    p.run(&r, Input::NONE, 300);
    assert!(!p.is_dead());
    // Held by the room-range clamp instead.
    assert_eq!(p.head().pos.y, -1200.0);
}

#[test]
fn long_drop_in_a_tall_room_stuns() {
    // 30 x 70 tiles, floor rows 0 and 1 (top 40).
    let mut rows = vec![".".repeat(30); 68];
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let mut p = Player::lying_at(Vec2::new(100.0, 1300.0));
    // Free fall v <- (v - 0.9) * 0.999, y += v from y 1300: after 52 ticks both
    // chunks are still above the floor; tick 53 takes them to y 35.41 (below 40 + rad)
    // at v = -46.434, so both report a floor first contact, head first.
    p.run(&r, Input::NONE, 52);
    assert_eq!(p.stun_ticks(), 0);
    assert!(p.body.impacts().is_empty());
    p.update(&r, Input::NONE);
    let impacts = p.body.impacts();
    assert_eq!(impacts.len(), 2, "{impacts:?}");
    for (i, imp) in impacts.iter().enumerate() {
        assert_eq!(
            (imp.chunk, imp.direction, imp.first_contact),
            (i, FLOOR, true)
        );
        assert!((imp.speed - 46.434).abs() < 1e-2, "{imp:?}");
    }
    // t = 11.434 / 25 = 0.45736, t^2.5 = 0.14147 => 54.
    assert_eq!(p.stun_ticks(), 54);
    assert!(p.is_stunned() && !p.is_dead());
    p.update(&r, Input::NONE);
    assert_eq!(p.body_mode(), BodyMode::Stunned);
}

#[test]
fn landing_above_60_kills() {
    let r = flat_room();
    let mut p = Player::lying_at(Vec2::new(100.0, 100.0));
    for c in &mut p.body.chunks {
        c.vel.y = -62.0;
    }
    // (-62 - 0.9) * 0.999 = -62.837: y 37.2, through the floor top 40 in one tick.
    p.update(&r, Input::NONE);
    assert!(p.is_dead());
}
