//! `RoomRain`: `GenerateShelterTex`'s `rainReach` and `splashTiles`, the intensity,
//! `InsidePushAround`/`OutsidePushAround`, and the per-chunk steps of `ThrowAroundObjects`
//! and `CreatureSmashedInGround` with their draws. Expected values from `RoomRain.cs`
//! (decompiled members named in each test), replayed by hand on a second generator.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::DangerType;
use rw_random::UnityRandom;

use super::*;
use crate::physics::chunk::Contact;
use crate::tests::room;

fn rain(danger: DangerType) -> RoomRain {
    let rows = [
        "##....", //
        "#..#..", //
        "#.....", //
        "#...#.", //
        "######",
    ];
    RoomRain::new(7, &room(&rows), danger, false)
}

/// A generator advanced by `n` `Random.value` draws.
fn after(seed: i32, n: usize) -> UnityRandom {
    let mut r = UnityRandom::new(seed);
    for _ in 0..n {
        r.value();
    }
    r
}

fn assert_draws(rng: &mut UnityRandom, seed: i32, n: usize) {
    assert_eq!(rng.value(), after(seed, n).value(), "{n} draws expected");
}

fn chunk_at(x: f32, y: f32) -> BodyChunk {
    BodyChunk::new(Vec2::new(x, y), 5.0, 2.0)
}

/// `GenerateShelterTex`: per column from the top row down to the first solid tile:
/// `rainReach[i] = y` and, below the top row, a splash tile. The room above is 6 x 5:
/// columns 0 and 1 are solid at the top row (reach 4, no splash tile), column 2 only at
/// the floor (0), column 3 has a block in row 3, column 4 one in row 1, column 5 the floor.
#[test]
fn rain_reach_and_splash_tiles_follow_the_first_solid_tile_from_the_top() {
    let r = rain(DangerType::Rain);
    assert_eq!(&*r.rain_reach, &[4, 4, 0, 3, 1, 0]);
    assert_eq!(r.splash_tiles, vec![(2, 0), (3, 3), (4, 1), (5, 0)]);
    // `splashTiles.Count * 2 / cameraPositions.Length`, one camera.
    assert_eq!(r.splashes, 8);
    assert_eq!((r.intensity, r.last_intensity), (0.0, 1.0));
    assert_eq!(r.update_order, 7);
}

/// The constructor makes a shelter's rain a `Flood` one.
#[test]
fn a_shelters_rain_is_a_flood_rain() {
    let rows = ["##", "##"];
    let r = RoomRain::new(0, &room(&rows), DangerType::Rain, true);
    assert_eq!(r.danger_type, DangerType::Flood);
}

/// `Update`'s first arm: `Lerp(intensity, globalRain.Intensity, 0.2)` in a rain type, a
/// `Flood` room's stays 0, the room's `RainIntensity` caps it (an `AerieBlizzard` at 1), and
/// `lastIntensity` is assigned after, so equal.
#[test]
fn the_intensity_follows_the_global_rain_to_the_cap() {
    let mut r = rain(DangerType::Rain);
    r.update_intensity(1.0, 1.0);
    assert!((r.intensity - 0.2).abs() < 1e-6);
    r.update_intensity(1.0, 1.0);
    assert!((r.intensity - 0.36).abs() < 1e-6);
    assert_eq!(r.last_intensity, r.intensity);
    r.update_intensity(1.0, 0.1);
    assert_eq!(r.intensity, 0.1);

    let mut f = rain(DangerType::Flood);
    f.update_intensity(1.0, 1.0);
    assert_eq!(f.intensity, 0.0);

    let mut a = rain(DangerType::AerieBlizzard);
    a.update_intensity(5.0, 0.3);
    assert_eq!(a.intensity, 1.0);
    assert_eq!(a.last_intensity, 1.0);
}

