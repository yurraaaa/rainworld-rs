//! Poses and hands on poles and beams, and the roll.

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::{Animation, HEAD, HIPS, Input, Player, Vec2};

use crate::player::tests::{grounded_player, pole_room, run};
use crate::tests::assert_vec;
use crate::{Color, PlayerGraphics, SlugcatName, elements};

/// A player standing on the pole room's ground in the grounded pole's column.
fn player_at_pole(rng: &mut UnityRandom, room: &Room) -> (Player, PlayerGraphics) {
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(212.0, 49.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(212.0, 66.0));
    let mut g = PlayerGraphics::new(&p, SlugcatName::White, rng);
    run(&mut p, &mut g, room, Input::NONE, 20, rng);
    (p, g)
}

/// Runs until `animation` (at most `ticks` ticks); true if reached.
fn run_until(
    (player, graphics): (&mut Player, &mut PlayerGraphics),
    room: &Room,
    input: Input,
    animation: Animation,
    ticks: usize,
    rng: &mut UnityRandom,
) -> bool {
    for _ in 0..ticks {
        run(player, graphics, room, input, 1, rng);
        if player.animation() == animation {
            return true;
        }
    }
    false
}

#[test]
fn vertical_pole_poses() {
    let mut rng = UnityRandom::new(7);
    let r = pole_room();
    let up = Input::new(0, 1, false);
    let (mut p, mut g) = player_at_pole(&mut rng, &r);
    let climbing = Animation::ClimbOnBeam;
    assert!(run_until((&mut p, &mut g), &r, up, climbing, 20, &mut rng));
    // Climbing: LegsAVerticalPole mirrored by the facing, kept within -6..+4 px of the
    // hips draw position; hands gripping the pole show OnTopOfTerrainHand caps at the
    // hand (no 3 px lift) and hide their arms.
    let mut caps = 0;
    for _ in 0..30 {
        run(&mut p, &mut g, &r, up, 1, &mut rng);
        if p.animation() != climbing {
            break;
        }
        let list = g.draw(&p, 1.0, Color::BLACK, Color::BLACK);
        let legs = list.sprite(4).unwrap();
        assert_eq!(legs.element, elements::LEGS_A_VERTICAL_POLE);
        let facing = if p.flip_direction() > 0 { 1.0 } else { -1.0 };
        assert_eq!(legs.scale.x, facing);
        let hips_y = g.draw_positions[1][0].y;
        assert!(
            (hips_y - 6.0..=hips_y + 4.0).contains(&legs.pos.y),
            "legs y {} hips y {hips_y}",
            legs.pos.y
        );
        for j in 0..2 {
            let hand = &g.hands[j].limb;
            if let Some(cap) = list.sprite(7 + j) {
                caps += 1;
                assert!(hand.reached_snap_position);
                assert_eq!(cap.element, elements::ON_TOP_OF_TERRAIN_HAND);
                assert_vec(cap.pos, hand.part.pos.x, hand.part.pos.y, "cap at hand");
                assert!(list.sprite(5 + j).is_none(), "gripping arm hidden");
            }
        }
    }
    assert!(caps > 0, "a hand gripped the pole");
    // Over the top: GetUpToBeamTip sets the disbalance to 120, then BeamTip draws
    // LegsAPole.
    let tip_climb = Animation::GetUpToBeamTip;
    assert!(run_until(
        (&mut p, &mut g),
        &r,
        up,
        tip_climb,
        200,
        &mut rng
    ));
    assert_eq!(g.disbalance_amount, 120.0);
    let tip = Animation::BeamTip;
    assert!(run_until(
        (&mut p, &mut g),
        &r,
        Input::NONE,
        tip,
        40,
        &mut rng
    ));
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    assert_eq!(p.animation(), tip);
    let legs = *g
        .draw(&p, 1.0, Color::BLACK, Color::BLACK)
        .sprite(4)
        .unwrap();
    assert_eq!(legs.element, elements::LEGS_A_POLE);

    // Below the hanging pole (bottom tile row 8, y 160..180), up held: hang under it.
    let mut p = Player::new();
    p.body.chunks[HEAD].reset_at(Vec2::new(410.0, 150.0));
    p.body.chunks[HIPS].reset_at(Vec2::new(410.0, 133.0));
    let mut g = PlayerGraphics::new(&p, SlugcatName::White, &mut rng);
    let hang = Animation::HangUnderVerticalBeam;
    assert!(run_until((&mut p, &mut g), &r, up, hang, 5, &mut rng));
    let mut arms = 0;
    for _ in 0..10 {
        run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
        if p.animation() != hang {
            break;
        }
        // The arms' scale y is +1 for hand 0 and -1 for hand 1.
        let list = g.draw(&p, 1.0, Color::BLACK, Color::BLACK);
        for j in 0..2 {
            if let Some(arm) = list.sprite(5 + j) {
                arms += 1;
                assert_eq!(arm.scale.y, if j == 0 { 1.0 } else { -1.0 });
            }
        }
        // Hand targets: the head tile's middle plus (-0.5, 20) / (0.5, 25).
        let mid = p.head().pos.tile_middle();
        let targets = [
            g.hands[0].limb.absolute_hunt_pos,
            g.hands[1].limb.absolute_hunt_pos,
        ];
        assert_vec(targets[0], mid.x - 0.5, mid.y + 20.0, "hand 0");
        assert_vec(targets[1], mid.x + 0.5, mid.y + 25.0, "hand 1");
    }
    assert!(arms > 0);
}

