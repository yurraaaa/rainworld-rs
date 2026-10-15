//! `OverseerGraphics` so far: what its constructor and `Reset` draw from the global
//! stream (the mycelia), without positions or sprites. The overseer is not drawn yet
//! (docs/DIVERGENCES.md D14).

use rw_random::UnityRandom;
use rw_sim::FloatExt;
use rw_sim::creature::EntityId;

/// An overseer's graphics module: its mycelia's point counts.
#[derive(Debug, Clone, PartialEq)]
pub struct OverseerGraphics {
    pub id: EntityId,
    /// Points per mycelium (`Mycelium.points`).
    pub mycelia: Vec<usize>,
}

impl OverseerGraphics {
    /// The constructor: the mycelium count from the ID's seed (global state saved and
    /// restored), then `Reset`.
    pub fn new(id: EntityId, size: f32, rng: &mut UnityRandom) -> Self {
        let saved = rng.state();
        rng.init_state(id.random_seed());
        let a = rng.value();
        let b = rng.value().mathf_pow(0.5);
        let inner = 2.0f32.lerp(6.0, a.lerp(size.inverse_lerp(0.5, 1.0), b));
        let count = inner.lerp(4.0, rng.value()) as usize;
        rng.set_state(saved);
        let length = size.lerp_map(0.5, 1.0, 55.0, 80.0);
        let reach = length.max(length.lerp(40.0, 0.5));
        let points = ((reach / 15.0) as usize).clamp(2, 20);
        let mut g = Self {
            id,
            mycelia: vec![points; count],
        };
        g.reset(rng);
        g
    }

    /// `Reset`: each mycelium's `Reset` draws its direction (`ResetDir`), two control
    /// point jitters, and two `Custom.RNV` per point.
    pub fn reset(&mut self, rng: &mut UnityRandom) {
        for &points in &self.mycelia {
            for _ in 0..3 + 2 * points {
                rng.value();
            }
        }
    }
}
