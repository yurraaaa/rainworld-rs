//! `CosmeticSlimeMold`'s dots and draws, the plain `SlimeMold`'s gate, the dead
//! `TokenStalk`'s constructor and per-tick draws, and `Room.Loaded` adding them in the
//! settings' order. Expected values follow `SlimeMold.CosmeticSlimeMold`'s constructor,
//! `Room.Loaded` and `CollectToken.TokenStalk` (`Update`), replayed on seeded streams.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, PlacedObject, Room, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::game::RoomObject;
use crate::tests::room;
use crate::world::World;
use crate::{CosmeticSlimeMold, FloatExt, Input, RainWorldGame, Sounds, Start, TokenStalk, Vec2};

/// A 5 x 5 box: open tiles (1..=3, 1..=3).
fn box_room() -> Room {
    room(&["#####", "#...#", "#...#", "#...#", "#####"])
}

/// A one-tile-high corridor: open tiles (1..=3, 1).
fn corridor_room() -> Room {
    room(&["#####", "#...#", "#####"])
}

/// `seed`'s stream after `n` values.
fn advanced(seed: i32, n: usize) -> [u32; 4] {
    let mut r = UnityRandom::new(seed);
    for _ in 0..n {
        r.value();
    }
    r.state()
}

/// The constructor leaves the shared stream where it was.
#[test]
fn slime_mold_construction_makes_no_shared_draws() {
    let mut rng = UnityRandom::new(77);
    rng.value();
    let before = rng.state();
    let mold = CosmeticSlimeMold::new(0, &box_room(), Vec2::new(50.0, 30.0), 19.9, false, &mut rng);
    assert!(!mold.positions.is_empty());
    assert_eq!(rng.state(), before);
}

/// `CosmeticSlimeMold` (not through walls) on the floor tile (2, 1) of the box, `rad`
/// under 20 so only the centre tile counts: the walls opposite (0, 1), (1, 1) and
/// (-1, 1) are solid, so the dots are traced straight down onto the floor's top
/// (y = 20). Seed `(int)(50 + 30 + 19.9)` = 99: one count draw
/// (`Lerp(2, 4, r)`), then per dot two jitter draws, both of `LerpMap`'s radius ends
/// and the brightness: 1 + 5n draws.
#[test]
fn open_slime_mold_traces_dots_onto_the_wall() {
    let (pos, rad) = (Vec2::new(50.0, 30.0), 19.9);
    let mut rng = UnityRandom::new(99);
    let mold = CosmeticSlimeMold::generate(0, &box_room(), pos, rad, false, &mut rng);
    let mut expect = UnityRandom::new(99);
    let count = 2.0_f32.lerp(4.0, expect.value()) as usize;
    assert_eq!(mold.positions.len(), count);
    for k in 0..count {
        let x = 50.0 + (-9.0_f32).lerp(9.0, expect.value());
        let _y = expect.value();
        let at = Vec2::new(x, 20.0);
        let near = 1.5 + 3.0 * expect.value();
        let far = 0.75 + 0.25 * expect.value();
        let r = at.distance(pos).lerp_map(rad / 3.0, rad, near, far);
        let top = 2.0_f32.lerp(4.0, expect.value().mathf_pow(1.5));
        assert_eq!(mold.positions[k], at);
        assert_eq!(mold.rads[k], r);
        assert_eq!(mold.brightnesses[k], r.inverse_lerp(0.75, top));
    }
    assert_eq!(rng.state(), advanced(99, 1 + 5 * count));
}

/// `CosmeticSlimeMold2` on the box's floor tile (2, 1): only (0, -1) is solid, its
/// opposite open, so the count is `Lerp(4, 7, r)`; each dot sits on the floor's face
/// (y = 20) jittered along x, with no push: 1 + 4n draws.
#[test]
fn through_walls_slime_mold_lines_the_wall_face() {
    let (pos, rad) = (Vec2::new(50.0, 30.0), 19.9);
    let mut rng = UnityRandom::new(99);
    let mold = CosmeticSlimeMold::generate(0, &box_room(), pos, rad, true, &mut rng);
    let mut expect = UnityRandom::new(99);
    let count = 4.0_f32.lerp(7.0, expect.value()) as usize;
    assert_eq!(mold.positions.len(), count);
    for k in 0..count {
        let at = Vec2::new(50.0 + (-9.0_f32).lerp(9.0, expect.value()), 20.0);
        let near = 1.5 + 3.0 * expect.value();
        let far = 0.75 + 0.25 * expect.value();
        let r = at.distance(pos).lerp_map(rad / 3.0, rad, near, far);
        expect.value();
        assert_eq!((mold.positions[k], mold.rads[k]), (at, r));
    }
    assert_eq!(rng.state(), advanced(99, 1 + 4 * count));
}

