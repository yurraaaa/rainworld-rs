//! Frame choice, draw offsets and breathing, the reset and the draw list, the landing blink.

use rw_random::UnityRandom;
use rw_sim::{BodyMode, HEAD, HIPS, Input, Player, Sounds, Vec2};

use crate::player::tests::{grounded_player, place, run, update_alone};
use crate::tests::{assert_close, assert_vec, flat_room, room};
use crate::{Color, DrawItem, Layer, PlayerGraphics, SPRITE_LAYERS, SlugcatName, elements};

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
    p.die(&mut UnityRandom::new(0));
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
    p.die(&mut UnityRandom::new(0));
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
                    DrawItem::ElementMesh(e) => e.mesh.vertices,
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
