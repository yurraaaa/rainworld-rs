//! `Creature.Grasp`, the `CreatureGripStick` it leaves on both abstract creatures, and the
//! grab and release calls (`Creature.Grab`, `Grasp.Release`, `ReleaseGrasp`,
//! `LoseAllGrasps`, `AbstractCreature.DropCarriedObject`, `GetAllConnectedObjects`).
//!
//! A realized creature's `grasps` and `grabbedBy` are kept on its `AbstractCreature`
//! (valid while it is realized) so that any creature's grab can reach both ends.
//! Only creatures grab and are grabbed so far.

use super::{CreatureId, RealizedCreature};
use crate::world::World;

/// `Creature.Grasp.Shareability`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shareability {
    NonExclusive,
    CanOnlyShareWithNonExclusive,
    CanNotShare,
}

impl Shareability {
    /// The game's spelling.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::NonExclusive => "NonExclusive",
            Self::CanOnlyShareWithNonExclusive => "CanOnlyShareWithNonExclusive",
            Self::CanNotShare => "CanNotShare",
        }
    }

    #[must_use]
    pub fn from_value(value: &str) -> Option<Self> {
        [
            Self::NonExclusive,
            Self::CanOnlyShareWithNonExclusive,
            Self::CanNotShare,
        ]
        .into_iter()
        .find(|s| s.value() == value)
    }

    /// `Grasp.ShareabilityConflict(other)` of a grasp with this shareability.
    #[must_use]
    pub fn conflicts(self, other: Self) -> bool {
        if self == Self::CanNotShare || other == Self::CanNotShare {
            return true;
        }
        if self == Self::CanOnlyShareWithNonExclusive && other != Self::NonExclusive {
            return true;
        }
        other == Self::CanOnlyShareWithNonExclusive && self != Self::NonExclusive
    }
}

/// `Creature.Grasp`; `serial` tells grasps apart (the game compares references).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grasp {
    pub serial: u64,
    pub grabber: CreatureId,
    pub grabbed: CreatureId,
    pub grasp_used: usize,
    pub chunk_grabbed: usize,
    pub shareability: Shareability,
    pub dominance: f32,
    pub pacifying: bool,
}

/// `AbstractPhysicalObject.CreatureGripStick`: `a` holds `b` with grasp `grasp`. Both
/// creatures list it (`stuckObjects`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GripStick {
    pub serial: u64,
    pub a: CreatureId,
    pub b: CreatureId,
    pub grasp: usize,
    pub carry: bool,
}

/// What [`World::grab`] is asked for (`Creature.Grab`'s arguments).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrabRequest {
    pub grabber: CreatureId,
    pub grabbed: CreatureId,
    pub grasp_used: usize,
    pub chunk_grabbed: usize,
    pub shareability: Shareability,
    pub dominance: f32,
    pub override_equally_dominant: bool,
    pub pacifying: bool,
}

impl World {
    fn next_grasp_serial(&mut self) -> u64 {
        self.grasp_serial += 1;
        self.grasp_serial
    }

    /// `TotalMass` of a realized creature (0 when not realized).
    fn total_mass(&self, id: CreatureId) -> f32 {
        match &self.creature(id).realized {
            Some(RealizedCreature::Fly(f)) => f.body.chunks.iter().map(|c| c.mass).sum(),
            Some(RealizedCreature::Player) => crate::slugcat::CHUNK_MASS * 2.0,
            _ => 0.0,
        }
    }

    /// `Creature.Grab`. Returns whether the grasp was made.
    pub fn grab(&mut self, r: GrabRequest) -> bool {
        let slots = self.creature(r.grabber).grasps.len();
        if slots == 0 || r.grasp_used > slots {
            return false;
        }
        let obj = self.creature(r.grabbed);
        if obj.slated_for_deletion || obj.realized_slated() {
            return false;
        }
        let carry_heavier = self.total_mass(r.grabbed) < self.total_mass(r.grabber);
        let same =
            self.creature(r.grabber).grasps[r.grasp_used].is_some_and(|g| g.grabbed == r.grabbed);
        if same {
            self.release_grasp(r.grabber, r.grasp_used);
            self.make_grasp(
                GrabRequest {
                    pacifying: true,
                    ..r
                },
                r.pacifying || carry_heavier,
            );
            return true;
        }
        let blocked = self.creature(r.grabbed).grabbed_by.iter().any(|g| {
            g.grabber == r.grabber
                || (g.shareability.conflicts(r.shareability)
                    && ((r.override_equally_dominant && g.dominance == r.dominance)
                        || g.dominance > r.dominance))
        });
        if blocked {
            return false;
        }
        let holders = self.creature(r.grabbed).grabbed_by.clone();
        for g in holders.iter().rev() {
            if g.shareability.conflicts(r.shareability) {
                self.release(*g);
            }
        }
        if self.creature(r.grabber).grasps[r.grasp_used].is_some() {
            self.release_grasp(r.grabber, r.grasp_used);
        }
        self.make_grasp(r, r.pacifying || carry_heavier);
        true
    }

