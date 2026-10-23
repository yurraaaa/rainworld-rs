use rw_random::UnityRandom;
use rw_sim::creature::CreatureId;
use rw_sim::{Cosmetic, CosmeticKind, NeuronSpark, Spark, Vec2};

use super::{CosmeticMesh, CosmeticSprites, LizardModule, NoLizards, SparkMesh};
use crate::core::draw::{Color, DrawItem, Layer, Shader};
use crate::tests::{MODULE, OneModule, room};

/// A spark takes its module's head colour only while the module's `whiteFlicker`
/// is above 0.
#[test]
fn a_spark_shows_its_modules_head_colour_while_it_flickers() {
    let room = crate::tests::room(&["...."]);
    let mut s = Spark::new(
        Vec2::new(50.0, 50.0),
        Vec2::new(5.0, 0.0),
        [0.0, 1.0, 0.0, 1.0],
        10,
        170,
        &mut UnityRandom::new(1),
    );
    s.lizard = Some((CreatureId(0), 7));
    let flickering = LizardModule {
        white_flicker: 1,
        ..MODULE
    };
    assert_eq!(
        s.lizard_mesh(&room, 0.0, &OneModule(flickering)).color,
        MODULE.head_color
    );
    assert_eq!(
        s.lizard_mesh(&room, 0.0, &OneModule(MODULE)).color,
        Color::rgb(0.0, 1.0, 0.0)
    );
}

#[test]
fn a_new_spark_streaks_at_least_nine_pixels() {
    let mut rng = UnityRandom::new(1);
    let s = Spark::new(
        Vec2::new(50.0, 50.0),
        Vec2::new(5.0, 0.0),
        [1.0; 4],
        6,
        18,
        &mut rng,
    );
    let room = crate::tests::room(&["...."]);
    let m = s.mesh(&room, 0.0);
    assert_eq!(m.vertices.len(), 3);
    assert!((m.vertices[2] - Vec2::new(50.0, 50.0)).length() > 8.99);
}

/// `NeuronSpark.DrawSprites`: a blue `FlatLight` disc and a rotated pixel in `Foreground`;
/// five draws, the pixel's glow first, then its size, the disc's alpha and its size.
#[test]
fn a_neuron_spark_draws_five() {
    let r = room(&["....", "....", "...."]);
    let mut spark = NeuronSpark::new(Vec2::new(10.0, 20.0), &mut UnityRandom::new(1));
    spark.last_pos = Vec2::new(10.0, 20.0);
    spark.pos = Vec2::new(14.0, 20.0);
    spark.last_life = 0.8;
    spark.life = 0.4;
    let cosmetic = Cosmetic {
        id: 1,
        room: 0,
        update_order: 2,
        slated: false,
        kind: CosmeticKind::NeuronSpark(spark),
    };
    let mut rng = UnityRandom::new(8);
    let mut e = UnityRandom::new(8);
    let list = cosmetic.draw(&r, 0.5, &NoLizards(Color::BLACK), &mut rng);
    let life = 0.6f32;
    let (a, b, c, d1, d2) = (e.value(), e.value(), e.value(), e.value(), e.value());
    assert_eq!(rng.state(), e.state());
    assert_eq!(list.items.len(), 2);
    let (DrawItem::Sprite(disc), DrawItem::Sprite(pixel)) = (&list.items[0], &list.items[1]) else {
        panic!("{:?}", list.items);
    };
    assert_eq!(
        (disc.element, disc.shader, disc.layer),
        ("Futile_White", Shader::FlatLight, Layer::Foreground)
    );
    assert_eq!(
        (pixel.element, pixel.rotation, pixel.layer),
        ("pixel", 45.0, Layer::Foreground)
    );
    assert_eq!(disc.pos, Vec2::new(12.0, 20.0));
    assert_eq!(pixel.pos, Vec2::new(12.0, 20.0));
    let glow = life.powf(3.0) * a;
    assert!((pixel.color.r - glow).abs() < 1e-6 && (pixel.color.b - 1.0).abs() < 1e-6);
    assert!((pixel.scale.x - 4.0 * b * life).abs() < 1e-5);
    assert!((disc.color.a - c * life).abs() < 1e-6);
    assert_eq!((disc.color.r, disc.color.g, disc.color.b), (0.0, 0.0, 1.0));
    assert!((disc.scale.x - (15.0 * d1 + 15.0 * d2.powf(3.0)) * life / 10.0).abs() < 1e-4);
}
