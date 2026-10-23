//! `CoralNeuronSystem`: a room's manager of the superstructure's coral (neurons, stems,
//! circuits, wall mycelia) and the mycelia that pair across them: the wind they share, its
//! age, and whether the room has been out of sight long enough for them to freeze. The
//! registry its owners' strands join is the [`crate::MyceliumPool`]'s, by room. What it
//! builds from the room's placed objects is `game_coral.rs`'. Behaviour:
//! `rainworld-re/notes/superstructure-coral.md`.
//!
//! Mirrors: CoralBrain/CoralNeuronSystem.cs (constructor, Update, Frozen)

use rw_random::UnityRandom;

use crate::Vec2;
use crate::mycelium::{SystemId, SystemState};
use crate::wall_mycelia::WallMycelia;

/// The `CoralNeuronSystem` in a room's update list.
#[derive(Debug, Clone, PartialEq)]
pub struct CoralNeuronSystem {
    /// Its place in the room's update list.
    pub update_order: u64,
    /// Its identity for the strands' registry.
    pub id: SystemId,
    pub wind: Vec2,
    pub age: i32,
    pub not_viewed_counter: i32,
    /// `AIMapReady` has built the owners.
    pub built: bool,
    /// `wallMycelia`.
    pub wall_mycelia: Vec<WallMycelia>,
}

impl CoralNeuronSystem {
    /// The constructor: `wind = RNV() * value` (two draws).
    pub fn new(update_order: u64, id: SystemId, rng: &mut UnityRandom) -> Self {
        let direction = Vec2::random_unit(rng);
        Self {
            update_order,
            id,
            wind: direction * rng.value(),
            age: 0,
            not_viewed_counter: 0,
            built: false,
            wall_mycelia: Vec::new(),
        }
    }

    /// What a strand reads of it.
    #[must_use]
    pub fn state(&self) -> SystemState {
        SystemState {
            id: self.id,
            frozen: self.frozen(),
        }
    }

    /// `Frozen`: its owners stop updating once it is older than 300 ticks and has not been
    /// viewed for more than 40.
    #[must_use]
    pub fn frozen(&self) -> bool {
        self.age > 300 && self.not_viewed_counter > 40
    }

    /// `Update`: the wind drifts (two draws), `age` counts, `notViewedCounter` resets
    /// while a camera shows the room.
    pub fn update(&mut self, being_viewed: bool, rng: &mut UnityRandom) {
        let direction = Vec2::random_unit(rng);
        self.wind += direction * (0.2 * rng.value());
        self.wind = self.wind.clamp_magnitude(1.0);
        self.age += 1;
        if being_viewed {
            self.not_viewed_counter = 0;
        } else {
            self.not_viewed_counter += 1;
        }
    }
}

#[cfg(test)]
#[path = "coral_system_tests.rs"]
mod tests;
