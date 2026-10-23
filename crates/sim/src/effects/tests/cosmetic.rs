
use rw_random::UnityRandom;

use super::{ExplosionSpikes, Spark};
use crate::tests::room;
use crate::{FloatExt, Vec2};

#[test]
fn a_spark_draws_three_or_four() {
    // `pos + vel.normalized * (30 * value)`, `Lerp(0.4, 0.9, value)`,
    // `Range(0, standard)`, `value < 0.1` and then `Range(standard, exceptional)`.
    for seed in 0..40 {
        let mut rng = UnityRandom::new(seed);
        let mut expect = UnityRandom::new(seed);
        let s = Spark::new(
            Vec2::new(10.0, 10.0),
            Vec2::new(3.0, 0.0),
            [1.0; 4],
            6,
            18,
            &mut rng,
        );
        let ahead = expect.value() * 30.0;
        let gravity = 0.4f32.lerp(0.9, expect.value());
        let mut life = expect.range_i32(0, 6);
        if expect.value() < 0.1 {
            life = expect.range_i32(6, 18);
        }
        assert_eq!(rng.state(), expect.state());
        assert_eq!(s.pos, Vec2::new(10.0 + ahead, 10.0));
        assert_eq!((s.gravity, s.life_time), (gravity, life));
    }
}

#[test]
fn spikes_draw_two_and_four_a_spike() {
    // `(int)(6 * Lerp(0.8, 1.2, value))` spikes, the start angle, then per spike
    // the direction jitter, lifetime, length and width.
    let r = room(&["....", "....", "...."]);
    let mut rng = UnityRandom::new(3);
    let mut expect = UnityRandom::new(3);
    let s = ExplosionSpikes::new(
        &r,
        Vec2::new(40.0, 30.0),
        6,
        5.5,
        4.0,
        4.5,
        21.0,
        [1.0, 1.0, 1.0, 0.25],
        &mut rng,
    );
    let n = (6.0 * 0.8f32.lerp(1.2, expect.value())) as i32;
    assert_eq!(s.spikes.len(), usize::try_from(n).unwrap());
    for _ in 0..1 + 4 * n {
        expect.value();
    }
    assert_eq!(rng.state(), expect.state());
}
