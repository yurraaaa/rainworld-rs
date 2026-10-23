//! Sky dandelions, green sparks, zero-gravity specks and corruption spores: draw counts
//! and orders per function, motion over a few ticks from seeded state, and `Room.Loaded`
//! / `Room.NowViewed` making them. Expected values follow `SkyDandelions.cs`,
//! `GreenSparks.cs`, `GenericZeroGSpeck.cs`, `BlinkSpeck.cs`, `CorruptionSpore.cs` and
//! `Room.cs`, replayed on seeded streams.
//!
//! Mirrors: - (port-only: tests)

use std::sync::Arc;

use rw_assets::world::{FixedRoom, LoadedRoom, WorldData};
use rw_assets::{ActiveMods, PlacedObject, Room, RoomEffect, RoomSettings};
use rw_random::UnityRandom;

use crate::campaign::{SlugcatCampaign, StoryStart};
use crate::game::camera::CameraView;
use crate::insect::InsectWater;
use crate::tests::room;
use crate::world::World;
use crate::{
    ChunkView, DandelionPatch, DandelionStalk, FloatExt, GreenSpark, GreenSparks, Input,
    LightSource, MadeDandelion, RainWorldGame, RoomExt, SkyDandelion, SkyDandelions, Sounds,
    SparkCx, SpeckCx, SpeckKind, SpeckType, Start, SuperStructureProjector, Vec2, ZeroGSpeck,
};

/// `seed`'s stream after `n` values.
fn advanced(seed: i32, n: usize) -> [u32; 4] {
    let mut r = UnityRandom::new(seed);
    for _ in 0..n {
        r.value();
    }
    r.state()
}

/// A 20 x 20 room, open inside.
fn open_room() -> Room {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 18]);
    rows.push("####################");
    room(&rows)
}

/// A 20 x 20 room with no walls at all.
fn air_room() -> Room {
    room(&["...................."; 20])
}

fn camera() -> CameraView {
    CameraView {
        room: 0,
        current: 0,
        next: 0,
    }
}

fn dry(room: &Room) -> InsectWater<'_> {
    InsectWater::new(None, room)
}

/// The first seed whose stream satisfies `f`.
fn seed_where(f: impl Fn(&mut UnityRandom) -> bool) -> i32 {
    (0..100_000)
        .find(|&s| f(&mut UnityRandom::new(s)))
        .expect("a seed")
}

/// `SkyDandelion`'s constructor: the drift angle, then with `playLayerOnly` nothing more
/// (1 draw); else `< 0.2` is depth 0 (2 draws), `< 0.5` after that `-0.5 * value`
/// (4 draws), else `Pow(value, 1.5) * 5` (4 draws).
#[test]
fn dandelion_constructor_draws() {
    let p = Vec2::new(10.0, 10.0);
    let mut rng = UnityRandom::new(3);
    let d = SkyDandelion::new(p, true, &mut rng);
    assert_eq!(rng.state(), advanced(3, 1));
    assert_eq!(d.depth, 0.0);
    assert_eq!(
        d.drift_dir,
        Vec2::from_degrees(UnityRandom::new(3).value() * 360.0)
    );

    let flat = seed_where(|r| {
        r.value();
        r.value() < 0.2
    });
    let mut rng = UnityRandom::new(flat);
    assert_eq!(SkyDandelion::new(p, false, &mut rng).depth, 0.0);
    assert_eq!(rng.state(), advanced(flat, 2));

    let behind = seed_where(|r| {
        r.value();
        r.value() >= 0.2 && r.value() < 0.5
    });
    let mut rng = UnityRandom::new(behind);
    let d = SkyDandelion::new(p, false, &mut rng);
    let mut expect = UnityRandom::new(behind);
    (0..3).for_each(|_| _ = expect.value());
    assert_eq!(d.depth, -0.5 * expect.value());
    assert_eq!(rng.state(), advanced(behind, 4));

    let far = seed_where(|r| {
        r.value();
        r.value() >= 0.2 && r.value() >= 0.5
    });
    let mut rng = UnityRandom::new(far);
    let d = SkyDandelion::new(p, false, &mut rng);
    let mut expect = UnityRandom::new(far);
    (0..3).for_each(|_| _ = expect.value());
    assert_eq!(d.depth, expect.value().mathf_pow(1.5) * 5.0);
    assert_eq!(rng.state(), advanced(far, 4));
}

/// `Update` in open air: one draw (the drift turns by `0.2` of a random direction),
/// `vel = (vel * 0.9 + drift * 0.1) + (0, 0.2)`, `pos += vel`, health stays 1; a stuck
/// one takes no drift.
#[test]
fn dandelion_update_drifts_up_with_one_draw() {
    let room = open_room();
    let start = Vec2::new(200.0, 200.0);
    let mut d = SkyDandelion::new(start, true, &mut UnityRandom::new(4));
    let drift = d.drift_dir;
    let mut rng = UnityRandom::new(9);
    d.update(&room, &dry(&room), &mut rng);
    assert_eq!(rng.state(), advanced(9, 1));
    let turned =
        (drift + Vec2::from_degrees(UnityRandom::new(9).value() * 360.0) * 0.2).normalized();
    let vel = turned * 0.1 + Vec2::new(0.0, 0.2);
    assert_eq!((d.vel, d.pos, d.last_pos), (vel, start + vel, start));
    assert_eq!((d.health, d.last_health), (1.0, 1.0));
    d.stuck = true;
    let before = d.vel;
    d.update(&room, &dry(&room), &mut UnityRandom::new(9));
    assert_eq!(d.vel, before * 0.9 + Vec2::new(0.0, 0.2));
}

