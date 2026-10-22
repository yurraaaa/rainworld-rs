//! The Hunter's `AbstractOnBackStick` (`A` the player, `B` the spear on its back): both
//! ends list it among their `stuckObjects`, so the spear goes where the player goes
//! (shortcuts, `AbstractPhysicalObject.Move`). The world keeps them beside the grip and
//! spear sticks.
//!
//! Mirrors: Player.cs (AbstractOnBackStick)

use super::ObjectId;
use crate::creature::CreatureId;
use crate::item::Entity;
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnBackStick {
    pub serial: u64,
    pub player: CreatureId,
    pub spear: ObjectId,
}

impl World {
    /// A new on-back stick (the constructor adds it to both ends); returns its serial.
    pub(crate) fn add_on_back_stick(&mut self, player: CreatureId, spear: ObjectId) -> u64 {
        self.grasp_serial += 1;
        let serial = self.grasp_serial;
        self.on_back_sticks.push(OnBackStick {
            serial,
            player,
            spear,
        });
        serial
    }

    /// `AbstractObjectStick.Deactivate` of an on-back stick.
    pub(crate) fn deactivate_on_back_stick(&mut self, serial: u64) {
        self.on_back_sticks.retain(|s| s.serial != serial);
    }

    #[must_use]
    pub fn on_back_sticks(&self) -> &[OnBackStick] {
        &self.on_back_sticks
    }

    /// The other ends of `e`'s on-back sticks, for `GetAllConnectedObjects`.
    pub(crate) fn on_back_partners(&self, e: Entity) -> Vec<Entity> {
        self.on_back_sticks
            .iter()
            .filter_map(|s| match e {
                Entity::Creature(id) if s.player == id => Some(Entity::Object(s.spear)),
                Entity::Object(id) if s.spear == id => Some(Entity::Creature(s.player)),
                _ => None,
            })
            .collect()
    }
}