/// In a one-tile corridor both the floor and the ceiling are walled in on both sides:
/// `Lerp(6, 8, r)` dots on each, pushed away from the wall tile's middle by
/// `max(0, rad - 1)`. The floor's (direction (0, -1)) come first.
#[test]
fn through_walls_slime_mold_pushes_dots_out_of_a_closed_tile() {
    let (pos, rad) = (Vec2::new(50.0, 30.0), 19.9);
    let mut rng = UnityRandom::new(99);
    let mold = CosmeticSlimeMold::generate(0, &corridor_room(), pos, rad, true, &mut rng);
    let mut expect = UnityRandom::new(99);
    let floor = 6.0_f32.lerp(8.0, expect.value()) as usize;
    let at = Vec2::new(50.0 + (-9.0_f32).lerp(9.0, expect.value()), 20.0);
    let near = 1.5 + 3.0 * expect.value();
    let far = 0.75 + 0.25 * expect.value();
    let r = at.distance(pos).lerp_map(rad / 3.0, rad, near, far);
    let pushed = at + Vec2::new(50.0, 10.0).direction_to(at) * (r - 1.0).max(0.0);
    assert_eq!(mold.positions[0], pushed);
    assert!(mold.positions.len() > floor);
    assert!(mold.positions[floor].y > 30.0, "ceiling dots follow");
}

/// `Room.Loaded`'s plain `SlimeMold` gate: `SeededRandom((int)(x + y))` above 0.3
/// gives `LerpMap(r, 0.3, 1, 30, 70)`; at or below it, no patch.
#[test]
fn placed_slime_mold_gate() {
    let mut rng = UnityRandom::new(3);
    let before = rng.state();
    for (x, y) in [
        (46.5614, 168.4334),
        (434.145, 81.88771),
        (45.17712, 123.3137),
    ] {
        let r = UnityRandom::new((x + y) as i32).value();
        let rad = CosmeticSlimeMold::placed_item_rad(Vec2::new(x, y), &mut rng);
        let expected = (r > 0.3).then(|| 30.0 + 40.0 * ((r - 0.3) / 0.7));
        match (rad, expected) {
            (Some(a), Some(b)) => assert!((a - b).abs() < 1e-4, "{a} {b}"),
            (a, b) => assert_eq!(a, b),
        }
    }
    assert_eq!(rng.state(), before);
}

/// The stalk of `cc_a10`'s first `DeadTokenStalk` line (hover (309.0585, 978.0576),
/// handle (-22, -46)): seed `(int)3090.585 + (int)9780.576` = 12870 gives the first
/// curve's angle, the second's head distance and the flip; ten rope segments
/// (`(int)(|handle| * 0.6 / 3)`), all at the arm; no shared draws.
#[test]
fn token_stalk_constructor() {
    let hover = Vec2::new(309.0585, 978.0576);
    let base = hover + Vec2::new(-22.0, -46.0);
    let mut rng = UnityRandom::new(5);
    let before = rng.state();
    let s = TokenStalk::new(0, hover, base, &mut rng);
    assert_eq!(rng.state(), before);
    let mut expect = UnityRandom::new(12870);
    assert_eq!(s.curve_lerps[0].to, expect.value() * 360.0);
    assert_eq!(s.curve_lerps[1].to, 10.0_f32.lerp(20.0, expect.value()));
    assert_eq!(s.flip, if expect.value() < 0.5 { -1.0 } else { 1.0 });
    let length = base.distance(hover);
    assert_eq!(s.coord.len(), 10);
    assert_eq!(s.arm_length, length / 2.0);
    assert_eq!(s.arm_pos, base + base.direction_to(hover) * (length / 2.0));
    assert!(s.coord.iter().all(|c| c.pos == s.arm_pos));
    assert_eq!(s.head, hover - base.direction_to(hover) * 15.0);
    assert_eq!((s.lamp_power, s.head_dist), (0.0, 15.0));
}

