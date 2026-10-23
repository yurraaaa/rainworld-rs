//! The player's side of the coral circuit: `CollideWithCoralCircuitBit` (resistance, jump
//! meter, the kick or the explosion, `swimBits` and their sound) and the `ZeroGSwim` terms
//! that read the three. Expected values come from `Player.cs` 8541-8670 and 8936-8962,
//! worked in the comments.

use rw_random::UnityRandom;

use super::sounds::{all_sounds, played};
use super::{assert_close, assert_vec};
use crate::player::{HEAD, HIPS, Input, Player};
use crate::tests::room;
use crate::{BitRef, SoundId, Sounds, Vec2};

const RIGHT: Input = Input::new(1, 0, false);

/// A player in zero gravity, hips at (250, 400), head 17 px above, at rest.
fn floating() -> Player {
    let mut p = Player::upright_at(Vec2::new(250.0, 400.0));
    p.set_room_gravity(0.0);
    p
}

fn bit(n: u32) -> BitRef {
    BitRef {
        room: 0,
        circuit: 0,
        bit: n,
    }
}

/// The wide open room the swim tests use.
fn open_room() -> rw_assets::Room {
    let rows = vec![".".repeat(30); 60];
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// With a direction held and the jump meter at 0, a collision raises the resistance to the
/// largest overlap, kicks the bit by `Lerp(dir, DirVec(hips, head), 0.3) * 3 *
/// InverseLerp(1.5, 2.5, swimCycle)` = `(0.7, 0.3) * 1.5` at swimCycle 2 and counts one on
/// the meter.
#[test]
fn a_collision_raises_the_resistance_and_kicks_the_bit() {
    let mut p = floating();
    p.input[0] = RIGHT;
    p.swim_cycle = 2.0;
    let hit = p.collide_with_coral_circuit_bit(1, bit(0), Vec2::new(260.0, 410.0), 0.6, true);
    assert_close(p.circuit_swim_resistance(), 0.6, "resistance");
    assert_close(p.circuit_jump_meter(), 1.0, "meter");
    assert!(hit.explosion.is_none() && !hit.swim_sound);
    assert_vec(hit.bit_kick, 1.05, 0.45);
    // A smaller overlap leaves the resistance; a larger one raises it.
    p.collide_with_coral_circuit_bit(1, bit(0), Vec2::new(260.0, 410.0), 0.3, true);
    assert_close(p.circuit_swim_resistance(), 0.6, "resistance");
    p.collide_with_coral_circuit_bit(1, bit(0), Vec2::new(260.0, 410.0), 0.9, true);
    assert_close(p.circuit_swim_resistance(), 0.9, "resistance");
    assert_close(p.circuit_jump_meter(), 3.0, "meter");
}

/// No direction held: `DirVec(0, (0, 0))` is straight up, so the kick is
/// `Lerp((0, 1), (0, 1), 0.3) * 3 * InverseLerp(1.5, 2.5, swimCycle)`.
#[test]
fn without_a_direction_the_game_takes_up() {
    let mut p = floating();
    p.swim_cycle = 2.5;
    let hit = p.collide_with_coral_circuit_bit(1, bit(0), Vec2::new(260.0, 410.0), 0.6, true);
    assert_vec(hit.bit_kick, 0.0, 3.0);
    p.swim_cycle = 1.5;
    let hit = p.collide_with_coral_circuit_bit(1, bit(0), Vec2::new(260.0, 410.0), 0.6, true);
    assert_vec(hit.bit_kick, 0.0, 0.0);
}

/// After the circuit jump (meter below 0) a collision explodes the circuit at the hips:
/// radius 40, force 12, direction `Lerp(a, DirVec(hips, head), 0.5) * -6 + DirVec(head, bit)
/// * 6` = `(-3, -3) + (10, -7)/|(10, -7)| * 6`, and does not kick the bit.
#[test]
fn a_collision_after_the_jump_explodes_the_circuit() {
    let mut p = floating();
    p.input[0] = RIGHT;
    p.circuit_jump_meter = -1.0;
    let hit = p.collide_with_coral_circuit_bit(1, bit(0), Vec2::new(260.0, 410.0), 0.6, true);
    let e = hit.explosion.expect("explosion");
    assert_vec(e.pos, 250.0, 400.0);
    assert_eq!((e.rad, e.force), (40.0, 12.0));
    let len = (10.0f32 * 10.0 + 7.0 * 7.0).sqrt();
    assert_vec(e.dir_vec, -3.0 + 60.0 / len, -3.0 - 42.0 / len);
    assert_vec(hit.bit_kick, 0.0, 0.0);
    // The meter climbs back by one a collision.
    assert_close(p.circuit_jump_meter(), 0.0, "meter");
}

/// Only the main chunk (with a graphics module) remembers a bit, on the side of the body
/// line it lies: `DistanceToLine(bit, head, hips) < 0` is side 0, else 1. The first bit on a
/// side sounds; another on that side replaces it only when nearer to the head; the same
/// bit again changes nothing.
#[test]
fn the_main_chunk_remembers_the_bits_it_swims_through() {
    let mut p = floating();
    // Hips (250, 400), head (250, 417): (240, 410) is -10 from the line, (260, 410) +10.
    let left = Vec2::new(240.0, 410.0);
    let right = Vec2::new(260.0, 410.0);
    let hit = p.collide_with_coral_circuit_bit(1, bit(1), right, 0.5, true);
    assert!(!hit.swim_sound && p.swim_bits() == [None, None]);
    let hit = p.collide_with_coral_circuit_bit(0, bit(1), right, 0.5, false);
    assert!(!hit.swim_sound && p.swim_bits() == [None, None]);
    let hit = p.collide_with_coral_circuit_bit(0, bit(1), right, 0.5, true);
    assert!(hit.swim_sound);
    assert_eq!(p.swim_bits()[1].map(|s| s.bit), Some(bit(1)));
    let hit = p.collide_with_coral_circuit_bit(0, bit(1), right, 0.5, true);
    assert!(!hit.swim_sound);
    let hit = p.collide_with_coral_circuit_bit(0, bit(2), left, 0.5, true);
    assert!(hit.swim_sound);
    assert_eq!(p.swim_bits()[0].map(|s| s.bit), Some(bit(2)));
    // Farther from the head (20.2) than the held (12.2) on side 1: not taken.
    let far = Vec2::new(270.0, 410.0);
    let hit = p.collide_with_coral_circuit_bit(0, bit(3), far, 0.5, true);
    assert!(!hit.swim_sound);
    assert_eq!(p.swim_bits()[1].map(|s| s.bit), Some(bit(1)));
    let near = Vec2::new(251.0, 415.0);
    let hit = p.collide_with_coral_circuit_bit(0, bit(4), near, 0.5, true);
    assert!(hit.swim_sound);
    assert_eq!(p.swim_bits()[1].map(|s| s.bit), Some(bit(4)));
}

/// `ZeroGSwim` with nothing held: `circuitSwimResistance *= InverseLerp(speeds, 15, 9)` =
/// `(9 - 3) / (15 - 3)` = 0.5 for speeds adding to 3, both chunks slowed by `Lerp(1, 0.9,
/// resistance)`, then the resistance is 0 and the meter decays by 0.5 (clamped 0 to 4).
#[test]
fn the_swim_scales_the_resistance_and_slows_the_body() {
    let r = open_room();
    let mut p = floating();
    p.circuit_swim_resistance = 0.8;
    p.circuit_jump_meter = 2.0;
    p.body.chunks[HEAD].vel = Vec2::new(3.0, 0.0);
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    // 0.8 * 0.5 = 0.4: the chunks are slowed by 1 + (0.9 - 1) * 0.4 = 0.96.
    assert_vec(p.body.chunks[HEAD].vel, 2.88, 0.0);
    assert_close(p.circuit_swim_resistance(), 0.0, "resistance");
    assert_close(p.circuit_jump_meter(), 1.5, "meter");
    p.circuit_jump_meter = 0.2;
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    assert_close(p.circuit_jump_meter(), 0.0, "meter clamped at 0");
    p.circuit_jump_meter = 9.0;
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    assert_close(p.circuit_jump_meter(), 4.0, "meter clamped at 4");
    p.circuit_jump_meter = -1.0;
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    assert_close(p.circuit_jump_meter(), -0.5, "meter below 0 climbs");
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    assert_close(p.circuit_jump_meter(), 0.0, "meter back at 0");
}

/// A held direction adds `dir * (resistance * 0.5)` to the main chunk before the thrust: at
/// resistance 0.4 (after the speed factor) that is 0.2, then the unbraced thrust `LerpMap(
/// |head - hips| = 3.08, 1, 4, 0.1, fast)` with `fast = LerpMap(3.08, 4, 8, 0.15, 0.1)` =
/// 0.15.
#[test]
fn the_resistance_adds_to_the_thrust() {
    let r = open_room();
    let mut p = floating();
    p.input[0] = RIGHT;
    p.circuit_swim_resistance = 0.8;
    p.body.chunks[HEAD].vel = Vec2::new(3.0, 0.0);
    let mut rng = UnityRandom::new(3);
    let mut mirror = rng.clone();
    p.zero_g_swim(&r, &mut rng, &mut Sounds::default());
    mirror.value();
    assert_eq!(rng, mirror);
    let slowed = 3.0 * 0.96 + 0.2;
    let thrust = 0.1 + (0.15 - 0.1) * ((slowed - 1.0) / 3.0);
    assert_vec(p.body.chunks[HEAD].vel, slowed + thrust, 0.0);
    assert_vec(p.body.chunks[HIPS].vel, -0.1, 0.0);
}

/// A jump press with the meter at 3 and no brace throws the body: `Lerp(dir, DirVec(hips,
/// head), 0.5) * (4.4 * (0.5 + 0.5 * resistance))` on the head, `* 4 * ...` on the hips,
/// the jump sound, the meter -1 (then up half a step), the press spent.
#[test]
fn the_circuit_jump_throws_the_body() {
    let r = open_room();
    let mut p = floating();
    p.input[0] = RIGHT;
    p.circuit_jump_meter = 3.0;
    p.want_to_jump = 3;
    let mut sounds = all_sounds();
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut sounds);
    // dir (1, 0), axis (0, 1): (0.5, 0.5); factor 4.4 * 0.5 and 4 * 0.5. The thrust gave the
    // head (0.1, 0) and the hips (-0.1, 0) first.
    assert_vec(p.body.chunks[HEAD].vel, 0.1 + 1.1, 1.1);
    assert_vec(p.body.chunks[HIPS].vel, -0.1 + 1.0, 1.0);
    assert_eq!((p.can_jump, p.want_to_jump), (0, 0));
    assert_close(p.circuit_jump_meter(), -0.5, "meter");
    p.flush_sounds(&mut UnityRandom::new(3), &mut sounds);
    assert_eq!(
        played(&mut sounds),
        [(SoundId::SlugcatNormalJump, HEAD, 1.0)]
    );
}

