//! Unit tests. Expected values are derived by hand from the formulas in
//! `rainworld-re/notes/slugcat-graphics.md` and `sprite-atlases.md` (derivations next
//! to each assertion), not from another run of this code.

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, Room, RoomSettings, Timeline};
use rw_random::UnityRandom;
use rw_sim::world::World;
use rw_sim::{Animation, BodyMode, HEAD, HIPS, Input, Player, PlayerSurroundings, Sounds, Vec2};

use crate::{
    Atlas, BodyPart, Color, DrawItem, Layer, PlayerGraphics, SPRITE_LAYERS, SlugcatName,
    TailAnchor, TailSegment, elements,
};

const EPS: f32 = 1e-4;

/// Builds a room from ASCII rows, top row first: `.` Air, `#` Solid, `/` Slope,
/// `=` Air with a horizontal beam, `|` Air with a vertical beam.
pub(crate) fn room(rows: &[&str]) -> Room {
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
                b'|' => "0,1",
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
fn grounded_player(rng: &mut UnityRandom) -> (Player, PlayerGraphics) {
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
    rng: &mut UnityRandom,
) {
    for _ in 0..ticks {
        player.update(
            room,
            None,
            input,
            &mut UnityRandom::new(0),
            &mut Sounds::default(),
        );
        update_alone(graphics, player, room, rng);
    }
}

/// `PlayerGraphics::update` with nothing else in the room and nothing held.
fn update_alone(g: &mut PlayerGraphics, p: &Player, r: &Room, rng: &mut UnityRandom) {
    let source = FixedRoom(LoadedRoom {
        room: r.clone(),
        settings: RoomSettings::ANCESTOR,
    });
    let world = World::new(
        WorldData::from_room(r),
        Arc::new(source),
        Timeline::White,
        ActiveMods::NONE,
        false,
    );
    let around = PlayerSurroundings {
        world: &world,
        grasps: [None; 2],
        stomach: false,
        room: 0,
        objects: Vec::new(),
        hungry: true,
    };
    g.update(p, r, None, &around, rng);
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
    assert_close(
        PlayerGraphics::body_scale_x(0.0, 1.0, 0.0, 0.0),
        0.95,
        "exhale",
    );
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 1.0, 0.0, 0.0),
        1.05,
        "inhale",
    );
    assert_close(PlayerGraphics::body_scale_x(0.5, 1.0, 0.0, 0.0), 1.0, "mid");
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 0.5, 0.0, 0.0),
        1.025,
        "half upright",
    );
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 0.0, 0.0, 0.0),
        1.0,
        "lying",
    );
    // Curled up asleep, lerp(..., 0.15, sleepCurlUp).
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 1.0, 1.0, 0.0),
        1.15,
        "curled",
    );
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 1.0, 0.5, 0.0),
        1.1,
        "half curled",
    );
}

#[test]
fn breath_advances_one_sixtieth_per_tick_until_death() {
    // breath += 1 / lerp(60, 15, aerobicLevel^1.5) with aerobicLevel 0.
    let mut rng = UnityRandom::new(1);
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
    let mut rng = UnityRandom::new(2);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    // Culled until the first update: nothing is drawn.
    assert!(g.draw(&p, 0.0, Color::BLACK, Color::BLACK).items.is_empty());
    p.update(
        &r,
        None,
        Input::NONE,
        &mut UnityRandom::new(0),
        &mut Sounds::default(),
    );
    update_alone(&mut g, &p, &r, &mut rng);
    // The reset puts both draw positions at the chunks, then the tick shifts
    // [1] = [0]: last tick's draw positions are the chunk positions.
    assert_eq!(g.draw_positions[0][1], p.head().pos);
    assert_eq!(g.draw_positions[1][1], p.hips().pos);
    assert!(!g.draw(&p, 0.5, Color::BLACK, Color::BLACK).items.is_empty());
}

