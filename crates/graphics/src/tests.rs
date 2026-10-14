//! Unit tests. Expected values are derived by hand from the formulas in
//! `rainworld-re/notes/slugcat-graphics.md` and `sprite-atlases.md` (derivations next
//! to each assertion), not from another run of this code.

use rand::SeedableRng;
use rand::rngs::StdRng;
use rw_assets::Room;
use rw_sim::{BodyMode, HEAD, HIPS, Input, Player, Vec2};

use crate::{
    Atlas, BodyPart, Color, DrawItem, Layer, PlayerGraphics, SPRITE_LAYERS, SlugcatName,
    TailAnchor, TailSegment, elements,
};

const EPS: f32 = 1e-4;

/// Builds a room from ASCII rows, top row first: `.` Air, `#` Solid, `/` Slope,
/// `=` Air with a horizontal beam.
fn room(rows: &[&str]) -> Room {
    let h = rows.len();
    let w = rows[0].len();
    let mut grid = String::new();
    for x in 0..w {
        for row in rows {
            grid.push_str(match row.as_bytes()[x] {
                b'.' => "0",
                b'#' => "1",
                b'/' => "2",
                b'=' => "0,2",
                other => panic!("unknown cell {}", other as char),
            });
            grid.push('|');
        }
    }
    let text =
        format!("Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{grid}\n");
    Room::parse(&text).expect("test room parses")
}

/// 30 x 20 tiles, Solid rows y = 0 and 1: ground top at y = 40.
fn flat_room() -> Room {
    let mut rows = vec![".".repeat(30); 18];
    rows.push("#".repeat(30));
    rows.push("#".repeat(30));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: expected {expected}, got {actual}"
    );
}

fn assert_vec(actual: Vec2, x: f32, y: f32, what: &str) {
    assert_close(actual.x, x, what);
    assert_close(actual.y, y, what);
}

/// A player lying on the flat room's ground, head to the right of the hips, with
/// its graphics.
fn grounded_player(rng: &mut StdRng) -> (Player, PlayerGraphics) {
    let mut player = Player::new();
    player.body.chunks[HIPS].reset_at(Vec2::new(100.0, 48.0));
    player.body.chunks[HEAD].reset_at(Vec2::new(117.0, 49.0));
    let graphics = PlayerGraphics::new(&player, SlugcatName::White, rng);
    (player, graphics)
}

fn run(
    player: &mut Player,
    graphics: &mut PlayerGraphics,
    room: &Room,
    input: Input,
    ticks: usize,
    rng: &mut StdRng,
) {
    for _ in 0..ticks {
        player.update(room, input);
        graphics.update(player, room, rng);
    }
}

// ---- tail -----------------------------------------------------------------------

#[test]
fn tail_chain_correction_matches_hand_derivation() {
    // Open air; anchor at x = 0, segments in a line at x = 10, 20, 30, 40, at rest.
    let r = room(&[".........."; 10]);
    let y = 100.0;
    let mut tail = [
        (6.0, 4.0, 1.0),
        (4.0, 7.0, 0.5),
        (2.5, 7.0, 0.5),
        (1.0, 7.0, 0.5),
    ]
    .map(|(rad, con, aff)| TailSegment::new(rad, con, aff));
    for (i, seg) in tail.iter_mut().enumerate() {
        seg.part.pos = Vec2::new(10.0 + 10.0 * i as f32, y);
        seg.part.last_pos = seg.part.pos;
    }
    let base = Vec2::new(0.0, y);
    for l in 0..4 {
        let (before, rest) = tail.split_at_mut(l);
        let anchor = match before.last_mut() {
            Some(prev) => TailAnchor::Segment(&mut prev.part),
            None => TailAnchor::Point(base),
        };
        rest[0].update(&r, anchor);
    }
    // Segment 0: 10 from the point, pulled fully to 4.
    // Segment 1: d = 20 - 4 = 16, correction 7 - 16 = -9 shared: s1 -> 15.5, s0 -> 8.5.
    // Segment 2: d = 30 - 15.5 = 14.5, -7.5 shared: s2 -> 26.25, s1 -> 19.25.
    // Segment 3: d = 40 - 26.25 = 13.75, -6.75 shared: s3 -> 36.625, s2 -> 29.625.
    let xs = [8.5, 19.25, 29.625, 36.625];
    for (seg, x) in tail.iter().zip(xs) {
        assert_vec(seg.part.pos, x, y, "segment");
    }
    // stretched = clamp((con / (d / 2) + 2) / 3, 0.2, 1): segment 1 had d = 16.
    assert_close(tail[1].stretched, (7.0 / 8.0 + 2.0) / 3.0, "stretched");
    assert_close(
        tail[1].stretched_rad(),
        4.0 * (7.0 / 8.0 + 2.0) / 3.0,
        "stretched rad",
    );
    // Velocity takes the same correction as the position: s3 moved -3.375.
    assert_close(tail[3].part.vel.x, -3.375, "vel");
}

