//! Expected values are derived by hand from `water.md` sections 1-2 and the
//! `Water` / `BodyChunk.Update` formulas quoted in the comments.

use rw_assets::{Room, RoomSettings};
use rw_random::UnityRandom;

mod flood;
mod rumble;

use super::{SurfaceChunk, WaterObject};
use crate::tests::room_with_water;
use crate::{Bubble, FloatExt, Splash};
use std::sync::Arc;

use rw_assets::{ClipNames, SoundTable};

use crate::{
    BodyChunk, EmitterId, PhysicalObject, SoundEvent, SoundId, SoundSource, Sounds, Vec2, Water,
    slugcat,
};

const EPS: f32 = 1e-3;

/// 20 x 10 open tiles, water row 3: still level 70, 43 surface points.
fn pool() -> (Room, Water) {
    let row = ".".repeat(20);
    let room = room_with_water(&[row.as_str(); 10], 3);
    let water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    (room, water)
}

fn chunk_at(y: f32, vel: Vec2) -> PhysicalObject {
    let mut c = BodyChunk::new(Vec2::new(100.0, y), 9.0, slugcat::CHUNK_MASS);
    c.vel = vel;
    PhysicalObject::new(vec![c], slugcat::PARAMS)
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: expected {expected}, got {actual}"
    );
}

#[test]
fn layout_and_flat_level() {
    let (_, w) = pool();
    // (220 + 400 + 220) / 20 + 1 points from x = -220; level 20 * 3 + 10.
    assert_eq!(w.point_count(), 43);
    assert_eq!(w.point(0), Vec2::new(-220.0, 70.0));
    assert_eq!(w.point(42), Vec2::new(620.0, 70.0));
    assert_eq!(w.surface_bounds(), (-220.0, 620.0));
    assert_eq!(w.level_at(-1000.0), 70.0);
    assert_eq!(w.level_at(133.0), 70.0);
}

#[test]
fn submersion_at_several_depths() {
    // InverseLerp(y - 9, y + 9, 70).
    let (_, w) = pool();
    for (y, expected) in [
        (79.0, 0.0),
        (90.0, 0.0),
        (70.0, 0.5),
        (65.5, 0.75),
        (61.0, 1.0),
    ] {
        let c = BodyChunk::new(Vec2::new(100.0, y), 9.0, 0.35);
        close(w.submersion(&c), expected, &format!("y {y}"));
    }
    let c = BodyChunk::new(Vec2::new(100.0, 70.0), 9.0, 0.35);
    assert_eq!(c.submersion(None), 0.0);
}

#[test]
fn buoyancy_and_drag_per_tick() {
    let (room, w) = pool();
    // Fully under, at rest: vy = -0.9 + 0.95 = 0.05 (the net upward acceleration),
    // times water friction 0.96.
    let mut o = chunk_at(40.0, Vec2::ZERO);
    o.update_with(&room, Some(&w), &mut ());
    close(o.chunks[0].vel.y, 0.05 * 0.96, "full");
    close(o.chunks[0].pos.y, 40.0 + 0.048, "moved");
    // Half under: vy = -0.9 + 0.95 * 0.5, times lerp(0.999, 0.96, 0.5) = 0.9795.
    let mut o = chunk_at(70.0, Vec2::ZERO);
    o.update_with(&room, Some(&w), &mut ());
    close(o.chunks[0].vel.y, -0.425 * 0.9795, "half");
    // Bottom just above the surface: air only, -0.9 * 0.999.
    let mut o = chunk_at(79.5, Vec2::ZERO);
    o.update_with(&room, Some(&w), &mut ());
    close(o.chunks[0].vel.y, -0.9 * 0.999, "dry");
}

#[test]
fn fast_chunks_drag_harder_unless_immune() {
    let (room, w) = pool();
    // vy = -20 - 0.9 + 0.95 = -19.95; (1 / (19.95 - 10))^0.5 = 0.317021;
    // immunity 0: lerp(0, 0.96, 0.317021) = 0.304340.
    let mut o = chunk_at(30.0, Vec2::new(0.0, -20.0));
    o.update_with(&room, Some(&w), &mut ());
    close(o.chunks[0].vel.y, -19.95 * 0.304_340, "no immunity");
    // Immunity 0.85: lerp(0.96 * 0.85, 0.96, 0.317021) = 0.861651.
    let mut o = chunk_at(30.0, Vec2::new(0.0, -20.0));
    o.params.water_retardation_immunity = 0.85;
    o.update_with(&room, Some(&w), &mut ());
    close(o.chunks[0].vel.y, -19.95 * 0.861_651, "immune");
}