    /// The new grasp, `obj.Grabbed` (`GoThroughFloors`, `grabbedBy.Add`) and the stick.
    fn make_grasp(&mut self, r: GrabRequest, carry: bool) {
        let grasp = Grasp {
            serial: self.next_grasp_serial(),
            grabber: r.grabber,
            grabbed: r.grabbed,
            grasp_used: r.grasp_used,
            chunk_grabbed: r.chunk_grabbed,
            shareability: r.shareability,
            dominance: r.dominance,
            pacifying: r.pacifying,
        };
        self.creature_mut(r.grabber).grasps[r.grasp_used] = Some(grasp);
        let obj = self.creature_mut(r.grabbed);
        if let Some(RealizedCreature::Fly(f)) = &mut obj.realized {
            f.body.set_go_through_floors(true);
        }
        obj.grabbed_by.push(grasp);
        let stick = GripStick {
            serial: self.next_grasp_serial(),
            a: r.grabber,
            b: r.grabbed,
            grasp: r.grasp_used,
            carry,
        };
        self.creature_mut(r.grabber).sticks.push(stick);
        self.creature_mut(r.grabbed).sticks.push(stick);
    }

    /// `Grasp.Release`: off the grabbed's `grabbedBy` and the grabber's slot, then
    /// `DropCarriedObject(graspUsed)`.
    pub fn release(&mut self, g: Grasp) {
        self.creature_mut(g.grabbed)
            .grabbed_by
            .retain(|o| o.serial != g.serial);
        if let Some(slot) = self.creature_mut(g.grabber).grasps.get_mut(g.grasp_used) {
            *slot = None;
        }
        self.drop_carried_object(g.grabber, g.grasp_used);
    }

    /// `Creature.ReleaseGrasp`.
    pub fn release_grasp(&mut self, id: CreatureId, grasp: usize) {
        if let Some(g) = self.creature(id).grasps.get(grasp).copied().flatten() {
            self.release(g);
        }
    }

    /// `Creature.LoseAllGrasps`.
    pub fn lose_all_grasps(&mut self, id: CreatureId) {
        for i in 0..self.creature(id).grasps.len() {
            self.release_grasp(id, i);
        }
    }

    /// `AbstractCreature.DropCarriedObject`: the sticks of that grasp deactivate, then the
    /// realized grasp (if still there) is released.
    pub fn drop_carried_object(&mut self, id: CreatureId, grasp: usize) {
        let sticks: Vec<GripStick> = self
            .creature(id)
            .sticks
            .iter()
            .rev()
            .filter(|s| s.a == id && s.grasp == grasp)
            .copied()
            .collect();
        for s in sticks {
            self.deactivate_stick(s);
        }
        if self
            .creature(id)
            .grasps
            .get(grasp)
            .copied()
            .flatten()
            .is_some()
        {
            self.release_grasp(id, grasp);
        }
    }

    /// `AbstractObjectStick.Deactivate`.
    pub(crate) fn deactivate_stick(&mut self, s: GripStick) {
        self.creature_mut(s.a)
            .sticks
            .retain(|o| o.serial != s.serial);
        self.creature_mut(s.b)
            .sticks
            .retain(|o| o.serial != s.serial);
    }

    /// `GetAllConnectedObjects`: the creature, then depth first through its sticks (A
    /// before B).
    #[must_use]
    pub fn connected_objects(&self, id: CreatureId) -> Vec<CreatureId> {
        let mut list = vec![id];
        self.add_connected(id, &mut list);
        list
    }

    fn add_connected(&self, id: CreatureId, list: &mut Vec<CreatureId>) {
        for s in &self.creature(id).sticks {
            for other in [s.a, s.b] {
                if !list.contains(&other) {
                    list.push(other);
                    self.add_connected(other, list);
                }
            }
        }
    }

    /// `PhysicalObject.Update`'s clean-up: grasps on it that their grabber no longer
    /// holds are dropped from `grabbedBy`.
    pub(crate) fn clean_grabbed_by(&mut self, id: CreatureId) {
        let held: Vec<Grasp> = self.creature(id).grabbed_by.clone();
        let keep: Vec<Grasp> = held
            .into_iter()
            .filter(|g| {
                self.creature(g.grabber)
                    .grasps
                    .get(g.grasp_used)
                    .copied()
                    .flatten()
                    .is_some_and(|o| o.serial == g.serial)
            })
            .collect();
        self.creature_mut(id).grabbed_by = keep;
    }
}