/// `InsidePushAround` (Rain: the global value times `RainIntensity`; AerieBlizzard: the
/// global value; else 0) and `OutsidePushAround` (Rain and FloodAndRain scaled, Blizzard
/// unscaled, Flood and None 0).
#[test]
fn the_push_properties_by_danger_type() {
    let g = GlobalRain {
        intensity: 0.8,
        ..GlobalRain::default()
    };
    let (inside, outside) = (g.inside_push_around(), g.outside_push_around());
    assert!(inside > 0.0 && outside > 0.0);
    assert_eq!(
        rain(DangerType::Rain).push_around(&g, 0.5),
        PushAround {
            inside: inside * 0.5,
            outside: outside * 0.5
        }
    );
    assert_eq!(
        rain(DangerType::FloodAndRain).push_around(&g, 0.5),
        PushAround {
            inside: 0.0,
            outside: outside * 0.5
        }
    );
    assert_eq!(
        rain(DangerType::Flood).push_around(&g, 0.5),
        PushAround::default()
    );
    assert_eq!(
        rain(DangerType::None).push_around(&g, 0.5),
        PushAround::default()
    );
    assert_eq!(
        rain(DangerType::AerieBlizzard).push_around(&g, 0.5),
        PushAround { inside, outside }
    );
}

/// `CreatureSmashedInGround` is only reached while `AnyPushAround`.
#[test]
fn the_smash_view_needs_a_pushing_rain() {
    let r = rain(DangerType::Rain);
    let calm = GlobalRain {
        intensity: 0.35,
        ..GlobalRain::default()
    };
    assert!(r.smash_view(&calm, 1.0).is_none());
    let pushing = GlobalRain {
        intensity: 0.4,
        ..GlobalRain::default()
    };
    assert!(r.smash_view(&pushing, 1.0).is_some());
}

/// A chunk's first step draws the jitter (x then y) and nothing more when `num` is not
/// above 0: tile (3, 3) is the block, below its rain reach 3? Chunk at (70, 70) with rad 5
/// and draws near 0.5 stays in tile (3, 3): the reach (3) is not below its row, so it is
/// inside and `num` is the inside push.
#[test]
fn a_chunk_without_push_makes_two_draws() {
    let r = rain(DangerType::Rain);
    let mut rng = UnityRandom::new(9);
    let c = chunk_at(70.0, 70.0);
    let none = r.chunk_push(PushAround::default(), None, &c, &mut rng);
    assert_eq!(none, None);
    assert_draws(&mut rng, 9, 2);
}

/// Above the rain reach the larger of the two pushes is used. The jitter is `Lerp(-rad,
/// rad, value)` per axis, so a chunk at the middle of tile (4, 3) (reach 1 < 3) stays in it
/// whatever the draws; tile (2, 1) has reach 0 < 1 too.
#[test]
fn a_chunk_above_the_reach_takes_the_larger_push() {
    let r = rain(DangerType::Rain);
    let mut rng = UnityRandom::new(3);
    let p = PushAround {
        inside: 0.2,
        outside: 0.7,
    };
    let got = r
        .chunk_push(p, None, &chunk_at(90.0, 70.0), &mut rng)
        .unwrap();
    assert_eq!(got.tile, (4, 3));
    assert!(got.outside);
    assert_eq!(got.num, 0.7);
    // Under the block at (3, 3): tile (3, 2) has reach 3 >= 2, so inside.
    let got = r
        .chunk_push(p, None, &chunk_at(70.0, 50.0), &mut rng)
        .unwrap();
    assert_eq!(got.tile, (3, 2));
    assert!(!got.outside);
    assert_eq!(got.num, 0.2);
    assert_draws(&mut rng, 3, 4);
}

/// `num *= InverseLerp(FloatWaterLevel - 100, FloatWaterLevel, y)`: halfway up the last
/// 100 px below the surface halves it; above the surface it is untouched.
#[test]
fn water_weakens_the_push_toward_the_surface() {
    let r = rain(DangerType::Rain);
    let p = PushAround {
        inside: 0.0,
        outside: 0.8,
    };
    let mut rng = UnityRandom::new(1);
    let got = r
        .chunk_push(p, Some(120.0), &chunk_at(90.0, 70.0), &mut rng)
        .unwrap();
    assert!((got.num - 0.4).abs() < 1e-6, "{}", got.num);
    let got = r
        .chunk_push(p, Some(40.0), &chunk_at(90.0, 70.0), &mut rng)
        .unwrap();
    assert_eq!(got.num, 0.8);
    let below = r.chunk_push(p, Some(500.0), &chunk_at(90.0, 70.0), &mut rng);
    assert_eq!(below, None, "100 px or more under the surface: num is 0");
}

