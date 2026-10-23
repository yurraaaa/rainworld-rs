//! Unit tests. Every expected value is derived by hand from the formulas in
//! `rainworld-re/notes/physics-core.md` / `slope-collision.md`; the derivation is in
//! the comment next to each assertion.

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::{
    BodyChunk, BodyChunkConnection, ConnectionKind, Contact, PhysicalObject, PhysicsParams,
    TerrainImpact, Vec2, determine_safe_spawn_tile, place_in_room, slugcat,
};

const EPS: f32 = 1e-4;

/// Builds a room from ASCII rows, top row first.
/// `.` Air, `#` Solid, `/` Slope, `-` Floor, `=` Air with a horizontal beam, `|` Air
/// with a vertical pole, `+` Air with both,
/// `E` shortcut entrance, `x` Solid shortcut path, `X` Solid room exit, `D` Solid den,
/// `T` Solid region transport.
pub(crate) fn room(rows: &[&str]) -> Room {
    room_with_water(rows, -1)
}

/// [`room`] with the file's water level (tile row; -1 for none).
pub(crate) fn room_with_water(rows: &[&str], water_level: i32) -> Room {
    room_file(rows, water_level, "")
}

/// [`room`] with line 5's items (`type,x,y|...`).
pub(crate) fn room_with_items(rows: &[&str], items: &str) -> Room {
    room_file(rows, -1, items)
}

fn room_file(rows: &[&str], water_level: i32, items: &str) -> Room {
    let h = rows.len();
    let w = rows[0].len();
    let mut grid = String::new();
    for x in 0..w {
        for row in rows {
            let cell = match row.as_bytes()[x] {
                b'.' => "0",
                b'w' => "0,8",
                b'#' => "1",
                b'/' => "2",
                b'-' => "3",
                b'=' => "0,2",
                b'|' => "0,1",
                b'+' => "0,1,2",
                b'E' => "4,3",
                b'x' => "1,3",
                b'X' => "1,3,4",
                b'D' => "1,3,5",
                b'T' => "1,3,12",
                other => panic!("unknown cell {}", other as char),
            };
            grid.push_str(cell);
            grid.push('|');
        }
    }
    let text = format!(
        "Test\n{w}*{h}|{water_level}|0\n1*1|0|0\n0,0\nBorder: Solid\n{items}\n\n\n\n17|0|0|\nblob\n{grid}\n"
    );
    Room::parse(&text).expect("test room parses")
}

/// An all-Air room, `w` x `h` tiles.
fn open_room(w: usize, h: usize) -> Room {
    let row = ".".repeat(w);
    room(&vec![row.as_str(); h])
}

/// No gravity, no drag: one tick moves exactly by `vel`.
fn frictionless(bounce: f32, surface_friction: f32) -> PhysicsParams {
    PhysicsParams {
        gravity: 0.0,
        air_friction: 1.0,
        bounce,
        surface_friction,
        impact_threshold: 1.0,
        ..slugcat::PARAMS
    }
}

fn single(pos: (f32, f32), vel: (f32, f32), rad: f32, params: PhysicsParams) -> PhysicalObject {
    let mut c = BodyChunk::new(Vec2::new(pos.0, pos.1), rad, 1.0);
    c.vel = Vec2::new(vel.0, vel.1);
    PhysicalObject::new(vec![c], params)
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < EPS,
        "expected {expected}, got {actual}"
    );
}

fn assert_vec(actual: Vec2, x: f32, y: f32) {
    assert_close(actual.x, x);
    assert_close(actual.y, y);
}

// ---- integration step -------------------------------------------------------------

