//! The player on climbable vines: the grab in `MovementUpdate` and the `VineGrab` animation.
//! Expected values come from the formulas of `Player.cs` (MovementUpdate 12362-12431,
//! UpdateAnimation 8815-8928), written next to each assertion. Vines are test-only
//! (`climbable_vines_tests::TestVine`): a straight one at x = 200 from y = 100 to 400
//! (points at 100, 250, 400, 300 px long) unless a test builds another. Heavy vines
//! (mass 1000) hardly move when pushed, so the geometry stays put.

use rw_random::UnityRandom;

use super::sounds::{all_sounds, played};
use super::{assert_close, assert_vec};
use crate::climbable_vines::tests::{TestVine, system_of};
use crate::climbable_vines::{VineEvent, VineId, VineKind, VinePosition};
use crate::player::{Animation, BodyMode, HEAD, HIPS, Input, Player};
use crate::tests::room;
use crate::{SoundId, Vec2};

fn vine(id: u64, mass: f32) -> TestVine {
    let mut v = TestVine::new(id, &[(200.0, 100.0), (200.0, 250.0), (200.0, 400.0)]);
    v.masses = vec![mass; 3];
    v
}

/// A player whose head is on the vine at (200, 300), hips 17 px below.
fn on_the_vine(gravity: f32) -> Player {
    let mut p = Player::upright_at(Vec2::new(200.0, 283.0));
    p.room_gravity = gravity;
    p
}

/// `on_the_vine`, already holding vine 1 at the head's place, with `vines` lent.
fn holding(gravity: f32, vines: Vec<TestVine>) -> Player {
    let mut p = on_the_vine(gravity);
    p.animation = Animation::VineGrab;
    p.vine_pos = Some(VinePosition::new(VineId(1), 200.0 / 300.0));
    p.lend_vines(Some(system_of(vines)));
    p
}

fn press(p: &mut Player, input: Input) {
    p.input[1] = p.input[0];
    p.input[0] = input;
}

const UP: Input = Input::new(0, 1, false);
const DOWN: Input = Input::new(0, -1, false);
const RIGHT: Input = Input::new(1, 0, false);

/// With gravity an up press grabs the first vine on the path the main chunk took, sampled
/// `(int)(distance / 5)` times (at most 10) from `lastPos` to `pos`; the grab clears
/// `wantToGrab`.
#[test]
fn up_press_at_gravity_grabs_the_first_vine_along_the_path() {
    // From x = 150 to 250 at y = 300: 20 samples capped to 10, at x = 150 + 100 m / 9.
    // The head (radius 9) is within 11 px of x = 200 for m = 4 and 5, of x = 230 for m = 7.
    let mut p = on_the_vine(1.0);
    p.body.chunks[HEAD].last_pos = Vec2::new(150.0, 300.0);
    p.body.chunks[HEAD].pos = Vec2::new(250.0, 300.0);
    let mut second = vine(2, 1.0);
    for point in &mut second.points {
        point.x = 230.0;
    }
    p.lend_vines(Some(system_of(vec![second.clone(), vine(1, 1.0)])));
    press(&mut p, UP);
    p.want_to_grab = 1;
    p.vine_grab();
    assert_eq!(p.animation(), Animation::VineGrab);
    assert_eq!(p.vine_pos().unwrap().vine, VineId(1));
    // y = 300 is 200 px of 300 along.
    assert_close(p.vine_pos().unwrap().float_pos, 200.0 / 300.0, "float");
    assert_eq!(p.want_to_grab(), 0);

    // Only the other vine: found at m = 7.
    let mut p = on_the_vine(1.0);
    p.body.chunks[HEAD].last_pos = Vec2::new(150.0, 300.0);
    p.body.chunks[HEAD].pos = Vec2::new(250.0, 300.0);
    p.lend_vines(Some(system_of(vec![second])));
    press(&mut p, UP);
    p.vine_grab();
    assert_eq!(p.vine_pos().unwrap().vine, VineId(2));
}

