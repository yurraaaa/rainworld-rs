//! Malnourishment and exhaustion: body, colours, eyes, breath.

use rw_random::UnityRandom;
use rw_sim::{HEAD, HIPS, Input, Player, Sounds, Vec2};

use crate::player::tests::{grounded_player, run, update_alone};
use crate::tests::{assert_close, flat_room};
use crate::{Color, DrawItem, PlayerGraphics, SlugcatName};

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