#[test]
fn free_fall_applies_gravity_then_air_friction() {
    let r = open_room(5, 40);
    let mut obj = single((50.0, 500.0), (0.0, 0.0), 9.0, slugcat::PARAMS);
    obj.update(&r);
    // vel = (0 - 0.9) * 0.999 = -0.8991; pos = 500 - 0.8991
    assert_vec(obj.chunks[0].vel, 0.0, -0.8991);
    assert_vec(obj.chunks[0].pos, 50.0, 499.1009);
    obj.update(&r);
    // vel = (-0.8991 - 0.9) * 0.999 = -1.797_300_9; pos = 499.1009 - 1.7973009
    assert_vec(obj.chunks[0].vel, 0.0, -1.797_300_9);
    assert_vec(obj.chunks[0].pos, 50.0, 497.303_6);
    assert_vec(obj.chunks[0].last_pos, 50.0, 499.1009);
    assert_vec(obj.chunks[0].last_last_pos, 50.0, 500.0);
    assert_eq!(obj.chunks[0].contact().y, 0);
}

// ---- flat tiles ---------------------------------------------------------------------

#[test]
fn chunk_settles_on_flat_ground_at_tile_top_plus_radius() {
    // Solid rows y=0,1: ground top at 40.
    let r = room(&[
        ".....", ".....", ".....", ".....", ".....", "#####", "#####",
    ]);
    let mut obj = single((50.0, 120.0), (0.0, 0.0), 9.0, slugcat::PARAMS);
    for _ in 0..100 {
        obj.update(&r);
    }
    let c = &obj.chunks[0];
    // Snap: (row + 1) * 20 + rad = 2 * 20 + 9 = 49. At rest the rebound 0.8991 * 0.1
    // is below gravity, so vel.y is zeroed every tick.
    assert_eq!(c.pos.y, 49.0);
    assert_eq!(c.vel.y, 0.0);
    assert_eq!(c.pos.x, 50.0);
    assert_eq!(c.contact().y, -1);
    assert_eq!(c.last_contact().y, -1);
}

const FLOOR_ROOM: [&str; 7] = [
    ".......", ".......", ".......", ".......", ".......", "#######", "#######",
];

