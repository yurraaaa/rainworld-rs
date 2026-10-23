use rw_random::UnityRandom;
use rw_sim::creature::EntityId;

use super::{PEBBLES, PebbleElement};

#[test]
fn the_pebble_follows_the_id_and_leaves_the_stream_alone() {
    // `InitState(seed); Range(1, 15)` on a generator of its own.
    let id = EntityId::new(-3, 1042);
    let mut expect = UnityRandom::new(id.random_seed());
    let n = expect.range_i32(1, 15);
    assert_eq!(id.pebble(), PEBBLES[usize::try_from(n - 1).unwrap()]);
    assert_eq!(id.pebble(), format!("Pebble{n}"));
}
