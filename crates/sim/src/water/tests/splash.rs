//! `Splash` (`SplashWater/Splash.cs`, `WaterParticle.cs`): expected values from
//! `Reset` and the two `Update`s, draws replayed on a copy of the seeded stream.

use std::sync::Arc;

use rw_assets::{ClipNames, Room, RoomSettings, SoundTable};
use rw_random::UnityRandom;

use super::Splash;
use crate::tests::room_with_water;
use crate::{FloatExt, SoundEvent, SoundId, Sounds, Vec2, Water};

const EPS: f32 = 1e-4;

/// 20 x 10 tiles, water row 6 (still level 130), a solid column at tile x 10 over rows
/// 7 to 9 (x 200 to 220, y 140 up).
fn pool() -> (Room, Water) {
    let open = ".".repeat(20);
    let wall = format!("{}#{}", ".".repeat(10), ".".repeat(9));
    let mut rows = vec![open.as_str(); 10];
    for row in rows.iter_mut().take(3) {
        *row = &wall;
    }
    let room = room_with_water(&rows, 6);
    let water = Water::new(&room, &RoomSettings::ANCESTOR).expect("water");
    (room, water)
}

/// Sounds that know both splash ids (one clip each).
fn splash_sounds() -> Sounds {
    let names = ClipNames::new(["splash".to_owned()], None);
    let text = "Volume: 6\nVolume exponent: 1.8\n\
                Splashing_Water_Into_Terrain : splash\n\
                Splashing_Water_Into_Water_Surface : splash\n";
    Sounds::new(Arc::new(SoundTable::parse(text, &names).unwrap()))
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: expected {expected}, got {actual}"
    );
}

fn played(sounds: &mut Sounds) -> Vec<(SoundId, Vec2, f32)> {
    sounds
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            SoundEvent::Play(p) => match p.source {
                crate::SoundSource::Position(pos) => Some((p.id, pos, p.emitter_volume)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// `Reset`: `pos + initRad * vel.normalized`, `lastPos` the given `pos`, the linger
/// point `30 + initRad` under the water level at `pos`, life 1 for 60 ticks, `rad`
/// `initRad` from a `lastRad` of 0, `makeSoundCounter` 4.
#[test]
fn reset_starts_along_its_velocity_over_its_linger_point() {
    let s = Splash::new(
        Vec2::new(100.0, 150.0),
        Vec2::new(3.0, 4.0),
        8.0,
        5.0,
        130.0,
    );
    assert_eq!(s.pos, Vec2::new(103.0, 154.0));
    assert_eq!(s.last_pos, Vec2::new(100.0, 150.0));
    assert_eq!(s.vel, Vec2::new(3.0, 4.0));
    assert_eq!(s.linger_pos, Vec2::new(100.0, 95.0));
    assert_eq!(s.last_linger_pos, s.linger_pos);
    assert_eq!(s.linger_pos_vel, Vec2::ZERO);
    assert_eq!((s.rad, s.last_rad, s.amount), (5.0, 0.0, 8.0));
    assert_eq!((s.life, s.last_life, s.life_time), (1.0, 1.0, 60.0));
    assert_eq!(s.make_sound_counter, 4);
}

/// In the air: `CosmeticSprite.Update` moves it, `WaterParticle.Update` slows it (0.97),
/// drops it 0.9, swells `rad` by `InverseLerp(1, 20, |vel|)` then 10% toward `amount`;
/// the linger point is held 20 px under the surface and starts falling; `Splash.Update`
/// drops it 0.9 more. No draws.
#[test]
fn air_update_moves_swells_and_falls() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(100.0, 150.0),
        Vec2::new(0.0, 5.0),
        8.0,
        4.0,
        130.0,
    );
    let mut rng = UnityRandom::new(5);
    let state = rng.state();
    let mut sounds = splash_sounds();
    let (destroyed, drip) = s.update(&room, &mut water, &mut rng, &mut sounds);
    assert!(!destroyed);
    assert!(drip.is_none());
    assert_eq!(rng.state(), state);
    assert!(sounds.take_events().is_empty());
    assert_eq!(s.last_pos, Vec2::new(100.0, 154.0));
    assert_eq!(s.pos, Vec2::new(100.0, 159.0));
    let vy = 5.0 * 0.97 - 0.9;
    let rad = 4.0 + vy.inverse_lerp(1.0, 20.0);
    close(s.rad, rad + (8.0 - rad) * 0.1, "rad");
    assert_eq!(s.last_rad, 4.0);
    close(s.vel.y, vy - 0.9, "vel");
    close(s.life, 1.0 - 1.0 / 60.0, "life");
    assert_eq!(s.last_life, 1.0);
    assert_eq!(s.make_sound_counter, 3);
    assert_eq!(s.last_linger_pos, Vec2::new(100.0, 96.0));
    assert_eq!(s.linger_pos, Vec2::new(100.0, 110.0));
    assert_eq!(s.linger_pos_vel, Vec2::new(0.0, -0.9));
}

/// The linger point is kept at most `rad` from the splash, toward where it was.
#[test]
fn linger_point_stays_within_rad() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(100.0, 150.0),
        Vec2::new(0.0, 5.0),
        8.0,
        4.0,
        130.0,
    );
    s.linger_pos = Vec2::new(100.0, 158.0);
    s.linger_pos_vel = Vec2::ZERO;
    s.update(
        &room,
        &mut water,
        &mut UnityRandom::new(1),
        &mut splash_sounds(),
    );
    // pos (100, 159), rad 4.538: the linger point is pulled to 159 - rad below it.
    close(s.linger_pos.y, 159.0 - s.rad, "linger y");
    assert_eq!(s.linger_pos.x, 100.0);
}