#[test]
fn floor_bounce_above_threshold_keeps_speed() {
    let r = room(&FLOOR_ROOM);
    // bounce 0.5 => threshold 1 + 9 * 0.5 = 5.5; friction factor clamp(2 * 0.3) = 0.6.
    let mut obj = single((50.0, 55.0), (3.0, -20.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    // pos.y 35 is below the ground top 40 + 10 => snapped to 50; vel.y = 20 * 0.5 = 10.
    assert_vec(c.pos, 53.0, 50.0);
    assert_vec(c.vel, 3.0 * 0.6, 10.0);
    assert_eq!(c.contact().y, -1);
}

#[test]
fn floor_bounce_below_threshold_is_cancelled() {
    let r = room(&FLOOR_ROOM);
    let mut obj = single((50.0, 55.0), (0.0, -10.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    // 10 * 0.5 = 5 < 5.5 => 0.
    assert_vec(obj.chunks[0].pos, 50.0, 50.0);
    assert_vec(obj.chunks[0].vel, 0.0, 0.0);
}

#[test]
fn floor_bounce_below_gravity_is_cancelled() {
    let r = room(&FLOOR_ROOM);
    // bounce 1 => threshold 1; gravity 12 makes the rebound (< 12) count as resting.
    let params = PhysicsParams {
        gravity: 12.0,
        air_friction: 1.0,
        bounce: 1.0,
        surface_friction: 0.5,
        impact_threshold: 1.0,
        ..slugcat::PARAMS
    };
    let mut obj = single((50.0, 55.0), (0.0, 0.0), 10.0, params);
    obj.update(&r);
    // vel.y = -12 -> pos 43 < 50 -> snapped; rebound 12 * 1 = 12 is not < 12 => kept.
    assert_vec(obj.chunks[0].vel, 0.0, 12.0);
    let mut obj = single((50.0, 55.0), (0.0, 1.0), 10.0, params);
    obj.update(&r);
    // vel.y = 1 - 12 = -11 -> pos 44 -> rebound 11 < gravity 12 => 0.
    assert_vec(obj.chunks[0].vel, 0.0, 0.0);
    assert_vec(obj.chunks[0].pos, 50.0, 50.0);
}

const CEILING_ROOM: [&str; 10] = [
    "#######", "#######", ".......", ".......", ".......", ".......", ".......", ".......",
    ".......", ".......",
];

#[test]
fn ceiling_bounce_above_and_below_threshold() {
    let r = room(&CEILING_ROOM);
    // Ceiling rows y=8,9: underside at 160. Up sweep snaps to 160 - 10 = 150.
    let mut obj = single((50.0, 145.0), (2.0, 20.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    assert_vec(c.pos, 52.0, 150.0);
    // vel.y = -20 * 0.5 = -10 (|10| >= 5.5 kept); vel.x = 2 * 0.6.
    assert_vec(c.vel, 1.2, -10.0);
    assert_eq!(c.contact().y, 1);

    let mut obj = single((50.0, 145.0), (0.0, 10.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    // -10 * 0.5 = -5, |5| < 5.5 => 0.
    assert_vec(obj.chunks[0].vel, 0.0, 0.0);
}

const WALL_ROOM: [&str; 10] = [
    ".....#.", ".....#.", ".....#.", ".....#.", ".....#.", ".....#.", ".....#.", ".....#.",
    ".....#.", ".....#.",
];

#[test]
fn wall_hit_from_left_snaps_bounces_and_rubs_vertical_speed() {
    let r = room(&WALL_ROOM);
    // Solid column x=5 (100..120). From x=85 moving +20: snap to 100 - 10 = 90.
    let mut obj = single((85.0, 100.0), (20.0, 4.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    assert_vec(c.pos, 90.0, 104.0);
    // vel.x = -20 * 0.5 = -10; vel.y = 4 * clamp(0.3 * 2) = 2.4.
    assert_vec(c.vel, -10.0, 2.4);
    assert_eq!(c.contact().x, 1);
}

#[test]
fn wall_hit_from_right_below_threshold_stops() {
    let r = room(&WALL_ROOM);
    // From x=135 moving -10: snap to (5 + 1) * 20 + 10 = 130; 10 * 0.5 = 5 < 5.5 => 0.
    let mut obj = single((135.0, 100.0), (-10.0, 0.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    assert_vec(c.pos, 130.0, 100.0);
    assert_vec(c.vel, 0.0, 0.0);
    assert_eq!(c.contact().x, -1);
}

// ---- terrain impacts (falls.md section 1) -------------------------------------------

const FLOOR: Contact = Contact { x: 0, y: -1 };
const CEILING: Contact = Contact { x: 0, y: 1 };
const WALL_RIGHT: Contact = Contact { x: 1, y: 0 };
const WALL_LEFT: Contact = Contact { x: -1, y: 0 };

fn impact(chunk: usize, direction: Contact, speed: f32, first_contact: bool) -> TerrainImpact {
    TerrainImpact {
        chunk,
        direction,
        speed,
        first_contact,
    }
}

#[test]
fn floor_impact_reports_speed_before_bounce() {
    let r = room(&FLOOR_ROOM);
    // Gravity and drag act first: vel.y = (-19 - 0.9) * 0.999 = -19.8801; the bounce
    // (x0.1) comes after the report.
    let mut obj = single((50.0, 55.0), (0.0, -19.0), 10.0, slugcat::PARAMS);
    obj.update(&r);
    let [hit] = obj.impacts() else {
        panic!("one impact: {:?}", obj.impacts());
    };
    assert_eq!(
        (hit.chunk, hit.direction, hit.first_contact),
        (0, FLOOR, true)
    );
    assert_close(hit.speed, 19.8801);
}

#[test]
fn impact_fires_only_above_threshold() {
    let r = room(&FLOOR_ROOM);
    // Threshold 1: exactly 1 is not reported, 1.5 is.
    let mut obj = single((50.0, 50.5), (0.0, -1.0), 10.0, frictionless(0.1, 0.5));
    obj.update(&r);
    assert_eq!(obj.chunks[0].contact().y, -1);
    assert!(obj.impacts().is_empty());
    let mut obj = single((50.0, 51.0), (0.0, -1.5), 10.0, frictionless(0.1, 0.5));
    obj.update(&r);
    assert_eq!(obj.impacts(), &[impact(0, FLOOR, 1.5, true)]);
}

#[test]
fn impact_on_resting_chunk_is_not_first_contact() {
    let r = room(&FLOOR_ROOM);
    let mut obj = single((50.0, 55.0), (0.0, -20.0), 10.0, frictionless(0.1, 0.5));
    obj.update(&r);
    // Rebound 20 * 0.1 = 2 < 1 + 9 * 0.9 => at rest on the floor, contact -1.
    assert_eq!(obj.impacts(), &[impact(0, FLOOR, 20.0, true)]);
    obj.chunks[0].vel.y = -5.0;
    obj.update(&r);
    // lastContactPoint.y was already -1.
    assert_eq!(obj.impacts(), &[impact(0, FLOOR, 5.0, false)]);
    obj.update(&r);
    // vel 0: nothing to report.
    assert!(obj.impacts().is_empty());
}

#[test]
fn ceiling_and_wall_impacts_carry_their_axis() {
    let mut obj = single((50.0, 145.0), (2.0, 20.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&room(&CEILING_ROOM));
    assert_eq!(obj.impacts(), &[impact(0, CEILING, 20.0, true)]);

    let r = room(&WALL_ROOM);
    let mut obj = single((85.0, 100.0), (20.0, 4.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    assert_eq!(obj.impacts(), &[impact(0, WALL_RIGHT, 20.0, true)]);
    let mut obj = single((135.0, 100.0), (-10.0, 0.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    assert_eq!(obj.impacts(), &[impact(0, WALL_LEFT, 10.0, true)]);
}

#[test]
fn corner_hit_reports_floor_then_wall() {
    // Wall column x = 5 standing on the floor rows y = 0, 1.
    let r = room(&[
        ".....#.", ".....#.", ".....#.", ".....#.", ".....#.", "#######", "#######",
    ]);
    let mut obj = single((85.0, 55.0), (10.0, -10.0), 10.0, frictionless(0.5, 0.3));
    obj.update(&r);
    // Floor first (vertical sweep): (4, 1) under (4, 2) Air. The floor hit's friction
    // leaves vel.x = 10 * clamp(2 * 0.3) = 6 for the wall sweep: (5, 2) from x 4.
    assert_eq!(
        obj.impacts(),
        &[
            impact(0, FLOOR, 10.0, true),
            impact(0, WALL_RIGHT, 6.0, true)
        ]
    );
}

#[test]
fn slope_landing_never_reports() {
    // Slope (2, 1): UpLeft (right and below Solid).
    let r = room(&[
        ".......", ".......", ".......", ".......", "../#...", "#######",
    ]);
    let mut obj = single((45.0, 50.0), (0.0, -12.0), 9.0, frictionless(0.1, 0.3));
    obj.update(&r);
    // pos (45, 38): no flat floor under columns 1..2 (the slope is not Solid); the
    // slope lifts it to 5 + 20 + 2 * 9 = 43 with contact -1.
    let c = &obj.chunks[0];
    assert_eq!(c.contact().y, -1);
    assert_eq!(c.on_slope(), -1);
    assert_close(c.pos.y, 43.0);
    assert!(obj.impacts().is_empty());
}

#[test]
fn impacts_follow_chunk_order_and_reset_each_update() {
    let r = room(&FLOOR_ROOM);
    let mut obj = PhysicalObject::new(
        vec![
            BodyChunk::new(Vec2::new(50.0, 55.0), 10.0, 1.0),
            BodyChunk::new(Vec2::new(90.0, 55.0), 10.0, 1.0),
        ],
        frictionless(0.1, 0.5),
    );
    obj.chunks[0].vel.y = -10.0;
    obj.chunks[1].vel.y = -12.0;
    obj.update(&r);
    assert_eq!(
        obj.impacts(),
        &[impact(0, FLOOR, 10.0, true), impact(1, FLOOR, 12.0, true)]
    );
    obj.update(&r);
    assert!(obj.impacts().is_empty());
}

// ---- one-way floors ---------------------------------------------------------------

/// Floor tiles on row y=3 (top at 80), ground rows y=0 (top at 20).
const PLATFORM_ROOM: [&str; 10] = [
    ".......", ".......", ".......", ".......", ".......", ".......", "-------", ".......",
    ".......", "#######",
];

#[test]
fn floor_tile_holds_a_body_landing_from_above() {
    let r = room(&PLATFORM_ROOM);
    let mut obj = single((70.0, 150.0), (0.0, 0.0), 10.0, slugcat::PARAMS);
    for _ in 0..100 {
        obj.update(&r);
    }
    // Rests on the platform top: (3 + 1) * 20 + 10 = 90.
    assert_eq!(obj.chunks[0].pos.y, 90.0);
    assert_eq!(obj.chunks[0].contact().y, -1);
}

#[test]
fn floor_tile_does_not_block_upward_motion() {
    let r = room(&PLATFORM_ROOM);
    let mut obj = single((70.0, 50.0), (0.0, 15.0), 10.0, frictionless(0.1, 0.5));
    for _ in 0..4 {
        obj.update(&r);
    }
    // 50 + 4 * 15: the upward sweep only tests Solid tiles.
    assert_vec(obj.chunks[0].pos, 70.0, 110.0);
    assert_eq!(obj.chunks[0].contact().y, 0);
}

#[test]
fn go_through_floors_drops_through_platform() {
    let r = room(&PLATFORM_ROOM);
    let mut obj = single((70.0, 150.0), (0.0, 0.0), 10.0, slugcat::PARAMS);
    obj.set_go_through_floors(true);
    for _ in 0..100 {
        obj.update(&r);
    }
    // Lands on the ground instead: (0 + 1) * 20 + 10 = 30.
    assert_eq!(obj.chunks[0].pos.y, 30.0);
}

#[test]
fn floor_is_passable_while_any_chunk_was_below_it() {
    let r = room(&PLATFORM_ROOM);
    // Chunk 1 sits under the platform (last_pos.y < 80 => its tile row <= 3), so the
    // platform is not solid for chunk 0 either. No connection between them.
    let above = BodyChunk::new(Vec2::new(70.0, 150.0), 10.0, 1.0);
    let below = BodyChunk::new(Vec2::new(30.0, 30.0), 10.0, 1.0);
    let mut obj = PhysicalObject::new(vec![above, below], slugcat::PARAMS);
    for _ in 0..100 {
        obj.update(&r);
    }
    assert_eq!(obj.chunks[0].pos.y, 30.0);
    assert_eq!(obj.chunks[1].pos.y, 30.0);
}

// ---- slopes -------------------------------------------------------------------------

#[test]
fn up_left_slope_lifts_to_surface_plus_two_radii() {
    // Slope (2,1): right (3,1) and below Solid => UpLeft, h = lx.
    let r = room(&[
        ".......", ".......", ".......", ".......", "../#...", "#######",
    ]);
    let mut obj = single((45.0, 36.0), (2.0, -1.0), 9.0, frictionless(0.1, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    // pos (47, 35) inside the slope tile; lx = 47 - 40 = 7; s = 7 + 20 = 27;
    // pos.y = 27 + 2 * 9 = 45.
    assert_vec(c.pos, 47.0, 45.0);
    // vel.x = 2 * (1 - 0.3) + |-1| * clamp(0.5 - 0.3) * (-1) * 0.2 = 1.4 - 0.04.
    assert_vec(c.vel, 1.36, 0.0);
    assert_eq!(c.on_slope(), -1);
    assert_eq!(c.contact().y, -1);
}

#[test]
fn up_right_slope_lifts_to_surface_plus_two_radii() {
    // Slope (4,1): left (3,1) and below Solid => UpRight, h = 20 - lx.
    let r = room(&[
        ".......", ".......", ".......", ".......", "...#/..", "#######",
    ]);
    let mut obj = single((93.0, 36.0), (-2.0, -1.0), 9.0, frictionless(0.1, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    // pos (91, 35); lx = 91 - 80 = 11; s = 20 - 11 + 20 = 29; pos.y = 29 + 18 = 47.
    assert_vec(c.pos, 91.0, 47.0);
    // vel.x = -2 * 0.7 + 1 * 0.2 * (+1) * 0.2 = -1.36.
    assert_vec(c.vel, -1.36, 0.0);
    assert_eq!(c.on_slope(), 1);
    assert_eq!(c.contact().y, -1);
}

#[test]
fn down_left_slope_pushes_below_surface_minus_two_radii() {
    // Slope (2,4): right (3,4) and above Solid => DownLeft, h = 20 - lx.
    let r = room(&[
        "#######", "../#...", ".......", ".......", ".......", ".......",
    ]);
    let mut obj = single((45.0, 84.0), (2.0, 1.0), 9.0, frictionless(0.1, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    // pos (47, 85); lx = 7; s = 20 - 7 + 80 = 93; pos.y = 93 - 18 = 75.
    assert_vec(c.pos, 47.0, 75.0);
    // vel.y = 0; vel.x = 2 * (1 - 0.3).
    assert_vec(c.vel, 1.4, 0.0);
    assert_eq!(c.on_slope(), 0);
    assert_eq!(c.contact().y, 1);
}

#[test]
fn down_right_slope_pushes_below_surface_minus_two_radii() {
    // Slope (4,4): left (3,4) and above Solid => DownRight, h = lx.
    let r = room(&[
        "#######", "...#/..", ".......", ".......", ".......", ".......",
    ]);
    let mut obj = single((93.0, 84.0), (-2.0, 1.0), 9.0, frictionless(0.1, 0.3));
    obj.update(&r);
    let c = &obj.chunks[0];
    // pos (91, 85); lx = 11; s = 11 + 80 = 91; pos.y = 91 - 18 = 73.
    assert_vec(c.pos, 91.0, 73.0);
    assert_vec(c.vel, -1.4, 0.0);
    assert_eq!(c.on_slope(), 0);
    assert_eq!(c.contact().y, 1);
}

#[test]
fn slope_below_is_found_when_bottom_extent_enters_it() {
    let r = room(&[
        ".......", ".......", ".......", ".......", "../#...", "#######",
    ]);
    let mut obj = single((45.0, 43.5), (0.0, -2.0), 9.0, frictionless(0.1, 0.5));
    obj.update(&r);
    // Centre at 41.5 is in tile (2,2) (Air); bottom 32.5 < 40 => slope (2,1) below.
    // s = (45 - 40) + 20 = 25; pos.y = 25 + 18 = 43.
    assert_vec(obj.chunks[0].pos, 45.0, 43.0);
    assert_eq!(obj.chunks[0].on_slope(), -1);
}

#[test]
fn slope_line_extends_into_neighbour_tile_and_meets_flat_floor() {
    let r = room(&[
        ".......", ".......", ".......", ".......", "../#...", "#######",
    ]);
    // Chunk in Air tile (1,1) on the ground; its right extent 31 + 9 = 40 reaches the
    // UpLeft slope (2,1): lx = 31 - 40 = -9, s = -9 + 20 = 11, rest 11 + 18 = 29,
    // equal to the flat rest height (0 + 1) * 20 + 9.
    let mut obj = single((31.0, 29.5), (0.0, -1.0), 9.0, frictionless(0.1, 0.5));
    obj.update(&r);
    assert_vec(obj.chunks[0].pos, 31.0, 29.0);
    assert_eq!(obj.chunks[0].on_slope(), -1);
}

#[test]
fn left_neighbour_slope_takes_priority_over_right() {
    // (1,1) UpRight (left (0,1) Solid), (3,1) UpLeft (right (4,1) Solid).
    let r = room(&[
        ".......", ".......", ".......", ".......", "#/./#..", "#######",
    ]);
    // Chunk rad 10 centred in (2,1) at x=50 touches both edges (40 and 60).
    let mut obj = single((50.0, 31.0), (0.0, -1.0), 10.0, frictionless(0.1, 0.5));
    obj.update(&r);
    // Left wins: UpRight at (1,1): lx = 30, s = 20 - 30 + 20 = 10, rest 30; onSlope +1
    // (the right slope would have given -1).
    assert_vec(obj.chunks[0].pos, 50.0, 30.0);
    assert_eq!(obj.chunks[0].on_slope(), 1);
}

#[test]
fn broken_slope_has_no_collision() {
    // Slope (2,2) with only a diagonal Solid neighbour: Broken; the chunk falls through
    // it to the ground: (0 + 1) * 20 + 9 = 29.
    let r = room(&[
        ".......", ".......", ".......", "../....", "...#...", "#######",
    ]);
    let mut obj = single((50.0, 70.0), (0.0, 0.0), 9.0, slugcat::PARAMS);
    for _ in 0..100 {
        obj.update(&r);
    }
    assert_eq!(obj.chunks[0].pos.y, 29.0);
    assert_eq!(obj.chunks[0].on_slope(), 0);
}

// ---- connections ----------------------------------------------------------------------

#[test]
fn rod_corrects_position_and_velocity_symmetrically() {
    let mut chunks = vec![
        BodyChunk::new(Vec2::new(0.0, 0.0), 9.0, 0.35),
        BodyChunk::new(Vec2::new(0.0, 20.0), 8.0, 0.35),
    ];
    let rod = BodyChunkConnection::new(0, 1, 17.0, ConnectionKind::Normal, 1.0, 0.5);
    rod.apply(&mut chunks);
    // dir (0,1); error 17 - 20 = -3; each side moves 1.5 toward the other.
    assert_vec(chunks[0].pos, 0.0, 1.5);
    assert_vec(chunks[1].pos, 0.0, 18.5);
    assert_vec(chunks[0].vel, 0.0, 1.5);
    assert_vec(chunks[1].vel, 0.0, -1.5);
}

#[test]
fn pull_and_push_connections_act_one_way() {
    let untouched = vec![
        BodyChunk::new(Vec2::new(0.0, 0.0), 5.0, 1.0),
        BodyChunk::new(Vec2::new(10.0, 0.0), 5.0, 1.0),
    ];
    let mut chunks = untouched.clone();
    BodyChunkConnection::new(0, 1, 17.0, ConnectionKind::Pull, 1.0, 0.5).apply(&mut chunks);
    assert_eq!(
        chunks, untouched,
        "pull does nothing when closer than its length"
    );
    BodyChunkConnection::new(0, 1, 5.0, ConnectionKind::Push, 1.0, 0.5).apply(&mut chunks);
    assert_eq!(
        chunks, untouched,
        "push does nothing when farther than its length"
    );
    BodyChunkConnection::new(0, 1, 17.0, ConnectionKind::Push, 1.0, 0.25).apply(&mut chunks);
    // error 7 along +x: chunk 0 moves -7 * 0.25, chunk 1 moves +7 * 0.75.
    assert_vec(chunks[0].pos, -1.75, 0.0);
    assert_vec(chunks[1].pos, 15.25, 0.0);
}

#[test]
fn slugcat_rod_keeps_distance_17_while_falling() {
    let r = open_room(10, 60);
    let mut obj = PhysicalObject::slugcat();
    obj.chunks[0].reset_at(Vec2::new(100.0, 1000.0));
    obj.chunks[1].reset_at(Vec2::new(103.0, 990.0));
    for _ in 0..20 {
        obj.update(&r);
        let d = obj.chunks[0].pos.distance(obj.chunks[1].pos);
        assert!((d - 17.0).abs() < 1e-3, "distance {d}");
    }
}

#[test]
fn slugcat_constructor_values() {
    let obj = PhysicalObject::slugcat();
    assert_eq!(obj.chunks[0].rad, 9.0);
    assert_eq!(obj.chunks[1].rad, 8.0);
    assert_eq!(obj.chunks[0].mass, 0.35);
    assert_eq!(obj.chunks[1].mass, 0.35);
    assert_eq!(obj.connections[0].distance, 17.0);
    assert_eq!(obj.connections[0].elasticity, 1.0);
    assert_eq!(obj.connections[0].weight_symmetry, 0.5);
    assert_eq!(obj.params, slugcat::PARAMS);
}

// ---- spawning ---------------------------------------------------------------------------

#[test]
fn spawn_tries_stay_near_the_exit_shortcut() {
    // Entrance (1,2) opens right (its first non-Solid neighbour in the order
    // left, down, right, up). Tries reach x = 1 + 0..=9 and y = 2 + -2..=2; the only
    // acceptable row is y=2 (three Solid tiles below). Fallback 2 would also allow
    // x = 11..=14, which the tries cannot reach.
    let r = room(&[
        "################",
        "#..............#",
        "#..............#",
        "XE.............#",
        "################",
        "################",
    ]);
    for seed in 0..32 {
        let (x, y) = determine_safe_spawn_tile(&r, &mut UnityRandom::new(seed));
        assert_eq!(y, 2, "seed {seed}");
        assert!((1..=10).contains(&x), "seed {seed}: x {x}");
    }
}

#[test]
fn spawn_fallback_two_picks_the_only_tile_with_ground() {
    // No shortcuts. Only (1,2) is non-Solid with three Solid tiles below.
    let r = room(&["#####", "#.###", "#####", "#####"]);
    for seed in 0..8 {
        assert_eq!(
            determine_safe_spawn_tile(&r, &mut UnityRandom::new(seed)),
            (1, 2)
        );
    }
}

#[test]
fn spawn_fallback_picks_with_unity_range() {
    // No shortcuts: the shortcut phase draws its node value and stops (D2), then
    // fallback 2 lists (1,2), (2,2), (3,2) (x outer) and takes `Random.Range(0, 3)`,
    // the next word mod 3.
    let r = room(&["#####", "#...#", "#####", "#####"]);
    for seed in 0..8 {
        let mut g = UnityRandom::new(seed);
        g.value();
        let want = (1 + (g.next_u32() % 3) as i32, 2);
        assert_eq!(
            determine_safe_spawn_tile(&r, &mut UnityRandom::new(seed)),
            want,
            "seed {seed}"
        );
    }
}

#[test]
fn spawn_fallback_three_needs_only_solid_directly_below() {
    // No tile has three Solid below; (1,2) is the only one with Solid directly below.
    let r = room(&[".....", ".#...", "....."]);
    assert_eq!(
        determine_safe_spawn_tile(&r, &mut UnityRandom::new(1)),
        (1, 2)
    );
}

#[test]
fn spawn_fallback_four_and_last_resort() {
    let r = open_room(3, 3);
    let (_, y) = determine_safe_spawn_tile(&r, &mut UnityRandom::new(1));
    assert!(y >= 1);
    let solid = room(&["###", "###"]);
    assert_eq!(
        determine_safe_spawn_tile(&solid, &mut UnityRandom::new(1)),
        (0, 0)
    );
}

#[test]
fn place_in_room_puts_chunks_on_unit_circle_around_tile_middle() {
    let mut obj = PhysicalObject::slugcat();
    obj.chunks[0].vel = Vec2::new(3.0, 4.0);
    place_in_room(&mut obj, (3, 2), &mut UnityRandom::new(9));
    // Middle of tile (3,2) = (10 + 60, 10 + 40).
    let mid = Vec2::new(70.0, 50.0);
    // Each chunk in turn: `Custom.DegToVec(Random.value * 360)`.
    let mut g = UnityRandom::new(9);
    for c in &obj.chunks {
        let want = mid + Vec2::from_degrees(g.value() * 360.0);
        assert_eq!(c.pos, want);
    }
    for c in &obj.chunks {
        assert!((c.pos.distance(mid) - 1.0).abs() < 1e-5);
        assert_eq!(c.last_pos, c.pos);
        assert_eq!(c.last_last_pos, c.pos);
        assert_eq!(c.vel, Vec2::ZERO);
    }
}
