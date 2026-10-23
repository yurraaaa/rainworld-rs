//! The player in zero gravity: entry and exit, `ZeroGSwim`, `ZeroGPoleGrab`, the jump's
//! early return. Expected values come from the formulas of `Player.cs` 8541-8815, 12257 and
//! 12327 (`rainworld-re/surveys/zero-g-player.md`), written next to each assertion.

use rw_random::UnityRandom;

use super::poles::Grid;
use super::{DRAG, assert_close, assert_vec};
use crate::Sounds;
use crate::Vec2;
use crate::player::{Animation, BodyMode, HEAD, HIPS, Input, Player};
use crate::tests::room;

const UP: Input = Input::new(0, 1, false);
const RIGHT: Input = Input::new(1, 0, false);

/// 30 x 20 tiles of air with a Solid floor of two rows (top at 40).
fn floor_room() -> rw_assets::Room {
    Grid::new(30, 20).room()
}

/// 20 x 40 tiles with one vertical pole in column 5 from row 2 to row 38.
fn tall_pole() -> rw_assets::Room {
    Grid::new(20, 40).set(5, 2..=38, b'|').room()
}

impl Player {
    /// Floating in zero gravity, body vertical, hips centre at `hips`, at rest.
    fn floating_at(hips: Vec2) -> Self {
        let mut p = Self::upright_at(hips);
        p.set_room_gravity(0.0);
        p
    }
}

/// With gravity 0 nothing falls: the body keeps its velocity under the air friction alone,
/// `pos += vel` after `vel *= 0.999` each tick.
#[test]
fn zero_gravity_body_drifts_under_air_friction_only() {
    let r = tall_pole();
    let mut p = Player::floating_at(Vec2::new(250.0, 400.0));
    for c in &mut p.body.chunks {
        c.vel = Vec2::new(2.0, 0.0);
    }
    let mut x = [p.body.chunks[HEAD].pos.x, p.body.chunks[HIPS].pos.x];
    let mut vel = 2.0_f32;
    for _ in 0..40 {
        p.step(&r, Input::NONE);
        vel *= DRAG;
        x[0] += vel;
        x[1] += vel;
    }
    assert_close(p.body.chunks[HEAD].pos.x, x[0], "head x");
    assert_close(p.body.chunks[HIPS].pos.x, x[1], "hips x");
    assert_close(p.body.chunks[HEAD].pos.y, 417.0, "head y");
    assert_eq!(p.body_mode(), BodyMode::ZeroG);
    assert_eq!(p.animation(), Animation::ZeroGSwim);
}

/// At gravity 0 the body mode is ZeroG, and the animation a swim away from beams; a gravity
/// above 0 ends it (`UpdateBodyMode`: back to Default and no animation).
#[test]
fn zero_g_mode_follows_the_room_gravity() {
    let r = tall_pole();
    let mut p = Player::floating_at(Vec2::new(250.0, 400.0));
    p.step(&r, Input::NONE);
    assert_eq!(
        (p.body_mode(), p.animation()),
        (BodyMode::ZeroG, Animation::ZeroGSwim)
    );
    p.set_room_gravity(0.4);
    p.step(&r, Input::NONE);
    assert_eq!(
        (p.body_mode(), p.animation()),
        (BodyMode::Default, Animation::None)
    );
}

/// Entering with the main chunk on a beam tile grabs it (`ZeroGPoleGrab`).
#[test]
fn zero_g_entry_on_a_pole_grabs_it() {
    let r = tall_pole();
    let mut p = Player::floating_at(Vec2::new(110.0, 300.0));
    p.step(&r, Input::NONE);
    assert_eq!(p.animation(), Animation::ZeroGPoleGrab);
    assert_eq!(p.body_mode(), BodyMode::ZeroG);
    assert!(p.is_standing());
}

/// Gravity back on a zero-gravity pole: hanging from a horizontal beam, else climbing.
#[test]
fn gravity_returning_on_a_pole_hangs_or_climbs() {
    let r = tall_pole();
    let mut p = Player::upright_at(Vec2::new(110.0, 300.0));
    p.animation = Animation::ZeroGPoleGrab;
    p.body_mode = BodyMode::ZeroG;
    p.step(&r, Input::NONE);
    assert_eq!(p.animation(), Animation::ClimbOnBeam);
    assert_eq!(p.body_mode(), BodyMode::ClimbingOnBeam);
    let beams = Grid::new(20, 40).row(15, 0..=19, b'=').room();
    let mut p = Player::upright_at(Vec2::new(110.0, 295.0));
    p.animation = Animation::ZeroGPoleGrab;
    p.step(&beams, Input::NONE);
    assert_eq!(p.animation(), Animation::HangFromBeam);
}

