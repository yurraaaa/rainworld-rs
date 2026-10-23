use super::*;

#[test]
fn covers_the_screen_in_shadows() {
    let list = SunBlocker::draw(Vec2::new(100.0, 50.0));
    let s = list.sprite(0).expect("one sprite");
    assert_eq!(s.pos, Vec2::new(783.0, 434.0));
    assert_eq!(s.scale, Vec2::new(1500.0, 900.0));
    assert_eq!(s.color.r, 1.0 / 255.0);
    assert_eq!(s.layer, Layer::Shadows);
}
