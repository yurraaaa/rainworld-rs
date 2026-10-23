//! Zero gravity: the wobble, tail, legs, hands and face.

use rw_random::UnityRandom;
use rw_sim::{Animation, HEAD, HIPS, Input, Player, Vec2};

use crate::player::tests::{pole_room, run, update_alone};
use crate::tests::{assert_close, assert_vec, flat_room};
use crate::{Color, PlayerGraphics, SlugcatName, elements};

/// A floating player (room gravity 0) with its graphics: body vertical, at rest.
fn floating_player(rng: &mut UnityRandom) -> (Player, PlayerGraphics) {
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(100.0, 200.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(100.0, 217.0));
    p.set_room_gravity(0.0);
    let g = PlayerGraphics::new(&p, SlugcatName::White, rng);
    (p, g)
}

/// `PlayerGraphics.Update` in `ZeroG`: the disbalance falls by 1, the balance counter
/// advances `1 + disbalance / 40 * (1 + Random.value)` (one draw), and the tail is thrown
/// back along the body and sideways by the wobble.
#[test]
fn zero_g_wobble_draws_once_and_throws_the_tail() {
    let mut rng = UnityRandom::new(4);
    let (p, mut g) = floating_player(&mut rng);
    g.disbalance_amount = 11.0;
    g.balance_counter = 100.0;
    for seg in &mut g.tail {
        seg.part.vel = Vec2::ZERO;
    }
    g.draw_positions[0][0] = p.head().pos;
    let mut mirror = rng.clone();
    g.zero_g_balance(&p, &mut rng);
    let v = mirror.value();
    assert_eq!(rng, mirror);
    let counter = 100.0 + 1.0 + 10.0 / 40.0 * (1.0 + v);
    assert_close(g.balance_counter, counter, "counter");
    assert_close(g.disbalance_amount, 10.0, "disbalance");
    // Body vertical: along (0, 1), across (-1, 0).
    let w = (counter / 300.0 * std::f32::consts::TAU).sin();
    assert_vec(g.tail[0].part.vel, w * 30.0 * 0.1, -(10.0 * 0.1), "tail 0");
    assert_vec(
        g.tail[1].part.vel,
        w * 30.0 * 0.04,
        -(10.0 * 0.04),
        "tail 1",
    );
    assert_vec(
        g.draw_positions[0][0],
        100.0 - w * 30.0 * 0.08,
        217.0,
        "head",
    );
}

/// The tail droops `Lerp(0.1, 0.5, drag) * EffectiveRoomGravity` a tick: nothing in a room
/// without gravity.
#[test]
fn the_tail_droops_with_the_room_gravity() {
    let r = flat_room();
    let mut vel_y = Vec::new();
    for gravity in [1.0, 0.0] {
        let mut rng = UnityRandom::new(4);
        let (mut p, mut g) = floating_player(&mut rng);
        p.set_room_gravity(gravity);
        for seg in &mut g.tail {
            seg.part.vel = Vec2::ZERO;
        }
        g.update_tail(&p, &r, None, false, 1.0);
        vel_y.push(g.tail[0].part.vel.y);
    }
    // Segment 0 at drag 1 and no water: 0.1 + 0.4 = 0.5.
    assert_close(vel_y[0] - vel_y[1], -0.5, "droop");
}

/// Floating, the legs trail along the body and `legsDirection` is head to hips.
#[test]
fn zero_g_legs_follow_the_body() {
    let r = pole_room();
    let mut rng = UnityRandom::new(4);
    let (mut p, mut g) = floating_player(&mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 2, &mut rng);
    assert_eq!(p.animation(), Animation::ZeroGSwim);
    let along = p.head().pos.direction_to(p.hips().pos);
    assert_vec(g.legs_direction, along.x, along.y, "legs direction");
}

/// On a vertical pole in zero gravity the hands alternate along it: standing, hand 0 leads
/// (`sin(0)` = 0 so 3 above the head) and hand 1 follows (`cos(0)` = 1: 9 below), both on
/// the tile middle.
#[test]
fn zero_g_hands_alternate_along_a_pole() {
    let r = pole_room();
    let mut rng = UnityRandom::new(4);
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(210.0, 150.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(210.0, 167.0));
    p.set_room_gravity(0.0);
    let mut g = PlayerGraphics::new(&p, SlugcatName::White, &mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    assert_eq!(p.animation(), Animation::ZeroGPoleGrab);
    let head = p.head().pos;
    assert_vec(
        g.hands[0].limb.absolute_hunt_pos,
        210.0,
        head.y + 3.0,
        "hand 0",
    );
    assert_vec(
        g.hands[1].limb.absolute_hunt_pos,
        210.0,
        head.y - 9.0,
        "hand 1",
    );
    // Set to 40, then the busy hand's `Max(retractCounter - 10, 0)`.
    assert_eq!(g.hands[0].retract_counter, 30);
}

/// Floating, a hand whose side swam through a coral circuit bit within 30 px of the head
/// hunts that bit's position (`swimBits[limbNumber]`); a bit beyond 30 px or none leaves the
/// hand to the rest of the swim.
#[test]
fn zero_g_hands_reach_for_the_swim_bits() {
    let r = flat_room();
    let mut rng = UnityRandom::new(4);
    let (mut p, mut g) = floating_player(&mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 2, &mut rng);
    let head = p.head().pos;
    // The bit lies on side 1 of the body line (a positive distance to the line head-hips).
    let bit = head + Vec2::new(12.0, -5.0);
    let bit_ref = rw_sim::BitRef {
        room: 0,
        circuit: 0,
        bit: 3,
    };
    p.collide_with_coral_circuit_bit(0, bit_ref, bit, 0.5, true);
    let side = p
        .swim_bits()
        .iter()
        .position(Option::is_some)
        .expect("remembered");
    update_alone(&mut g, &p, &r, &mut rng);
    assert_eq!(
        g.hands[side].limb.mode,
        crate::LimbMode::HuntAbsolutePosition
    );
    assert_vec(
        g.hands[side].limb.absolute_hunt_pos,
        bit.x,
        bit.y,
        "hand on the bit",
    );
    // Out of reach (35 px from the head): the hand is not sent to it.
    p.refresh_swim_bits([Some(head + Vec2::new(35.0, 0.0)); 2]);
    update_alone(&mut g, &p, &r, &mut rng);
    assert_ne!(
        g.hands[side].limb.absolute_hunt_pos,
        head + Vec2::new(35.0, 0.0)
    );
}

/// The face in zero gravity is the default one (frame 0, by blink), the head frame 0, and
/// it turns with the head; unconscious it is the dead or stunned one.
#[test]
fn zero_g_face_is_the_default_one() {
    let r = flat_room();
    let mut rng = UnityRandom::new(4);
    let (mut p, mut g) = floating_player(&mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 2, &mut rng);
    let black = Color::rgb(0.1, 0.0, 0.05);
    g.blink = 0;
    let list = g.draw(&p, 1.0, black, Color::BLACK);
    let (face, head) = (*list.sprite(9).unwrap(), *list.sprite(3).unwrap());
    assert_eq!(face.element, elements::FACE_A[0]);
    assert_eq!(head.element, "HeadA0");
    assert_eq!(face.rotation, head.rotation);
    g.blink = 3;
    let face = *g.draw(&p, 1.0, black, Color::BLACK).sprite(9).unwrap();
    assert_eq!(face.element, elements::FACE_B[0]);
    p.die(&mut UnityRandom::new(0));
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    let face = *g.draw(&p, 1.0, black, Color::BLACK).sprite(9).unwrap();
    assert_eq!(face.element, elements::FACE_DEAD);
}