/// A play-layer dandelion in a wall loses `1/30` health a tick and is put back where it
/// was; it is destroyed once `lastHealth < 0`, the health of the tick before. One past
/// the top of the room plus 300 px is destroyed.
#[test]
fn dandelion_in_a_wall_wilts_and_dies() {
    let room = room(&["#####", "#...#", "#####"]);
    let mut d = SkyDandelion::new(Vec2::new(50.0, 30.0), true, &mut UnityRandom::new(1));
    d.vel = Vec2::new(0.0, 12.0);
    d.pos = Vec2::new(50.0, 39.0);
    let mut rng = UnityRandom::new(2);
    d.update(&room, &dry(&room), &mut rng);
    assert!(d.vel.y > 0.0);
    assert_eq!(d.pos, d.last_pos);
    assert_eq!(d.health, 1.0 - 1.0 / 30.0);
    d.health = 0.02;
    d.update(&room, &dry(&room), &mut rng);
    assert!(!d.slated, "lastHealth was still positive");
    assert!(d.health < 0.0);
    d.update(&room, &dry(&room), &mut rng);
    assert!(d.slated);

    let mut high = SkyDandelion::new(Vec2::new(50.0, 40.0), false, &mut UnityRandom::new(1));
    high.depth = 1.0;
    high.pos.y = room.pixel_height() + 301.0;
    high.update(&room, &dry(&room), &mut rng);
    assert!(high.slated);
}

/// A dandelion under water is held at the surface (`FloatWaterLevel`).
#[test]
fn dandelion_under_water_is_held_at_the_surface() {
    let room = crate::tests::room_with_water(&["#####", "#...#", "#...#", "#####"], 1);
    let water = InsectWater::new(None, &room);
    let mut d = SkyDandelion::new(Vec2::new(50.0, 25.0), true, &mut UnityRandom::new(1));
    assert!(water.submerged(d.pos));
    d.update(&room, &water, &mut UnityRandom::new(2));
    assert_eq!(d.pos.y, water.level(Vec2::new(50.0, 25.0)));
}

/// `SkyDandelions.Update`: the wait counts down one a tick; at zero it grows by
/// `Lerp(30, 0.4, amount)` and one draw is tested against `SpawnChance`; a spawn takes
/// one more draw for x and the constructor's two or four; the dandelion is below the
/// room (`y = -100`).
#[test]
fn dandelion_spawner_update() {
    let room = open_room();
    let mut s = SkyDandelions {
        update_order: 0,
        amount: 0.5,
        wait: 2.0,
    };
    let weather = (0.0, 0.0);
    let mut rng = UnityRandom::new(5);
    assert!(s.update(&room, weather, &mut rng).is_none());
    assert!(s.update(&room, weather, &mut rng).is_none());
    assert_eq!((s.wait, rng.state()), (0.0, UnityRandom::new(5).state()));
    let seed = seed_where(|r| r.value() < 0.5);
    let mut rng = UnityRandom::new(seed);
    let made = s.update(&room, weather, &mut rng).expect("a spawn");
    assert_eq!(s.wait, 30.0_f32.lerp(0.4, 0.5));
    let mut expect = UnityRandom::new(seed);
    expect.value();
    let x = (-150.0_f32).lerp(room.pixel_width() + 150.0, expect.value());
    assert_eq!(made.pos, Vec2::new(x, -100.0));
    let draws = if made.depth == 0.0 { 4 } else { 6 };
    assert_eq!(rng.state(), advanced(seed, draws));
    // The middle of the cycle's chance is `amount`; the cycle's ends have none.
    assert_eq!(s.spawn_chance(0.0, 0.0), 0.5);
    assert_eq!(s.spawn_chance(1.0, 0.0), 0.0);
}

/// The constructor's scatter: `ceil(W * H * 20 * chance / 1000)` tries of two
/// `Random.Range` draws, an open tile adding the two jitter draws and the dandelion's.
#[test]
fn dandelion_spawner_constructor_scatters() {
    let room = room(&["#####", "#...#", "#...#", "#...#", "#####"]);
    let s = SkyDandelions {
        update_order: 0,
        amount: 1.0,
        wait: 0.0,
    };
    // 25 * 20 * 1 / 1000 = 0.5: one try.
    let seed = seed_where(|r| {
        let (x, y) = (r.range_i32(0, 5), r.range_i32(0, 5));
        (1..4).contains(&x) && (1..4).contains(&y)
    });
    let mut rng = UnityRandom::new(seed);
    let made = s.generate(&room, 0, &[], (0.0, 0.0), &mut rng);
    let mut expect = UnityRandom::new(seed);
    let tile = (expect.range_i32(0, 5), expect.range_i32(0, 5));
    let jitter = Vec2::new(-10.0 + 20.0 * expect.value(), -10.0 + 20.0 * expect.value());
    let MadeDandelion::Free(d) = &made[0] else {
        panic!("free");
    };
    assert_eq!(made.len(), 1);
    assert_eq!(d.pos, Vec2::middle_of_tile(tile) + jitter);
    let mut check = expect.clone();
    let reference = SkyDandelion::new(d.pos, false, &mut check);
    assert_eq!(d.depth, reference.depth);
    assert_eq!(rng.state(), check.state());

    // A solid tile ends the try after the two position draws.
    let seed = seed_where(|r| {
        let (x, y) = (r.range_i32(0, 5), r.range_i32(0, 5));
        x == 0 && y == 0
    });
    let mut rng = UnityRandom::new(seed);
    assert!(s.generate(&room, 0, &[], (0.0, 0.0), &mut rng).is_empty());
    assert_eq!(rng.state(), advanced(seed, 2));
    // No chance at the cycle's ends: no tries, no draws.
    let mut rng = UnityRandom::new(1);
    assert!(s.generate(&room, 0, &[], (1.0, 0.0), &mut rng).is_empty());
    assert_eq!(rng.state(), UnityRandom::new(1).state());
}