#[test]
fn skim_bounces_off_the_surface() {
    let (room, w) = pool();
    // Submersion 0.25 (y 74.5): vel (12, -1) -> gravity -1.9 -> vy * -0.5, vx * 0.75.
    let mut o = chunk_at(74.5, Vec2::new(12.0, -1.0));
    o.update_with(&room, Some(&w), &mut ());
    close(o.chunks[0].vel.y, 0.95, "vy");
    close(o.chunks[0].vel.x, 9.0, "vx");
    // Not fast enough sideways (|vx| 9): ordinary buoyancy and drag instead.
    let mut o = chunk_at(74.5, Vec2::new(9.0, -1.0));
    o.update_with(&room, Some(&w), &mut ());
    assert!(o.chunks[0].vel.y < 0.0);
}

#[test]
fn single_disturbance_propagates() {
    let (room, mut w) = pool();
    // No sines or jitter, to follow the wave equation alone.
    w.wave_amplitude = 0.0;
    w.set_wave_height(20, 10.0);
    let mut rng = UnityRandom::new(1);
    w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
    // R = 0.45, R^2 = 0.2025. next[20] = 2 (1 - R^2) 10 * 0.99 = 15.7905,
    // next[19, 21] = R^2 10 * 0.99 = 2.00475; mean term 10 / (43 * 1.5) = 0.155039;
    // interior heights blend 1% toward the mean of their neighbours' next values.
    close(w.wave_height(20), 15.499_154, "centre");
    close(w.wave_height(19), 1.910_167, "left");
    close(w.wave_height(21), 1.910_167, "right");
    close(w.wave_height(22), -0.143_465, "two away");
    close(w.wave_height(30), -0.153_488, "far");
    close(w.wave_height(0), -0.155_039, "end");
    // Positions use the heights before the step: the peak alone (3 * 10 / 3), its
    // neighbours averaged with it (10 / 4).
    close(w.point(20).y, 80.0, "peak level");
    close(w.point(21).y, 72.5, "shoulder level");
    close(w.level_at(w.point(20).x + 10.0), 76.25, "between");
    w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
    // Tick 2: next = 0.99 (R^2 (h19 + h21) + 2 (1 - R^2) h20 - 10), minus the new
    // mean term, blended 1% toward next[21].
    close(w.wave_height(20), 15.044_812, "centre tick 2");
}

/// `ThroughSurfaceSound`: a 0.35 kg chunk going down at 20 px/tick plays the medium
/// splashes, fast at volume inverseLerp(-30, -60, -20) = 0 and slow at
/// sin(inverseLerp(-1, -50, -20)) = sin(19 / 49); going up plays nothing.
#[test]
fn crossing_down_plays_the_splash_sounds_by_mass() {
    let names = ClipNames::new(["splash".to_owned()], None);
    let text = "Volume: 6\nVolume exponent: 1.8\n\
                    Medium_Object_Into_Water_Fast : splash\n\
                    Medium_Object_Into_Water_Slow : splash\n";
    let mut sounds = Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()));
    let (room, mut w) = pool();
    let mut c = BodyChunk::new(Vec2::new(100.0, 65.0), 9.0, 0.35);
    c.last_pos = Vec2::new(100.0, 85.0);
    c.vel = Vec2::new(0.0, -20.0);
    let mut up = BodyChunk::new(Vec2::new(300.0, 75.0), 9.0, 0.35);
    up.last_pos = Vec2::new(300.0, 55.0);
    up.vel = Vec2::new(0.0, 20.0);
    let mut chunks = [c, up];
    w.update(
        &room,
        SurfaceChunk::player(&mut chunks),
        &mut UnityRandom::new(1),
        &mut sounds,
    );
    assert_eq!(chunks[1].splash_stop, 10, "the upward crossing splashed");
    let played: Vec<_> = sounds
        .take_events()
        .into_iter()
        .map(|e| match e {
            SoundEvent::Play(p) => (p.id, p.source, p.emitter_volume),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        played,
        [
            (
                SoundId::MediumObjectIntoWaterFast,
                SoundSource::Chunk {
                    emitter: EmitterId(0),
                    chunk: 0
                },
                0.0
            ),
            (
                SoundId::MediumObjectIntoWaterSlow,
                SoundSource::Chunk {
                    emitter: EmitterId(1),
                    chunk: 0
                },
                (19.0_f32 / 49.0).sin()
            ),
        ]
    );
}