/// The grab needs an up press, or last tick's `wantToGrab`; the regrab delay counts down
/// first and blocks until it is 0; no direction held at gravity 0 grabs nothing.
#[test]
fn grab_conditions() {
    let mut p = on_the_vine(1.0);
    p.lend_vines(Some(system_of(vec![vine(1, 1.0)])));
    press(&mut p, Input::NONE);
    p.vine_grab();
    assert_eq!(p.animation(), Animation::None);
    // Last tick's up press.
    p.want_to_grab = 1;
    p.vine_grab();
    assert_eq!(p.animation(), Animation::VineGrab);

    // A delay of 2 blocks one tick, then the countdown reaches 0 and the grab is made.
    let mut p = on_the_vine(1.0);
    p.lend_vines(Some(system_of(vec![vine(1, 1.0)])));
    press(&mut p, UP);
    p.vine_grab_delay = 2;
    p.vine_grab();
    assert_eq!((p.animation(), p.vine_grab_delay()), (Animation::None, 1));
    p.vine_grab();
    assert_eq!(
        (p.animation(), p.vine_grab_delay()),
        (Animation::VineGrab, 0)
    );

    // At gravity 0 only a held direction grabs, up, down, left or right alike.
    for (input, grabs) in [
        (Input::NONE, false),
        (UP, true),
        (DOWN, true),
        (RIGHT, true),
        (Input::new(-1, 0, false), true),
    ] {
        let mut p = on_the_vine(0.0);
        p.lend_vines(Some(system_of(vec![vine(1, 1.0)])));
        press(&mut p, input);
        p.vine_grab();
        assert_eq!(p.animation() == Animation::VineGrab, grabs, "{input:?}");
    }
    // At gravity 0 the one sample is the chunk's position: 12 px off is out of reach.
    let mut p = on_the_vine(0.0);
    p.body.chunks[HEAD].pos.x = 212.0;
    p.lend_vines(Some(system_of(vec![vine(1, 1.0)])));
    press(&mut p, UP);
    p.vine_grab();
    assert_eq!(p.animation(), Animation::None);
}

/// With Remix off a vine can be grabbed while climbing a pole; a room without vines grabs
/// nothing; a grabbed vine that left the list lets go.
#[test]
fn grab_on_a_pole_and_without_vines() {
    let mut p = on_the_vine(1.0);
    p.animation = Animation::ClimbOnBeam;
    p.lend_vines(Some(system_of(vec![vine(1, 1.0)])));
    press(&mut p, UP);
    p.vine_grab();
    assert_eq!(p.animation(), Animation::VineGrab);

    let mut p = on_the_vine(1.0);
    press(&mut p, UP);
    p.vine_grab();
    assert_eq!(p.animation(), Animation::None);

    let mut p = holding(1.0, vec![vine(1, 1.0)]);
    p.vine_grab();
    assert_eq!(p.animation(), Animation::VineGrab);
    p.lend_vines(Some(system_of(vec![vine(2, 1.0)])));
    p.vine_grab();
    assert_eq!(p.animation(), Animation::None);
    let mut p = holding(1.0, vec![vine(1, 1.0)]);
    p.lend_vines(None);
    p.vine_grab();
    assert_eq!(p.animation(), Animation::None);
}

/// The vine's class picks the grab sound; a pole mimic's has none here.
#[test]
fn the_grab_sound_follows_the_vine_class() {
    for (kind, sound) in [
        (VineKind::CoralNeuron, Some(SoundId::GrabNeuron)),
        (VineKind::CoralStem, Some(SoundId::GrabCoralStem)),
        (
            VineKind::ClimbableCorruptionTube,
            Some(SoundId::GrabCorruptionTube),
        ),
        (VineKind::PoleMimic, None),
    ] {
        let mut v = vine(1, 1.0);
        v.kind = kind;
        let mut p = on_the_vine(1.0);
        p.lend_vines(Some(system_of(vec![v])));
        press(&mut p, UP);
        p.vine_grab();
        let mut sounds = all_sounds();
        p.flush_sounds(&mut UnityRandom::new(1), &mut sounds);
        let heard: Vec<SoundId> = played(&mut sounds).into_iter().map(|(id, ..)| id).collect();
        assert_eq!(heard, sound.into_iter().collect::<Vec<_>>(), "{kind:?}");
    }
}

