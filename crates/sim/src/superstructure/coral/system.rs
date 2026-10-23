//! `CoralNeuronSystem`: a room's manager of the superstructure's coral (neurons, stems,
//! circuits, wall mycelia) and the mycelia that pair across them: the wind they share, its
//! age, and whether the room has been out of sight long enough for them to freeze. The
//! registry its owners' strands join is the [`crate::MyceliumPool`]'s, by room. What it
//! builds from the room's placed objects is `superstructure/coral/turn.rs`'. Behaviour:
//! `rainworld-re/notes/superstructure-coral.md`.
//!
//! Mirrors: CoralBrain/CoralNeuronSystem.cs (constructor, Update, Frozen, neurons, circuits)

use rw_random::UnityRandom;

use crate::Vec2;
use crate::superstructure::coral::circuit::CoralCircuit;
use crate::superstructure::coral::mycelium::{SystemId, SystemState};
use crate::superstructure::coral::neuron::CoralNeuron;
use crate::superstructure::coral::stem::CoralStem;
use crate::superstructure::coral::wall_mycelia::WallMycelia;
use crate::superstructure::projections::projected_circle::CircleOwner;

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
    /// `neurons`, in the order `AIMapReady` made them (stems add theirs).
    pub neurons: Vec<CoralNeuron>,
    /// `stems`, in the order `AIMapReady` made them.
    pub stems: Vec<CoralStem>,
    /// `wallMycelia`.
    pub wall_mycelia: Vec<WallMycelia>,
    /// `circuits`, in the order `AIMapReady` made them.
    pub circuits: Vec<CoralCircuit>,
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
            neurons: Vec::new(),
            stems: Vec::new(),
            wall_mycelia: Vec::new(),
            circuits: Vec::new(),
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

    /// `owner.CanHostCircle()` of a circle's owner.
    #[must_use]
    pub fn circle_owner_can_host(&self, owner: CircleOwner) -> bool {
        match owner {
            CircleOwner::Neuron { order } => self
                .neurons
                .iter()
                .find(|n| n.update_order == order)
                .is_some_and(CoralNeuron::can_host_circle),
            CircleOwner::Stem { stem } => self.stems.get(stem as usize).is_some(),
        }
    }

    /// `owner.CircleCenter(index, timeStacker)`.
    #[must_use]
    pub fn circle_center(
        &self,
        owner: CircleOwner,
        index: usize,
        time_stacker: f32,
    ) -> Option<Vec2> {
        match owner {
            CircleOwner::Neuron { order } => self
                .neurons
                .iter()
                .find(|n| n.update_order == order)
                .map(|n| n.geometry.circle_center(index, time_stacker)),
            CircleOwner::Stem { stem } => self
                .stems
                .get(stem as usize)
                .and_then(|s| s.circle_center(time_stacker)),
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
#[path = "tests/system.rs"]
mod tests;