/// A `DandelionPatch` on a floor: every candidate tile (open, solid below, open above,
/// not walled in on both sides) within `Rad` gets `3 * density` seeded tries, a planted
/// dandelion (stuck, in the play layer) with its stalk for each success; the shared
/// stream gives one draw per planted dandelion (its drift angle); `SeededRandom`'s `num`
/// counts every try from the room's index.
#[test]
fn dandelion_patch_plants_with_seeded_draws() {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 6]);
    rows.push("####################");
    let room = room(&rows);
    let s = SkyDandelions {
        update_order: 0,
        amount: 0.0,
        wait: 0.0,
    };
    let patch = DandelionPatch {
        pos: Vec2::new(210.0, 30.0),
        rad: 100.0,
    };
    let mut rng = UnityRandom::new(7);
    let made = s.generate(&room, 3, &[patch], (0.0, 0.0), &mut rng);
    let planted: Vec<_> = made
        .iter()
        .filter_map(|m| match m {
            MadeDandelion::Planted(st, d) => Some((st, d)),
            MadeDandelion::Free(_) => None,
        })
        .collect();
    assert_eq!(planted.len(), made.len());
    assert!(!planted.is_empty());
    assert_eq!(rng.state(), advanced(7, planted.len()));
    let mut num = 3;
    let mut expect = Vec::new();
    for k in 5..=15 {
        for l in -4..=6 {
            if k <= 0 || k >= 20 || l <= 0 || l >= 8 || room.is_solid_tile(k, l) {
                continue;
            }
            if room.is_solid_tile(k, l + 1)
                || !room.is_solid_tile(k, l - 1)
                || (room.is_solid_tile(k - 1, l) && room.is_solid_tile(k + 1, l))
            {
                continue;
            }
            let middle = Vec2::middle_of_tile((k, l));
            let density = patch.pos.distance(middle).inverse_lerp(100.0, 100.0 / 3.0)
                * 100.0_f32.lerp_map(40.0, 200.0, 0.5, 1.0);
            let mut m = 0;
            while (m as f32) < 3.0 * density {
                if UnityRandom::new(num).value() < density.mathf_pow(0.5) {
                    let height = 0.1
                        + 0.9
                            * UnityRandom::new(num + 1000)
                                .value()
                                .mathf_pow(0.5_f32.lerp(2.5, density));
                    let x = -10.0 + UnityRandom::new(num - 1000).value() * 20.0;
                    expect.push(middle + Vec2::new(x, -10.0 + height * 30.0));
                }
                num += 1;
                m += 1;
            }
        }
    }
    let got: Vec<Vec2> = planted.iter().map(|(_, d)| d.pos).collect();
    assert_eq!(got, expect);
    for (stalk, d) in planted {
        assert!(d.stuck && d.in_play_layer());
        // Rooted 11 px under the tile's middle, 20 more when that is still open.
        let root = d.pos.tile_middle().y - 11.0;
        let root = if room.is_solid_at(Vec2::new(d.pos.x, root)) {
            root
        } else {
            root - 20.0
        };
        assert_eq!(stalk.pos, Vec2::new(d.pos.x, root));
        assert_eq!(stalk.stalk_length, stalk.pos.distance(d.pos));
        let seed = stalk.pos.x as i32 + stalk.pos.y as i32 + 3;
        assert_eq!(
            stalk.stalk_dir,
            Vec2::from_degrees((-45.0_f32).lerp(45.0, UnityRandom::new(seed).value()))
        );
    }
}

fn stalk_and_dandelion() -> (DandelionStalk, SkyDandelion) {
    let room = open_room();
    let mut d = SkyDandelion::new(Vec2::new(200.0, 200.0), true, &mut UnityRandom::new(1));
    d.stuck = true;
    d.update_order = 5;
    let mut stalk = DandelionStalk::new(&d, &room, 0, &mut UnityRandom::new(1));
    stalk.dandelion = Some(5);
    (stalk, d)
}

/// An attached stalk pulls its dandelion toward `pos + (0, length)` (spring 0.05) and
/// draws nothing while no chunk is in reach; a fast chunk touching it takes two draws
/// (the velocity blend, then the blow-off test, certain at speed 8) and the stalk lets
/// go, remembering the dandelion's place as its tip.
#[test]
fn stalk_pulls_and_a_fast_chunk_blows_the_dandelion_loose() {
    let room = open_room();
    let (mut stalk, mut d) = stalk_and_dandelion();
    let top = stalk.pos + Vec2::new(0.0, stalk.stalk_length);
    d.pos = top - Vec2::new(0.0, 4.0);
    let mut rng = UnityRandom::new(6);
    stalk.update(Some(&mut d), &room, 0, &camera(), &[], &mut rng);
    assert_eq!(d.vel, Vec2::new(0.0, 4.0) * 0.05);
    assert_eq!(rng.state(), UnityRandom::new(6).state());
    let chunk = ChunkView {
        pos: d.pos + Vec2::new(-2.0, 0.0),
        last_pos: d.pos + Vec2::new(-12.0, 0.0),
        rad: 5.0,
    };
    stalk.update(Some(&mut d), &room, 0, &camera(), &[chunk], &mut rng);
    assert_eq!(rng.state(), advanced(6, 2));
    assert_eq!(stalk.dandelion, None);
    assert!(!d.stuck);
    assert_eq!((stalk.tip, stalk.last_tip), (d.pos, d.pos));
    assert!(d.vel.x > 4.0, "pushed away and along: {:?}", d.vel);
}

/// A chunk touching it too slowly lerps its velocity (one draw), the blow-off test
/// fails (`InverseLerp(1, 8, 0.5)` is 0) and the draw is still made.
#[test]
fn a_slow_chunk_only_nudges() {
    let room = open_room();
    let (mut stalk, mut d) = stalk_and_dandelion();
    d.pos = stalk.pos + Vec2::new(0.0, stalk.stalk_length);
    let chunk = ChunkView {
        pos: d.pos,
        last_pos: d.pos - Vec2::new(0.5, 0.0),
        rad: 5.0,
    };
    let mut rng = UnityRandom::new(6);
    stalk.update(Some(&mut d), &room, 0, &camera(), &[chunk], &mut rng);
    assert_eq!(rng.state(), advanced(6, 2));
    assert_eq!(stalk.dandelion, Some(5));
}