/// Climbing up: the cursor takes the held direction at `Lerp`-mapped strength (6.5 from
/// rest, 3 once it already points that way), the place moves `Lerp(2.1, 1.5, gravity)` px
/// of the 300 along the sign of the goal against the vine, the head leans by `cursor /
/// 190` and the hips against it, and the hand cycle counts (wrapping past 30).
#[test]
fn climbing_up_at_gravity_1_and_0() {
    for (gravity, speed) in [(1.0, 1.5), (0.0, 2.1)] {
        let mut p = holding(gravity, vec![vine(1, 1000.0)]);
        press(&mut p, UP);
        p.animation_frame = 30;
        p.vine_grab_animation();
        assert_eq!(p.animation(), Animation::VineGrab, "gravity {gravity}");
        assert_eq!(p.body_mode(), BodyMode::Default);
        assert_vec(p.vine_climb_cursor(), 0.0, 6.5);
        assert_close(
            p.vine_pos().unwrap().float_pos,
            200.0 / 300.0 + speed / 300.0,
            "float",
        );
        assert_eq!(p.animation_frame(), 0);
        if gravity == 1.0 {
            assert_close(p.body.chunks[HIPS].vel.y, -6.5 / 190.0, "hips lean");
        }
        // The cursor already points up: 3 more, 9.5.
        press(&mut p, UP);
        p.vine_grab_animation();
        assert_vec(p.vine_climb_cursor(), 0.0, 9.5);
        assert_eq!(p.animation_frame(), 1);
        assert_close(
            p.vine_pos().unwrap().float_pos,
            200.0 / 300.0 + 2.0 * speed / 300.0,
            "float 2",
        );
    }
}

/// A goal below the point moves the place toward the start; with no direction held the
/// cursor decays by 0.8 and the place stays.
#[test]
fn climbing_down_and_letting_the_cursor_decay() {
    let mut p = holding(1.0, vec![vine(1, 1000.0)]);
    press(&mut p, DOWN);
    p.vine_grab_animation();
    assert_vec(p.vine_climb_cursor(), 0.0, -6.5);
    assert_close(
        p.vine_pos().unwrap().float_pos,
        200.0 / 300.0 - 1.5 / 300.0,
        "float",
    );
    let float = p.vine_pos().unwrap().float_pos;
    press(&mut p, Input::NONE);
    let frame = p.animation_frame();
    p.vine_grab_animation();
    assert_vec(p.vine_climb_cursor(), 0.0, -6.5 * 0.8);
    assert_eq!(p.vine_pos().unwrap().float_pos, float);
    assert_eq!(p.animation_frame(), frame);
}

/// A held direction across the vine (right) moves the cursor sideways and the place not at
/// all (dot 0); the crossing vine under the cursor takes over with a regrab delay of 10.
#[test]
fn a_crossing_vine_takes_over() {
    let mut across = TestVine::new(2, &[(50.0, 250.0), (200.0, 250.0), (350.0, 250.0)]);
    across.masses = vec![1000.0; 3];
    let mut p = holding(1.0, vec![vine(1, 1000.0), across]);
    p.body.chunks[HEAD].pos = Vec2::new(200.0, 250.0);
    p.body.chunks[HEAD].last_pos = Vec2::new(200.0, 250.0);
    p.vine_pos = Some(VinePosition::new(VineId(1), 0.5));
    press(&mut p, RIGHT);
    p.vine_grab_animation();
    assert_vec(p.vine_climb_cursor(), 6.5, 0.0);
    let now = p.vine_pos().unwrap();
    assert_eq!(now.vine, VineId(2));
    assert_close(now.float_pos, 0.5, "float");
    assert_eq!(p.vine_grab_delay(), 10);
    // With the delay running no further switch is looked for.
    p.vine_pos = Some(VinePosition::new(VineId(1), 0.5));
    press(&mut p, RIGHT);
    p.vine_grab_animation();
    assert_eq!(p.vine_pos().unwrap().vine, VineId(1));
}