/// An open 20 x 20 room for the stalk's updates.
fn open_room() -> Room {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 18]);
    rows.push("####################");
    room(&rows)
}

/// `Update` with no token: `sinCounter`'s draw and the 1-in-300 roll, two draws a tick;
/// the lamp stays off.
#[test]
fn token_stalk_update_draws_two_a_tick() {
    let room = open_room();
    let hover = Vec2::new(200.0, 250.0);
    let mut s = TokenStalk::new(0, hover, Vec2::new(200.0, 200.0), &mut UnityRandom::new(1));
    // Seed 8's second and fourth values are not below 1/300.
    let mut rng = UnityRandom::new(8);
    for tick in 0..2 {
        s.update(&room, &mut rng);
        assert_eq!(rng.state(), advanced(8, 2 * (tick + 1)), "tick {tick}");
    }
    assert_eq!((s.lamp_power, s.sin_counter), (0.0, 0.0));
}

/// A roll under 1/300 picks a new pose: `RNV`, the radius, then `NewCurveLerp`'s two
/// arguments, six draws in all; both curves start toward the new values.
#[test]
fn token_stalk_new_pose_draws_four_more() {
    let room = open_room();
    let seed = (0..100_000)
        .find(|&s| {
            let mut r = UnityRandom::new(s);
            r.value();
            r.value() < 1.0 / 300.0
        })
        .expect("a seed rolling a new pose");
    let mut s = TokenStalk::new(
        0,
        Vec2::new(200.0, 250.0),
        Vec2::new(200.0, 200.0),
        &mut UnityRandom::new(1),
    );
    let start = s.curve_lerps[0].to;
    let mut rng = UnityRandom::new(seed);
    s.update(&room, &mut rng);
    assert_eq!(rng.state(), advanced(seed, 6));
    let mut expect = UnityRandom::new(seed);
    for _ in 0..4 {
        expect.value();
    }
    assert_eq!(
        s.curve_lerps[0].to,
        start + (-180.0_f32).lerp(180.0, expect.value())
    );
    assert_eq!(
        s.curve_lerps[1].to,
        10.0_f32.lerp(20.0, expect.value().mathf_pow(0.75))
    );
    assert_eq!(s.curve_lerps[0].from, start);
    assert!(
        s.curve_lerps
            .iter()
            .all(|l| l.progress > 0.0 && l.progress < 1.0)
    );
}

/// A placed light, a dead stalk, a through-walls slime mold and a plain `SlimeMold`
/// whose gate passes (`SeededRandom(214)` for (46.5614, 168.4334)), each joining the
/// update list in the settings' order; only the stalk takes turns.
#[test]
fn room_loaded_adds_them_in_settings_order() {
    const PLACED: &str = "DeadTokenStalk><100><100><0~-40, \
         CosmeticSlimeMold2><200><30><0~19, \
         LightSource><300><60><1~EffectColor1~0~100~0~0~0~1, \
         SlimeMold><46.5614><168.4334><-154~390~2~3";
    let gate = UnityRandom::new(214).value();
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 10]);
    rows.push("####################");
    let room = room(&rows);
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        placed_objects: PlacedObject::parse_list(PLACED),
        ..RoomSettings::ANCESTOR
    };
    let campaign = &SlugcatCampaign::SURVIVOR;
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    let mut g = RainWorldGame::new_story(
        world,
        campaign,
        StoryStart::new_game(campaign),
        Start {
            room: 0,
            tile: Some((2, 1)),
        },
        UnityRandom::new(5),
        Sounds::default(),
        &mut (),
    );
    for _ in 0..3 {
        g.tick(Input::NONE, &mut ());
    }
    let r = g.world.realized(0).expect("realized");
    assert_eq!(r.token_stalks.len(), 1);
    assert_eq!(r.slime_molds.len(), if gate > 0.3 { 2 } else { 1 });
    let stalk = r.token_stalks[0].update_order;
    let mold = r.slime_molds[0].update_order;
    let light = r.light_sources[0].id;
    assert!(stalk < mold && mold < light);
    if gate > 0.3 {
        assert!(light < r.slime_molds[1].update_order);
    }
    assert_eq!(r.token_stalks[0].base_pos, Vec2::new(100.0, 60.0));
    assert_eq!(
        g.room_token_stalk_objects(0),
        [(stalk, RoomObject::TokenStalk(stalk))]
    );
}