/// A stalk whose dandelion is out of camera 0's reach (100 px margin) reads no chunks.
#[test]
fn an_unseen_stalk_reads_no_chunks() {
    let room = open_room();
    let (mut stalk, mut d) = stalk_and_dandelion();
    d.pos = stalk.pos + Vec2::new(0.0, stalk.stalk_length);
    let far = CameraView {
        room: 1,
        current: 0,
        next: 0,
    };
    let chunk = ChunkView {
        pos: d.pos,
        last_pos: d.pos - Vec2::new(10.0, 0.0),
        rad: 5.0,
    };
    let mut rng = UnityRandom::new(6);
    stalk.update(Some(&mut d), &room, 0, &far, &[chunk], &mut rng);
    assert_eq!(rng.state(), UnityRandom::new(6).state());
    assert_eq!(stalk.dandelion, Some(5));
}

/// A loose stalk: one draw a tick, the tip eases 0.1 toward the root, the stalk
/// straightens by `value / 15` and is destroyed when the tip is within 1 px.
#[test]
fn a_loose_stalk_straightens_and_goes() {
    let room = open_room();
    let (mut stalk, _) = stalk_and_dandelion();
    stalk.dandelion = None;
    stalk.tip = stalk.pos + Vec2::new(0.0, 30.0);
    let mut rng = UnityRandom::new(8);
    stalk.update(None, &room, 0, &camera(), &[], &mut rng);
    assert_eq!(rng.state(), advanced(8, 1));
    assert_eq!(stalk.tip, stalk.pos + Vec2::new(0.0, 27.0));
    assert_eq!(stalk.straight_stalk, UnityRandom::new(8).value() / 15.0);
    let mut ticks = 1;
    while !stalk.slated {
        stalk.update(None, &room, 0, &camera(), &[], &mut rng);
        ticks += 1;
    }
    assert_eq!(rng.state(), advanced(8, ticks));
    assert!(ticks > 20 && stalk.tip.dist_less(stalk.pos, 1.0));
}

/// `GreenSpark`'s constructor: `lifeTime = Lerp(600, 1200, v)`, then `v < 0.4` is depth 0
/// (2 draws in all), else `v < 0.3` depth `-0.5 * v`, else `Pow(v, 1.5) * 3` (4 draws).
#[test]
fn spark_constructor_draws() {
    let p = Vec2::new(30.0, 30.0);
    let flat = seed_where(|r| {
        r.value();
        r.value() < 0.4
    });
    let mut rng = UnityRandom::new(flat);
    let s = GreenSpark::new(p, &mut rng);
    assert_eq!(s.depth, 0.0);
    assert_eq!(
        s.life_time,
        600.0_f32.lerp(1200.0, UnityRandom::new(flat).value())
    );
    assert_eq!(rng.state(), advanced(flat, 2));
    assert_eq!((s.life, s.dir, s.vel), (1.0, Vec2::ZERO, Vec2::ZERO));

    let behind = seed_where(|r| {
        r.value();
        r.value() >= 0.4 && r.value() < 0.3
    });
    let mut rng = UnityRandom::new(behind);
    let s = GreenSpark::new(p, &mut rng);
    let mut e = UnityRandom::new(behind);
    (0..3).for_each(|_| _ = e.value());
    assert_eq!(s.depth, -0.5 * e.value());
    assert_eq!(rng.state(), advanced(behind, 4));

    let far = seed_where(|r| {
        r.value();
        r.value() >= 0.4 && r.value() >= 0.3
    });
    let mut rng = UnityRandom::new(far);
    let s = GreenSpark::new(p, &mut rng);
    let mut e = UnityRandom::new(far);
    (0..3).for_each(|_| _ = e.value());
    assert_eq!(s.depth, e.value().mathf_pow(1.5) * 3.0);
    assert_eq!(rng.state(), advanced(far, 4));
}

fn spark_cx<'a>(room: &'a Room, water: &'a InsectWater<'a>, cam: &'a CameraView) -> SparkCx<'a> {
    SparkCx {
        room,
        room_index: 0,
        water,
        camera: cam,
        darkness: 0.0,
        being_viewed: true,
    }
}

/// `GreenSpark.Update` in open air: one draw (`RNV` for the heading), `vel = vel * 0.99
/// + (0.11, LerpMap(life, 0, 0.5, -0.1, 0.05)) + dir * 0.2`, `life -= 1 / lifeTime`,
/// `pos += vel / (depth + 1)`, the position history shifted; no light in a lit room.
#[test]
fn spark_update_moves_with_one_draw() {
    let room = open_room();
    let (water, cam) = (dry(&room), camera());
    let mut s = GreenSpark::new(Vec2::new(200.0, 200.0), &mut UnityRandom::new(2));
    s.depth = 0.0;
    s.dir = Vec2::new(1.0, 0.0);
    let mut rng = UnityRandom::new(3);
    let light = s.update(&spark_cx(&room, &water, &cam), &mut rng);
    assert_eq!(light, None);
    assert_eq!(rng.state(), advanced(3, 1));
    let rise = 1.0_f32.lerp_map(0.0, 0.5, -0.1, 0.05);
    let vel = Vec2::new(0.11, rise) + Vec2::new(0.2, 0.0);
    assert_eq!(s.vel, vel);
    assert_eq!(s.pos, Vec2::new(200.0, 200.0) + vel);
    assert_eq!(
        (s.last_pos, s.last_last_pos),
        (Vec2::new(200.0, 200.0), Vec2::new(200.0, 200.0))
    );
    let turned = (Vec2::new(1.0, 0.0)
        + Vec2::from_degrees(UnityRandom::new(3).value() * 360.0) * 0.6)
        .normalized();
    assert_eq!(s.dir, turned);
    assert_eq!(s.life, 1.0 - 1.0 / s.life_time);
}