#[test]
fn hanging_tail_settles_to_connection_lengths() {
    // A fixed anchor, the graphics loop's slack-tail damping (tail drag 0:
    // vel *= 0.75, vel.y -= 0.1): the chain hangs taut. The tip link is corrected
    // last each tick, so it ends at exactly its connection length; every earlier
    // link is then pulled by half of the later corrections (affectPrevious 0.5) and
    // stays slightly stretched, never slack.
    let r = room(&[".........."; 20]);
    let base = Vec2::new(100.0, 300.0);
    let mut tail = [
        (6.0, 4.0, 1.0),
        (4.0, 7.0, 0.5),
        (2.5, 7.0, 0.5),
        (1.0, 7.0, 0.5),
    ]
    .map(|(rad, con, aff)| TailSegment::new(rad, con, aff));
    for (i, seg) in tail.iter_mut().enumerate() {
        seg.part.pos = base + Vec2::new(3.0 * i as f32, 1.0);
        seg.part.last_pos = seg.part.pos;
    }
    for _ in 0..400 {
        for l in 0..4 {
            let (before, rest) = tail.split_at_mut(l);
            let anchor = match before.last_mut() {
                Some(prev) => TailAnchor::Segment(&mut prev.part),
                None => TailAnchor::Point(base),
            };
            rest[0].update(&r, anchor);
            rest[0].part.vel *= 0.75;
            rest[0].part.vel.y -= 0.1;
        }
    }
    let links = [
        (base, tail[0].part.pos, 4.0),
        (tail[0].part.pos, tail[1].part.pos, 7.0),
        (tail[1].part.pos, tail[2].part.pos, 7.0),
        (tail[2].part.pos, tail[3].part.pos, 7.0),
    ];
    for (i, (a, b, len)) in links.into_iter().enumerate() {
        let d = a.distance(b);
        assert!(d >= len - EPS && d < len + 0.5, "link {i}: {d} vs {len}");
        assert!(b.y < a.y, "link {i} hangs down");
    }
    assert!((tail[2].part.pos.distance(tail[3].part.pos) - 7.0).abs() < 1e-3);
}

// ---- terrain push-out -------------------------------------------------------------

#[test]
fn push_out_lifts_part_onto_floor_top() {
    // Solid ground rows y = 0, 1: tile (2, 1) has centre (50, 30), top edge 40.
    let r = flat_room();
    let mut p = BodyPart::new(1.0, 0.8, 1.0);
    p.pos = Vec2::new(50.0, 39.5);
    p.last_pos = Vec2::new(50.0, 41.0);
    p.vel = Vec2::new(3.0, -2.0);
    p.push_out_of_terrain(&r);
    // Own tile (2, 1) is Solid: pos.x in [40, 60] and lastPos.y >= 30, pos.y < 41,
    // (2, 2) free: push y = 30 + 10 + rad = 41. No x push (the side tiles are
    // Solid). |50 - 0| < |39.5 - 41| is false, so y wins: vel.y = 0, vel.x *= 0.8.
    assert_vec(p.pos, 50.0, 41.0, "pos");
    assert_vec(p.vel, 2.4, 0.0, "vel");
    assert!(p.terrain_contact);
}