#[test]
fn climb_offsets_and_hand_targets() {
    // ClimbOnBeam at animation frame k, facing f: body draw x += 2.5f + 0.5f sin(k/20 2pi),
    // hips draw x += 2.5f cos(k/20 2pi); the near hand (limb 1 when facing right) hunts
    // (mid.x - f, head.y - 3 + 6 sin), the other (mid.x + f, head.y + 3 + 6 cos).
    let mut rng = UnityRandom::new(8);
    let r = pole_room();
    let up = Input::new(0, 1, false);
    let (mut p, mut g) = player_at_pole(&mut rng, &r);
    assert!(run_until(
        (&mut p, &mut g),
        &r,
        up,
        Animation::ClimbOnBeam,
        20,
        &mut rng
    ));
    for _ in 0..8 {
        run(&mut p, &mut g, &r, up, 1, &mut rng);
        assert_eq!(p.animation(), Animation::ClimbOnBeam);
        let f = p.flip_direction() as f32;
        let phase = p.animation_frame() as f32 / 20.0 * std::f32::consts::TAU;
        let (head, hips) = (p.head().pos, p.hips().pos);
        // No look direction and no balance wobble while climbing.
        let body_x = head.x + f * 2.5 + f * 0.5 * phase.sin();
        assert_vec(g.draw_positions[0][0], body_x, head.y, "body draw");
        let hips_x = hips.x + f * 2.5 * phase.cos();
        assert_vec(g.draw_positions[1][0], hips_x, hips.y, "hips draw");
        let mid_x = head.tile_middle().x;
        let near = usize::from(p.flip_direction() == 1);
        let near_y = head.y - 3.0 + 6.0 * phase.sin();
        assert_vec(
            g.hands[near].limb.absolute_hunt_pos,
            mid_x - f,
            near_y,
            "near hand",
        );
        let far_y = head.y + 3.0 + 6.0 * phase.cos();
        let far = g.hands[1 - near].limb.absolute_hunt_pos;
        assert_vec(far, mid_x + f, far_y, "far hand");
    }
}

#[test]
fn roll_flings_the_tail_along_the_body() {
    // Push 6 at the root, divided by 1.7 per segment: 6, 3.5294, 2.0761, 1.2213.
    let mut rng = UnityRandom::new(9);
    let (_, mut g) = grounded_player(&mut rng);
    for seg in &mut g.tail {
        seg.part.vel = Vec2::ZERO;
    }
    g.fling_tail(Vec2::new(-1.0, 0.0));
    for (seg, push) in g.tail.iter().zip([6.0, 3.529_412, 2.076_125, 1.221_25]) {
        assert_vec(seg.part.vel, -push, 0.0, "tail push");
    }
}