/// A play-layer spark that hits a wall loses 0.025 life, is put on the tile's rect grown
/// by 2 px and bounces off with 0.3 of its speed; in a dark room it asks for a light
/// (`0.4 * InverseLerp(0, 0.2, life) * InverseLerp(-0.6, 0, depth)`); out of view
/// beyond 100 px of the room it dies, and any spark dies once the camera leaves.
#[test]
fn spark_walls_light_and_death() {
    let room = room(&["#####", "#...#", "#####"]);
    let (water, cam) = (dry(&room), camera());
    let mut s = GreenSpark::new(Vec2::new(30.0, 30.0), &mut UnityRandom::new(2));
    s.depth = 0.0;
    s.pos = Vec2::new(60.0, 30.0);
    s.vel = Vec2::new(30.0, 0.0);
    let dark = SparkCx {
        darkness: 0.5,
        ..spark_cx(&room, &water, &cam)
    };
    let life = s.life;
    let light = s.update(&dark, &mut UnityRandom::new(3));
    // The step to x = 89.9 enters the wall; the spark is put on its rect grown by 2.
    assert!((life - s.life - 0.025).abs() < 0.002);
    assert_eq!(s.pos.x, 78.0);
    assert!(s.vel.x < 0.0 && s.vel.x > -10.0, "bounced: {:?}", s.vel);
    let (pos, alpha) = light.expect("a light in the dark");
    assert_eq!(pos, s.pos);
    assert_eq!(alpha, 0.4 * s.life.inverse_lerp(0.0, 0.2) * 1.0);

    let mut gone = GreenSpark::new(Vec2::new(30.0, 30.0), &mut UnityRandom::new(2));
    gone.depth = 1.0;
    let outside = SparkCx {
        being_viewed: false,
        ..spark_cx(&room, &water, &cam)
    };
    assert_eq!(gone.update(&outside, &mut UnityRandom::new(3)), None);
    assert!(gone.slated, "not being viewed");

    let mut far = GreenSpark::new(Vec2::new(30.0, 30.0), &mut UnityRandom::new(2));
    far.pos = Vec2::new(-500.0, 30.0);
    far.depth = 1.0;
    let other_room = CameraView {
        room: 1,
        current: 0,
        next: 0,
    };
    far.update(
        &spark_cx(&room, &water, &other_room),
        &mut UnityRandom::new(3),
    );
    assert!(far.slated, "100 px off the room and not in view");
    let mut near = GreenSpark::new(Vec2::new(30.0, 30.0), &mut UnityRandom::new(2));
    near.depth = 1.0;
    near.update(
        &spark_cx(&room, &water, &other_room),
        &mut UnityRandom::new(3),
    );
    assert!(!near.slated);
}

/// `GreenSparks`: `Clamp((int)(W * H * Lerp(0, 0.1, amount)), 1, 300)`.
#[test]
fn green_sparks_cap() {
    let big = open_room();
    assert_eq!(GreenSparks::new(&big, 0.5).tot_sparks, 20);
    assert_eq!(GreenSparks::new(&big, 0.0).tot_sparks, 1);
    let huge = room(
        &{
            let mut rows = vec!["#".repeat(100)];
            rows.extend((0..98).map(|_| format!("#{}#", ".".repeat(98))));
            rows.push("#".repeat(100));
            rows
        }
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
    );
    assert_eq!(GreenSparks::new(&huge, 1.0).tot_sparks, 300);
}

/// `GreenSparks.Update` under the cap: `AddSpark` draws the edge (`v < H / W`), the
/// place on it, then the spark's constructor; the wind's `RNV` last. At the cap only the
/// wind draws. A spark it already has is pushed by `wind * 0.2` and a slated one dropped.
#[test]
fn green_sparks_update_adds_one_spark_and_moves_the_wind() {
    let room = air_room();
    let cam = CameraView {
        room: 1,
        current: 0,
        next: 0,
    };
    let mut spawner = GreenSparks::new(&room, 1.0);
    let mut rng = UnityRandom::new(21);
    let mut all = Vec::<GreenSpark>::new();
    let added = spawner
        .update(&mut all, &room, 0, &cam, &mut rng)
        .expect("a spark");
    // H / W = 1: the left edge, a place on it, a spark, the wind.
    let mut e = UnityRandom::new(21);
    assert!(e.value() < 1.0);
    let tile = (0, e.range_i32(0, 20));
    let spark = GreenSpark::new(Vec2::middle_of_tile(tile), &mut e);
    e.value();
    assert_eq!((added.pos, added.life_time), (spark.pos, spark.life_time));
    assert_eq!(rng.state(), e.state());
    // At the cap: only the wind's draw.
    spawner.tot_sparks = 0;
    let mut rng = UnityRandom::new(4);
    assert!(spawner.update(&mut all, &room, 0, &cam, &mut rng).is_none());
    assert_eq!(rng.state(), advanced(4, 1));
    // Sparks of its own: pushed by the wind, slated ones forgotten.
    let mut a = GreenSpark::new(Vec2::new(50.0, 50.0), &mut UnityRandom::new(1));
    a.update_order = 1;
    let mut b = a.clone();
    b.update_order = 2;
    b.slated = true;
    let mut all = vec![a, b];
    spawner.sparks = vec![1, 2];
    spawner.wind = Vec2::new(1.0, 0.0);
    spawner.update(&mut all, &room, 0, &cam, &mut UnityRandom::new(4));
    assert_eq!(spawner.sparks, vec![1]);
    assert_eq!(all[0].vel, Vec2::new(0.2, 0.0));
    assert!(spawner.wind.length() <= 1.0);
}

