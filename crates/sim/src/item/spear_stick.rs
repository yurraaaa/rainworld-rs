//! A spear's sticks into creatures (`AbstractPhysicalObject.ImpaledOnSpearStick` and
//! `AbstractSpearStick`, `A` the spear, `B` the creature): both ends list them among
//! their `stuckObjects`, and they carry the creature with the spear (and the spear with
//! the creature) through shortcuts. The world keeps them beside the grip sticks.
//!
//! Mirrors: AbstractPhysicalObject.cs (AbstractSpearStick, ImpaledOnSpearStick)

use super::ObjectId;
use crate::creature::CreatureId;
use crate::item::Entity;
use crate::world::World;
use crate::{FloatExt, Vec2};

/// What kind of spear stick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpearStickKind {
    /// `ImpaledOnSpearStick`: a small creature skewered at `on_spear_position` (0 to
    /// 4, from the tip back), held on the shaft by the spear's update.
    Impaled {
        chunk: usize,
        on_spear_position: i32,
    },
    /// `AbstractSpearStick`: the spear lodged in the creature's `chunk`, turned `angle`
    /// degrees from the chunk's rotation (`bodyPart` -1: no body part).
    Lodged {
        chunk: usize,
        body_part: i32,
        angle: f32,
    },
}

/// One spear stick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpearStick {
    pub serial: u64,
    pub spear: ObjectId,
    pub creature: CreatureId,
    pub kind: SpearStickKind,
}

impl World {
    /// A new spear stick (the stick's constructor adds it to both ends).
    pub(crate) fn add_spear_stick(
        &mut self,
        spear: ObjectId,
        creature: CreatureId,
        kind: SpearStickKind,
    ) {
        self.grasp_serial += 1;
        let serial = self.grasp_serial;
        self.spear_sticks.push(SpearStick {
            serial,
            spear,
            creature,
            kind,
        });
    }

    /// The spear sticks with `e` at one end, oldest first.
    #[must_use]
    pub fn spear_sticks_of(&self, e: Entity) -> Vec<SpearStick> {
        self.spear_sticks
            .iter()
            .filter(|s| match e {
                Entity::Object(id) => s.spear == id,
                Entity::Creature(id) => s.creature == id,
            })
            .copied()
            .collect()
    }

    /// `PulledOutOfStuckObject` after a `ChangeMode` out of `StuckInCreature`: the
    /// spear's first lodged stick goes, and its sound waits for the game.
    pub(crate) fn pull_out_of_stuck_object(&mut self, spear: ObjectId) {
        let Some(w) = self.object_mut(spear).realized.as_deref_mut() else {
            return;
        };
        let (room, pos) = (w.room, w.chunk().pos);
        let Some(s) = w.spear_mut().filter(|s| s.pulled_out) else {
            return;
        };
        s.pulled_out = false;
        self.dislodged.push((room, pos));
        let lodged = self
            .spear_sticks_of(Entity::Object(spear))
            .into_iter()
            .find(|s| matches!(s.kind, SpearStickKind::Lodged { .. }));
        if let Some(s) = lodged {
            self.deactivate_spear_stick(s.serial);
        }
    }

    /// `AbstractObjectStick.Deactivate` of a spear stick.
    pub(crate) fn deactivate_spear_stick(&mut self, serial: u64) {
        self.spear_sticks.retain(|s| s.serial != serial);
    }

    /// The other ends of `e`'s spear sticks, for `GetAllConnectedObjects`.
    pub(crate) fn spear_stick_partners(&self, e: Entity) -> Vec<Entity> {
        self.spear_sticks_of(e)
            .iter()
            .map(|s| match e {
                Entity::Object(_) => Entity::Creature(s.creature),
                Entity::Creature(_) => Entity::Object(s.spear),
            })
            .collect()
    }

    /// `TryImpaleSmallCreature` (Remix's vanilla-exploit rule off): with at most five
    /// creatures on the spear and the first free place below 5, `creature` is impaled
    /// there.
    pub(crate) fn try_impale_small_creature(&mut self, spear: ObjectId, creature: CreatureId) {
        let mut count = 0;
        let mut place = 0;
        for s in self.spear_sticks_of(Entity::Object(spear)) {
            if let SpearStickKind::Impaled {
                on_spear_position, ..
            } = s.kind
            {
                if on_spear_position == place {
                    place += 1;
                }
                count += 1;
            }
        }
        if count <= 5 && place < 5 {
            self.add_spear_stick(
                spear,
                creature,
                SpearStickKind::Impaled {
                    chunk: 0,
                    on_spear_position: place,
                },
            );
        }
    }

    /// The end of `Spear.Update` for spear `id` in room `index`: each creature impaled
    /// on it that is held or slated comes off; one in the spear's room is held still on
    /// the shaft, 15 px ahead of the middle at place 0 to 15 px behind at place 4.
    pub(crate) fn hold_impaled(&mut self, index: i32, id: ObjectId, eu: bool) {
        let Some(w) = self.realized_weapon(id, index) else {
            return;
        };
        let (pos, rotation) = (w.chunk().pos, w.rotation);
        for s in self.spear_sticks_of(Entity::Object(id)).iter().rev() {
            let SpearStickKind::Impaled {
                on_spear_position, ..
            } = s.kind
            else {
                continue;
            };
            let c = self.creature(s.creature);
            let Some(f) = c.fly() else { continue };
            if f.slated || !c.grabbed_by.is_empty() {
                self.deactivate_spear_stick(s.serial);
            } else if f.room == Some(index) {
                let to =
                    pos + rotation * (on_spear_position as f32).lerp_map(0.0, 4.0, 15.0, -15.0);
                let f = self.fly_at(s.creature);
                let even = f.even_update;
                let chunk = f.chunk_mut();
                chunk.move_from_outside_my_update(even, eu, to);
                chunk.vel = Vec2::ZERO;
            }
        }
    }
}
