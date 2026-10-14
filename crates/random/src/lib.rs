//! Unity's `UnityEngine.Random`: Marsaglia's xorshift128 seeded by
//! `InitState`, with Unity's float and integer conversions.
//!
//! Sources and what is verified: `rainworld-re/notes/unity-random.md`.
//!
//! The generator also implements rand_core's [`TryRng`] (infallible, so
//! [`rand_core::Rng`] and rand's `RngExt` apply) and [`SeedableRng`]. rand's
//! own conversions (`random::<f32>()`, `random_range`) draw from the same
//! stream but do not map words the way Unity does; use the inherent methods
//! for game parity.

use core::convert::Infallible;
use core::f32::consts::TAU;

use rand_core::{SeedableRng, TryRng, utils};

/// The generator state, `Random.State` in Unity: four xorshift128 words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnityRandom {
    s: [u32; 4],
}

impl UnityRandom {
    const SEED_MULTIPLIER: u32 = 1_812_433_253;
    const MANTISSA_MASK: u32 = 0x7F_FFFF;
    /// `f32(1 / 0x7FFFFF)`, so a full mantissa maps to exactly 1.0.
    const MANTISSA_SCALE: f32 = 1.0 / 0x7F_FFFF as f32;

    /// `Random.InitState(seed)`.
    pub fn new(seed: i32) -> Self {
        let mut rng = Self { s: [0; 4] };
        rng.init_state(seed);
        rng
    }

    /// `Random.InitState`: s0 = seed, each next word = previous * 1812433253 + 1.
    pub fn init_state(&mut self, seed: i32) {
        self.s[0] = seed as u32;
        for i in 1..4 {
            self.s[i] = self.s[i - 1]
                .wrapping_mul(Self::SEED_MULTIPLIER)
                .wrapping_add(1);
        }
    }

    /// Raw words `[s0, s1, s2, s3]`; an all-zero state only ever yields zero.
    pub fn from_state(s: [u32; 4]) -> Self {
        Self { s }
    }

    /// `Random.state` getter.
    pub fn state(&self) -> [u32; 4] {
        self.s
    }

    /// `Random.state` setter.
    pub fn set_state(&mut self, s: [u32; 4]) {
        self.s = s;
    }

    /// One xorshift128 step; returns the new s3.
    pub fn next_u32(&mut self) -> u32 {
        let [x, y, z, w] = self.s;
        let t = x ^ (x << 11);
        let next = w ^ (w >> 19) ^ t ^ (t >> 8);
        self.s = [y, z, w, next];
        next
    }

    /// `Random.value`: the low 23 bits scaled to [0, 1], both ends inclusive.
    pub fn value(&mut self) -> f32 {
        (self.next_u32() & Self::MANTISSA_MASK) as f32 * Self::MANTISSA_SCALE
    }

    /// `Random.Range(float, float)`: `(1 - t) * max + t * min` for `t = value`,
    /// so `t == 0` gives `max`. Either order of bounds works.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        let t = self.value();
        (1.0 - t) * max + t * min
    }

    /// `Random.Range(int, int)`: `[min, max)` for `min < max`; `(max, min]`
    /// for `min > max`; `min == max` returns `min` without advancing.
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min < max {
            let span = max.wrapping_sub(min) as u32;
            min.wrapping_add((self.next_u32() % span) as i32)
        } else if max < min {
            let span = min.wrapping_sub(max) as u32;
            min.wrapping_sub((self.next_u32() % span) as i32)
        } else {
            min
        }
    }

    /// `Random.insideUnitCircle`: angle draw, then radius `sqrt(value)`.
    pub fn inside_unit_circle(&mut self) -> [f32; 2] {
        let angle = self.range_f32(0.0, TAU);
        let radius = self.range_f32(0.0, 1.0).sqrt();
        [radius * angle.cos(), radius * angle.sin()]
    }

    /// `Random.onUnitSphere`: z uniform in [-1, 1], then the angle around z.
    pub fn on_unit_sphere(&mut self) -> [f32; 3] {
        let z = self.range_f32(-1.0, 1.0);
        let angle = self.range_f32(0.0, TAU);
        let radius_xy = (1.0 - z * z).sqrt();
        [angle.cos() * radius_xy, angle.sin() * radius_xy, z]
    }

    /// `Random.insideUnitSphere`: an `onUnitSphere` point scaled by `value^(1/3)`.
    pub fn inside_unit_sphere(&mut self) -> [f32; 3] {
        let [x, y, z] = self.on_unit_sphere();
        let scale = self.value().powf(1.0 / 3.0);
        [x * scale, y * scale, z * scale]
    }
}

impl TryRng for UnityRandom {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        Ok(UnityRandom::next_u32(self))
    }

    /// Two steps, first word in the low half.
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        utils::next_u64_via_u32(self)
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        utils::fill_bytes_via_next_word(dst, || self.try_next_u32())
    }
}

impl SeedableRng for UnityRandom {
    /// The raw state words `s0..s3`, little-endian.
    type Seed = [u8; 16];

    fn from_seed(seed: [u8; 16]) -> Self {
        Self::from_state(utils::read_words(&seed))
    }

    /// `InitState` with the low 32 bits of `state` as the seed.
    fn seed_from_u64(state: u64) -> Self {
        Self::new(state as u32 as i32)
    }
}