/// Under 3 on the meter, or without a press, there is no circuit jump.
#[test]
fn the_circuit_jump_needs_the_meter_and_a_press() {
    let r = open_room();
    for (meter, press) in [(2.5, 3), (3.0, 0)] {
        let mut p = floating();
        p.input[0] = RIGHT;
        p.circuit_jump_meter = meter;
        p.want_to_jump = press;
        p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
        assert_vec(p.body.chunks[HEAD].vel, 0.1, 0.0);
        assert_eq!(p.want_to_jump, press);
    }
}

/// A remembered bit more than 50 px from the main chunk is let go, one nearer kept.
#[test]
fn swim_bits_are_dropped_beyond_fifty_pixels() {
    let r = open_room();
    let mut p = floating();
    p.collide_with_coral_circuit_bit(0, bit(1), Vec2::new(260.0, 410.0), 0.5, true);
    p.collide_with_coral_circuit_bit(0, bit(2), Vec2::new(240.0, 410.0), 0.5, true);
    assert!(p.swim_bits().iter().all(Option::is_some));
    // The main chunk is at (250, 417): 49 px away is kept.
    p.refresh_swim_bits([Some(Vec2::new(250.0, 466.0)), Some(Vec2::new(250.0, 466.0))]);
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    assert!(p.swim_bits().iter().all(Option::is_some));
    p.refresh_swim_bits([Some(Vec2::new(250.0, 468.0)), None]);
    p.zero_g_swim(&r, &mut UnityRandom::new(3), &mut Sounds::default());
    assert!(p.swim_bits()[0].is_none() && p.swim_bits()[1].is_some());
}
