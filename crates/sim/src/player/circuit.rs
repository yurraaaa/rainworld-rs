//! The player's side of the coral circuit: `CollideWithCoralCircuitBit` (called by
//! `CoralCircuit.Update` for each bit a chunk pushes), which raises the swim resistance,
//! bumps the jump meter, kicks the bit or, after a jump, throws the circuit, and remembers
//! the bits it swims through (`swimBits`, one a side). `ZeroGSwim` reads the three
//! (`zero_g.rs`).
//!
//! Mirrors: Player.cs (CollideWithCoralCircuitBit, circuitSwimResistance, curcuitJumpMeter, swimBits)

use super::{HEAD, HIPS, Player};
use crate::math::float_ext::FloatExt;
use crate::{BitRef, Vec2};

/// A bit in `swimBits`, with where it was last seen (its position is read when the player
/// checks that it is still near).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwimBit {
    pub bit: BitRef,
    pub pos: Vec2,
}

/// The `Explosion` a collision asks of the bit's circuit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircuitExplosion {
    pub pos: Vec2,
    pub rad: f32,
    pub force: f32,
    pub dir_vec: Vec2,
}

/// What a collision does outside the player, in the game's order: the circuit's explosion
/// or the bit's kick, then the swim sound.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CircuitCollision {
    /// `bit.circuit.Explosion(...)`, after a jump (`curcuitJumpMeter` below 0).
    pub explosion: Option<CircuitExplosion>,
    /// Taken from the bit's velocity (`bit.vel -= ...`), when there is no explosion.
    pub bit_kick: Vec2,
    /// `Player_Coral_Circuit_Swim` on the main chunk.
    pub swim_sound: bool,
}

impl Player {
    /// `circuitSwimResistance`.
    #[must_use]
    pub fn circuit_swim_resistance(&self) -> f32 {
        self.circuit_swim_resistance
    }

    /// `curcuitJumpMeter`.
    #[must_use]
    pub fn circuit_jump_meter(&self) -> f32 {
        self.circuit_jump_meter
    }

    /// `swimBits`.
    #[must_use]
    pub fn swim_bits(&self) -> [Option<SwimBit>; 2] {
        self.swim_bits
    }

    /// Where the bits of `swimBits` are now, as the game's references see them (the player
    /// checks their distance in its own turn); `None` keeps a bit's last position.
    pub fn refresh_swim_bits(&mut self, positions: [Option<Vec2>; 2]) {
        for (slot, pos) in self.swim_bits.iter_mut().zip(positions) {
            if let (Some(slot), Some(pos)) = (slot, pos) {
                slot.pos = pos;
            }
        }
    }

    /// `CollideWithCoralCircuitBit(chunk, bit, overLapFac)` for a bit of `bit_ref` at
    /// `bit_pos`; `has_graphics` is `graphicsModule != null`. The `input[0].analogueDir` of
    /// a keyboard is the held direction clamped to length 1 (D34); with no direction held
    /// `DirVec(0, (0, 0))` is straight up.
    pub fn collide_with_coral_circuit_bit(
        &mut self,
        chunk: usize,
        bit_ref: BitRef,
        bit_pos: Vec2,
        overlap: f32,
        has_graphics: bool,
    ) -> CircuitCollision {
        self.circuit_swim_resistance = self.circuit_swim_resistance.max(overlap);
        let held = Vec2::new(self.input_x() as f32, self.input_y() as f32);
        let mut dir = held.clamp_magnitude(1.0);
        if dir.x == 0.0 && dir.y == 0.0 {
            dir = Vec2::ZERO.direction_to(held);
        }
        let (head, hips) = (self.body.chunks[HEAD].pos, self.body.chunks[HIPS].pos);
        let along = hips.direction_to(head);
        let mut result = CircuitCollision::default();
        if self.circuit_jump_meter < 0.0 {
            result.explosion = Some(CircuitExplosion {
                pos: hips,
                rad: 40.0,
                force: 12.0,
                dir_vec: dir.lerp(along, 0.5) * -6.0 + head.direction_to(bit_pos) * 6.0,
            });
        } else {
            result.bit_kick = dir.lerp(along, 0.3) * (3.0 * self.swim_cycle.inverse_lerp(1.5, 2.5));
        }
        if chunk == 0 && has_graphics && self.swim_bits.iter().flatten().all(|s| s.bit != bit_ref) {
            let side = if bit_pos.distance_to_line(head, hips) < 0.0 {
                0
            } else {
                1
            };
            let nearer =
                self.swim_bits[side].is_none_or(|s| bit_pos.distance(head) < s.pos.distance(head));
            if nearer {
                result.swim_sound = true;
                self.swim_bits[side] = Some(SwimBit {
                    bit: bit_ref,
                    pos: bit_pos,
                });
            }
        }
        self.circuit_jump_meter += 1.0;
        result
    }
}
