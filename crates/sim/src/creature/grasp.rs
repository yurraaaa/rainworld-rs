//! `Creature.Grasp`, the `CreatureGripStick` it leaves on both abstract creatures, and the
//! grab and release calls (`Creature.Grab`, `Grasp.Release`, `ReleaseGrasp`,
//! `LoseAllGrasps`, `AbstractCreature.DropCarriedObject`, `GetAllConnectedObjects`).
//!
//! A realized creature's `grasps` and `grabbedBy` are kept on its `AbstractCreature`
//! (valid while it is realized), an object's `grabbedBy` and `stuckObjects` on its
//! `AbstractObject`, so that any grab can reach both ends. Creatures grab; creatures and
//! objects (rocks, spears) are grabbed.
//!
//! Mirrors: Creature.cs (Grasp, Grab, ReleaseGrasp, LoseAllGrasps), AbstractPhysicalObject.cs (CreatureGripStick,
//!   GetAllConnectedObjects), AbstractCreature.cs (DropCarriedObject)

use rw_random::UnityRandom;

use super::{CreatureId, RealizedCreature};
use crate::item::{Entity, WeaponMode};
use crate::world::{SpriteMove, World};

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
    pub grabbed: Entity,
    pub grasp_used: usize,
    pub chunk_grabbed: usize,
    pub shareability: Shareability,
    pub dominance: f32,
    pub pacifying: bool,
}

/// `AbstractPhysicalObject.CreatureGripStick`: `a` holds `b` with grasp `grasp`. Both
/// ends list it (`stuckObjects`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GripStick {
    pub serial: u64,
    pub a: CreatureId,
    pub b: Entity,
    pub grasp: usize,
    pub carry: bool,
}

/// What [`World::grab`] is asked for (`Creature.Grab`'s arguments).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrabRequest {
    pub grabber: CreatureId,
    pub grabbed: Entity,
    pub grasp_used: usize,
    pub chunk_grabbed: usize,
    pub shareability: Shareability,
    pub dominance: f32,
    pub override_equally_dominant: bool,
    pub pacifying: bool,
}

impl Grasp {
    /// The grabbed creature, if it is one.
    #[must_use]
    pub fn grabbed_creature(&self) -> Option<CreatureId> {
        self.grabbed.creature()
    }
}

impl World {
    fn next_grasp_serial(&mut self) -> u64 {
        self.grasp_serial += 1;
        self.grasp_serial
    }

    /// `grabbedBy` of a creature or an object.
    #[must_use]
    pub fn grabbed_by(&self, e: Entity) -> &[Grasp] {
        match e {
            Entity::Creature(id) => &self.creature(id).grabbed_by,
            Entity::Object(id) => &self.object(id).grabbed_by,
        }
    }

    fn grabbed_by_mut(&mut self, e: Entity) -> &mut Vec<Grasp> {
        match e {
            Entity::Creature(id) => &mut self.creature_mut(id).grabbed_by,
            Entity::Object(id) => &mut self.object_mut(id).grabbed_by,
        }
    }

    /// The grip sticks of a creature or an object (its `stuckObjects`).
    #[must_use]
    pub fn sticks_of(&self, e: Entity) -> &[GripStick] {
        match e {
            Entity::Creature(id) => &self.creature(id).sticks,
            Entity::Object(id) => &self.object(id).sticks,
        }
    }

    fn sticks_mut(&mut self, e: Entity) -> &mut Vec<GripStick> {
        match e {
            Entity::Creature(id) => &mut self.creature_mut(id).sticks,
            Entity::Object(id) => &mut self.object_mut(id).sticks,
        }
    }