/// Under the surface: life -0.06, `WaterfallHitSurface(pos, 10, InverseLerp(-2, -8,
/// vel.y))` over points 15 to 17 (x 80 to 120: `PreviousPoint(95)` to
/// `PreviousPoint(105) + 1`), each a draw against `flow` and on a hit `1.9 *
/// Lerp(-1, 0.5, value) * Lerp(0.6, 1, flow)` on its height (negated at the two end
/// points) and `DegToVec(value * 360) * 5` on its offset; `vel.y = |vel.y| * 0.4`; with
/// `makeSoundCounter` run out and `|vel| > 4`, `Splashing_Water_Into_Water_Surface` at
/// `InverseLerp(4, 14, |vel|)` and the counter set to `int.MaxValue`.
#[test]
fn submerged_upsets_the_surface_and_bounces_with_a_sound() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(100.0, 140.0),
        Vec2::new(0.0, -20.0),
        8.0,
        4.0,
        130.0,
    );
    s.make_sound_counter = 1;
    let mut rng = UnityRandom::new(11);
    let mut copy = rng.clone();
    let mut sounds = splash_sounds();
    s.update(&room, &mut water, &mut rng, &mut sounds);
    // pos (100, 136) + vel -20 -> 116; vel.y -20 * 0.97 - 0.9 = -20.3.
    assert_eq!(s.pos, Vec2::new(100.0, 116.0));
    let flow = (-20.3_f32).inverse_lerp(-2.0, -8.0);
    assert_eq!(flow, 1.0);
    for i in 15..=17 {
        let x = -220.0 + 20.0 * i as f32;
        if copy.value() < flow {
            let edge = if i == 15 || i == 17 { -1.0 } else { 1.0 };
            let kick = 1.9 * (-1.0_f32).lerp(0.5, copy.value()) * edge * 0.6_f32.lerp(1.0, flow);
            close(water.wave_height(i), kick, "height");
            let off = Vec2::from_degrees(copy.value() * 360.0) * 5.0;
            close(water.point(i).x, x + off.x, "point x");
            close(water.point(i).y, 130.0 + off.y, "point y");
        } else {
            assert_eq!(water.wave_height(i), 0.0);
        }
    }
    assert_eq!(water.wave_height(14), 0.0);
    assert_eq!(water.wave_height(18), 0.0);
    close(s.vel.y, 20.3 * 0.4, "bounce");
    close(s.life, 1.0 - 1.0 / 60.0 - 0.06, "life");
    assert_eq!(s.make_sound_counter, i32::MAX);
    let volume = s.vel.length().inverse_lerp(4.0, 14.0);
    let mut expect = splash_sounds();
    expect.play_at(
        SoundId::SplashingWaterIntoWaterSurface,
        s.pos,
        volume,
        1.0,
        &mut copy,
    );
    assert_eq!(rng.state(), copy.state());
    assert_eq!(
        played(&mut sounds),
        [(SoundId::SplashingWaterIntoWaterSurface, s.pos, volume)]
    );
}

/// The sound waits for `makeSoundCounter`: at 4 from `Reset` the first submerged tick
/// plays nothing, only the surface draws.
#[test]
fn no_sound_before_the_counter_runs_out() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(100.0, 140.0),
        Vec2::new(0.0, -20.0),
        8.0,
        4.0,
        130.0,
    );
    let mut sounds = splash_sounds();
    s.update(&room, &mut water, &mut UnityRandom::new(2), &mut sounds);
    assert!(sounds.take_events().is_empty());
    assert_eq!(s.make_sound_counter, 3);
}

