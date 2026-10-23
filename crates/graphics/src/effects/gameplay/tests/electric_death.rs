use rw_sim::Vec3;

use super::*;

fn sprite(list: &DrawList, i: usize) -> Sprite {
    match &list.items[i] {
        DrawItem::Sprite(s) => *s,
        other => panic!("{other:?}"),
    }
}

/// Sprite 0 is the web on the `ElectricDeath` shader at the screen centre with the
/// colour walk and `Pow(I, 0.5) * Lerp(0.5 + 0.5 sin, 1, I^4)`; sprite 1 stays a unit
/// disc at the corner; flashes 1 to 8 are sprites 2 to 9, two draws each (the scale's
/// then the alpha's), `alpha = sin(life pi) * Lerp(0.6, 1, v) * 0.6 * I`.
#[test]
fn the_web_and_its_flashes() {
    let mut d = ElectricDeath::new(1, 1.0);
    d.last_color = Vec3::new(0.2, 0.4, 0.6);
    d.color = Vec3::new(0.4, 0.8, 0.2);
    d.last_sin = 1.0;
    d.sin = 1.25;
    d.flashes[2].pos = Vec2::new(300.0, 100.0);
    d.flashes[2].rad = 400.0;
    d.flashes[2].life = 0.5;
    d.flashes[2].last_life = 0.5;
    let mut rng = UnityRandom::new(3);
    let mut e = UnityRandom::new(3);
    let list = d.draw(
        0.64,
        Vec2::new(1000.0, 200.0),
        Vec2::new(1024.0, 768.0),
        0.5,
        &mut rng,
    );
    assert_eq!(list.items.len(), 10);
    let web = sprite(&list, 0);
    assert_eq!(web.shader, Shader::ElectricDeath);
    assert_eq!(web.pos, Vec2::new(1512.0, 584.0));
    assert_eq!(web.scale, Vec2::new(87.5, 50.0));
    assert_eq!(web.layer, Layer::Water);
    assert!((web.color.r - 0.3).abs() < 1e-6 && (web.color.g - 0.6).abs() < 1e-6);
    assert!((web.color.b - 0.4).abs() < 1e-6);
    let sin = 1.125f32;
    let want = 0.64f32.powf(0.5)
        * (0.5 + 0.5 * (sin * std::f32::consts::TAU).sin()).lerp(1.0, 0.64f32.powf(4.0));
    assert!((web.color.a - want).abs() < 1e-5, "{} {want}", web.color.a);
    let corner = sprite(&list, 1);
    assert_eq!(corner.pos, Vec2::new(1000.0, 200.0));
    assert_eq!((corner.scale.x, corner.color.a), (1.0, 1.0));
    assert_eq!(corner.shader, Shader::LightSource);
    // Sprite 3 is `flashes[2]`: life 0.5 gives sin(pi/2) = 1.
    for i in 2..10 {
        let s = sprite(&list, i);
        let scale_draw = e.value();
        let alpha_draw = e.value();
        if i == 3 {
            let w = 0.5f32.lerp(1.0, 1.0) * 0.8f32.lerp(1.2, scale_draw) * 400.0 / 8.0;
            assert!((s.scale.x - w).abs() < 1e-3, "{} {w}", s.scale.x);
            let a = 0.6f32.lerp(1.0, alpha_draw) * 0.6 * 0.64;
            assert!((s.color.a - a).abs() < 1e-5);
            assert_eq!(s.pos, Vec2::new(1300.0, 300.0));
        }
        assert_eq!(s.index, i);
        assert_eq!((s.color.r, s.color.g, s.color.b), (0.0, 1.0, 0.0));
    }
    assert_eq!(rng.state(), e.state(), "sixteen draws");
}

/// A flash's three sprites: `LightSource` and `FlatLight` discs scaled by `Pow(sin,
/// 0.5)` times `Lerp(0.8, 1.2, v)` times `size * 4 / 8`, the flare by `Lerp(0.5, 1,
/// sin)` times `Lerp(0.8, 1.2, v)` times `size / 8`, with `size = Lerp(20, 120,
/// flashSize^1.5)`: six draws in the order scale, alpha.
#[test]
fn a_spark_flash_draws_six() {
    let mut seed_rng = UnityRandom::new(9);
    let mut f = SparkFlash::new(Vec2::new(50.0, 70.0), 0.4, &mut seed_rng);
    // A newborn flash (life 1) has `Sin(1 * (float)PI)` just below 0, so `Pow(., 0.5)`
    // is NaN for its first frame, as in the game.
    let born = f.sprites(0.5, &mut UnityRandom::new(5));
    assert!(matches!(&born[0], DrawItem::Sprite(s) if s.scale.x.is_nan()));
    f.life = 0.5;
    f.last_life = 0.5;
    let mut rng = UnityRandom::new(5);
    let mut e = UnityRandom::new(5);
    let items = f.sprites(0.5, &mut rng);
    let sine = (0.5f32 * std::f32::consts::PI).sin();
    let size = 20.0f32.lerp(120.0, 0.4f32.powf(1.5));
    let wide = sine.powf(0.5);
    let v: Vec<f32> = (0..6).map(|_| e.value()).collect();
    let want = [
        wide * 0.8f32.lerp(1.2, v[0]) * size * 4.0 / 8.0,
        wide * 0.6f32.lerp(1.0, v[1]),
        wide * 0.8f32.lerp(1.2, v[2]) * size * 4.0 / 8.0,
        wide * 0.6f32.lerp(1.0, v[3]) * 0.2,
        0.5f32.lerp(1.0, sine) * 0.8f32.lerp(1.2, v[4]) * size / 8.0,
        sine * v[5],
    ];
    assert_eq!(rng.state(), e.state());
    let s: Vec<Sprite> = items
        .iter()
        .map(|i| match i {
            DrawItem::Sprite(s) => *s,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        (s[0].shader, s[1].shader, s[2].shader),
        (Shader::LightSource, Shader::FlatLight, Shader::FlareBomb)
    );
    assert!((s[0].scale.x - want[0]).abs() < 1e-4);
    assert!((s[0].color.a - want[1]).abs() < 1e-5);
    assert!((s[1].scale.x - want[2]).abs() < 1e-4);
    assert!((s[1].color.a - want[3]).abs() < 1e-5);
    assert!((s[2].scale.x - want[4]).abs() < 1e-4);
    assert!((s[2].color.a - want[5]).abs() < 1e-5);
    assert!(
        s.iter()
            .all(|s| s.pos == Vec2::new(50.0, 70.0) && s.layer == Layer::Water)
    );
    assert!(
        s.iter()
            .all(|s| (s.color.r, s.color.g, s.color.b) == (0.0, 1.0, 0.0))
    );
}