    /// `TotalMass` of a realized creature or object (0 when not realized).
    #[must_use]
    pub fn total_mass(&self, e: Entity) -> f32 {
        match e {
            Entity::Creature(id) => match &self.creature(id).realized {
                Some(RealizedCreature::Fly(f)) => f.body.chunks.iter().map(|c| c.mass).sum(),
                Some(RealizedCreature::Lizard(l)) => l.total_mass(),
                Some(RealizedCreature::Player) => self.player_mass,
                _ => 0.0,
            },
            Entity::Object(id) => self
                .object(id)
                .realized
                .as_ref()
                .map_or(0.0, |w| w.body.chunks.iter().map(|c| c.mass).sum()),
        }
    }

    /// `PhysicalObject.gravity` of a realized lizard, fly or weapon (0 otherwise).
    #[must_use]
    pub fn object_gravity(&self, e: Entity) -> f32 {
        match e {
            Entity::Creature(id) => match &self.creature(id).realized {
                Some(RealizedCreature::Fly(f)) => f.body.params.gravity(),
                Some(RealizedCreature::Lizard(l)) => l.body.params.gravity(),
                _ => 0.0,
            },
            Entity::Object(id) => self
                .object(id)
                .realized
                .as_ref()
                .map_or(0.0, |w| w.body.params.gravity()),
        }
    }

    /// `bodyChunkConnections` of a realized lizard (none for the others ported).
    #[must_use]
    pub fn chunk_connections(&self, e: Entity) -> &[crate::physics::object::BodyChunkConnection] {
        match e {
            Entity::Creature(id) => match &self.creature(id).realized {
                Some(RealizedCreature::Lizard(l)) => &l.body.connections,
                _ => &[],
            },
            Entity::Object(_) => &[],
        }
    }

    /// The grabbed object is slated for deletion (or, an item, not realized).
    fn grab_refused(&self, e: Entity) -> bool {
        match e {
            Entity::Creature(id) => {
                let obj = self.creature(id);
                obj.slated_for_deletion || obj.realized_slated()
            }
            Entity::Object(id) => self.object(id).realized.is_none(),
        }
    }