/// `Room.NowViewed`'s sparks: `W * H * amount / 50` tries of two position draws, the
/// `Pow(value, 1 - amount) > proximity * 0.05` draw on open tiles only (proximity 5
/// before `readyForAI`), and the constructor's draws on success.
#[test]
fn spark_seeding_draws() {
    let room = room(&["#####", "#...#", "#...#", "#...#", "#####"]);
    // 25 * 1 / 50 = 0.5: one try; `Pow(v, 0)` is 1, past 5 * 0.05.
    let inside = seed_where(|r| {
        let (x, y) = (r.value() * 100.0, r.value() * 100.0);
        (20.0..80.0).contains(&x) && (20.0..80.0).contains(&y)
    });
    let mut rng = UnityRandom::new(inside);
    let made = GreenSpark::seed_room(&room, 1.0, None, &mut rng);
    let mut e = UnityRandom::new(inside);
    let pos = Vec2::new(e.value() * 100.0, e.value() * 100.0);
    e.value();
    let spark = GreenSpark::new(pos, &mut e);
    assert_eq!(made.len(), 1);
    assert_eq!((made[0].pos, made[0].life_time), (pos, spark.life_time));
    assert_eq!(rng.state(), e.state());
    // A solid tile is rejected before the third draw.
    let wall = seed_where(|r| r.value() < 0.2 && r.value() < 0.2);
    let mut rng = UnityRandom::new(wall);
    assert!(GreenSpark::seed_room(&room, 1.0, None, &mut rng).is_empty());
    assert_eq!(rng.state(), advanced(wall, 2));
    // Amount 0.5 (0.25 tries, one): `Pow(v, 0.5) > 0.25` fails for `v` under 1/16.
    let weak = seed_where(|r| {
        let (x, y) = (r.value() * 100.0, r.value() * 100.0);
        (20.0..80.0).contains(&x) && (20.0..80.0).contains(&y) && r.value() < 0.06
    });
    let mut rng = UnityRandom::new(weak);
    assert!(GreenSpark::seed_room(&room, 0.5, None, &mut rng).is_empty());
    assert_eq!(rng.state(), advanced(weak, 3));
}

/// `BlinkSpeck`'s constructor takes `ResetMe`'s two draws (speed, heading); a
/// `CorruptionSpore`'s takes eight, in the game's order.
#[test]
fn speck_constructor_draws() {
    let mut rng = UnityRandom::new(5);
    let b = ZeroGSpeck::new(SpeckType::Blink, &mut rng);
    assert_eq!(rng.state(), advanced(5, 2));
    let mut e = UnityRandom::new(5);
    let speed = 0.1_f32.lerp(3.0, e.value());
    assert_eq!(b.my_float_speed, speed);
    assert_eq!(b.vel, Vec2::from_degrees(e.value() * 360.0) * speed);
    assert_eq!((b.saved_cam_pos, b.reset, b.pos), (-1, false, Vec2::ZERO));

    let mut rng = UnityRandom::new(5);
    let c = ZeroGSpeck::new(SpeckType::CorruptionSpore, &mut rng);
    assert_eq!(rng.state(), advanced(5, 8));
    let SpeckKind::CorruptionSpore(s) = c.kind else {
        panic!("spore");
    };
    let mut e = UnityRandom::new(5);
    e.value();
    e.value();
    assert_eq!(s.col, e.value());
    assert_eq!(s.scale, e.value());
    let rot = e.value() * 360.0;
    assert_eq!((s.rot, s.last_rot), (rot, rot));
    assert_eq!(s.rot_speed, (-10.0_f32).lerp(10.0, e.value()));
    let y_rot = e.value() * std::f32::consts::PI;
    assert_eq!((s.y_rot, s.last_y_rot), (y_rot, y_rot));
    assert_eq!(s.y_rot_speed, (-1.0_f32).lerp(1.0, e.value()) * 0.05);
}

/// How many specks `Room.NowViewed` makes: `k < 1000 * amount` and `k < 200 * amount`.
#[test]
fn speck_counts() {
    assert_eq!(SpeckType::Blink.count(0.3), 300);
    assert_eq!(SpeckType::Blink.count(0.0), 0);
    assert_eq!(SpeckType::Blink.count(0.0005), 1);
    assert_eq!(SpeckType::CorruptionSpore.count(0.5), 100);
    assert_eq!(SpeckType::CorruptionSpore.count(0.003), 1);
}

fn speck_cx<'a>(room: &'a Room, lights: &'a [LightSource]) -> SpeckCx<'a> {
    SpeckCx {
        room,
        gravity: 0.0,
        ready_for_ai: false,
        ai_map: None,
        camera_pos: Vec2::new(100.0, 50.0),
        current_camera: 0,
        being_viewed: true,
        darkness: 0.0,
        lights,
    }
}