#[test]
fn draw_list_follows_the_game_sprite_order() {
    // AddToContainer: 7, 8, 10, 11 in Foreground, the rest in Midground, index order.
    let foreground: Vec<usize> = (0..12)
        .filter(|&i| SPRITE_LAYERS[i] == Layer::Foreground)
        .collect();
    assert_eq!(foreground, [7, 8, 10, 11]);

    let mut rng = UnityRandom::new(3);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    // Crawling: the hands are engaged (EngageInMovement returns false), arms drawn.
    run(&mut p, &mut g, &r, Input::new(1, 0, false), 40, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    let list = g.draw(&p, 0.5, Color::BLACK, Color::BLACK);
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
    let list = g.draw(&p, 0.5, Color::BLACK, Color::BLACK);
    let order: Vec<usize> = list.items.iter().map(DrawItem::index).collect();
    assert_eq!(order, [0, 1, 2, 3, 4, 9]);
}

#[test]
fn legs_frame_by_state() {
    let mut rng = UnityRandom::new(4);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    // Crawl: LegsACrawling{frame / 2}, mirrored by the facing.
    run(&mut p, &mut g, &r, Input::new(-1, 0, false), 30, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Crawl);
    let legs = *g
        .draw(&p, 0.0, Color::BLACK, Color::BLACK)
        .sprite(4)
        .unwrap();
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
        let legs = *g
            .draw(&p, 0.0, Color::BLACK, Color::BLACK)
            .sprite(4)
            .unwrap();
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
    let legs = *g
        .draw(&p, 0.0, Color::BLACK, Color::BLACK)
        .sprite(4)
        .unwrap();
    assert_eq!(legs.element, elements::LEGS_A_AIR);
    assert_eq!(legs.scale.x, 1.0);
}

#[test]
fn face_by_blink_and_consciousness() {
    let mut rng = UnityRandom::new(5);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    run(&mut p, &mut g, &r, Input::new(0, 1, false), 1, &mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 40, &mut rng);
    assert_eq!(p.body_mode(), BodyMode::Stand);
    let black = Color::rgb(0.1, 0.0, 0.05);
    // Standing without x input: FaceA/B by blink, frame from the head direction.
    g.blink = 0;
    let face = *g.draw(&p, 1.0, black, Color::BLACK).sprite(9).unwrap();
    assert!(elements::FACE_A.contains(&face.element), "{}", face.element);
    assert_eq!(face.color, black);
    assert_eq!(face.rotation, 0.0);
    // Body colour: the Survivor's white everywhere else.
    assert_eq!(
        g.draw(&p, 1.0, black, Color::BLACK)
            .sprite(0)
            .unwrap()
            .color,
        Color::WHITE
    );
    g.blink = 3;
    let face = *g.draw(&p, 1.0, black, Color::BLACK).sprite(9).unwrap();
    assert!(elements::FACE_B.contains(&face.element), "{}", face.element);
    // Face 2 px below the head (no look direction).
    let head = *g.draw(&p, 1.0, black, Color::BLACK).sprite(3).unwrap();
    assert_vec(face.pos, head.pos.x, head.pos.y - 2.0, "face pos");
    // Stunned: FaceStunned, head frame 0, face rotated with the head.
    p.stun(40, &mut UnityRandom::new(0), &mut Sounds::default());
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    let list = g.draw(&p, 1.0, black, Color::BLACK);
    let (face, head) = (list.sprite(9).unwrap(), list.sprite(3).unwrap());
    assert_eq!(face.element, elements::FACE_STUNNED);
    assert_eq!(head.element, "HeadA0");
    assert_eq!(face.rotation, head.rotation);
    // Unconscious graphics force the eyes shut (blink = 10 each tick).
    assert_eq!(g.blink, 9);
    p.die();
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    assert_eq!(
        g.draw(&p, 1.0, black, Color::BLACK)
            .sprite(9)
            .unwrap()
            .element,
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
    let mut rng = UnityRandom::new(6);
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
            let list = g.draw(&p, 0.5, Color::BLACK, Color::BLACK);
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

// ---- poles ------------------------------------------------------------------------

/// 30 x 20 tiles, ground top at y = 40; a pole in column 10 from the ground (tile
/// row 2) up to tile row 10, and one in column 20 hanging from tile row 15 down to
/// tile row 8.
fn pole_room() -> Room {
    let mut rows = Vec::new();
    for y in (0..20).rev() {
        let mut cells = vec![b'.'; 30];
        if (2..=10).contains(&y) {
            cells[10] = b'|';
        }
        if (8..=15).contains(&y) {
            cells[20] = b'|';
        }
        if y < 2 {
            cells.fill(b'#');
        }
        rows.push(String::from_utf8(cells).unwrap());
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room(&refs)
}

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

// ---- corridors ---------------------------------------------------------------------

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

/// Moves the chunks back to `at` without updating the player.
fn place(p: &mut Player, (head, hips): (Vec2, Vec2)) {
    p.body.chunks[HEAD].reset_at(head);
    p.body.chunks[HIPS].reset_at(hips);
}

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

#[test]
fn hard_landing_blinks() {
    // Player.TerrainImpact: speed > 12 calls Blink(IntClamp((int)speed, 12, 60) / 2),
    // 6..=30 ticks; the graphics take the larger blink before the tick's blink--.
    let mut rng = UnityRandom::new(25);
    let r = flat_room();
    let mut p = Player::new();
    place(&mut p, (Vec2::new(300.0, 237.0), Vec2::new(300.0, 220.0)));
    let mut g = PlayerGraphics::new(&p, SlugcatName::White, &mut rng);
    let mut blinked = false;
    for _ in 0..60 {
        let before = g.blink;
        run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
        let request = p.blink_request();
        if request > 0 {
            assert!((6..=30).contains(&request), "request {request}");
            if request > before && p.is_conscious() {
                assert_eq!(g.blink, request - 1);
            }
            blinked = true;
        }
    }
    assert!(blinked, "a 180 px fall lands faster than 12 px/tick");
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
    // Reads the rainWorld atlas from the installed game (`RW_DIR`) when present.
    let game = std::env::var_os("RW_DIR").map_or_else(
        || std::path::PathBuf::from("/home/yura/.local/share/Steam/steamapps/common/Rain World"),
        std::path::PathBuf::from,
    );
    let Ok(resources) = rw_assets::UnityResources::open(game.join("RainWorld_Data")) else {
        eprintln!("skipped: no game under {}", game.display());
        return;
    };
    let atlas = Atlas::from_resources(&resources, "rainWorld").unwrap();
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
        .chain(&elements::LEGS_A_CLIMBING)
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

// ---- malnourishment and exhaustion ------------------------------------------------

/// A malnourished Survivor on the flat floor and its graphics (`malnourished` 1).
fn malnourished_player(rng: &mut UnityRandom) -> (Player, PlayerGraphics) {
    let mut player = Player::new();
    player.set_stats(
        rw_sim::campaign::SlugcatCampaign::SURVIVOR
            .stats
            .malnourished(),
    );
    player.body.chunks[HIPS].reset_at(Vec2::new(100.0, 48.0));
    player.body.chunks[HEAD].reset_at(Vec2::new(117.0, 49.0));
    let graphics = PlayerGraphics::new(&player, SlugcatName::White, rng);
    (player, graphics)
}

#[test]
fn malnourished_body_is_thinner() {
    // Body: 1 + lerp(lerp(lerp(-0.05, -0.15, 1), 0.05, breath) * vert, 0.15, curl).
    assert_close(
        PlayerGraphics::body_scale_x(0.0, 1.0, 0.0, 1.0),
        0.85,
        "exhale",
    );
    assert_close(
        PlayerGraphics::body_scale_x(1.0, 1.0, 0.0, 1.0),
        1.05,
        "inhale",
    );
    assert_close(
        PlayerGraphics::body_scale_x(0.5, 1.0, 0.0, 1.0),
        0.95,
        "mid",
    );
    // Hips: 1 + 0.2 curl + 0.05 breath - 0.05 malnourished.
    assert_close(PlayerGraphics::hips_scale_x(0.0, 0.0, 1.0), 0.95, "hips");
    assert_close(
        PlayerGraphics::hips_scale_x(1.0, 1.0, 0.5),
        1.225,
        "hips curled",
    );
}

#[test]
fn malnourished_colours_are_grey() {
    let mut rng = UnityRandom::new(5);
    let r = flat_room();
    let (mut p, mut g) = malnourished_player(&mut rng);
    assert_eq!(g.malnourished, 1.0);
    run(&mut p, &mut g, &r, Input::NONE, 2, &mut rng);
    let fog = Color::rgb(0.5, 0.3, 0.1);
    let list = g.draw(&p, 1.0, Color::BLACK, fog);
    // ApplyPalette: white lerped 0.4 toward Color.gray (0.5): 0.8.
    let body = list.sprite(0).unwrap().color;
    assert_close(body.r, 0.8, "body r");
    assert_close(body.b, 0.8, "body b");
    // The face: black lerped 0.2 * 1^2 toward lerp(white, fog, 0.5) = (0.75, 0.65, 0.55).
    let face = list.sprite(9).unwrap().color;
    assert_close(face.r, 0.15, "face r");
    assert_close(face.g, 0.13, "face g");
    assert_close(face.b, 0.11, "face b");
    // The tail's root half-width: 6 * (1 - 0.2) = 4.8.
    let tail = list
        .items
        .iter()
        .find_map(|i| match i {
            DrawItem::Mesh(m) if m.index == 2 => Some(m.clone()),
            _ => None,
        })
        .unwrap();
    assert_close(
        tail.vertices[0].distance(tail.vertices[1]),
        9.6,
        "tail root width",
    );
    assert_eq!(tail.color, body);
}

#[test]
fn cured_malnourishment_fades_by_0_005_a_tick() {
    let mut rng = UnityRandom::new(5);
    let r = flat_room();
    let (mut p, mut g) = malnourished_player(&mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    assert_eq!(g.malnourished, 1.0);
    p.set_stats(rw_sim::campaign::SlugcatCampaign::SURVIVOR.stats);
    run(&mut p, &mut g, &r, Input::NONE, 10, &mut rng);
    assert_close(g.malnourished, 0.95, "faded");
    // Drawn with the colour a further 0.005 faded: lerp(1, 0.5, 0.4 * 0.945).
    let body = g
        .draw(&p, 1.0, Color::BLACK, Color::BLACK)
        .sprite(0)
        .unwrap()
        .color;
    assert_close(body.r, 1.0 - 0.5 * 0.4 * 0.945, "fading body");
}

#[test]
fn exhausted_eyes_draw_only_at_0_8_or_below() {
    let mut rng = UnityRandom::new(5);
    let (mut p, mut g) = malnourished_player(&mut rng);
    // `aerobicLevel` 1 makes the slugcat exhausted (and draws for the stun and fall).
    p.set_aerobic_level(1.0);
    p.exhaustion_update(&mut UnityRandom::new(1), &mut Sounds::default());
    assert!(p.exhausted());
    // Above 0.8: eyes shut, no draw.
    let mut draws = UnityRandom::new(9);
    g.out_of_breath(&p, &mut draws);
    assert_eq!(draws, UnityRandom::new(9));
    assert_eq!(g.blink, 1);
    // At 0.8 or below: `Random.value < 0.02`, then `Random.Range(10, 20)` if it passes.
    p.set_aerobic_level(0.5);
    let seed = (0..).find(|&s| UnityRandom::new(s).value() < 0.02).unwrap();
    let mut draws = UnityRandom::new(seed);
    g.out_of_breath(&p, &mut draws);
    let mut expected = UnityRandom::new(seed);
    expected.value();
    expected.range_i32(10, 20);
    assert_eq!(draws, expected);
    // Exhausted: the eyes close regardless (blink = 1 after the head bob).
    assert_eq!(g.blink, 1);
    let seed = (0..)
        .find(|&s| UnityRandom::new(s).value() >= 0.02)
        .unwrap();
    let mut draws = UnityRandom::new(seed);
    g.out_of_breath(&p, &mut draws);
    let mut expected = UnityRandom::new(seed);
    expected.value();
    assert_eq!(draws, expected);
}

#[test]
fn breath_quickens_with_the_aerobic_level() {
    // breath += 1 / lerp(60, 15, aerobicLevel^1.5): 1/15 a tick at aerobicLevel 1.
    let mut rng = UnityRandom::new(1);
    let r = flat_room();
    let (mut p, mut g) = grounded_player(&mut rng);
    run(&mut p, &mut g, &r, Input::NONE, 1, &mut rng);
    let before = g.breath;
    p.set_aerobic_level(1.0);
    update_alone(&mut g, &p, &r, &mut rng);
    assert_close(g.breath - before, 1.0 / 15.0, "panting");
}

// ---- swallowing ---------------------------------------------------------------------

#[test]
fn the_swallow_shakes_with_six_draws_from_halfway() {
    // `SlugcatHand.Update` (241815): t = InverseLerp(10, 90, counter); below 0.5 the
    // hand is drawn in without draws, from 0.5 the eyes shut and the hand, the head and
    // the main chunk each take an `RNV` and a `value`.
    let r = flat_room();
    let mut rng = UnityRandom::new(3);
    let (p, _) = grounded_player(&mut rng);
    for (counter, draws) in [(49, 0), (50, 6), (90, 6)] {
        let mut hand = crate::hand::SlugcatHand::new(0);
        let (mut disbalance, mut head_vel, mut draw_pos0) = (0.0, Vec2::ZERO, Vec2::ZERO);
        let (mut blink, mut overlap, mut body_push) = (0, [None; 2], Vec2::ZERO);
        let mut rng = UnityRandom::new(9);
        let mut cx = crate::hand::HandContext {
            player: &p,
            room: &r,
            water: None,
            balance_counter: 0.0,
            disbalance: &mut disbalance,
            head_vel: &mut head_vel,
            airborne_counter: 0.0,
            hand0: (Vec2::ZERO, false),
            rng: &mut rng,
            grasps: crate::hand::HandGrasp::default(),
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