/// A thrust from rest, free of walls: `ZeroGSwim` adds `0.1 * dir` to the main chunk
/// (`LerpMap(0, 1, 4, 0.1, ..)` with the chunks at rest) and takes `0.1 * dir` from the
/// hips, with one `Random.value` for the swim cycle and none otherwise.
#[test]
fn zero_g_swim_thrust_from_rest() {
    let r = tall_pole();
    let mut p = Player::floating_at(Vec2::new(250.0, 400.0));
    let mut rng = UnityRandom::new(3);
    let mut mirror = rng.clone();
    p.update(&r, None, RIGHT, &mut rng, &mut Sounds::default());
    mirror.value();
    assert_eq!(rng, mirror);
    assert_vec(p.body.chunks[HEAD].vel, 0.1, 0.0);
    assert_vec(p.body.chunks[HIPS].vel, -0.1, 0.0);
    // swimCycle: 4 / LerpMap(0, 0, 2, 120, 60) = 1/30, then 1 / Lerp(2, 6, v).
    let v = UnityRandom::new(3).value();
    assert_close(
        p.swim_cycle(),
        4.0 / 120.0 + 1.0 / (2.0 + 4.0 * v),
        "swim cycle",
    );
    // Without a direction there is no thrust and no draw.
    let mut rng = UnityRandom::new(3);
    let mut p = Player::floating_at(Vec2::new(250.0, 400.0));
    p.update(&r, None, Input::NONE, &mut rng, &mut Sounds::default());
    assert_eq!(rng, UnityRandom::new(3));
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.0);
}

/// Thrust against a brace (`canJump > 0`: touching a wall) adds `0.2 * dir`.
#[test]
fn zero_g_swim_braced_thrust_is_a_fifth() {
    let r = floor_room();
    let mut p = Player::floating_at(Vec2::new(100.0, 70.0));
    p.can_jump = 6;
    p.step(&r, UP);
    // can_jump is 5 after its countdown, so the thrust is braced.
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.2);
    assert_vec(p.body.chunks[HIPS].vel, 0.0, -0.1);
}

/// A jump press while braced pushes off the nearest solid neighbour: `a2 = Lerp(Lerp(dir,
/// wall, 0.5), DirVec(hips, head), 0.25)`, `head.vel = ClampMagnitude(vel + a2 * 5.4, 5.4)`,
/// `hips.vel = ClampMagnitude(vel + a2 * 5, 5)`, then `head.vel += wall`.
#[test]
fn zero_g_swim_pushes_off_the_floor() {
    let r = floor_room();
    // Lying on the floor: hips (100, 48), head 17 px to the right.
    let mut p = Player::lying_at(Vec2::new(100.0, 48.0));
    p.set_room_gravity(0.0);
    p.can_jump = 12;
    // A press seen by the jump buffer in an earlier tick.
    p.want_to_jump = 3;
    p.step(&r, UP);
    // Thrust (braced): head (0, 0.2), hips (0, -0.1). Wall (0, 1) below, a2 = (0.25, 0.75).
    assert_vec(p.body.chunks[HEAD].vel, 1.35, 0.2 + 4.05 + 1.0);
    assert_vec(p.body.chunks[HIPS].vel, 1.25, -0.1 + 3.75);
    assert_eq!((p.can_jump, p.want_to_jump), (0, 0));
}

/// Holding up through a vertical pole's tile grabs it (`ZeroGPoleGrab`, standing).
#[test]
fn zero_g_swim_grabs_a_pole_passing_up() {
    let r = tall_pole();
    let mut p = Player::floating_at(Vec2::new(98.0, 300.0));
    p.animation = Animation::ZeroGSwim;
    p.body.chunks[HEAD].pos = Vec2::new(98.0, 317.0);
    p.body.chunks[HEAD].last_pos = Vec2::new(98.0, 317.0);
    for c in &mut p.body.chunks {
        c.vel = Vec2::new(3.0, 0.0);
    }
    p.step(&r, UP);
    assert_eq!(p.animation(), Animation::ZeroGPoleGrab);
    assert!(p.is_standing());
}

/// On a vertical pole, up climbs: the main chunk's speed is scaled by `LerpMap(|v|, 2, 5,
/// 0.7, 0.3)`, pulled to the pole's line and given `1.05 * poleClimbSpeedFac` along it.
#[test]
fn zero_g_pole_grab_climbs_a_vertical_pole() {
    let r = tall_pole();
    let mut p = Player::floating_at(Vec2::new(110.0, 300.0));
    p.step(&r, UP);
    assert_eq!(p.animation(), Animation::ZeroGPoleGrab);
    assert_vec(
        p.body.chunks[HEAD].vel,
        0.0,
        1.05 * p.stats.pole_climb_speed_fac,
    );
    // A jump press lets go with a push along the held direction (x scaled by 0.1 across a
    // vertical pole): SwimDir(true) = (0, 1), head 5.4, hips 5.
    let before = p.body.chunks[HEAD].vel;
    p.step(&r, Input::new(0, 1, true));
    assert_eq!(p.animation(), Animation::ZeroGSwim);
    assert!(!p.is_standing());
    assert!(p.body.chunks[HEAD].vel.y > before.y);
}