#[test]
fn entering_chunk_splashes_once() {
    let (room, mut w) = pool();
    w.wave_amplitude = 0.0;
    // Crossing down at -20: force = lerp(-20 * 9 * lerp(0.35, 1, 0.3) / 3, 10, 0.5)
    // = -11.35; the closest point (x 100 -> index 16) gets half, neighbours a sixth.
    let mut c = BodyChunk::new(Vec2::new(100.0, 65.0), 9.0, 0.35);
    c.last_pos = Vec2::new(100.0, 85.0);
    c.vel = Vec2::new(0.0, -20.0);
    let mut chunks = [c];
    w.update(
        &room,
        SurfaceChunk::player(&mut chunks),
        &mut UnityRandom::new(1),
        &mut Sounds::default(),
    );
    assert_eq!(chunks[0].splash_stop, 10);
    // Point 16 had height -5.675 when the surface stepped; its neighbours'
    // -1.891667 differ from it less than the next ones' 0, so it is not smoothed.
    close(w.point(16).y, 70.0 - 5.675, "splash point");
    close(
        w.point(15).y,
        70.0 + (-1.891_667 * 3.0 - 5.675) / 4.0,
        "beside",
    );
    // splash_stop blocks a second splash (the chunk crossing again, fully under).
    chunks[0].pos.y = 50.0;
    let before = w.wave_height(16);
    w.disturb(
        &mut SurfaceChunk::player(&mut chunks).next().expect("chunk"),
        &mut 0,
        &mut UnityRandom::new(1),
        &mut Sounds::default(),
        &mut Vec::new(),
    );
    assert_eq!(w.wave_height(16), before);
}

fn bubble(o: &WaterObject) -> &Bubble {
    match o {
        WaterObject::Bubble(b) => b,
        WaterObject::Splash(s) => panic!("expected a bubble, got {s:?}"),
    }
}

fn splash(o: &WaterObject) -> &Splash {
    match o {
        WaterObject::Splash(s) => s,
        WaterObject::Bubble(b) => panic!("expected a splash, got {b:?}"),
    }
}

/// `rng` after the surface step's draws for each of `w`'s points: the jitter, and in a room
/// with rain danger (the `ANCESTOR` settings' `Rain`) the rumble's three.
fn after_surface(rng: &mut UnityRandom, w: &Water) {
    let per_point = if w.rumble.active { 4 } else { 1 };
    for _ in 0..per_point * w.point_count() {
        rng.value();
    }
}

/// `Water.Update` with nothing in the water: `Range(0, TileWidth)`, `Range(0,
/// defaultWaterLevel)`, then the surface's jitter draws. All-air rows below the
/// surface (row -1 reads row 0): no bottom bubble.
#[test]
fn update_rolls_two_draws_for_a_bottom_bubble() {
    let (room, mut w) = pool();
    let mut rng = UnityRandom::new(4);
    let mut copy = rng.clone();
    let made = w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
    assert!(made.is_empty());
    copy.range_i32(0, 20);
    copy.range_i32(0, 3);
    after_surface(&mut copy, &w);
    assert_eq!(rng.state(), copy.state());
}

/// Row 0 solid, row 1 air, water row 2: a roll on row 1 makes a growing bubble at the
/// tile's middle `+ (Lerp(-10, 10, value), -10)` (three more draws: x and the
/// constructor's two); on row 0 nothing.
#[test]
fn bottom_bubble_on_air_over_solid() {
    let open = ".".repeat(20);
    let floor = "#".repeat(20);
    let mut rows = vec![open.as_str(); 6];
    rows.push(&floor);
    let room = room_with_water(&rows, 2);
    let mut w = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    let mut seen = [false; 2];
    for seed in 0..20 {
        let mut rng = UnityRandom::new(seed);
        let mut copy = rng.clone();
        let made = w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
        let x = copy.range_i32(0, 20);
        let y = copy.range_i32(0, 2);
        seen[y as usize] = true;
        if y == 1 {
            assert_eq!(made.len(), 1, "seed {seed}");
            let b = bubble(&made[0]);
            let dx = (-10.0_f32).lerp(10.0, copy.value());
            assert_eq!(b.pos, Vec2::middle_of_tile((x, 1)) + Vec2::new(dx, -10.0));
            assert_eq!(b.mode, crate::BubbleMode::Growing);
            assert_eq!(b.vel, Vec2::ZERO);
            copy.value();
            copy.value();
        } else {
            assert!(made.is_empty(), "seed {seed}");
        }
        after_surface(&mut copy, &w);
        assert_eq!(rng.state(), copy.state(), "seed {seed}");
    }
    assert_eq!(seen, [true, true]);
}