#[test]
fn push_out_stops_part_at_wall_side() {
    // Column x = 2 (px 40..60) Solid, everything else Air.
    let rows = vec!["..#....."; 8];
    let r = room(&rows);
    let mut p = BodyPart::new(3.0, 0.8, 1.0);
    p.pos = Vec2::new(38.0, 50.0);
    p.last_pos = Vec2::new(36.0, 50.0);
    p.vel = Vec2::new(2.0, 1.0);
    p.push_out_of_terrain(&r);
    // Neighbour (1, 0) is tile (2, 2), centre (50, 50): pos.y in [40, 60], lastPos.x
    // < 50, pos.x > 50 - 10 - 3 = 37 with (1, 2) free: push x = 37. pos.x is not in
    // [40, 60], so no y push; |38 - 37| < |50 - 0|: x wins, vel.x = 0, vel.y *= 0.8.
    // The diagonal tiles' corners (40, 40), (40, 60) are farther than 3 px.
    assert_vec(p.pos, 37.0, 50.0, "pos");
    assert_vec(p.vel, 0.0, 0.8, "vel");
}

#[test]
fn push_out_pushes_from_corner() {
    // One Solid tile (2, 2), px 40..60 x 40..60; the part is diagonally off its
    // top-left corner (40, 60): neither axis range contains it.
    let mut rows = vec!["........"; 8];
    rows[5] = "..#.....";
    let r = room(&rows);
    let mut p = BodyPart::new(3.0, 0.8, 1.0);
    p.pos = Vec2::new(39.0, 61.0);
    p.last_pos = p.pos;
    p.vel = Vec2::new(1.0, -1.0);
    p.push_out_of_terrain(&r);
    // Nearest point (40, 60), d = sqrt 2: vel *= 0.8, then pos and vel move by
    // (3 - sqrt 2) away from the corner along (-1, 1) / sqrt 2.
    let k = (3.0 - 2.0_f32.sqrt()) / 2.0_f32.sqrt();
    assert_vec(p.pos, 39.0 - k, 61.0 + k, "pos");
    assert_vec(p.vel, 0.8 - k, -0.8 + k, "vel");
}

#[test]
fn push_out_lifts_part_onto_slope() {
    // Tile (2, 2) is a Slope with Solid left (1, 2) and below (2, 1): UpRight, a
    // floor whose surface is h = 20 - lx (slope-collision.md).
    let mut rows = vec!["........"; 8];
    rows[5] = ".#/.....";
    rows[6] = "########";
    rows[7] = "########";
    let r = room(&rows);
    let mut p = BodyPart::new(1.0, 0.8, 1.0);
    p.pos = Vec2::new(55.0, 44.0);
    p.last_pos = p.pos;
    p.vel = Vec2::new(1.0, -1.0);
    p.push_out_of_terrain(&r);
    // UpRight: pos.y < 50 + (50 - 55) + 1 = 46 -> pos.y = 46 (surface 45 + rad),
    // vel.y = 0, vel.x *= 0.8. The Solid tiles below and left are out of reach.
    assert_vec(p.pos, 55.0, 46.0, "pos");
    assert_vec(p.vel, 0.8, 0.0, "vel");
}

// ---- frame choice -----------------------------------------------------------------

#[test]
fn head_frame_from_angle() {
    // round(|a| / 360 * 34), halves to even (Mathf.RoundToInt).
    assert_eq!(PlayerGraphics::head_frame(0.0), 0);
    assert_eq!(PlayerGraphics::head_frame(10.0), 1); // 0.944
    assert_eq!(PlayerGraphics::head_frame(-45.0), 4); // 4.25
    assert_eq!(PlayerGraphics::head_frame(90.0), 8); // 8.5 -> 8
    assert_eq!(PlayerGraphics::head_frame(-100.0), 9); // 9.44
    assert_eq!(PlayerGraphics::head_frame(180.0), 17);
    assert_eq!(PlayerGraphics::head_frame(-180.0), 17);
}

#[test]
fn face_frame_from_direction() {
    // round(|aim(0, v)| / 22.5): up 0, right 4 (90 / 22.5), down 8.
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(0.0, 1.0)), 0);
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(1.0, 0.0)), 4);
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(-1.0, 0.0)), 4);
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(1.0, 1.0)), 2); // 45
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(-1.0, -1.0)), 6); // -135
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(0.0, -1.0)), 8);
    // 33.75 deg / 22.5 = 1.5 -> 2 (halves to even).
    let a = 33.75_f32.to_radians();
    assert_eq!(PlayerGraphics::face_frame(Vec2::new(a.sin(), a.cos())), 2);
}

