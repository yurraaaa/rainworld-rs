use rand::{Rng, RngExt, SeedableRng};
use rw_random::UnityRandom;

/// With s0 = 0 the step output is `w ^ (w >> 19)`; these `w` make it all ones
/// and zero.
const ALL_ONES_NEXT: [u32; 4] = [0, 0, 0, 0xFFFF_E000];
const ZERO_NEXT: [u32; 4] = [0, 1, 0, 0];

#[test]
fn value_reaches_both_ends() {
    let mut rng = UnityRandom::from_state(ALL_ONES_NEXT);
    assert_eq!(rng.value(), 1.0);
    let mut rng = UnityRandom::from_state(ZERO_NEXT);
    assert_eq!(rng.value(), 0.0);
}

#[test]
fn value_is_a_23_bit_fraction() {
    let mut rng = UnityRandom::new(12345);
    for _ in 0..10_000 {
        let v = rng.value();
        assert!((0.0..=1.0).contains(&v));
        let mantissa = (f64::from(v) * f64::from(0x7F_FFFFu32)).round() as u32;
        assert!(mantissa <= 0x7F_FFFF);
        assert_eq!(mantissa as f32 * (1.0 / 0x7F_FFFF as f32), v);
    }
}

#[test]
fn range_f32_maps_value_zero_to_max_and_one_to_min() {
    let mut rng = UnityRandom::from_state(ZERO_NEXT);
    assert_eq!(rng.range_f32(-3.0, 5.0), 5.0);
    let mut rng = UnityRandom::from_state(ALL_ONES_NEXT);
    assert_eq!(rng.range_f32(-3.0, 5.0), -3.0);
}

#[test]
fn range_f32_bounds_in_either_order() {
    let mut rng = UnityRandom::new(7);
    for _ in 0..10_000 {
        assert!((2.0..=4.0).contains(&rng.range_f32(2.0, 4.0)));
        assert!((2.0..=4.0).contains(&rng.range_f32(4.0, 2.0)));
    }
}

#[test]
fn range_i32_excludes_max() {
    let mut rng = UnityRandom::new(99);
    let mut seen = [false; 5];
    for _ in 0..10_000 {
        let v = rng.range_i32(-2, 3);
        assert!((-2..3).contains(&v));
        seen[(v + 2) as usize] = true;
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn range_i32_reversed_is_max_exclusive_min_inclusive() {
    let mut rng = UnityRandom::new(5);
    let mut seen = [false; 3];
    for _ in 0..10_000 {
        let v = rng.range_i32(5, 2);
        assert!((3..=5).contains(&v), "{v}");
        seen[(v - 3) as usize] = true;
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn range_i32_equal_bounds_does_not_advance() {
    let mut rng = UnityRandom::new(42);
    let before = rng.state();
    assert_eq!(rng.range_i32(8, 8), 8);
    assert_eq!(rng.state(), before);
}

#[test]
fn range_i32_full_span() {
    let mut rng = UnityRandom::new(3);
    let mut copy = rng.clone();
    let raw = copy.next_u32();
    let v = rng.range_i32(i32::MIN, i32::MAX);
    assert_eq!(v, i32::MIN.wrapping_add((raw % u32::MAX) as i32));
    assert!(v < i32::MAX);
}

#[test]
fn unit_shapes() {
    let mut rng = UnityRandom::new(2024);
    for _ in 0..10_000 {
        let [x, y] = rng.inside_unit_circle();
        assert!(x * x + y * y <= 1.0 + 1e-5);
        let [x, y, z] = rng.on_unit_sphere();
        assert!((x * x + y * y + z * z - 1.0).abs() < 1e-5);
        let [x, y, z] = rng.inside_unit_sphere();
        assert!(x * x + y * y + z * z <= 1.0 + 1e-5);
    }
}

#[test]
fn state_round_trip() {
    let mut rng = UnityRandom::new(11);
    rng.value();
    let saved = rng.state();
    let a = [rng.value(), rng.value()];
    rng.set_state(saved);
    assert_eq!([rng.value(), rng.value()], a);
}

#[test]
fn seedable_rng() {
    let rng = UnityRandom::seed_from_u64(0xDEAD_BEEF_0000_0001);
    assert_eq!(rng, UnityRandom::new(1));

    let words = UnityRandom::new(77).state();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let seed: [u8; 16] = bytes.try_into().unwrap();
    assert_eq!(UnityRandom::from_seed(seed), UnityRandom::new(77));
}

#[test]
fn works_through_rand() {
    let mut rng = UnityRandom::new(1);
    let mut unity = rng.clone();
    assert_eq!(Rng::next_u32(&mut rng), unity.next_u32());

    let lo = u64::from(unity.next_u32());
    let hi = u64::from(unity.next_u32());
    assert_eq!(Rng::next_u64(&mut rng), hi << 32 | lo);

    let mut bytes = [0u8; 6];
    rng.fill_bytes(&mut bytes);
    let a = unity.next_u32().to_le_bytes();
    let b = unity.next_u32().to_le_bytes();
    assert_eq!(bytes, [a[0], a[1], a[2], a[3], b[0], b[1]]);

    for _ in 0..1000 {
        let f: f32 = rng.random();
        assert!((0.0..1.0).contains(&f));
        assert!((10..20).contains(&rng.random_range(10..20)));
    }
}
