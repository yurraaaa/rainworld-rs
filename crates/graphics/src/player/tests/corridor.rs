//! Corridor climbing: offsets, head, legs and hands in tunnels and shafts.

use rw_assets::Room;
use rw_random::UnityRandom;
use rw_sim::{Animation, BodyMode, HEAD, HIPS, Input, Player, Sounds, Vec2};

use crate::player::tests::{place, run, update_alone};
use crate::tests::{EPS, assert_close, assert_vec, room};
use crate::{Color, PlayerGraphics, SlugcatName};

/// Solid 30 x 10 tiles with a horizontal tunnel along row y = 2 (x 1..=28, pixel
/// y 40..60).
fn tunnel_room() -> Room {
    let mut rows = Vec::new();
    for y in (0..10).rev() {
        let mut cells = vec![b'#'; 30];
        if y == 2 {
            cells[1..29].fill(b'.');
        }
        rows.push(String::from_utf8(cells).unwrap());
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// Solid 20 x 12 tiles with a vertical shaft in column x = 10 (y 1..=10, pixel x
/// 200..220).
fn shaft_room() -> Room {
    let mut rows = Vec::new();
    for y in (0..12).rev() {
        let mut cells = vec![b'#'; 20];
        if (1..=10).contains(&y) {
            cells[10] = b'.';
        }
        rows.push(String::from_utf8(cells).unwrap());
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// An open 20 x 12 room with Solid tiles at `solid` (x, y).
fn room_with_solid(solid: &[(usize, usize)]) -> Room {
    let mut cells = vec![vec![b'.'; 20]; 12];
    for &(x, y) in solid {
        cells[11 - y][x] = b'#';
    }
    let rows: Vec<String> = cells
        .into_iter()
        .map(|r| String::from_utf8(r).unwrap())
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

/// A player wedged in `room` with the head at `head` and the hips at `hips`, run
/// until it is in `CorridorClimb`.
fn corridor_player(
    room: &Room,
    (head, hips): (Vec2, Vec2),
    input: Input,
    rng: &mut UnityRandom,
) -> (Player, PlayerGraphics) {
    let mut p = Player::new();
    p.body.chunks[HEAD].reset_at(head);
    p.body.chunks[HIPS].reset_at(hips);
    let mut g = PlayerGraphics::new(&p, SlugcatName::White, rng);
    for _ in 0..10 {
        run(&mut p, &mut g, room, input, 1, rng);
        if p.body_mode() == BodyMode::CorridorClimb {
            return (p, g);
        }
    }
    panic!("never entered CorridorClimb: {:?}", p.body_mode());
}

/// Head at the middle of tunnel tile (10, 2), hips 17 px behind it.
const IN_TUNNEL: (Vec2, Vec2) = (Vec2::new(210.0, 50.0), Vec2::new(193.0, 50.0));

#[test]
fn corridor_has_no_draw_offsets() {
    // PlayerGraphics.Update has no CorridorClimb branch in its body-mode block and the
    // look direction is 0, so drawPositions[i][0] are the chunk positions.
    let mut rng = UnityRandom::new(20);
    let r = tunnel_room();
    let right = Input::new(1, 0, false);
    let start = (Vec2::new(110.0, 50.0), Vec2::new(93.0, 50.0));
    let (mut p, mut g) = corridor_player(&r, start, right, &mut rng);
    for _ in 0..20 {
        run(&mut p, &mut g, &r, right, 1, &mut rng);
        assert_eq!(p.body_mode(), BodyMode::CorridorClimb);
        assert_vec(
            g.draw_positions[0][0],
            p.head().pos.x,
            p.head().pos.y,
            "body",
        );
        assert_vec(
            g.draw_positions[1][0],
            p.hips().pos.x,
            p.hips().pos.y,
            "hips",
        );
    }
}

#[test]
fn corridor_head_leads_twice_as_far_downward() {
    // v5 = 3 dir(d1 -> d0); CorridorClimb doubles its y when negative (head below).
    let corridor = BodyMode::CorridorClimb;
    let down = PlayerGraphics::head_lead(corridor, Vec2::ZERO, Vec2::new(0.0, 20.0));
    assert_vec(down, 0.0, -6.0, "head first down a shaft");
    let up = PlayerGraphics::head_lead(corridor, Vec2::new(0.0, 20.0), Vec2::ZERO);
    assert_vec(up, 0.0, 3.0, "head up");
    // 3 * (0.6, -0.8) = (1.8, -2.4); y doubled to -4.8, x untouched.
    let (d0, d1) = (Vec2::new(6.0, 0.0), Vec2::new(0.0, 8.0));
    assert_vec(
        PlayerGraphics::head_lead(corridor, d0, d1),
        1.8,
        -4.8,
        "diagonal",
    );
    // Crawl multiplies x by 2.5 instead.
    let crawl = PlayerGraphics::head_lead(BodyMode::Crawl, d0, d1);
    assert_vec(crawl, 4.5, -2.4, "crawl");
}

#[test]
fn corridor_legs_in_a_tunnel_and_a_shaft() {
    let mut rng = UnityRandom::new(21);
    // Horizontal tunnel, head right of the hips: aim(hips -> head) = 90, base
    // round((270 - 90) / 45) = 4; m = 0 checks eightDirections[14 % 8 = 6] (N) and
    // [18 % 8 = 2] (S), both solid: turn 0. legsDirection += DegToVec(aim(head -> hips)
    // = -90) = (-1, 0); from (0, -1) that normalises to (-0.7071, -0.7071), and the
    // legs sprite's rotation aim(legsDirection -> 0) = VecToDeg(0.7071, 0.7071) = 45.
    let r = tunnel_room();
    let (mut p, mut g) = corridor_player(&r, IN_TUNNEL, Input::NONE, &mut rng);
    place(&mut p, IN_TUNNEL);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&r, &p), 0.0);
    g.legs_direction = Vec2::new(0.0, -1.0);
    update_alone(&mut g, &p, &r, &mut rng);
    let h = std::f32::consts::FRAC_1_SQRT_2;
    assert_vec(g.legs_direction, -h, -h, "tunnel legs direction");
    let legs = *g
        .draw(&p, 1.0, Color::BLACK, Color::BLACK)
        .sprite(4)
        .unwrap();
    assert_close(legs.rotation, 45.0, "tunnel legs rotation");
    assert_eq!(p.animation_frame(), 0);
    assert_eq!(legs.element, "LegsAClimbing0");
    assert_eq!(legs.scale.x, 1.0);

    // Vertical shaft, head up: aim 0, base 6; m = 0 checks [16 % 8 = 0] (W) and
    // [20 % 8 = 4] (E): turn 0, legsDirection += DegToVec(180) = (0, -1).
    let r = shaft_room();
    let in_shaft = (Vec2::new(210.0, 110.0), Vec2::new(210.0, 93.0));
    let (mut p, mut g) = corridor_player(&r, in_shaft, Input::NONE, &mut rng);
    place(&mut p, in_shaft);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&r, &p), 0.0);
    g.legs_direction = Vec2::new(1.0, 0.0);
    update_alone(&mut g, &p, &r, &mut rng);
    // (1, 0) + (0, -1) normalised; rotation VecToDeg(-0.7071, 0.7071) = -45.
    assert_vec(g.legs_direction, h, -h, "shaft legs direction");
    let legs = *g
        .draw(&p, 1.0, Color::BLACK, Color::BLACK)
        .sprite(4)
        .unwrap();
    assert_close(legs.rotation, -45.0, "shaft legs rotation");
    assert!(legs.element.starts_with("LegsAClimbing"));
}

#[test]
fn corridor_legs_turn_picks_the_braced_diagonal() {
    // Hips at tile (10, 5), head to the right (base 4). m = 1 checks [15 % 8 = 7] (NW)
    // and [19 % 8 = 3] (SE) -> 45; m = 3 checks [17 % 8 = 1] (SW) and [21 % 8 = 5] (NE)
    // -> -45; m = 2 checks [16 % 8 = 0] (W) and [20 % 8 = 4] (E) -> 90 facing right.
    // Facing right ranks m = 0 (0), m = 3 (1), m = 1 (2), m = 2 (3).
    let mut p = Player::new();
    place(&mut p, (Vec2::new(227.0, 110.0), Vec2::new(210.0, 110.0)));
    assert_eq!(p.flip_direction(), 1);
    let nw_se = room_with_solid(&[(9, 6), (11, 4)]);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&nw_se, &p), 45.0);
    let diagonals = room_with_solid(&[(9, 6), (11, 4), (9, 4), (11, 6)]);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&diagonals, &p), -45.0);
    let ends = room_with_solid(&[(9, 5), (11, 5), (9, 6), (11, 4)]);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&ends, &p), 45.0);
    let ends_only = room_with_solid(&[(9, 5), (11, 5)]);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&ends_only, &p), 90.0);
    let open = room_with_solid(&[]);
    assert_eq!(PlayerGraphics::corridor_legs_turn(&open, &p), 0.0);
}

