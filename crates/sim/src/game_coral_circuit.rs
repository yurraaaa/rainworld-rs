//! The game's side of the coral circuits: `CoralNeuronSystem.AIMapReady` grouping the placed
//! `CoralCircuit` objects into clusters and building one circuit per cluster, the turns of
//! the bits, connectors, crabs and circuits in the update list, and the circuit's push on
//! every nearby body chunk (layer 1 and up) with the player's collision.
//! Types: [`crate::coral_circuit`], [`crate::circuit_connector`], [`crate::circuit_crab`].
//!
//! The circuit joins the update list after its bits, connectors and crabs, so the list
//! (walked from its end) runs the circuit first, then the crabs, connectors and bits last
//! to first. `CoralCircuit.Update` is not gated by the system's `Frozen`.
//!
//! Mirrors: CoralBrain/CoralNeuronSystem.cs (AIMapReady: circuits), CoralBrain/CoralCircuit.cs (Update), CoralBrain/CircuitConnector.cs (Update), CoralBrain/CircuitCrab.cs (Update), Player.cs (CollideWithCoralCircuitBit)

use std::sync::Arc;

use super::{RainWorldGame, RoomObject};
use crate::world::World;
use crate::{
    BitCx, BitRef, CircuitBit, CircuitCx, CircuitPlace, CoralCircuit, MyceliumEnv, PushChunk,
    SystemId,
};

/// A circuit object in a room's update list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CircuitRef {
    /// A `CircuitBit`: circuit and bit indices.
    Bit(u32, u32),
    /// A `CircuitConnector`: circuit and connector indices.
    Connector(u32, u32),
    /// A `CircuitCrab`: circuit and crab indices.
    Crab(u32, u32),
    /// A `CoralCircuit`.
    Circuit(u32),
}

impl World {
    /// The bit `bit` names, if its room is realized.
    #[must_use]
    pub fn circuit_bit(&self, bit: BitRef) -> Option<&CircuitBit> {
        self.realized(bit.room)?
            .coral
            .as_ref()?
            .circuits
            .get(bit.circuit as usize)?
            .bits
            .get(bit.bit as usize)
    }
}