/// A 0.35 mass, radius 9 chunk crossing down at -20: force -11.35 (see
/// `entering_chunk_splashes_once`). The big splash (no draws) at `(x, level - 20)`
/// with `vel (vel.x / 2, LerpMap(11.35, 10, 40, 4, 14))`, `amount LerpMap(20 * 9 *
/// 0.35, 4, 300, 7, 90)`, `initRad 9`; two small splashes (`(int)|force * 0.25|`) at
/// `(x + Lerp(-9, 9, value), level + 5)` with `vel DegToVec(-45 + 90 value) * value *
/// (20 * 0.3 + 10)`, `amount LerpMap(63, 7, 200, 2, 10)`; then a bubble emitter of `amount =
/// Clamp((1 + 20) * Lerp(11.35, 5, 0.75) * 0.2, 3, 25) = 25` and `(int)|force * 0.75|
/// = 8` bubbles at `level - 5` (five draws each). The emitter updates the same tick:
/// `amount` 24.6, a draw `value * 40 < 24.6` and on success a bubble in the chunk.
#[test]
fn crossing_down_makes_bubbles_and_an_emitter() {
    let (room, mut w) = pool();
    let mut c = BodyChunk::new(Vec2::new(100.0, 65.0), 9.0, 0.35);
    c.last_pos = Vec2::new(100.0, 85.0);
    c.vel = Vec2::new(0.0, -20.0);
    let mut chunks = [c.clone()];
    let mut rng = UnityRandom::new(9);
    let mut copy = rng.clone();
    let made = w.update(
        &room,
        SurfaceChunk::player(&mut chunks),
        &mut rng,
        &mut Sounds::default(),
    );
    copy.range_i32(0, 20);
    copy.range_i32(0, 3);
    let big = splash(&made[0]);
    assert_eq!(big.last_pos, Vec2::new(100.0, 50.0));
    close(big.vel.y, 4.0 + 10.0 * 1.35 / 30.0, "big speed");
    assert_eq!(big.vel.x, 0.0);
    close(big.amount, 7.0 + 83.0 * 59.0 / 296.0, "big amount");
    assert_eq!((big.rad, big.life_time), (9.0, 60.0));
    assert_eq!(big.pos, Vec2::new(100.0, 59.0));
    assert_eq!(big.linger_pos, Vec2::new(100.0, 70.0 - 30.0 - 9.0));
    for s in &made[1..3] {
        let s = splash(s);
        let x = 100.0 + (-9.0_f32).lerp(9.0, copy.value());
        let dir = Vec2::from_degrees(-45.0 + 90.0 * copy.value());
        let speed = copy.value() * (20.0 * 0.3 + 10.0);
        assert_eq!(s.last_pos, Vec2::new(x, 75.0));
        close(s.vel.x, (dir * speed).x, "small vel x");
        close(s.vel.y, (dir * speed).y, "small vel y");
        close(s.amount, 2.0 + 8.0 * 56.0 / 193.0, "small amount");
    }
    let force: f32 = -11.35;
    for b in made[3..11].iter().map(bubble) {
        let x = 100.0 + (-9.0_f32).lerp(9.0, copy.value());
        let dir = Vec2::from_degrees(135.0 + 90.0 * copy.value());
        let speed = copy.value() * ((force * 1.5).abs() + 10.0);
        copy.value();
        copy.value();
        close(b.pos.x, x, "bubble x");
        assert_eq!(b.pos.y, 65.0);
        let v = dir * speed;
        close(b.vel.x, v.x, "vel x");
        close(b.vel.y, v.y, "vel y");
        assert_eq!(b.mode, crate::BubbleMode::Free);
    }
    assert_eq!(w.bubble_emitters().len(), 1);
    let e = &w.bubble_emitters()[0];
    assert_eq!(e.owner, super::ChunkOwner::Player);
    close(e.amount, 24.6, "amount");
    if copy.value() * 40.0 < 24.6 {
        assert_eq!(made.len(), 12);
        let dir = Vec2::from_degrees(copy.value() * 360.0);
        let pos = c.pos + dir * (copy.value() * 9.0);
        assert_eq!(bubble(&made[11]).pos, pos);
        assert_eq!(bubble(&made[11]).vel, c.vel);
        copy.value();
        copy.value();
    } else {
        assert_eq!(made.len(), 11);
    }
    after_surface(&mut copy, &w);
    assert_eq!(rng.state(), copy.state());
}