#[test]
fn corridor_legs_frames_mirror_after_six() {
    // animationFrame 0..=6 -> LegsAClimbing{frame}, scaleX 1; 7..=12 ->
    // LegsAClimbing{frame % 6}, scaleX -1.
    let mut rng = UnityRandom::new(22);
    let r = tunnel_room();
    let right = Input::new(1, 0, false);
    let start = (Vec2::new(110.0, 50.0), Vec2::new(93.0, 50.0));
    let (mut p, mut g) = corridor_player(&r, start, right, &mut rng);
    let mut seen = [false; 13];
    for _ in 0..30 {
        run(&mut p, &mut g, &r, right, 1, &mut rng);
        let frame = p.animation_frame() as usize;
        seen[frame] = true;
        let legs = *g
            .draw(&p, 1.0, Color::BLACK, Color::BLACK)
            .sprite(4)
            .unwrap();
        let (n, scale) = if frame > 6 {
            (frame % 6, -1.0)
        } else {
            (frame, 1.0)
        };
        assert_eq!(legs.element, format!("LegsAClimbing{n}"), "frame {frame}");
        assert_eq!(legs.scale.x, scale, "frame {frame}");
    }
    assert!(seen[3] && seen[9] && seen[12], "frames seen {seen:?}");
}

/// Puts hand `j` 30 px behind the tunnel head, at rest on its own target.
fn hand_behind(g: &mut PlayerGraphics, j: usize) {
    let limb = &mut g.hands[j].limb;
    limb.mode = crate::LimbMode::HuntAbsolutePosition;
    limb.part.pos = Vec2::new(180.0, 50.0);
    limb.part.vel = Vec2::ZERO;
    limb.absolute_hunt_pos = limb.part.pos;
}

