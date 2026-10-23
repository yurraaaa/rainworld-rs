//! `WaterFall`: the falls the tile parse makes, its constructor, its update's draws and the
//! push on bodies. Expected values follow `WaterFall.cs` and `Room.LoadFromDataString`.
//!
//! Mirrors: - (port-only: tests)

use rw_assets::{Room, RoomSettings};
use rw_random::UnityRandom;

use super::{FallObject, FallSpec, WaterFall};
use crate::tests::{room, room_with_water};
use crate::{BodyChunk, FloatExt, Vec2, Water};

/// A `w`idth x `h`eight room of air with `cells` (column, row from the top, character).
fn rows(w: usize, h: usize, cells: &[(usize, usize, u8)]) -> Vec<String> {
    let mut grid = vec![vec![b'.'; w]; h];
    for &(x, row, c) in cells {
        grid[row][x] = c;
    }
    grid.into_iter()
        .map(|r| String::from_utf8(r).expect("ascii"))
        .collect()
}

fn specs(cells: &[(usize, usize, u8)]) -> Vec<FallSpec> {
    let rows = rows(12, 10, cells);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    WaterFall::specs(&room(&refs))
}

fn spec(tile: (i32, i32), flow: f32, width: i32) -> FallSpec {
    FallSpec { tile, flow, width }
}

/// A block of waterfall tiles two high and three wide is one fall of flow 1 starting at its
/// top left; a single row of them is one of flow 0.5.
#[test]
fn double_and_single_height_falls() {
    let block: Vec<_> = (2..5).flat_map(|x| [(x, 2, b'w'), (x, 3, b'w')]).collect();
    // Row 2 from the top of 10 is tile y 7.
    assert_eq!(specs(&block), vec![spec((2, 7), 1.0, 3)]);
    let row: Vec<_> = (2..5).map(|x| (x, 2, b'w')).collect();
    assert_eq!(specs(&row), vec![spec((2, 7), 0.5, 3)]);
    assert_eq!(specs(&[]), vec![]);
}

/// A step: the top-left tile starts a single-height fall one tile wide (the tile below its
/// neighbour stops the run), and the neighbour with the tile below it a double one.
#[test]
fn step_makes_two_falls() {
    let step = [(2, 2, b'w'), (3, 2, b'w'), (3, 3, b'w')];
    assert_eq!(
        specs(&step),
        vec![spec((2, 7), 0.5, 1), spec((3, 7), 1.0, 1)]
    );
}

/// Falls are found column by column from the left, so the left fall is made first.
#[test]
fn falls_in_column_order() {
    let cells = [(8, 1, b'w'), (8, 2, b'w'), (3, 4, b'w')];
    assert_eq!(
        specs(&cells),
        vec![spec((3, 5), 0.5, 1), spec((8, 8), 1.0, 1)]
    );
}

/// 10 x 10 tiles with a floor, the fall at tiles (2..5, 7) of flow `flow`.
fn fall_room(water: i32) -> Room {
    let mut cells: Vec<(usize, usize, u8)> = (0..10).map(|x| (x, 9, b'#')).collect();
    cells.extend((2..5).map(|x| (x, 2, b'w')));
    let rows = rows(10, 10, &cells);
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    room_with_water(&refs, water)
}

/// The constructor: `pos = MiddleOfTile - (10, -15)`; the top at `pos.y`, the bottom at
/// `strikeLevel` of a room with no water object yet (-200); the bubbles `Lerp(5, 10, flow)`
/// a tile only in a room with water; the terrain tiles under it a solid tile with air above
/// it, in the columns `x..=x + width`.
#[test]
fn constructor() {
    let dry = fall_room(-1);
    let f = WaterFall::new(6, &dry, spec((2, 7), 0.5, 3), false);
    assert_eq!(f.pos, Vec2::new(40.0, 165.0));
    assert_eq!(
        (f.top_pos, f.bottom_pos),
        ([165.0, 165.0, 0.0], [-200.0, -200.0, 0.0])
    );
    assert_eq!((f.bubbles_per_tile, f.bubbles.len()), (0, 0));
    assert_eq!(f.hit_terrain_tiles, vec![(2, 0), (3, 0), (4, 0), (5, 0)]);
    assert_eq!(
        (f.flow, f.set_flow, f.last_flow, f.original_flow),
        (0.5, 0.5, 0.5, 0.5)
    );
    let wet = fall_room(3);
    let f = WaterFall::new(6, &wet, spec((2, 7), 0.5, 3), true);
    assert_eq!((f.bubbles_per_tile, f.bubbles.len()), (7, 21));
    let f = WaterFall::new(6, &wet, spec((2, 7), 1.0, 3), true);
    assert_eq!((f.bubbles_per_tile, f.bubbles.len()), (10, 30));
    assert_eq!(f.update_order, 6);
}