/// Resting on something (`ContactPoint.y < 0`): `DegToVec(Lerp(-30, 30, v1) + side * 16) *
/// (v2 * 9 * num) / mass` (outside), side from the neighbour columns' reach; two draws.
/// Tile (4, 3): column 3's reach is 3 >= 3 and its tile (3, 3) is the block (solid), so no
/// left side; column 5's reach is 0 < 3: side 0.
#[test]
fn a_chunk_on_the_ground_is_thrown_up_and_aside() {
    let rows = [
        "##....", //
        "#..#..", //
        "#.....", //
        "#...#.", //
        "######",
    ];
    let room = room(&rows);
    let r = rain(DangerType::Rain);
    let mut c = chunk_at(90.0, 70.0);
    c.set_contacts(Contact { x: 0, y: -1 }, Contact::default());
    let p = ChunkPush {
        num: 0.7,
        outside: true,
        tile: (4, 3),
    };
    let mut rng = UnityRandom::new(5);
    r.push_velocity(&room, &mut c, &p, &mut rng);
    let mut e = UnityRandom::new(5);
    let angle = (-30.0_f32).lerp(30.0, e.value());
    let size = e.value() * 9.0 * 0.7;
    let want = Vec2::from_degrees(angle) * size / 2.0;
    assert_eq!(c.vel, want);
    assert_draws(&mut rng, 5, 2);

    // Tile (2, 2): column 1 has reach 4 >= 2 and (1, 2) is open: -1; column 3 has reach 3
    // >= 2 and (3, 2) is open: +1; sides cancel.
    let mut c = chunk_at(50.0, 50.0);
    c.set_contacts(Contact { x: 0, y: -1 }, Contact::default());
    let p = ChunkPush {
        num: 0.5,
        outside: false,
        tile: (2, 2),
    };
    let mut rng = UnityRandom::new(5);
    r.push_velocity(&room, &mut c, &p, &mut rng);
    let size = after(5, 1).value() * 4.0 * 0.5;
    let want = Vec2::from_degrees((-30.0_f32).lerp(30.0, after(5, 0).value())) * size / 2.0;
    assert_eq!(c.vel, want);
}

/// Tile (4, 2): column 3 has reach 3 >= 2 and (3, 2) is open (side -1); column 5 has reach
/// 0 < 2 (no +1): the angle is shifted by -16.
#[test]
fn the_ground_throw_leans_away_from_the_rain_edge() {
    let room = room(&["##....", "#..#..", "#.....", "#...#.", "######"]);
    let r = rain(DangerType::Rain);
    let mut c = chunk_at(90.0, 50.0);
    c.set_contacts(Contact { x: 0, y: -1 }, Contact::default());
    let p = ChunkPush {
        num: 1.0,
        outside: true,
        tile: (4, 2),
    };
    let mut rng = UnityRandom::new(8);
    r.push_velocity(&room, &mut c, &p, &mut rng);
    let angle = (-30.0_f32).lerp(30.0, after(8, 0).value()) - 16.0;
    let size = {
        let mut e = after(8, 1);
        e.value() * 9.0 * 1.0
    };
    assert_eq!(c.vel, Vec2::from_degrees(angle) * size / 2.0);
}

/// Not resting: `vel.y -= Pow(v, 5) * 16.5 * num / mass`, one draw.
#[test]
fn an_airborne_chunk_is_pushed_down_with_one_draw() {
    let room = room(&["######", "#....#", "######"]);
    let r = rain(DangerType::Rain);
    let mut c = chunk_at(50.0, 30.0);
    c.vel = Vec2::new(1.0, 2.0);
    let p = ChunkPush {
        num: 0.6,
        outside: true,
        tile: (2, 1),
    };
    let mut rng = UnityRandom::new(2);
    r.push_velocity(&room, &mut c, &p, &mut rng);
    let v = UnityRandom::new(2).value();
    assert_eq!(c.vel.x, 1.0);
    assert_eq!(c.vel.y, 2.0 - v.mathf_pow(5.0) * 16.5 * 0.6 / 2.0);
    assert_draws(&mut rng, 2, 1);
}