    /// `Creature.Grab`. Returns whether the grasp was made. `rng` is for the object's
    /// `Grabbed` (a weapon's `ChangeMode(Carried)`, which draws nothing).
    pub fn grab(&mut self, r: GrabRequest, rng: &mut UnityRandom) -> bool {
        let slots = self.creature(r.grabber).grasps.len();
        if slots == 0 || r.grasp_used > slots {
            return false;
        }
        if self.grab_refused(r.grabbed) {
            return false;
        }
        let carry_heavier =
            self.total_mass(r.grabbed) < self.total_mass(Entity::Creature(r.grabber));
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
                rng,
            );
            return true;
        }
        let blocked = self.grabbed_by(r.grabbed).iter().any(|g| {
            g.grabber == r.grabber
                || (g.shareability.conflicts(r.shareability)
                    && ((r.override_equally_dominant && g.dominance == r.dominance)
                        || g.dominance > r.dominance))
        });
        if blocked {
            return false;
        }
        let holders = self.grabbed_by(r.grabbed).to_vec();
        for g in holders.iter().rev() {
            if g.shareability.conflicts(r.shareability) {
                self.release(*g);
            }
        }
        if self.creature(r.grabber).grasps[r.grasp_used].is_some() {
            self.release_grasp(r.grabber, r.grasp_used);
        }
        self.make_grasp(r, r.pacifying || carry_heavier, rng);
        true
    }

    /// The new grasp, `obj.Grabbed` (a weapon's `ChangeMode(Carried)`, then
    /// `GoThroughFloors`, `grabbedBy.Add`; the player's own `Grabbed` is the game's,
    /// `RainWorldGame::creature_grab`) and the stick.
    fn make_grasp(&mut self, r: GrabRequest, carry: bool, rng: &mut UnityRandom) {
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
        match r.grabbed {
            Entity::Creature(id) => match &mut self.creature_mut(id).realized {
                Some(RealizedCreature::Fly(f)) => f.body.set_go_through_floors(true),
                Some(RealizedCreature::Lizard(l)) => l.body.set_go_through_floors(true),
                _ => {}
            },
            Entity::Object(id) => {
                self.change_weapon_mode(id, WeaponMode::Carried, rng);
                if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
                    w.body.set_go_through_floors(true);
                }
            }
        }
        self.grabbed_by_mut(r.grabbed).push(grasp);
        self.sync_held(r.grabbed);
        let stick = GripStick {
            serial: self.next_grasp_serial(),
            a: r.grabber,
            b: r.grabbed,
            grasp: r.grasp_used,
            carry,
        };
        self.creature_mut(r.grabber).sticks.push(stick);
        self.sticks_mut(r.grabbed).push(stick);
    }

    /// `new CreatureGripStick(a, b, grasp, carry)` between abstract creatures, with no
    /// realized grasp.
    pub(crate) fn add_grip_stick(&mut self, a: CreatureId, b: Entity, grasp: usize, carry: bool) {
        let stick = GripStick {
            serial: self.next_grasp_serial(),
            a,
            b,
            grasp,
            carry,
        };
        self.creature_mut(a).sticks.push(stick);
        self.sticks_mut(b).push(stick);
    }

    /// An object's realized body is `held` while something grasps it.
    fn sync_held(&mut self, e: Entity) {
        if let Entity::Object(id) = e {
            let held = !self.object(id).grabbed_by.is_empty();
            if let Some(w) = self.object_mut(id).realized.as_deref_mut() {
                w.body.held = held;
            }
        }
    }

    /// A realized lizard's graphics link, if its graphics module exists.
    fn lizard_graphics_link(
        &mut self,
        id: CreatureId,
    ) -> Option<&mut crate::creature::lizard::LizardGraphicsLink> {
        self.creature_mut(id).lizard_mut()?.graphics.as_mut()
    }

    /// A lizard's `graphicsModule.ReleaseAllInternallyContainedSprites()`, if it has one.
    fn release_all_internally_contained(&mut self, id: CreatureId) {
        if let Some(link) = self.lizard_graphics_link(id) {
            let released = link.release_all_internally_contained();
            self.sprites_to_default(released);
        }
    }

    /// `MoveObjectToContainer(obj, null)` of each of `released`, in order.
    fn sprites_to_default(&mut self, released: Vec<Entity>) {
        let seq = self.update_seq();
        self.sprite_moves
            .extend(released.into_iter().map(|obj| SpriteMove {
                seq,
                obj,
                into: None,
            }));
    }

    /// The internal-container moves since the last call, in order.
    pub fn take_sprite_moves(&mut self) -> Vec<SpriteMove> {
        std::mem::take(&mut self.sprite_moves)
    }

    /// `Grasp.Release`: the grabber's graphics let the object out of its container
    /// (`ReleaseSpecificInternallyContainedObjectSprites`), then off the grabbed's
    /// `grabbedBy` and the grabber's slot, then `DropCarriedObject(graspUsed)`.
    pub fn release(&mut self, g: Grasp) {
        if let Some(link) = self.lizard_graphics_link(g.grabber) {
            let released = link.release_internally_contained(g.grabbed);
            self.sprites_to_default(released);
        }
        self.grabbed_by_mut(g.grabbed)
            .retain(|o| o.serial != g.serial);
        self.sync_held(g.grabbed);
        if let Some(slot) = self.creature_mut(g.grabber).grasps.get_mut(g.grasp_used) {
            *slot = None;
        }
        self.drop_carried_object(g.grabber, g.grasp_used);
    }

    /// `Creature.ReleaseGrasp` (a lizard's graphics first let all it holds out of its
    /// jaws, `Lizard.ReleaseGrasp`).
    pub fn release_grasp(&mut self, id: CreatureId, grasp: usize) {
        self.release_all_internally_contained(id);
        if let Some(g) = self.creature(id).grasps.get(grasp).copied().flatten() {
            self.release(g);
        }
    }

    /// `Creature.LoseAllGrasps` (`Lizard.LoseAllGrasps` first lets all its jaws hold
    /// out of its graphics' container).
    pub fn lose_all_grasps(&mut self, id: CreatureId) {
        self.release_all_internally_contained(id);
        for i in 0..self.creature(id).grasps.len() {
            self.release_grasp(id, i);
        }
    }

    /// `Creature.SwitchGrasps(from, to)` with `UpdateGraspIndexes`: the two hands swap
    /// what they hold.
    pub fn switch_grasps(&mut self, id: CreatureId, from: usize, to: usize) {
        let c = self.creature_mut(id);
        c.grasps.swap(from, to);
        let grasps: Vec<Grasp> = c
            .grasps
            .iter_mut()
            .enumerate()
            .filter_map(|(i, g)| {
                let g = g.as_mut()?;
                g.grasp_used = i;
                Some(*g)
            })
            .collect();
        for g in &grasps {
            for held in self.grabbed_by_mut(g.grabbed) {
                if held.serial == g.serial {
                    held.grasp_used = g.grasp_used;
                }
            }
        }
        let sticks = self.creature(id).sticks.clone();
        for s in sticks.iter().filter(|s| s.a == id) {
            let Some(k) = grasps
                .iter()
                .find(|g| g.grabbed == s.b)
                .map(|g| g.grasp_used)
            else {
                continue;
            };
            for o in [Entity::Creature(s.a), s.b] {
                for t in self.sticks_mut(o) {
                    if t.serial == s.serial {
                        t.grasp = k;
                    }
                }
            }
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
        self.sticks_mut(s.b).retain(|o| o.serial != s.serial);
    }

    /// `GetAllConnectedObjects`: the object, then depth first through its sticks (A
    /// before B).
    #[must_use]
    pub fn connected_objects(&self, e: Entity) -> Vec<Entity> {
        let mut list = vec![e];
        self.add_connected(e, &mut list);
        list
    }

    fn add_connected(&self, e: Entity, list: &mut Vec<Entity>) {
        for (a, b) in self.stuck_objects(e) {
            for other in [a, b] {
                if !list.contains(&other) {
                    list.push(other);
                    self.add_connected(other, list);
                }
            }
        }
    }

    /// `stuckObjects` of `e` as (A, B) pairs in creation order: its grip, spear and
    /// on-back sticks, which the port keeps in three lists sharing one serial counter.
    #[must_use]
    pub fn stuck_objects(&self, e: Entity) -> Vec<(Entity, Entity)> {
        let mut sticks: Vec<(u64, Entity, Entity)> = self
            .sticks_of(e)
            .iter()
            .map(|s| (s.serial, Entity::Creature(s.a), s.b))
            .collect();
        sticks.extend(self.spear_sticks_of(e).iter().map(|s| {
            (
                s.serial,
                Entity::Object(s.spear),
                Entity::Creature(s.creature),
            )
        }));
        sticks.extend(
            self.on_back_sticks()
                .iter()
                .filter(|s| e == Entity::Creature(s.player) || e == Entity::Object(s.spear))
                .map(|s| {
                    (
                        s.serial,
                        Entity::Creature(s.player),
                        Entity::Object(s.spear),
                    )
                }),
        );
        sticks.sort_unstable_by_key(|&(serial, _, _)| serial);
        sticks.into_iter().map(|(_, a, b)| (a, b)).collect()
    }

    /// `PhysicalObject.Update`'s clean-up: grasps on it that their grabber no longer
    /// holds are dropped from `grabbedBy`.
    pub(crate) fn clean_grabbed_by(&mut self, e: Entity) {
        let held: Vec<Grasp> = self.grabbed_by(e).to_vec();
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
        *self.grabbed_by_mut(e) = keep;
        self.sync_held(e);
    }
}