/// A flow of 0 starts with the top where the bottom is.
#[test]
fn a_dry_fall_starts_with_its_top_at_the_bottom() {
    let dry = fall_room(-1);
    let f = WaterFall::new(1, &dry, spec((2, 7), 0.0, 3), false);
    assert_eq!(f.top_pos, [-200.0, -200.0, 0.0]);
}

/// With no water the fall reaches -200: `strikeLevel` -200, never flooded, never in
/// contact; its bottom is clamped up to `Min(start, strike)` on the first update. Each
/// hit tile draws `value < flow` and, past it, `value < 1/3`; then two for the place and,
/// when it lies between the ends, `DegToVec` and two for the speed, and the drip's life.
#[test]
fn dry_fall_drips_off_the_floor() {
    let dry = fall_room(-1);
    let mut any = false;
    for seed in 0..60 {
        let mut f = WaterFall::new(1, &dry, spec((2, 7), 1.0, 3), false);
        let mut rng = UnityRandom::new(seed);
        let mut want = rng.clone();
        let made = f.update(None, &mut rng);
        let mut drips = 0;
        for &tile in &f.hit_terrain_tiles {
            if want.value() < 1.0 && want.value() < 1.0 / 3.0 {
                let x = (-10.0_f32).lerp(10.0, want.value());
                let y = (-10.0_f32).lerp(10.0, want.value());
                let at = Vec2::middle_of_tile(tile) + Vec2::new(x, y);
                // Top 165, bottom -200: the floor tile's 0..20 is between them.
                let dir = Vec2::from_degrees(360.0 * want.value()) + Vec2::new(0.0, 1.0);
                let speed = want.value() * 7.0 * want.value() * 1.0;
                let life = want.range_i32(10, 120);
                match &made[drips] {
                    FallObject::Drip(d) => {
                        assert_eq!((d.pos, d.vel, d.life_time), (at, dir * speed, life));
                        assert!(d.water_color && d.must_exit);
                    }
                    FallObject::Bubble(_) => panic!("a bubble"),
                }
                drips += 1;
            }
        }
        assert_eq!(made.len(), drips);
        assert_eq!(rng.state(), want.state(), "seed {seed}");
        any |= drips > 0;
        assert_eq!(f.bottom_pos[0], -200.0);
        assert!(!f.flooded(None));
        assert_eq!(f.visual_density, 0.0_f32.lerp(1.0, 0.1));
    }
    assert!(any);
}

fn pool_water(room: &Room) -> Water {
    Water::new(room, &RoomSettings::ANCESTOR).expect("water")
}

/// With water at row 3 (still level 70) the fall's strike level is 60; the first update
/// clamps the bottom up to it (`Min(205, 60)`), which is within 10 of the strike, so the
/// fall is in contact and every bubble (all reset, none born yet) is placed at
/// `strike - 25` above the middle of the row-0 tile, with 6 draws each.
#[test]
fn wet_fall_resets_its_bubbles_and_hits_the_surface() {
    let wet = fall_room(3);
    let mut water = pool_water(&wet);
    let mut f = WaterFall::new(1, &wet, spec((2, 6), 0.5, 3), true);
    assert_eq!(f.pos.y, 6.0 * 20.0 + 25.0);
    assert_eq!(WaterFall::strike_level(Some(&water)), 60.0);
    let mut rng = UnityRandom::new(5);
    f.update(Some(&mut water), &mut rng);
    assert_eq!(f.bottom_pos[0], 60.0);
    assert!(f.water_contact(Some(&water)));
    for b in &f.bubbles {
        assert_eq!(b[3].x, 1.0);
        assert!((5.0..20.0).contains(&b[3].y));
        assert_eq!(b[0].y, b[1].y);
        assert_eq!(b[0].y, 10.0 + 60.0 - 25.0);
    }
}