/// `Jump()` returns at once in the zero-gravity animations: the press is spent, nothing
/// moves.
#[test]
fn zero_g_jump_does_nothing() {
    let r = floor_room();
    let mut p = Player::floating_at(Vec2::new(100.0, 200.0));
    p.step(&r, Input::NONE);
    p.can_jump = 6;
    p.want_to_jump = 3;
    p.step(&r, Input::NONE);
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.0);
    assert_eq!((p.can_jump, p.want_to_jump), (0, 0));
}

/// In ZeroG `privSneak` is 0.5 and both chunks are `0.5 * loudnessFac` loud.
#[test]
fn zero_g_loudness() {
    let r = floor_room();
    let mut p = Player::floating_at(Vec2::new(100.0, 200.0));
    p.step(&r, Input::NONE);
    p.step(&r, Input::NONE);
    assert_eq!(p.priv_sneak(), 0.5);
    assert_close(
        p.body.chunks[HEAD].loudness,
        0.5 * p.stats.loudness_fac,
        "head",
    );
    assert_close(
        p.body.chunks[HIPS].loudness,
        0.5 * p.stats.loudness_fac,
        "hips",
    );
}

/// Away from beams and walls the zero-gravity animation list is the swim; the `room`
/// helper builds the plain open rooms the other tests use.
#[test]
fn open_room_is_a_swim() {
    let rows = vec![".".repeat(20); 10];
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let mut p = Player::floating_at(Vec2::new(100.0, 100.0));
    p.step(&r, Input::NONE);
    assert_eq!(p.animation(), Animation::ZeroGSwim);
}

/// A jump press in a tunnel at gravity 0 kicks off the wall behind the hips: `vector` = (1,
/// 0), +12 px and +7 velocity on both chunks (then the slide damping 0.9), the slide
/// counter 25 and stun 5; four gated drips (one draw each at no ceiling drips).
#[test]
fn zero_g_tunnel_kick_off_the_back_wall() {
    use super::corridor::tunnel_room;
    let r = tunnel_room();
    let mut p = Player::in_corridor_at(Vec2::new(87.0, 112.0), Vec2::new(70.0, 112.0));
    p.set_room_gravity(0.0);
    for c in &mut p.body.chunks {
        c.vel.y = 0.0;
    }
    let mut rng = UnityRandom::new(9);
    let mut mirror = rng.clone();
    p.update(
        &r,
        None,
        Input::new(0, 0, true),
        &mut rng,
        &mut Sounds::default(),
    );
    for _ in 0..4 {
        mirror.value();
    }
    assert_eq!(rng, mirror);
    assert_close(p.body.chunks[HEAD].pos.x, 99.0, "head x");
    assert_close(p.body.chunks[HIPS].pos.x, 82.0, "hips x");
    assert_close(p.body.chunks[HIPS].vel.x, 6.3, "hips vel");
    assert_eq!(p.horizontal_corridor_slide_counter, 25);
    assert_eq!(p.slow_movement_stun, 5);
}

/// In a wet room each of the four tries draws the gate, `RNV`'s angle and the drip's life.
#[test]
fn zero_g_tunnel_kick_drips_draw_in_order() {
    use super::corridor::tunnel_room;
    let r = tunnel_room();
    let mut p = Player::in_corridor_at(Vec2::new(87.0, 112.0), Vec2::new(70.0, 112.0));
    p.set_room_gravity(0.0);
    p.set_ceiling_drips(0.5);
    let mut rng = UnityRandom::new(9);
    let mut mirror = rng.clone();
    p.update(
        &r,
        None,
        Input::new(0, 0, true),
        &mut rng,
        &mut Sounds::default(),
    );
    let drips = p.take_drips();
    assert_eq!(drips.len(), 4);
    let mut first = None;
    for _ in 0..4 {
        mirror.value();
        let angle = mirror.value() * 360.0;
        let life = mirror.range_i32(10, 120);
        first.get_or_insert((angle, life));
    }
    assert_eq!(rng, mirror);
    let (angle, life) = first.expect("drip");
    assert_eq!(drips[0].life_time, life);
    let want = Vec2::new(5.0, 0.0) + Vec2::from_degrees(angle) * 3.0;
    assert_vec(drips[0].vel, want.x, want.y);
}

/// A corridor drop (jump with down over an open bottom) needs gravity.
#[test]
fn corridor_drop_needs_gravity() {
    use super::corridor::shaft_room;
    let r = shaft_room();
    let mut p = Player::in_corridor_at(Vec2::new(110.0, 193.0), Vec2::new(110.0, 210.0));
    p.set_room_gravity(0.0);
    p.step(&r, Input::new(0, -1, true));
    assert!(!p.corridor_drop);
}
