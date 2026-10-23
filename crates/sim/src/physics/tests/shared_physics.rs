use super::*;

#[test]
fn a_walk_along_a_row_counts_every_tile() {
    let tiles: Vec<_> = TileWalk::new(Vec2::new(5.0, 5.0), Vec2::new(65.0, 5.0)).collect();
    assert_eq!(tiles, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
}

#[test]
fn a_diagonal_walk_steps_one_axis_at_a_time() {
    let tiles: Vec<_> = TileWalk::new(Vec2::new(5.0, 5.0), Vec2::new(25.0, 35.0)).collect();
    assert_eq!(tiles, vec![(0, 0), (0, 1), (1, 1)]);
}

#[test]
fn rect_collision_takes_the_entered_edge() {
    let r = TileRect::new(Vec2::new(20.0, 0.0), Vec2::new(40.0, 20.0));
    let hit = r.collision(Vec2::new(30.0, 10.0), Vec2::new(5.0, 10.0));
    assert_eq!(hit, Vec2::new(20.0, 10.0));
}
