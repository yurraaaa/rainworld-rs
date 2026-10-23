//! `BodyPart.PushOutOfTerrain`.

use rw_sim::Vec2;

use crate::BodyPart;
use crate::tests::{assert_vec, flat_room, room};

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