#[test]
fn arm_frame_from_distance() {
    // round(clamp(d / 2, 0, 12)).
    assert_eq!(PlayerGraphics::arm_frame(0.0), 0);
    assert_eq!(PlayerGraphics::arm_frame(5.0), 2); // 2.5 -> 2
    assert_eq!(PlayerGraphics::arm_frame(7.0), 4); // 3.5 -> 4
    assert_eq!(PlayerGraphics::arm_frame(13.0), 6); // 6.5 -> 6
    assert_eq!(PlayerGraphics::arm_frame(24.0), 12);
    assert_eq!(PlayerGraphics::arm_frame(80.0), 12);
}

// ---- draw positions and breathing ----------------------------------------------------

#[test]
fn stand_offsets_follow_the_gait() {
    // Frame 0, facing right, hips still: d0 = (6 * clamp(-0.2), cos 0 * 2) = (0, 2);
    // d1 = (-(1.5 - 0), 2 + sin 0 * 4) = (-1.5, 2).
    let [d0, d1] = PlayerGraphics::stand_offsets(1.0, 0, 0.0);
    assert_vec(d0, 0.0, 2.0, "d0 frame 0");
    assert_vec(d1, -1.5, 2.0, "d1 frame 0");
    // Frame 3, facing left, hips at 0.7 px/tick: d0.x = -6 * 0.5 = -3,
    // d0.y = cos(pi) * 2 = -2; d1.x = 1 * (1.5 - 0.5) = 1, d1.y = 2 + sin(pi) * 4 = 2.
    let [d0, d1] = PlayerGraphics::stand_offsets(-1.0, 3, 0.7);
    assert_vec(d0, -3.0, -2.0, "d0 frame 3");
    assert_vec(d1, 1.0, 2.0, "d1 frame 3");
}

#[test]
fn crawl_offsets_follow_the_gait() {
    // Frame 0: s = 0, c = 1. d0 = (c * 2, -(s * -1.5 - 3)) = (2, 3);
    // d1 = (-3 s, -(c * 1.5 - 7 + 3)) = (0, 2.5); charged super jump: -(1.5 - 7) = 5.5.
    let [d0, d1] = PlayerGraphics::crawl_offsets(1.0, 0, 0);
    assert_vec(d0, 2.0, 3.0, "d0");
    assert_vec(d1, 0.0, 2.5, "d1");
    let [_, d1] = PlayerGraphics::crawl_offsets(1.0, 0, 20);
    assert_vec(d1, 0.0, 5.5, "d1 charged");
    // Frame 7, facing left: s = sin(2pi/3), c = cos(pi) = -1.
    let s = (7.0_f32 / 21.0 * std::f32::consts::TAU).sin();
    let [d0, d1] = PlayerGraphics::crawl_offsets(-1.0, 7, 0);
    assert_vec(d0, 2.0, 1.5 * s + 3.0, "d0 frame 7");
    assert_vec(d1, 3.0 * s, -(-1.5 - 4.0), "d1 frame 7");
}

#[test]
fn breathing_scales_the_body() {
    // 1 + lerp(-0.05, 0.05, breath) * vert (healthy, awake).
    assert_close(PlayerGraphics::body_scale_x(0.0, 1.0), 0.95, "exhale");
    assert_close(PlayerGraphics::body_scale_x(1.0, 1.0), 1.05, "inhale");
    assert_close(PlayerGraphics::body_scale_x(0.5, 1.0), 1.0, "mid");
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 0.5),
        1.025,
        "half upright",
    );
    assert_close(PlayerGraphics::body_scale_x(1.0, 0.0), 1.0, "lying");
}

#[test]
fn breath_advances_one_sixtieth_per_tick_until_death() {
    // breath += 1 / lerp(60, 15, aerobicLevel^1.5) with aerobicLevel 0.
    let mut rng = StdRng::seed_from_u64(1);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 30, &mut rng);
    assert_close(g.breath, 30.0 / 60.0, "breath");
    assert_close(g.last_breath, 29.0 / 60.0, "last breath");
    p.die();
    run(&mut p, &mut g, &r, Input::NONE, 5, &mut rng);
    assert_close(g.breath, 0.5, "dead: unchanged");
}

// ---- reset and the draw list ------------------------------------------------------

