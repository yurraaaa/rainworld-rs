//! `SlugcatHand`: the swallow's reach and the swing along a vine.

use rw_random::UnityRandom;
use rw_sim::{
    Animation, ClimbableVine, ClimbableVinesSystem, HEAD, HIPS, Input, NoiseSink, PlacedVines,
    Player, ScreenMovement, Sounds, Vec2, VineAccess, VineClimber, VineKind, VineRef,
};

use crate::player::tests::grounded_player;
use crate::tests::{assert_vec, flat_room, room};

#[test]
fn the_swallow_shakes_with_six_draws_from_halfway() {
    // `SlugcatHand.Update`: t = InverseLerp(10, 90, counter); below 0.5 the
    // hand is drawn in without draws, from 0.5 the eyes shut and the hand, the head and
    // the main chunk each take an `RNV` and a `value`.
    let r = flat_room();
    let mut rng = UnityRandom::new(3);
    let (p, _) = grounded_player(&mut rng);
    for (counter, draws) in [(49, 0), (50, 6), (90, 6)] {
        let mut hand = crate::player::hand::SlugcatHand::new(0);
        let (mut disbalance, mut head_vel, mut draw_pos0) = (0.0, Vec2::ZERO, Vec2::ZERO);
        let (mut blink, mut overlap, mut body_push) = (0, [None; 2], Vec2::ZERO);
        let mut rng = UnityRandom::new(9);
        let mut cx = crate::player::hand::HandContext {
            flail: 0.0,
            player: &p,
            room: &r,
            water: None,
            vines: None,
            balance_counter: 0.0,
            disbalance: &mut disbalance,
            head_vel: &mut head_vel,
            airborne_counter: 0.0,
            hand0: (Vec2::ZERO, false),
            rng: &mut rng,
            grasps: crate::player::hand::HandGrasp::default(),
            spear_dir: 0.0,
            draw_pos0: &mut draw_pos0,
            blink: &mut blink,
            overlap: &mut overlap,
            body_push: &mut body_push,
        };
        hand.swallow_reach(&mut cx, counter);
        let mut expect = UnityRandom::new(9);
        for _ in 0..draws {
            expect.value();
        }
        assert_eq!(rng.state(), expect.state(), "counter {counter}");
        assert_eq!(blink, if draws > 0 { 5 } else { 0 });
    }
}

/// A straight, heavy vine at x = 200 from y = 100 to 400 (points at 100, 250, 400, radius 2).
#[derive(Debug, Clone)]
struct StraightVine {
    points: [Vec2; 3],
}

impl ClimbableVine for StraightVine {
    fn id(&self) -> u64 {
        1
    }

    fn kind(&self) -> VineKind {
        VineKind::CoralNeuron
    }

    fn total_positions(&self) -> i32 {
        3
    }

    fn pos(&self, index: i32) -> Vec2 {
        self.points[index as usize]
    }

    fn rad(&self, _: i32) -> f32 {
        2.0
    }

    fn mass(&self, _: i32) -> f32 {
        1.0e6
    }

    fn push(&mut self, index: i32, movement: Vec2) {
        self.points[index as usize] += movement;
    }

    fn being_climbed_on(&mut self, _: &VineClimber, _: &mut UnityRandom, _: &mut Sounds) {}

    fn currently_climbable(&self) -> bool {
        true
    }
}

/// The room's vines as the player's update reaches them.
struct VineSink {
    system: ClimbableVinesSystem,
    host: PlacedVines,
}

impl NoiseSink for VineSink {
    fn in_game_noise(&mut self, _: Vec2, _: f32, _: f32, _: &mut UnityRandom) {}

    fn screen_movement(&mut self, _: ScreenMovement, _: &mut UnityRandom) {}

    fn vines(&mut self) -> Option<VineAccess<'_>> {
        Some(VineAccess {
            system: &self.system,
            host: rw_sim::RoomVines::placed_only(&mut self.host),
        })
    }
}

#[test]
fn hands_swing_along_the_vine() {
    // `SlugcatHand.EngageInMovement`, `VineGrab`: target = OnVinePos(float + 20 * side *
    // sin(frame / 30 * 2 pi) / length) + PerpendicularVector(VineDir) * (VineRad * side),
    // side -1 for hand 0, +1 for hand 1. The vine runs up, so its perpendicular is (-1, 0).
    let row = ".".repeat(30);
    let r = room(&[row.as_str(); 20]);
    let mut sink = VineSink {
        system: ClimbableVinesSystem::new(),
        host: PlacedVines::default(),
    };
    sink.system.register(VineRef::Placed { room: 0, id: 1 });
    sink.host.add(Box::new(StraightVine {
        points: [
            Vec2::new(200.0, 100.0),
            Vec2::new(200.0, 250.0),
            Vec2::new(200.0, 400.0),
        ],
    }));
    let mut player = Player::new();
    player.body.chunks[HIPS].reset_at(Vec2::new(200.0, 283.0));
    player.body.chunks[HEAD].reset_at(Vec2::new(200.0, 300.0));
    player.set_room_gravity(0.0);
    let (mut rng0, mut sounds) = (UnityRandom::new(0), Sounds::default());
    player.update_before_catch(&r, None, Input::new(0, 1, false), &mut rng0, &mut sounds);
    player.update_after_catch(&r, None, &mut rng0, &mut sounds, &mut sink);
    assert_eq!(player.animation(), Animation::VineGrab);
    let on = player.vine_pos().unwrap();
    let frame = player.animation_frame();
    assert_eq!(frame, 1);
    for limb in 0..2 {
        let side = if limb == 0 { -1.0_f32 } else { 1.0 };
        let mut hand = crate::player::hand::SlugcatHand::new(limb);
        let (mut disbalance, mut head_vel, mut draw_pos0) = (0.0, Vec2::ZERO, Vec2::ZERO);
        let (mut blink, mut overlap, mut body_push) = (0, [None; 2], Vec2::ZERO);
        let mut rng = UnityRandom::new(9);
        let mut cx = crate::player::hand::HandContext {
            flail: 0.0,
            player: &player,
            room: &r,
            water: None,
            vines: Some((
                &sink.system,
                rw_sim::RoomVinesView {
                    placed: &sink.host,
                    neurons: &[],
                    stems: &[],
                },
            )),
            balance_counter: 0.0,
            disbalance: &mut disbalance,
            head_vel: &mut head_vel,
            airborne_counter: 0.0,
            hand0: (Vec2::ZERO, false),
            rng: &mut rng,
            grasps: crate::player::hand::HandGrasp::default(),
            spear_dir: 0.0,
            draw_pos0: &mut draw_pos0,
            blink: &mut blink,
            overlap: &mut overlap,
            body_push: &mut body_push,
        };
        hand.update(&mut cx);
        let swing =
            20.0 * side * (1.0_f64 / 30.0 * 2.0 * std::f64::consts::PI).sin() as f32 / 300.0;
        let along = on.float_pos + swing;
        assert_vec(
            hand.limb.absolute_hunt_pos,
            200.0 - 2.0 * side,
            100.0 + along * 300.0,
            "hand",
        );
        assert_eq!(
            hand.limb.mode,
            crate::body::limb::LimbMode::HuntAbsolutePosition
        );
    }
}