/// The first crossing chunk counts 1 + 2 + 8 = 11 splashes and bubbles: a second
/// one in the same tick makes only its big splash, no small splashes, emitter or
/// bubbles.
#[test]
fn ten_splashes_stop_later_chunks() {
    let (room, mut w) = pool();
    let mut c = BodyChunk::new(Vec2::new(100.0, 65.0), 9.0, 0.35);
    c.last_pos = Vec2::new(100.0, 85.0);
    c.vel = Vec2::new(0.0, -20.0);
    let mut other = c.clone();
    other.pos.x = 300.0;
    other.last_pos.x = 300.0;
    let mut chunks = [c, other];
    let made = w.update(
        &room,
        SurfaceChunk::player(&mut chunks),
        &mut UnityRandom::new(9),
        &mut Sounds::default(),
    );
    assert_eq!(w.bubble_emitters().len(), 1);
    let far: Vec<&WaterObject> = made
        .iter()
        .filter(|o| match o {
            WaterObject::Bubble(b) => b.pos.x > 200.0,
            WaterObject::Splash(s) => s.last_pos.x > 200.0,
        })
        .collect();
    assert_eq!(far.len(), 1);
    assert_eq!(splash(far[0]).last_pos, Vec2::new(300.0, 50.0));
    assert_eq!(
        chunks[1].splash_stop, 10,
        "the surface splash still happens"
    );
}

/// `WaterfallHitSurface((150, 70), 40, 0.5)`: points `PreviousPoint(130)` = 17 to
/// `PreviousPoint(170) + 1` = 20; each draws `value < 0.5`, a hit two more (height
/// `1.9 * Lerp(-1, 0.5, value) * Lerp(0.6, 1, 0.5)`, negated at 17 and 20; offset
/// `DegToVec(value * 360) * 5`). Flow 0 still draws once per point and changes nothing.
#[test]
fn waterfall_hit_surface_kicks_points_by_chance() {
    let (_, mut w) = pool();
    let mut rng = UnityRandom::new(21);
    let mut copy = rng.clone();
    w.waterfall_hit_surface(Vec2::new(150.0, 70.0), 40.0, 0.5, &mut rng);
    let mut hits = 0;
    for i in 17..=20 {
        if copy.value() < 0.5 {
            hits += 1;
            let edge = if i == 17 || i == 20 { -1.0 } else { 1.0 };
            let kick = 1.9 * (-1.0_f32).lerp(0.5, copy.value()) * edge * 0.8;
            close(w.wave_height(i), kick, "height");
            let off = Vec2::from_degrees(copy.value() * 360.0) * 5.0;
            close(w.point(i).y, 70.0 + off.y, "offset");
        } else {
            assert_eq!(w.wave_height(i), 0.0);
        }
    }
    assert!(hits > 0);
    assert_eq!(rng.state(), copy.state());
    assert_eq!((w.wave_height(16), w.wave_height(21)), (0.0, 0.0));
    let (_, mut w) = pool();
    let mut rng = UnityRandom::new(21);
    w.waterfall_hit_surface(Vec2::new(150.0, 70.0), 40.0, 0.0, &mut rng);
    let mut copy = UnityRandom::new(21);
    for _ in 0..4 {
        copy.value();
    }
    assert_eq!(rng.state(), copy.state());
    assert!((0..w.point_count()).all(|i| w.wave_height(i) == 0.0));
}

/// An emitter loses 0.4 a tick and is dropped once at or below 0.
#[test]
fn emitter_runs_out() {
    let (room, mut w) = pool();
    w.bubble_emitters.push(super::BubbleEmitter {
        owner: super::ChunkOwner::Player,
        index: 0,
        pos: Vec2::new(100.0, 40.0),
        vel: Vec2::ZERO,
        rad: 9.0,
        amount: 0.5,
        decrease: 0.4,
    });
    let mut rng = UnityRandom::new(2);
    w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
    close(w.bubble_emitters()[0].amount, 0.1, "after one");
    w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
    close(w.bubble_emitters()[0].amount, -0.3, "after two");
    w.update(&room, std::iter::empty(), &mut rng, &mut Sounds::default());
    assert!(w.bubble_emitters().is_empty());
}