/// Entering the solid column from the left: back to where the line crosses the tile's
/// rectangle grown by 2 (x 198), `vel.x = -|vel.x| * 0.4`, `Splashing_Water_Into_Terrain`
/// at `InverseLerp(4, 16, |vel|)` (counter then 10), a water-coloured drip with `vel +
/// RNV * |vel|` (two draws with its life) and life -0.02.
#[test]
fn terrain_hit_bounces_and_sheds_a_drip() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(195.0, 170.0),
        Vec2::new(20.0, 1.0),
        8.0,
        4.0,
        130.0,
    );
    s.pos = Vec2::new(195.0, 170.0);
    s.make_sound_counter = 1;
    let mut rng = UnityRandom::new(7);
    let mut copy = rng.clone();
    let mut sounds = splash_sounds();
    let (destroyed, drip) = s.update(&room, &mut water, &mut rng, &mut sounds);
    assert!(!destroyed);
    // pos (215, 171): the line from (195, 170) meets x 198 at y 170.15.
    close(s.pos.x, 198.0, "x");
    close(s.pos.y, 170.15, "y");
    assert_eq!(s.last_pos, Vec2::new(195.0, 170.0));
    let vel = Vec2::new(-20.0 * 0.97 * 0.4, 0.97 - 0.9);
    close(s.vel.x, vel.x, "vel x");
    let speed = vel.length();
    let volume = speed.inverse_lerp(4.0, 16.0);
    let mut expect = splash_sounds();
    expect.play_at(
        SoundId::SplashingWaterIntoTerrain,
        s.pos,
        volume,
        1.0,
        &mut copy,
    );
    let rnv = Vec2::from_degrees(copy.value() * 360.0);
    let life_time = copy.range_i32(10, 120);
    assert_eq!(rng.state(), copy.state());
    assert_eq!(s.make_sound_counter, 10);
    let drip = drip.expect("a drip");
    assert!(drip.water_color);
    assert_eq!(drip.life_time, life_time);
    assert_eq!(drip.pos, s.pos);
    close(drip.vel.x, vel.x + rnv.x * speed, "drip vel x");
    close(drip.vel.y, vel.y + rnv.y * speed, "drip vel y");
    close(s.life, 1.0 - 1.0 / 60.0 - 0.02, "life");
    // Above the water: `Splash.Update`'s own fall.
    close(s.vel.y, vel.y - 0.9, "vel y");
    let sound = played(&mut sounds);
    assert_eq!(sound.len(), 1);
    assert_eq!(sound[0].0, SoundId::SplashingWaterIntoTerrain);
}

/// `rad` over 1.5 times `amount` costs 0.05 life; it never passes twice `amount`.
#[test]
fn oversized_splash_loses_life_and_is_capped() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(100.0, 150.0),
        Vec2::new(0.0, 5.0),
        2.0,
        4.5,
        130.0,
    );
    s.update(
        &room,
        &mut water,
        &mut UnityRandom::new(1),
        &mut splash_sounds(),
    );
    assert_eq!(s.rad, 4.0);
    close(s.life, 1.0 - 1.0 / 60.0 - 0.05, "life");
}

/// `lastLife < 0` at the head of the update destroys it: `lastLife` takes the
/// negative life one update after `life` went below 0, so the update after that one
/// destroys it, and still runs whole.
#[test]
fn destroyed_once_its_last_life_is_spent() {
    let (room, mut water) = pool();
    let mut s = Splash::new(
        Vec2::new(100.0, 150.0),
        Vec2::new(0.0, 5.0),
        8.0,
        4.0,
        130.0,
    );
    s.life = 0.01;
    let mut rng = UnityRandom::new(1);
    let (destroyed, _) = s.update(&room, &mut water, &mut rng, &mut splash_sounds());
    assert!(!destroyed);
    assert!(s.life < 0.0);
    let (destroyed, _) = s.update(&room, &mut water, &mut rng, &mut splash_sounds());
    assert!(!destroyed);
    assert!(s.last_life < 0.0);
    let y = s.pos.y;
    let vy = s.vel.y;
    let (destroyed, _) = s.update(&room, &mut water, &mut rng, &mut splash_sounds());
    assert!(destroyed);
    assert_eq!(s.pos.y, y + vy);
}
