//! `CircuitConnector`: a dot on one `CircuitBit` that five to nine black mycelia reach out
//! from, in the coral system's registry. The dot rides its bit (position, flip and spin); each
//! tick every strand updates and is nudged along its own spoke. The sprites are
//! `rw-graphics`' `superstructure/coral/circuit.rs`.
//!
//! Mirrors: CoralBrain/CircuitConnector.cs (constructor, Update, OnCircuitPos, ConnectionPos, ResetDir)

use rw_random::UnityRandom;

use crate::Vec2;
use crate::superstructure::coral::circuit::{CircuitBit, CircuitCx};
use crate::superstructure::coral::mycelium::{
    MyceliumEnv, MyceliumId, MyceliumOwner, MyceliumPool, OwnerKey,
};

/// Where a connector's strands hang from: the dot, which has no direction to reach in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectorAnchor(pub Vec2);

impl MyceliumOwner for ConnectorAnchor {
    fn connection_pos(&self, _index: usize) -> Vec2 {
        self.0
    }

    fn reset_dir(&self, _index: usize, _rng: &mut UnityRandom) -> Vec2 {
        Vec2::ZERO
    }
}

/// A `CircuitConnector` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct CircuitConnector {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// The bit it rides, as an index into the circuit's `bits`.
    pub bit: usize,
    /// Its place on the bit, from the bit's middle.
    pub c_pos: Vec2,
    pub owner: OwnerKey,
    pub connections: Vec<MyceliumId>,
}

impl CircuitConnector {
    /// The constructor (and `room.AddObject`): the place on the bit (two draws), the
    /// number of strands (`Range(5, 10)`), each a 30 px `Mycelium` made at the dot.
    pub fn new(
        bit: usize,
        bits: &[CircuitBit],
        cx: &mut CircuitCx<'_>,
        rng: &mut UnityRandom,
    ) -> Self {
        let size = bits[bit].size;
        let x = rng.value() * size.x;
        let y = rng.value() * size.y;
        let c_pos = Vec2::new(x, y) - size * 0.5;
        let count = usize::try_from(rng.range_i32(5, 10)).unwrap_or(5);
        let owner = cx.pool.new_owner();
        let at = bits[bit].on_circuit_pos(c_pos, 1.0);
        let connections = (0..count)
            .map(|i| {
                let id = cx.pool.add(
                    Some(cx.system),
                    (owner, &ConnectorAnchor(at)),
                    i,
                    30.0,
                    at,
                    rng,
                );
                if let Some(m) = cx.pool.get_mut(id) {
                    m.use_static_culling = false;
                }
                id
            })
            .collect();
        Self {
            update_order: cx.next_seq(),
            bit,
            c_pos,
            owner,
            connections,
        }
    }

    /// `ConnectionPos(index, timeStacker)` / `OnCircuitPos(cPos, bit, timeStacker)`.
    #[must_use]
    pub fn dot_pos(&self, bits: &[CircuitBit], time_stacker: f32) -> Vec2 {
        bits[self.bit].on_circuit_pos(self.c_pos, time_stacker)
    }

    /// `Update`: each strand's turn, then its spoke's push on the second point.
    pub fn update(
        &self,
        bits: &[CircuitBit],
        pool: &mut MyceliumPool,
        env: &MyceliumEnv<'_>,
        rng: &mut UnityRandom,
    ) {
        let anchor = ConnectorAnchor(self.dot_pos(bits, 1.0));
        let count = self.connections.len();
        for (i, &id) in self.connections.iter().enumerate() {
            pool.update(id, &anchor, env, rng);
            if let Some(m) = pool.get_mut(id) {
                m.points[1][2] += Vec2::from_degrees(i as f32 / count as f32 * 360.0);
            }
        }
    }
}