#[test]
fn first_update_resets_to_the_chunks() {
    let mut rng = StdRng::seed_from_u64(2);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    // Culled until the first update: nothing is drawn.
    assert!(g.draw(&p, 0.0, Color::BLACK).items.is_empty());
    p.update(&r, Input::NONE);
    g.update(&p, &r, &mut rng);
    // The reset puts both draw positions at the chunks, then the tick shifts
    // [1] = [0]: last tick's draw positions are the chunk positions.
    assert_eq!(g.draw_positions[0][1], p.head().pos);
    assert_eq!(g.draw_positions[1][1], p.hips().pos);
    assert!(!g.draw(&p, 0.5, Color::BLACK).items.is_empty());
}

#[test]
fn draw_list_follows_the_game_sprite_order() {
    // AddToContainer: 7, 8, 10, 11 in Foreground, the rest in Midground, index order.
    let foreground: Vec<usize> = (0..12)
        .filter(|&i| SPRITE_LAYERS[i] == Layer::Foreground)
        .collect();
    assert_eq!(foreground, [7, 8, 10, 11]);

    let mut rng = StdRng::seed_from_u64(3);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    // Crawling: the hands are engaged (EngageInMovement returns false), arms drawn.
    run(&mut p, &mut g, &r, Input::new(1, 0, false), 40, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    let list = g.draw(&p, 0.5, Color::BLACK);
    let order: Vec<(usize, Layer)> = list.items.iter().map(|i| (i.index(), i.layer())).collect();
    let mid = Layer::Midground;
    assert_eq!(
        order,
        [
            (0, mid),
            (1, mid),
            (2, mid),
            (3, mid),
            (4, mid),
            (5, mid),
            (6, mid),
            (9, mid)
        ]
    );
    assert!(matches!(list.items[2], DrawItem::Mesh(ref m) if m.vertices.len() == 15));

    // Standing still: Stand engages no hand, so after 5 idle ticks they retract and
    // the arms are hidden.
    run(&mut p, &mut g, &r, Input::new(0, 1, false), 1, &mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 60, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    let list = g.draw(&p, 0.5, Color::BLACK);
    let order: Vec<usize> = list.items.iter().map(DrawItem::index).collect();
    assert_eq!(order, [0, 1, 2, 3, 4, 9]);
}

#[test]
fn legs_frame_by_state() {
    let mut rng = StdRng::seed_from_u64(4);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    // Crawl: LegsACrawling{frame / 2}, mirrored by the facing.
    run(&mut p, &mut g, &r, Input::new(-1, 0, false), 30, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    let legs = *g.draw(&p, 0.0, Color::BLACK).sprite(4).unwrap();
    let frame = usize::try_from(p.animation_frame()).unwrap();
    assert_eq!(legs.element, elements::LEGS_A_CRAWLING[frame / 2]);
    assert_eq!(legs.scale.x, -1.0);
    // Stand (walking right): LegsA{frame}.
    run(&mut p, &mut g, &r, Input::new(1, 1, false), 1, &mut rng);
    for _ in 0..12 {
        run(&mut p, &mut g, &r, Input::new(1, 0, false), 1, &mut rng);
        if p.body_mode() != BodyMode::Stand {
            continue;
        }
        let legs = *g.draw(&p, 0.0, Color::BLACK).sprite(4).unwrap();
        let frame = usize::try_from(p.animation_frame()).unwrap();
        assert_eq!(legs.element, elements::LEGS_A[frame]);
        assert_eq!(legs.scale.x, 1.0);
        // Legs anchor 0.25 up the sprite; rotation = aim(legsDirection, 0).
        assert_eq!(legs.anchor, Vec2::new(0.5, 0.25));
    }
    assert_eq!(p.body_mode(), BodyMode::Stand);
    // Airborne (Default): LegsAAir0, the last scale kept.
    let mut p = Player::new();
    p.body.chunks[HIPS].reset_at(Vec2::new(300.0, 300.0));
    p.body.chunks[HEAD].reset_at(Vec2::new(300.0, 317.0));
    let mut g = PlayerGraphics::new(&p, SlugcatName::White, &mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 2, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Default);
    let legs = *g.draw(&p, 0.0, Color::BLACK).sprite(4).unwrap();
    assert_eq!(legs.element, elements::LEGS_A_AIR);
    assert_eq!(legs.scale.x, 1.0);
}

#[test]
fn face_by_blink_and_consciousness() {
    let mut rng = StdRng::seed_from_u64(5);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    run(&mut p, &mut g, &r, Input::new(0, 1, false), 1, &mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 40, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    let black = Color::rgb(0.1, 0.0, 0.05);
    // Standing without x input: FaceA/B by blink, frame from the head direction.
    g.blink = 0;
    let face = *g.draw(&p, 1.0, black).sprite(9).unwrap();
    assert!(elements::FACE_A.contains(&face.element), "{}", face.element);
    assert_eq!(face.color, black);
    assert_eq!(face.rotation, 0.0);
    // Body colour: the Survivor's white everywhere else.
    assert_eq!(
        g.draw(&p, 1.0, black).sprite(0).unwrap().color,
        Color::WHITE
    );
    g.blink = 3;
    let face = *g.draw(&p, 1.0, black).sprite(9).unwrap();
    assert!(elements::FACE_B.contains(&face.element), "{}", face.element);
    // Face 2 px below the head (no look direction).
    let head = *g.draw(&p, 1.0, black).sprite(3).unwrap();
    assert_vec(face.pos, head.pos.x, head.pos.y - 2.0, "face pos");
    // Stunned: FaceStunned, head frame 0, face rotated with the head.
    p.stun(40);
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    let list = g.draw(&p, 1.0, black);
    let (face, head) = (list.sprite(9).unwrap(), list.sprite(3).unwrap());
    assert_eq!(face.element, elements::FACE_STUNNED);
    assert_eq!(head.element, "HeadA0");
    assert_eq!(face.rotation, head.rotation);
    // Unconscious graphics force the eyes shut (blink = 10 each tick).
    assert_eq!(g.blink, 9);
    p.die();
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    assert_eq!(
        g.draw(&p, 1.0, black).sprite(9).unwrap().element,
        elements::FACE_DEAD
    );
}

#[test]
fn sprites_stay_on_the_body_through_mixed_movement() {
    // Walk, crawl, jump, hang from and stand on a beam, push into a wall:
    // every drawn point stays finite and near the body (hands reach at most 20 px
    // from the head chunk, the tail at most 36 px from the hips).
    let mut rows = vec![".".repeat(30); 20];
    for row in &mut rows[12..18] {
        *row = format!("{}{}", ".".repeat(25), "#".repeat(5));
    }
    rows[15] = format!("{}{}{}", ".".repeat(4), "=".repeat(16), ".....#####");
    rows[18] = "#".repeat(30);
    rows[19] = "#".repeat(30);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let r = room(&refs);
    let mut rng = StdRng::seed_from_u64(6);
    let (mut p, mut g) = grounded_player(&mut rng);
    let mut caps_drawn = 0;
    let script = [
        (Input::new(0, 1, false), 1),
        (Input::new(1, 0, false), 20),
        (Input::new(0, 1, true), 12),
        (Input::new(0, 1, false), 40),
        (Input::new(1, 0, false), 60),
        (Input::new(1, 0, true), 10),
        (Input::new(1, 0, false), 30),
        (Input::new(-1, 0, false), 30),
        (Input::new(0, -1, false), 30),
        (Input::new(-1, 0, false), 40),
        (Input::new(1, 0, true), 20),
    ];
    for (input, ticks) in script {
        for _ in 0..ticks {
            run(&mut p, &mut g, &r, input, 1, &mut rng);
            let body = p.head().pos;
            let list = g.draw(&p, 0.5, Color::BLACK);
            caps_drawn += list.layer(Layer::Foreground).count();
            for item in list.items {
                let points = match item {
                    DrawItem::Sprite(s) => vec![s.pos],
                    DrawItem::Mesh(m) => m.vertices,
                };
                for v in points {
                    assert!(v.x.is_finite() && v.y.is_finite());
                    assert!(v.distance(body) < 80.0, "{v:?} far from {body:?}");
                }
            }
        }
    }
    // Hanging from the beam with a hand at its grip shows that hand's cap.
    assert!(caps_drawn > 0);
}

// ---- atlas ------------------------------------------------------------------------

#[test]
fn atlas_json_parses_frames() {
    // Sizes from sprite-atlases.md section 4; positions are made up.
    let json = r#"{"frames": {
        "BodyA.png": {"frame": {"x":1,"y":2,"w":14,"h":19}, "rotated": false, "trimmed": false,
            "spriteSourceSize": {"x":0,"y":0,"w":14,"h":19}, "sourceSize": {"w":14,"h":19}},
        "FaceA0.png": {"frame": {"x":20,"y":2,"w":12,"h":7}, "rotated": false, "trimmed": true,
            "spriteSourceSize": {"x":2,"y":3,"w":12,"h":7}, "sourceSize": {"w":16,"h":18}},
        "PlayerArm0.png": {"frame": {"x":40,"y":2,"w":5,"h":5}, "rotated": false, "trimmed": true,
            "spriteSourceSize": {"x":23,"y":0,"w":5,"h":5}, "sourceSize": {"w":28,"h":8}}
    },
    "meta": {"image": "rainWorld.png", "size": {"w":464,"h":512}, "scale": "1"}}"#;
    let atlas = Atlas::parse(json).unwrap();
    assert_eq!(atlas.frames.len(), 3);
    assert_eq!((atlas.size.w, atlas.size.h), (464, 512));
    let body = atlas.get("BodyA").unwrap();
    assert_eq!((body.frame.w, body.frame.h, body.trimmed), (14, 19, false));
    let face = atlas.get("FaceA0").unwrap();
    assert_eq!((face.source_size.w, face.source_size.h), (16, 18));
    assert_eq!(
        (face.sprite_source_size.x, face.sprite_source_size.y),
        (2, 3)
    );
    let arm = atlas.get("PlayerArm0").unwrap();
    assert_eq!(arm.sprite_source_size.x, 23);
    let rotated = json.replacen("\"rotated\": false", "\"rotated\": true", 1);
    assert!(Atlas::parse(&rotated).is_err());
}

#[test]
fn installed_atlas_has_the_slugcat_frames() {
    // Reads the extracted rainWorld atlas from the asset cache when present
    // (`RW_ASSET_CACHE`); skipped otherwise, as the cache holds game data.
    let Some(cache) = crate::AssetCache::from_env() else {
        return;
    };
    let path = cache.atlas_json("rainWorld");
    if !path.exists() {
        eprintln!("skipped: no atlas at {}", path.display());
        return;
    }
    let atlas = cache.load_atlas("rainWorld").unwrap();
    // sprite-atlases.md: 620 frames, 464 x 512.
    assert_eq!(atlas.frames.len(), 620);
    assert_eq!((atlas.size.w, atlas.size.h), (464, 512));
    // (frame w, h, source w, h)
    for (name, expected) in [
        ("BodyA", (14, 19, 14, 19)),
        ("HipsA", (14, 20, 14, 20)),
        ("HeadA15", (24, 15, 24, 15)),
        ("FaceA0", (12, 7, 16, 18)),
        ("FaceStunned", (12, 6, 16, 18)),
        ("LegsAVerticalPole", (7, 10, 7, 10)),
        ("OnTopOfTerrainHand2", (4, 3, 4, 3)),
    ] {
        let f = atlas.get(name).unwrap_or_else(|| panic!("{name} missing"));
        let got = (f.frame.w, f.frame.h, f.source_size.w, f.source_size.h);
        assert_eq!(got, expected, "{name}");
    }
    assert_eq!(atlas.get("PlayerArm0").unwrap().sprite_source_size.x, 23);
    // Every element name the slugcat code can request is present.
    let names = elements::HEAD_A
        .iter()
        .chain(&elements::FACE_A)
        .chain(&elements::FACE_B)
        .chain(&elements::LEGS_A)
        .chain(&elements::LEGS_A_CRAWLING)
        .chain(&elements::LEGS_A_ON_POLE)
        .chain(&elements::PLAYER_ARM)
        .chain(&[
            elements::BODY_A,
            elements::HIPS_A,
            elements::FACE_DEAD,
            elements::FACE_STUNNED,
            elements::LEGS_A_AIR,
            elements::LEGS_A_WALL,
            elements::ON_TOP_OF_TERRAIN_HAND,
            elements::ON_TOP_OF_TERRAIN_HAND_2,
        ]);
    for name in names {
        assert!(atlas.get(name).is_some(), "{name} missing");
    }
}