#[test]
fn corridor_hands_grip_the_walls_ahead() {
    // Head at (210, 50) (tile (10, 2) middle), hips at (193, 50): along = (1, 0),
    // perpendicular (0, 1). A hand 30 px behind is pulled to 20 px by the 20 px
    // ConnectToPoint, so !DistLess(pos, head, 20) and FindGrip runs with goal
    // head + 20 (1, 0) + (0, 1) (6 - 12 limb): (230, 56) and (230, 44). In the 3x3
    // tiles around (10, 2) the nearest grips are the free edges of the solid tiles
    // (11, 3) (bottom edge y 60) and (11, 1) (top edge y 40), x clamp(230, 220..240).
    let mut rng = UnityRandom::new(23);
    let r = tunnel_room();
    for input in [Input::NONE, Input::new(1, 0, false)] {
        let (mut p, mut g) = corridor_player(&r, IN_TUNNEL, input, &mut rng);
        place(&mut p, IN_TUNNEL);
        hand_behind(&mut g, 0);
        hand_behind(&mut g, 1);
        update_alone(&mut g, &p, &r, &mut rng);
        assert_vec(g.hands[0].limb.absolute_hunt_pos, 230.0, 60.0, "hand 0");
        assert_vec(g.hands[1].limb.absolute_hunt_pos, 230.0, 40.0, "hand 1");
    }
    // Up held bends the goal: (1, 0) + 1.5 (0, 1) normalised is (0.5547, 0.8321), so
    // hand 0's goal (221.094, 72.641) lies in tile (10, 3), whose bottom edge (y 60)
    // is free: (221.094, 60).
    let up = Input::new(0, 1, false);
    let (mut p, mut g) = corridor_player(&r, IN_TUNNEL, up, &mut rng);
    place(&mut p, IN_TUNNEL);
    hand_behind(&mut g, 0);
    update_alone(&mut g, &p, &r, &mut rng);
    assert_vec(
        g.hands[0].limb.absolute_hunt_pos,
        221.094_01,
        60.0,
        "hand 0, up",
    );
}

#[test]
fn corridor_turn_pose_and_push() {
    // Facing right in the tunnel, jump with left held starts a CorridorTurn. While it
    // runs the body and hips draw positions jitter by at most 3 and 2 px and blink is
    // set to 5 before the tick's blink-- (4 after the update): FaceB.
    let mut rng = UnityRandom::new(24);
    let r = tunnel_room();
    let (mut p, mut g) = corridor_player(&r, IN_TUNNEL, Input::NONE, &mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 5, &mut rng);
    run(&mut p, &mut g, &r, Input::new(-1, 0, true), 1, &mut rng);
    assert_eq!(p.corridor_turn_dir(), Some((-1, 0)));
    let mut turning = 0;
    for _ in 0..20 {
        run(&mut p, &mut g, &r, Input::new(-1, 0, false), 1, &mut rng);
        if p.animation() != Animation::CorridorTurn {
            continue;
        }
        turning += 1;
        assert!(g.draw_positions[0][0].distance(p.head().pos) <= 3.0 + EPS);
        assert!(g.draw_positions[1][0].distance(p.hips().pos) <= 2.0 + EPS);
        assert_eq!(g.blink, 4);
        let face = *g
            .draw(&p, 1.0, Color::BLACK, Color::BLACK)
            .sprite(9)
            .unwrap();
        assert!(face.element.starts_with("FaceB"), "{}", face.element);
    }
    assert!(turning > 10, "turned for {turning} ticks");
    // The tick the turn ends, Player.UpdateAnimation pushes every body part (4 tail
    // segments, 2 hands, head, legs) by -10 * corridorTurnDir = (10, 0).
    g.player_pokes(&p);
    while p.corridor_turn_dir().is_some() {
        p.update(
            &r,
            None,
            Input::NONE,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
    }
    for part in g.body_parts_mut() {
        part.vel = Vec2::ZERO;
    }
    g.player_pokes(&p);
    let parts: Vec<Vec2> = g.body_parts_mut().map(|part| part.vel).collect();
    assert_eq!(parts.len(), 8);
    for vel in parts {
        assert_vec(vel, 10.0, 0.0, "turn-end push");
    }
}