/// A jump press (rising edge) with a direction flings the body that way (`+4` head, `+3.5`
/// hips, halfway to a cap of 5), kicks the vine back by 15 and sets the regrab delay 10; with
/// none held it lets go without delay.
#[test]
fn jumping_off_a_vine() {
    let mut p = holding(1.0, vec![vine(1, 1.0)]);
    press(&mut p, RIGHT);
    p.input[0].jump = true;
    p.input[1].jump = false;
    p.vine_grab_animation();
    assert_eq!(p.animation(), Animation::None);
    assert_eq!(p.vine_grab_delay(), 10);
    // Cursor (6.5, 0): lean 6.5 / 190 on top of the fling.
    let lean = 6.5 / 190.0;
    assert_close(p.body.chunks[HEAD].vel.x, 4.0 + lean, "head vx");
    assert_close(p.body.chunks[HIPS].vel.x, 3.5 - lean, "hips vx");
    // The last two pushes are the kick, (-15, 0) shared between the two points.
    let events = p.vines.as_mut().unwrap().take_events();
    let kick: Vec<Vec2> = events
        .iter()
        .rev()
        .take(2)
        .map(|e| match e {
            VineEvent::Push { movement, .. } => *movement,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_close(kick[0].x + kick[1].x, -15.0, "kick");

    let mut p = holding(1.0, vec![vine(1, 1.0)]);
    press(&mut p, Input::NONE);
    p.input[0].jump = true;
    p.input[1].jump = false;
    p.vine_grab_animation();
    assert_eq!((p.animation(), p.vine_grab_delay()), (Animation::None, 0));
    assert_vec(p.body.chunks[HEAD].vel, 0.0, 0.0);

    // Held jump (no edge) keeps the grip.
    let mut p = holding(1.0, vec![vine(1, 1.0)]);
    press(&mut p, RIGHT);
    p.input[0].jump = true;
    p.input[1].jump = true;
    p.vine_grab_animation();
    assert_eq!(p.animation(), Animation::VineGrab);
}

/// A vine that stops being climbable lets go with the delay; so does being 40 + its radius
/// away from the place on it.
#[test]
fn letting_go() {
    let mut v = vine(1, 1000.0);
    v.climbable = false;
    let mut p = holding(1.0, vec![v]);
    press(&mut p, Input::NONE);
    p.vine_grab_animation();
    assert_eq!((p.animation(), p.vine_grab_delay()), (Animation::None, 10));

    // Radius 2 there: 42 px is out, 41.9 holds.
    for (dx, holds) in [(41.9, true), (42.1, false)] {
        let mut p = holding(1.0, vec![vine(1, 1000.0)]);
        p.body.chunks[HEAD].pos.x += dx;
        press(&mut p, Input::NONE);
        p.vine_grab_animation();
        assert_eq!(p.animation() == Animation::VineGrab, holds, "{dx}");
        assert_eq!(p.vine_grab_delay(), 0);
    }
}

/// While held the chunk is leashed to the vine by its radius (a heavy vine hardly moves),
/// and the vine is told it is being climbed, once a tick.
#[test]
fn the_chunk_is_held_to_the_vine() {
    let mut p = holding(1.0, vec![vine(1, 1000.0)]);
    p.body.chunks[HEAD].pos.x += 10.0;
    press(&mut p, Input::NONE);
    p.vine_grab_animation();
    // 10 px away, leash 2: 8 px pulled in (the share of the heavy vine is ~0).
    assert!((p.body.chunks[HEAD].pos.x - 202.0).abs() < 0.01);
    assert!((p.body.chunks[HEAD].vel.x + 8.0).abs() < 0.01);
    let events = p.vines.as_mut().unwrap().take_events();
    assert!(matches!(
        events[0],
        VineEvent::BeingClimbedOn {
            vine: VineId(1),
            ..
        }
    ));
}

/// A light vine (mass 1) climbed from rest: the place moves, the vine gets the push
/// `(before - after) * 0.05` shared between the two points by the fraction there.
#[test]
fn climbing_pushes_the_vine() {
    let mut p = holding(1.0, vec![vine(1, 1.0)]);
    press(&mut p, UP);
    p.vine_grab_animation();
    // The place moved 1.5 px up (201.5 of 300 px along): before - after = (0, -1.5), times
    // 0.05. It is (201.5 - 150) / 150 along the second segment, between points 1 and 2.
    let events = p.vines.as_mut().unwrap().take_events();
    let pushes: Vec<(i32, Vec2)> = events
        .iter()
        .filter_map(|e| match e {
            VineEvent::Push {
                index, movement, ..
            } => Some((*index, *movement)),
            VineEvent::BeingClimbedOn { .. } => None,
        })
        .collect();
    assert_eq!(pushes.len(), 2);
    assert_eq!(pushes[0].0, 1);
    let t = 51.5 / 150.0;
    assert_vec(pushes[0].1, 0.0, -0.075 * (1.0 - t));
    assert_eq!(pushes[1].0, 2);
    assert_vec(pushes[1].1, 0.0, -0.075 * t);
}

/// Gravity 0 draws the hips toward the nearer of the places 20 px behind and ahead on the
/// vine.
#[test]
fn zero_gravity_pulls_the_hips_along_the_vine() {
    let mut p = holding(0.0, vec![vine(1, 1000.0)]);
    press(&mut p, Input::NONE);
    p.vine_grab_animation();
    // Place at (200, 300); 20 px behind is (200, 280), 3 px below the hips at (200, 283) and
    // nearer than the point ahead: the pull is (0, -3) / 20 on the hips, the head the other
    // way.
    assert_close(p.body.chunks[HIPS].vel.y, -3.0 / 20.0, "hips");
    assert_close(p.body.chunks[HEAD].vel.y, 3.0 / 20.0, "head");
    // At gravity 1 there is no pull.
    let mut p = holding(1.0, vec![vine(1, 1000.0)]);
    press(&mut p, Input::NONE);
    p.vine_grab_animation();
    assert_vec(p.body.chunks[HIPS].vel, 0.0, 0.0);
}

/// The side push along the vine's perpendicular uses `zeroGPoleGrabDir` on the axis the
/// perpendicular lies more along, and the held direction updates it.
#[test]
fn the_side_push_follows_the_last_held_direction() {
    let mut p = holding(1.0, vec![vine(1, 1000.0)]);
    press(&mut p, RIGHT);
    p.vine_grab_animation();
    assert_eq!(p.zero_g_pole_grab_dir(), (1, 0));
    // Perpendicular of the up vine is (-1, 0): x dominates, 0.2 * dir.x = 0.2 to the left
    // (plus the cursor's lean of 6.5 / 190).
    assert_close(p.body.chunks[HEAD].vel.x, -0.2 + 6.5 / 190.0, "head vx");
}

/// The animation's and the grab's only random draws are their sounds' (the original's
/// own are behind the More Slugcats `ClimbableVine`).
#[test]
fn the_vine_code_makes_no_draws_of_its_own() {
    let mut p = on_the_vine(1.0);
    p.lend_vines(Some(system_of(vec![vine(1, 1.0)])));
    press(&mut p, UP);
    p.vine_grab();
    p.vine_grab_animation();
    let mut sounds = all_sounds();
    let mut rng = UnityRandom::new(3);
    p.flush_sounds(&mut rng, &mut sounds);
    let mut alone = on_the_vine(1.0);
    alone.play_sound(SoundId::GrabNeuron, HEAD, 1.0, 1.0);
    let mut expected = UnityRandom::new(3);
    alone.flush_sounds(&mut expected, &mut all_sounds());
    assert_eq!(rng, expected);
    assert_ne!(rng, UnityRandom::new(3));
}

/// Through `Player::update`: at gravity 0 an up press with the head on a vine grabs it in
/// the zero-gravity switch's place, the body mode stays `Default` (the switch skips
/// `VineGrab`), the lent vines come back, and a room without vines grabs nothing.
#[test]
fn a_whole_tick_at_gravity_0() {
    let row = ".".repeat(30);
    let r = room(&[row.as_str(); 30]);
    let mut p = on_the_vine(0.0);
    p.lend_vines(Some(system_of(vec![vine(1, 1000.0)])));
    let mut sounds = all_sounds();
    p.update(&r, None, UP, &mut UnityRandom::new(5), &mut sounds);
    assert_eq!(p.animation(), Animation::VineGrab);
    assert_eq!(p.body_mode(), BodyMode::Default);
    assert!(
        played(&mut sounds)
            .iter()
            .any(|(id, ..)| *id == SoundId::GrabNeuron)
    );
    assert!(p.take_vines().is_some());
    p.update(&r, None, UP, &mut UnityRandom::new(5), &mut all_sounds());
    assert_eq!(p.animation(), Animation::None, "no vines lent");
}