/// The stun roll is one draw, and a second for the length only when `Pow(v, 1.2) * 2 *
/// chunks < num`: with `num` 0 never (0 draws more), with `num` 100 always, `Range(1,
/// 1 + (int)(9 * 100))`.
#[test]
fn the_stun_roll_draws_once_or_twice() {
    let mut rng = UnityRandom::new(4);
    assert_eq!(RoomRain::stun_roll(0.0, 2, &mut rng), None);
    assert_draws(&mut rng, 4, 1);

    let mut rng = UnityRandom::new(4);
    let got = RoomRain::stun_roll(100.0, 2, &mut rng);
    let mut e = UnityRandom::new(4);
    e.value();
    assert_eq!(got, Some(e.range_i32(1, 901)));
}

/// `Range(1, 1 + (int)(9 * num))` with `num` below 1/9 is `Range(1, 1)`: the generator does
/// not advance for equal bounds. A creature with no chunks (`0 < num`) takes that path.
#[test]
fn a_tiny_stun_range_makes_no_second_draw() {
    let mut rng = UnityRandom::new(6);
    assert_eq!(RoomRain::stun_roll(0.05, 0, &mut rng), Some(1));
    assert_draws(&mut rng, 6, 1);
}

/// The kill roll draws only with `num > 0.5` and `rainDeath > 1`, and kills below 0.025.
#[test]
fn the_kill_roll_is_gated_by_num_and_rain_death() {
    for (num, rain_death) in [(0.5, 2.0), (0.9, 1.0), (0.4, 0.0)] {
        let mut rng = UnityRandom::new(1);
        assert!(!RoomRain::kill_roll(num, rain_death, &mut rng));
        assert_draws(&mut rng, 1, 0);
    }
    let mut rng = UnityRandom::new(1);
    let first = UnityRandom::new(1).value();
    assert_eq!(RoomRain::kill_roll(0.9, 1.5, &mut rng), first < 0.025);
    assert_draws(&mut rng, 1, 1);
}

/// The last push is `DegToVec(Lerp(90, 270, v1)) * (v2 * 5 * InsidePushAround)`, two draws
/// even with an inside push of 0.
#[test]
fn the_final_push_always_draws_twice() {
    let mut c = chunk_at(50.0, 50.0);
    let mut rng = UnityRandom::new(12);
    RoomRain::final_push(&mut c, 0.5, &mut rng);
    let mut e = UnityRandom::new(12);
    let dir = Vec2::from_degrees(90.0_f32.lerp(270.0, e.value()));
    assert_eq!(c.vel, dir * (e.value() * 5.0 * 0.5));
    assert_draws(&mut rng, 12, 2);

    let mut c = chunk_at(50.0, 50.0);
    let mut rng = UnityRandom::new(12);
    RoomRain::final_push(&mut c, 0.0, &mut rng);
    assert_eq!(c.vel, Vec2::ZERO);
    assert_draws(&mut rng, 12, 2);
}

/// `CreatureSmashedInGround`: under speed 2.5 nothing is drawn; from 2.5 on a random chunk
/// (`Range(0, count)`, which advances the generator) and its jitter (two draws) are drawn.
/// `InverseLerp(-2.5, -15, speed)` of a positive speed is 0, so nothing is added.
#[test]
fn a_smash_draws_a_chunk_and_a_jitter_and_adds_nothing() {
    let r = rain(DangerType::Rain);
    let g = GlobalRain {
        intensity: 0.9,
        ..GlobalRain::default()
    };
    let view = r.smash_view(&g, 1.0).unwrap();
    let chunks = [(Vec2::new(90.0, 70.0), 5.0), (Vec2::new(50.0, 50.0), 5.0)];
    let mut rng = UnityRandom::new(21);
    assert_eq!(view.smashed_in_ground(&chunks, 2.4, &mut rng), 0.0);
    assert_draws(&mut rng.clone(), 21, 0);
    assert_eq!(view.smashed_in_ground(&chunks, 20.0, &mut rng), 0.0);
    let mut e = UnityRandom::new(21);
    e.range_i32(0, 2);
    e.value();
    e.value();
    assert_eq!(rng.value(), e.value());
}