/// In zero gravity a speck's first update places it in the window grown by 200 px (two
/// draws, after the drift's two and the blink's one or two); later ticks make the
/// drift's two, the blink's and no placement.
#[test]
fn zero_g_blink_speck_draws() {
    let room = open_room();
    let cx = speck_cx(&room, &[]);
    let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut UnityRandom::new(1));
    let vel0 = s.vel;
    let mut rng = UnityRandom::new(12);
    s.update(&cx, &mut rng);
    let mut e = UnityRandom::new(12);
    // Drift: RNV, nudge size; placement x, y; blink test (and value).
    let heading = Vec2::from_degrees(e.value() * 360.0);
    let nudge = heading * (e.value() * 0.05);
    let eased = vel0.lerp(vel0.normalized() * s.my_float_speed, 0.01);
    assert_eq!(s.vel, eased + nudge);
    let first = s.pos;
    let x = (-200.0_f32).lerp(1224.0, e.value());
    let y = (-200.0_f32).lerp(968.0, e.value());
    assert_eq!(first, Vec2::new(100.0 + x, 50.0 + y));
    assert_eq!(s.last_pos, first);
    assert_eq!(s.saved_cam_pos, 0);
    let blinks = e.value() < 1.0 / 3.0;
    assert_eq!(rng.state(), advanced(12, 5 + usize::from(blinks)));
    let SpeckKind::Blink(b) = s.kind else {
        panic!("speck");
    };
    assert_eq!(b.last_blink, 0.0);
    let after = s.vel;
    // Second tick: no placement draws.
    let mut rng = UnityRandom::new(13);
    s.update(&cx, &mut rng);
    let blinks = {
        let mut e = UnityRandom::new(13);
        e.value();
        e.value();
        e.value() < 1.0 / 3.0
    };
    assert_eq!(rng.state(), advanced(13, 3 + usize::from(blinks)));
    assert_eq!(s.last_pos, first);
    assert_eq!(s.pos, first + after);
}

/// Zero gravity, past the window's edge the speck wraps to the other side (the window
/// is 1024 x 768 plus 200 px each way); with gravity it falls out (`reset`).
#[test]
fn speck_wraps_or_resets() {
    let room = air_room();
    let cx = speck_cx(&room, &[]);
    let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut UnityRandom::new(1));
    s.saved_cam_pos = 0;
    s.pos = Vec2::new(100.0 - 199.0, 400.0);
    s.vel = Vec2::new(-5.0, 0.0);
    s.update(&cx, &mut UnityRandom::new(2));
    assert!(s.pos.x > 1000.0 && !s.reset);

    let falling = SpeckCx {
        gravity: 1.0,
        ..speck_cx(&room, &[])
    };
    let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut UnityRandom::new(1));
    s.pos = Vec2::new(100.0, 50.0 - 199.0);
    s.vel = Vec2::new(0.0, -5.0);
    s.update(&falling, &mut UnityRandom::new(2));
    assert!(s.reset);
}

/// With gravity the speck falls: one draw (the angle of `DegToVec(180 + Lerp(-45, 45,
/// v))`), `vel.y -= 0.2 * g`; in a wall (position and last) it resets. A fallen speck
/// waits for zero gravity and then, 2.5 % of ticks, picks a window point: no draw
/// while gravity is on.
#[test]
fn speck_falls_with_gravity_and_waits_to_be_placed() {
    let room = open_room();
    let cx = SpeckCx {
        gravity: 1.0,
        camera_pos: Vec2::ZERO,
        ..speck_cx(&room, &[])
    };
    let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut UnityRandom::new(1));
    s.pos = Vec2::new(200.0, 200.0);
    s.last_pos = s.pos;
    s.vel = Vec2::ZERO;
    let mut rng = UnityRandom::new(14);
    s.update(&cx, &mut rng);
    let angle = 180.0 + (-45.0_f32).lerp(45.0, UnityRandom::new(14).value());
    assert_eq!(
        s.vel,
        Vec2::new(0.0, -0.2) + Vec2::from_degrees(angle) * 0.1
    );
    let blinks = {
        let mut e = UnityRandom::new(14);
        e.value();
        e.value() < 1.0 / 3.0
    };
    assert_eq!(rng.state(), advanced(14, 2 + usize::from(blinks)));

    s.reset = true;
    let blinks_only = SpeckCx { gravity: 1.0, ..cx };
    let mut rng = UnityRandom::new(15);
    s.update(&blinks_only, &mut rng);
    let blinks = {
        let mut e = UnityRandom::new(15);
        e.value() < 1.0 / 3.0
    };
    assert_eq!(rng.state(), advanced(15, 1 + usize::from(blinks)));
    assert!(s.reset);

    // Gravity 0: a roll under 0.025 draws the window point (two more).
    let zero = SpeckCx { gravity: 0.0, ..cx };
    let seed = seed_where(|r| r.value() < 0.025);
    let mut rng = UnityRandom::new(seed);
    s.update(&zero, &mut rng);
    let blinks = {
        let mut e = UnityRandom::new(seed);
        (0..3).for_each(|_| _ = e.value());
        e.value() < 1.0 / 3.0
    };
    assert_eq!(rng.state(), advanced(seed, 4 + usize::from(blinks)));
    assert!(s.reset, "not ready for AI: stays down");
    let seed = seed_where(|r| r.value() >= 0.025);
    let mut rng = UnityRandom::new(seed);
    s.update(&zero, &mut rng);
    let blinks = {
        let mut e = UnityRandom::new(seed);
        e.value();
        e.value() < 1.0 / 3.0
    };
    assert_eq!(rng.state(), advanced(seed, 2 + usize::from(blinks)));
}

/// A spore's update adds two draws after the base class's: the spin and the turn.
#[test]
fn spore_update_spins_with_two_more_draws() {
    let room = open_room();
    let cx = SpeckCx {
        gravity: 1.0,
        camera_pos: Vec2::ZERO,
        ..speck_cx(&room, &[])
    };
    let mut s = ZeroGSpeck::new(SpeckType::CorruptionSpore, &mut UnityRandom::new(1));
    s.pos = Vec2::new(200.0, 200.0);
    s.last_pos = s.pos;
    let SpeckKind::CorruptionSpore(before) = s.kind else {
        panic!("spore");
    };
    let mut rng = UnityRandom::new(16);
    s.update(&cx, &mut rng);
    assert_eq!(rng.state(), advanced(16, 3));
    let mut e = UnityRandom::new(16);
    e.value();
    let rot_turn = (-1.0_f32).lerp(1.0, e.value()) / 30.0;
    let y_turn = (-1.0_f32).lerp(1.0, e.value()) / 320.0;
    let SpeckKind::CorruptionSpore(after) = s.kind else {
        panic!("spore");
    };
    assert_eq!(after.last_rot, before.rot);
    assert_eq!(after.rot, before.rot + before.rot_speed);
    assert_eq!(
        after.rot_speed,
        (before.rot_speed + rot_turn).clamp(-10.0, 10.0)
    );
    assert_eq!(after.y_rot, before.y_rot + before.y_rot_speed);
    assert_eq!(
        after.y_rot_speed,
        (before.y_rot_speed + y_turn).clamp(-0.05, 0.05)
    );
}

