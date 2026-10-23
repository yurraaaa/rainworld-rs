use super::*;

#[test]
fn small_discover_draws_four_and_greys_neighbours() {
    let mut t = DiscoveryTexture(UnityTexture::black(10, 10));
    let mut rng = UnityRandom::new(5);
    let mut copy = rng.clone();
    t.small_discover((4, 4), &mut rng);
    let draws: Vec<f32> = (0..4).map(|_| copy.value()).collect();
    assert_eq!(rng.state(), copy.state());
    assert_eq!(t.0.r(4, 4), 1.0);
    for ((dx, dy), v) in FOUR_DIRECTIONS.into_iter().zip(draws) {
        let want = if v < 0.5 { 64.0 / 255.0 } else { 0.0 };
        assert_eq!(t.0.r(4 + dx, 4 + dy), want);
    }
}

#[test]
fn discover_whitens_cross_and_greys_diagonals() {
    let mut t = DiscoveryTexture(UnityTexture::black(10, 10));
    let mut rng = UnityRandom::new(1);
    let before = rng.state();
    t.0.set_grey(6, 6, 1.0);
    t.discover((5, 5), false, &mut rng);
    assert_eq!(rng.state(), before);
    for (x, y) in [(5, 5), (4, 5), (6, 5), (5, 4), (5, 6)] {
        assert_eq!(t.0.r(x, y), 1.0);
    }
    assert_eq!(t.0.r(4, 4), 128.0 / 255.0);
    assert_eq!(t.0.r(6, 6), 1.0);
    assert_eq!(t.0.r(7, 7), 0.0);
}

#[test]
fn pixels_wrap_as_repeat() {
    let mut t = UnityTexture::black(4, 3);
    t.set_grey(-1, 3, 1.0);
    assert_eq!(t.r(3, 0), 1.0);
}

#[test]
fn base64_png_round_trips() {
    let mut t = DiscoveryTexture(UnityTexture::black(12, 5));
    t.0.set_grey(0, 0, 1.0);
    t.0.set_grey(11, 4, 0.5);
    let text = t.to_base64_png();
    assert_eq!(DiscoveryTexture::from_base64_png(&text), Some(t));
    let narrow = DiscoveryTexture(UnityTexture::black(9, 5)).to_base64_png();
    assert_eq!(DiscoveryTexture::from_base64_png(&narrow), None);
}

#[test]
fn texture_pos_centres_room_on_its_map_pos() {
    let room = MapRoom {
        index: 3,
        pos: Vec2::new(300.0, 150.0),
        size: (48, 35),
        layer: 2,
    };
    let size = Vec2::new(1189.0, 570.0);
    // Room centre: mapPos / 3 + (10, 10); layer 2 adds two thirds of the texture.
    let c = room.on_texture_pos(Vec2::new(480.0, 350.0), size, false);
    assert_eq!(c, Vec2::new(110.0, 60.0));
    let c = room.on_texture_pos(Vec2::new(480.0, 350.0), size, true);
    assert_eq!(c, Vec2::new(110.0, 1200.0));
    assert_eq!(
        room.discovery_pixel(Vec2::new(480.0, 350.0), size),
        (15, 171)
    );
}