/// `ResetBubble` draws the place (1), the rising direction and speed (2), the drift
/// direction and speed (2) and the life time (1); a fall not touching the water parks it
/// at (-1000, 1000).
#[test]
fn reset_bubble_draws_six() {
    let wet = fall_room(3);
    let water = pool_water(&wet);
    let mut f = WaterFall::new(1, &wet, spec((2, 6), 0.5, 3), true);
    f.bottom_pos[0] = 60.0;
    let mut rng = UnityRandom::new(9);
    let mut want = rng.clone();
    f.reset_bubble(4, 3, 60.0, &water, &mut rng);
    let x = (-10.0_f32).lerp(10.0, want.value());
    let up = Vec2::from_degrees(160.0 + want.value() * 40.0);
    let up = up * (want.value() * 8.0_f32.lerp(12.0, 0.5));
    let drift = Vec2::from_degrees(want.value() * 360.0);
    let vel = up + drift * (want.value() * 2.0);
    let life = want.range_i32(5, 20);
    assert_eq!(rng.state(), want.state());
    let b = f.bubbles[4];
    assert_eq!(b[0], Vec2::new(70.0 + x, 10.0 + 60.0 - 25.0));
    assert_eq!((b[1], b[2], b[3]), (b[0], vel, Vec2::new(1.0, life as f32)));
    // Out of contact (the bottom still above the strike by more than 10).
    f.bottom_pos[0] = 100.0;
    f.reset_bubble(5, 3, 60.0, &water, &mut rng);
    assert_eq!(
        (f.bubbles[5][0], f.bubbles[5][1]),
        (Vec2::new(-1000.0, 1000.0), Vec2::new(-1000.0, 1000.0))
    );
}

/// Water above the fall's start floods it: no drips are drawn, there is no contact, and
/// every bubble is reset out of sight (6 draws each).
#[test]
fn flooded_fall() {
    let wet = fall_room(8);
    let mut water = pool_water(&wet);
    let mut f = WaterFall::new(1, &wet, spec((2, 6), 0.5, 3), true);
    // Still level 170, strike 160 > start 145.
    assert!(f.flooded(Some(&water)) && !f.water_contact(Some(&water)));
    let mut rng = UnityRandom::new(2);
    let mut want = rng.clone();
    let made = f.update(Some(&mut water), &mut rng);
    assert!(made.is_empty());
    for _ in 0..21 * 6 {
        want.value();
    }
    // `Range` draws once like `value`.
    assert_eq!(rng.state(), want.state());
    assert!(f.bubbles.iter().all(|b| b[0] == Vec2::new(-1000.0, 1000.0)));
}

/// A body in the water under the fall is pushed down in the fall's width and toward its
/// sides by the edges. Chunk of mass 1 at (90, 50) in water of still level 70, the fall
/// at x 80..120 of flow 1: `num = Pow(InverseLerp(-50, 30, 50), 2) * Pow(1, 0.1) * 1 = 1`;
/// `vel.y -= 0.3`; the left edge `InverseLerp(100, 80, 90) = 0.5` times
/// `(InverseLerp(10, 60, 50) - 0.5) * 2 = 0.6`: `vel.x += 0.17 * 0.3`.
#[test]
fn push_on_a_body_in_the_water() {
    let wet = fall_room(3);
    let water = pool_water(&wet);
    let mut f = WaterFall::new(1, &wet, spec((4, 6), 1.0, 2), true);
    assert_eq!((f.float_left(), f.float_right()), (80.0, 120.0));
    f.flow = 1.0;
    let mut c = BodyChunk::new(Vec2::new(90.0, 50.0), 9.0, 1.0);
    f.push_chunks(&water, [&mut c]);
    assert!((c.vel.y + 0.3).abs() < 1e-6, "{}", c.vel.y);
    assert!((c.vel.x - 0.17 * 0.3).abs() < 1e-6, "{}", c.vel.x);
    // Twice the mass, half the push; out of the water's reach, none.
    let mut heavy = BodyChunk::new(Vec2::new(90.0, 50.0), 9.0, 2.0);
    f.push_chunks(&water, [&mut heavy]);
    assert!((heavy.vel.y + 0.15).abs() < 1e-6);
    let mut high = BodyChunk::new(Vec2::new(90.0, 150.0), 9.0, 1.0);
    f.push_chunks(&water, [&mut high]);
    assert_eq!(high.vel, Vec2::ZERO);
}