impl RainWorldGame {
    /// `new CoralCircuit(system, places)` and its `AddObject` in room `index`.
    pub(super) fn add_coral_circuit(
        &mut self,
        index: i32,
        system: SystemId,
        places: Vec<CircuitPlace>,
    ) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let (Some(ai_map), room) = (r.ai_map.clone(), Arc::clone(&r.room)) else {
            return;
        };
        let mut seq = self.world.update_seq();
        let circuit = CoralCircuit::build(
            places,
            &mut CircuitCx {
                room: &room,
                ai_map: &ai_map,
                system,
                pool: &mut self.world.mycelia,
                seq: &mut seq,
            },
            &mut self.rng,
        );
        self.world.set_update_seq(seq);
        if let Some(c) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.coral.as_mut())
        {
            c.circuits.push(circuit);
        }
    }

    /// Room `index`'s circuit objects in its update list.
    pub(super) fn room_circuit_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let Some(coral) = self.world.realized(index).and_then(|r| r.coral.as_ref()) else {
            return Vec::new();
        };
        let mut objects = Vec::new();
        for (c, circuit) in coral.circuits.iter().enumerate() {
            let c = c as u32;
            for (b, bit) in circuit.bits.iter().enumerate() {
                objects.push((
                    bit.update_order,
                    Self::circuit_object(CircuitRef::Bit(c, b as u32)),
                ));
            }
            for (k, x) in circuit.connectors.iter().enumerate() {
                let which = CircuitRef::Connector(c, k as u32);
                objects.push((x.update_order, Self::circuit_object(which)));
            }
            for (k, x) in circuit.crabs.iter().enumerate() {
                objects.push((
                    x.update_order,
                    Self::circuit_object(CircuitRef::Crab(c, k as u32)),
                ));
            }
            objects.push((
                circuit.update_order,
                Self::circuit_object(CircuitRef::Circuit(c)),
            ));
        }
        objects
    }

    fn circuit_object(which: CircuitRef) -> RoomObject {
        RoomObject::Circuit(which)
    }

    /// The `Update` of a circuit object of room `index`.
    pub(super) fn run_circuit_object(&mut self, index: i32, which: CircuitRef) {
        match which {
            CircuitRef::Bit(c, b) => self.run_circuit_bit(index, c as usize, b as usize),
            CircuitRef::Connector(c, k) => {
                self.run_circuit_connector(index, c as usize, k as usize)
            }
            CircuitRef::Crab(c, k) => {
                let Some(circuit) = self
                    .world
                    .realized_mut(index)
                    .and_then(|r| r.coral.as_mut())
                    .and_then(|s| s.circuits.get_mut(c as usize))
                else {
                    return;
                };
                let CoralCircuit { bits, crabs, .. } = circuit;
                if let Some(crab) = crabs.get_mut(k as usize) {
                    crab.update(bits, &mut self.rng);
                }
            }
            CircuitRef::Circuit(c) => self.run_circuit(index, c as usize),
        }
    }

    /// `CircuitBit.Update`.
    fn run_circuit_bit(&mut self, index: i32, circuit: usize, bit: usize) {
        let viewed = index == self.camera_room;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let (room, ai_map) = (Arc::clone(&r.room), r.ai_map.clone());
        let (Some(ai_map), Some(circuit)) = (
            ai_map,
            r.coral.as_mut().and_then(|s| s.circuits.get_mut(circuit)),
        ) else {
            return;
        };
        let sounds = if viewed {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = BitCx {
            room: &room,
            ai_map: &ai_map,
            sounds,
            rng: &mut self.rng,
        };
        circuit.update_bit(bit, &mut cx);
    }

    /// `CircuitConnector.Update`.
    fn run_circuit_connector(&mut self, index: i32, circuit: usize, connector: usize) {
        let camera = self.camera_view;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let (room, ai_map, tick) = (
            Arc::clone(&r.room),
            r.ai_map.clone(),
            r.cameras_changed_tick,
        );
        let Some(system) = r.coral.take() else {
            return;
        };
        let env = MyceliumEnv {
            room: &room,
            room_index: index,
            ai_map: ai_map.as_deref(),
            camera,
            cameras_changed_tick: tick,
            system: Some(system.state()),
        };
        if let Some(circuit) = system.circuits.get(circuit)
            && let Some(x) = circuit.connectors.get(connector)
        {
            x.update(&circuit.bits, &mut self.world.mycelia, &env, &mut self.rng);
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.coral = Some(system);
        }
        self.flush_mycelium_sparks(index);
    }

    /// `CoralCircuit.Update`: every chunk of every physical object of layer 1 and up is
    /// pushed off the bits near it, the player told of each bit it touched; then the next
    /// tick's chance of a link breaking.
    fn run_circuit(&mut self, index: i32, circuit: usize) {
        let player_here = self.player_here(index);
        let player_seq = player_here.then_some(self.player_seq);
        for (layer, _, owner) in self.world.physical_objects(index, player_seq) {
            if layer < 1 {
                continue;
            }
            for i in 0.. {
                let Some(chunk) = self.electric_chunk(owner, i) else {
                    break;
                };
                let (start, candidates) = {
                    let Some(c) = self.circuit_at(index, circuit) else {
                        return;
                    };
                    let strength = c.push_strength(chunk.pos);
                    if strength <= 0.0 {
                        continue;
                    }
                    (strength, c.push_candidates(&PushChunk::from(&chunk)))
                };
                for bit in candidates {
                    self.push_chunk_off_bit(index, circuit, bit, (owner, i), start);
                }
            }
        }
        if let Some(c) = self.circuit_at_mut(index, circuit) {
            c.finish_update();
        }
    }

    /// One bit's push on chunk `chunk.1` of `chunk.0`, and the player's collision.
    fn push_chunk_off_bit(
        &mut self,
        index: i32,
        circuit: usize,
        bit: usize,
        chunk: (crate::ChunkOwner, usize),
        strength: f32,
    ) {
        let (owner, i) = chunk;
        let Some(mut push) = self.electric_chunk(owner, i).map(|c| PushChunk::from(&c)) else {
            return;
        };
        let Some(overlap) = self
            .circuit_at_mut(index, circuit)
            .and_then(|c| c.push_bit(bit, &mut push, strength))
        else {
            return;
        };
        if let Some(c) = self.electric_chunk_mut(owner, i) {
            c.pos = push.pos;
            c.vel = push.vel;
        }
        if owner == crate::ChunkOwner::Player {
            self.player_collides_with_bit(index, circuit, bit, i, overlap);
        }
    }

    /// `Player.CollideWithCoralCircuitBit` and what it asks of the circuit.
    fn player_collides_with_bit(
        &mut self,
        index: i32,
        circuit: usize,
        bit: usize,
        chunk: usize,
        overlap: f32,
    ) {
        let bit_ref = BitRef {
            room: index,
            circuit: circuit as u32,
            bit: bit as u32,
        };
        let Some(pos) = self.world.circuit_bit(bit_ref).map(|b| b.pos) else {
            return;
        };
        self.refresh_player_swim_bits();
        let has_graphics = self.player_graphics;
        let Some(hit) = self
            .player
            .as_mut()
            .map(|p| p.collide_with_coral_circuit_bit(chunk, bit_ref, pos, overlap, has_graphics))
        else {
            return;
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        if let Some(c) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.coral.as_mut())
            .and_then(|s| s.circuits.get_mut(circuit))
        {
            if let Some(e) = hit.explosion {
                c.explosion(e.pos, e.rad, e.force, e.dir_vec, sounds, &mut self.rng);
            } else {
                c.bits[bit].vel -= hit.bit_kick;
            }
        }
        if hit.swim_sound {
            sounds.play_on_chunk(
                crate::SoundId::PlayerCoralCircuitSwim,
                0,
                false,
                1.0,
                1.0,
                &mut self.rng,
            );
        }
    }

    /// The positions the player's `swimBits` have now.
    pub(super) fn refresh_player_swim_bits(&mut self) {
        let Some(refs) = self.player.as_ref().map(|p| p.swim_bits()) else {
            return;
        };
        let positions = refs.map(|s| s.and_then(|s| self.world.circuit_bit(s.bit).map(|b| b.pos)));
        if let Some(p) = self.player.as_mut() {
            p.refresh_swim_bits(positions);
        }
    }

    fn circuit_at(&self, index: i32, circuit: usize) -> Option<&CoralCircuit> {
        self.world
            .realized(index)?
            .coral
            .as_ref()?
            .circuits
            .get(circuit)
    }

    fn circuit_at_mut(&mut self, index: i32, circuit: usize) -> Option<&mut CoralCircuit> {
        self.world
            .realized_mut(index)?
            .coral
            .as_mut()?
            .circuits
            .get_mut(circuit)
    }
}

#[cfg(test)]
#[path = "game_coral_circuit_tests.rs"]
mod tests;