/// A speck is destroyed on its update once its room is not being viewed.
#[test]
fn specks_end_when_unviewed() {
    let room = open_room();
    let cx = SpeckCx {
        being_viewed: false,
        gravity: 1.0,
        ..speck_cx(&room, &[])
    };
    let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut UnityRandom::new(1));
    s.update(&cx, &mut UnityRandom::new(2));
    assert!(s.slated);
}

fn game_in(room: Room, effects: &str, placed: &str) -> RainWorldGame {
    let data = WorldData::from_room(&room);
    let settings = RoomSettings {
        random_item_density: 0.0,
        effects: RoomEffect::parse_list(effects),
        placed_objects: PlacedObject::parse_list(placed),
        ..RoomSettings::ANCESTOR
    };
    let campaign = &SlugcatCampaign::SURVIVOR;
    let rooms = Arc::new(FixedRoom(LoadedRoom { room, settings }));
    let world = World::new(data, rooms, campaign.timeline, ActiveMods::NONE, true);
    RainWorldGame::new_story(
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
    )
}

/// `Room.Loaded` adds the effect objects in the settings' order, the spawner's
/// dandelions before the spawner; `Room.NowViewed` seeds the specks (`1000 * amount`)
/// and spores (`200 * amount`) when camera 0 first shows the room.
#[test]
fn room_loaded_and_viewed_make_the_particles() {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 10]);
    rows.push("####################");
    let mut g = game_in(
        room(&rows),
        "SkyDandelions-1, GreenSparks-0.5, ZeroGSpecks-0.1, CorruptionSpores-0.2",
        "",
    );
    for _ in 0..3 {
        g.tick(Input::NONE, &mut ());
    }
    let r = g.world.realized(0).expect("realized");
    let p = &r.particles;
    assert_eq!(p.dandelion_spawners.len(), 1);
    assert_eq!(p.spark_spawners.len(), 1);
    let specks = p
        .specks
        .iter()
        .filter(|s| matches!(s.kind, SpeckKind::Blink(_)))
        .count();
    let spores = p.specks.len() - specks;
    assert_eq!((specks, spores), (100, 40));
    assert!(
        p.specks
            .windows(2)
            .all(|w| w[0].update_order < w[1].update_order)
    );
    assert!(
        p.dandelions
            .iter()
            .all(|d| d.update_order < p.dandelion_spawners[0].update_order)
    );
    assert!(p.spark_spawners[0].update_order > p.dandelion_spawners[0].update_order);
    // The specks and spores come after the sparks `NowViewed` seeds, in effect order.
    let first_speck = p.specks[0].update_order;
    assert!(p.sparks.iter().all(|s| s.update_order < first_speck));
}

/// A room with a `DandelionPatch` makes its stalks and dandelions at `Room.Loaded`.
#[test]
fn room_loaded_plants_the_patch() {
    let mut rows = vec!["####################"];
    rows.extend(["#..................#"; 10]);
    rows.push("####################");
    let mut g = game_in(
        room(&rows),
        "SkyDandelions-0",
        "DandelionPatch><200><40><100~0",
    );
    g.tick(Input::NONE, &mut ());
    let p = &g.world.realized(0).expect("realized").particles;
    assert!(!p.stalks.is_empty());
    assert_eq!(p.stalks.len(), p.dandelions.len());
    for s in &p.stalks {
        let d = p
            .dandelions
            .iter()
            .find(|d| Some(d.update_order) == s.dandelion)
            .expect("its dandelion");
        assert!(s.update_order + 1 == d.update_order && d.stuck);
    }
}

/// `Room.NowViewed` runs the effects in the settings' order: the specks, then the
/// projector with its parts (before it in the update list); the draws are the two
/// objects' setup draws in that order.
#[test]
fn now_viewed_interleaves_specks_and_the_projector() {
    let mut rows = vec!["#".repeat(20)];
    rows.extend(vec![".".repeat(20); 8]);
    rows.push("#".repeat(20));
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut g = game_in(
        room(&refs),
        "ZeroGSpecks-0.01, SuperStructureProjector-1",
        "",
    );
    assert!(
        g.world
            .realized(0)
            .expect("room")
            .projections
            .projectors
            .is_empty()
    );
    let mut replay = g.rng.clone();
    let mut seq = g.world.update_seq();
    let first = seq;
    for _ in 0..SpeckType::Blink.count(0.01) {
        let _ = ZeroGSpeck::new(SpeckType::Blink, &mut replay);
        seq += 1;
    }
    let projector = SuperStructureProjector::new((20, 10), 1.0, &mut seq, &mut replay);
    g.particles_now_viewed(0);
    assert_eq!(g.rng.state(), replay.state());
    let r = g.world.realized(0).expect("room");
    let specks = &r.particles.specks;
    let made = &r.projections.projectors[0];
    assert_eq!(specks.len(), 10);
    assert_eq!(specks[0].update_order, first + 1);
    assert!(specks.iter().all(|s| s.update_order <= first + 10));
    assert!(made.parts.iter().all(|q| q.update_order > first + 10));
    assert!(
        made.parts
            .iter()
            .all(|q| q.update_order < made.update_order)
    );
    assert_eq!(made.update_order, projector.update_order);
    assert_eq!(g.world.update_seq(), seq);
}
