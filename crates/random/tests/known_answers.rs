//! Reference sequences published with the `unity-random` crate
//! (github.com/Minavoii/unity-random, `tests/*.rs`, MIT), which its author
//! presents as Unity's output. Floats there are printed to 7 significant
//! digits; integers are exact.

use rw_random::UnityRandom;

const SEEDS: [i32; 5] = [0, 1, 358118, 30029247, 719188662];

/// A float as printed by Unity: 7 significant digits.
#[derive(Clone, Copy)]
struct Printed(f64);

impl Printed {
    /// `actual` prints as the same 7 significant digits.
    fn assert_exact(self, actual: f32) {
        assert_eq!(format!("{:.6e}", actual), format!("{:.6e}", self.0));
    }

    /// Within half a unit of the 7th digit plus a few ulps, for sin, cos,
    /// sqrt and pow, which may round differently from Unity's.
    fn assert_matches(self, actual: f32) {
        let expected = self.0;
        let digit = 10f64.powi(expected.abs().log10().floor() as i32 - 6);
        let tolerance = 0.5 * digit + 4.0 * f64::from(f32::EPSILON) * expected.abs().max(1e-3);
        assert!(
            (f64::from(actual) - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }
}

/// Exact to the printed digits; a 2^-23 scale (giving [0, 1)) would print
/// differently for 14 of these 25.
#[test]
fn value() {
    let expected: [[f64; 5]; 5] = [
        [0.5841396, 0.5840824, 0.6736069, 0.766507, 0.3050319],
        [0.9996847, 0.7742628, 0.6809838, 0.4604562, 0.5944274],
        [0.6642595, 0.1477097, 0.9248888, 0.5928424, 0.9549153],
        [0.4087697, 0.510399, 0.8909144, 0.3268396, 0.1220958],
        [0.2724452, 0.1936961, 0.95676, 0.05701066, 0.1853699],
    ];
    for (seed, values) in SEEDS.into_iter().zip(expected) {
        let mut rng = UnityRandom::new(seed);
        for v in values {
            Printed(v).assert_exact(rng.value());
        }
    }
}

#[test]
fn range_float() {
    let expected: [[f64; 5]; 5] = [
        [0.4158604, 0.4159176, 0.3263931, 0.233493, 0.6949681],
        [0.0003153086, 0.2257372, 0.3190162, 0.5395438, 0.4055726],
        [0.3357405, 0.8522903, 0.07511115, 0.4071576, 0.04508471],
        [0.5912303, 0.489601, 0.1090856, 0.6731604, 0.8779042],
        [0.7275548, 0.806304, 0.04323995, 0.9429893, 0.8146302],
    ];
    for (seed, values) in SEEDS.into_iter().zip(expected) {
        let mut rng = UnityRandom::new(seed);
        for v in values {
            Printed(v).assert_exact(rng.range_f32(0.0, 1.0));
        }
    }
}

#[test]
fn range_int() {
    let expected: [[i32; 5]; 5] = [
        [1900725526, 1900725046, 559298752, 107093222, 556206921],
        [1543501227, 199432971, 752298619, 138080315, 743183923],
        [2136278644, 1595074600, 1928749762, 1103880771, 377109161],
        [607408785, 1212241089, 1349650812, 1000986081, 1024434390],
        [1596120957, 890817289, 1727690525, 42421281, 1268234803],
    ];
    for (seed, values) in SEEDS.into_iter().zip(expected) {
        let mut rng = UnityRandom::new(seed);
        for v in values {
            assert_eq!(rng.range_i32(0, i32::MAX), v);
        }
    }
}

#[test]
fn inside_unit_circle() {
    let expected: [[[f64; 2]; 5]; 5] = [
        [
            [-0.5568735, 0.3252837],
            [-0.2231333, 0.4286076],
            [-0.2005469, -0.5566905],
            [-0.405624, 0.5776114],
            [-0.1846074, -0.9116018],
        ],
        [
            [0.4751172, 0.0009412759],
            [-0.308636, 0.666549],
            [-0.3846249, 0.2593728],
            [0.7976097, 0.475883],
            [-0.02884089, -0.6656236],
        ],
        [
            [-0.4736371, 0.7924381],
            [0.568339, 0.2900833],
            [0.4668945, 0.1359149],
            [-0.9719155, 0.01098891],
            [0.6989536, 0.7077643],
        ],
        [
            [-0.5878728, -0.3794819],
            [0.6351724, 0.5193423],
            [0.5896927, -0.5685552],
            [-0.4813436, 0.7190998],
            [0.4176237, 0.8590526],
        ],
        [
            [-0.1262154, -0.8890297],
            [0.9354575, 0.2605929],
            [0.3340298, -0.7768455],
            [-0.5394579, 0.7793431],
            [-0.6767728, -0.0851628],
        ],
    ];
    for (seed, points) in SEEDS.into_iter().zip(expected) {
        let mut rng = UnityRandom::new(seed);
        for point in points {
            let actual = rng.inside_unit_circle();
            for (e, a) in point.into_iter().zip(actual) {
                Printed(e).assert_matches(a);
            }
        }
    }
}

#[test]
fn on_unit_sphere() {
    let expected: [[[f64; 3]; 5]; 5] = [
        [
            [-0.8513461, 0.4968821, -0.1682793],
            [0.09708977, 0.9327465, -0.3472139],
            [-0.5418345, 0.744557, 0.3899362],
            [-0.9522503, 0.01097478, -0.3051211],
            [0.5954313, -0.6745495, 0.4363994],
        ],
        [
            [0.005392162, 0.03509618, -0.9993694],
            [-0.9035649, -0.2292374, -0.3619677],
            [0.2129496, 0.9586377, -0.1888548],
            [0.3638166, -0.4251846, -0.8287675],
            [-0.8200869, 0.3017466, 0.4862165],
        ],
        [
            [0.5660996, -0.7560467, -0.328519],
            [-0.4399648, 0.2903598, -0.8497777],
            [0.03525296, 0.4134798, -0.9098306],
            [0.9403212, -0.3402691, -0.003598809],
            [0.6622385, -0.04389393, -0.7480063],
        ],
        [
            [-0.9811152, 0.06419647, 0.1824607],
            [-0.2894619, -0.5522277, -0.7818289],
            [-0.3118641, -0.5757556, 0.7558084],
            [-0.007185814, -0.9499767, -0.3122381],
            [0.6519667, -0.4001966, -0.6440358],
        ],
        [
            [0.3084782, -0.8352942, 0.4551096],
            [0.380973, -0.1426206, -0.9135201],
            [-0.1692309, -0.7585462, 0.6292603],
            [0.7641817, -0.5671062, -0.3072733],
            [-0.9755156, 0.2162906, 0.03984559],
        ],
    ];
    for (seed, points) in SEEDS.into_iter().zip(expected) {
        let mut rng = UnityRandom::new(seed);
        for point in points {
            let actual = rng.on_unit_sphere();
            for (e, a) in point.into_iter().zip(actual) {
                Printed(e).assert_matches(a);
            }
        }
    }
}

#[test]
fn inside_unit_sphere() {
    let expected: [[[f64; 3]; 5]; 5] = [
        [
            [-0.7462912, 0.4355676, -0.1475139],
            [-0.2483938, -0.6895068, -0.4616884],
            [-0.624307, 0.007195201, -0.2000411],
            [0.4255203, 0.4062259, 0.628723],
            [0.09393974, 0.7890219, -0.3996925],
        ],
        [
            [0.00474397, 0.03087726, -0.879235],
            [-0.7623613, 0.5141004, 0.07295015],
            [0.231277, -0.2702885, -0.5268449],
            [0.1381525, 0.6717938, -0.07746036],
            [0.516916, -0.8369448, 0.07885869],
        ],
        [
            [0.5515557, -0.7366227, -0.3200788],
            [0.8623052, 0.2510206, -0.1697148],
            [0.8990432, -0.3253321, -0.00344083],
            [0.120665, -0.05546359, 0.636704],
            [-0.1364788, 0.4184574, 0.5434878],
        ],
        [
            [-0.9440579, 0.06177174, 0.175569],
            [0.4662213, -0.4495096, 0.2390822],
            [-0.006731363, -0.8898976, -0.2924913],
            [-0.1447194, -0.5299388, 0.801227],
            [-0.5350057, -0.5384245, -0.4412423],
        ],
        [
            [0.3039664, -0.8230771, 0.448453],
            [0.1205376, -0.2803316, 0.5830047],
            [0.5983661, -0.444053, -0.2405997],
            [-0.2023555, 0.3666469, -0.02915556],
            [-0.3058136, -0.1750615, 0.7583968],
        ],
    ];
    for (seed, points) in SEEDS.into_iter().zip(expected) {
        let mut rng = UnityRandom::new(seed);
        for point in points {
            let actual = rng.inside_unit_sphere();
            for (e, a) in point.into_iter().zip(actual) {
                Printed(e).assert_matches(a);
            }
        }
    }
}

/// Hand-derived from the algorithm, not published: seed 1 seeds
/// [1, 1812433254, 1900727103, 3690981084]; t = 1 ^ (1 << 11) = 0x801 and
/// w ^ (w >> 19) ^ t ^ (t >> 8) = 3690984874, whose remainder mod
/// i32::MAX (1543501227) and low 23 bits / 0x7FFFFF (0.9996847) are the
/// published first values for seed 1 above.
#[test]
fn seed_one_by_hand() {
    let mut rng = UnityRandom::new(1);
    assert_eq!(rng.state(), [1, 1812433254, 1900727103, 3690981084]);
    assert_eq!(rng.next_u32(), 3690984874);
    assert_eq!(
        rng.state(),
        [1812433254, 1900727103, 3690981084, 3690984874]
    );
}
