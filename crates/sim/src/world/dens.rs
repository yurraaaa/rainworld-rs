//! Going into and out of dens with what is stuck to a creature: `AbstractRoom
//! .MoveEntityToDen`/`MoveEntityOutOfDen` (304148..), `AbstractPhysicalObject
//! .IsEnteringDen`/`IsExitingDen` (303528..: whatever is stuck comes along) and
//! `AbstractCreature.IsEnteringDen` (301221: carried prey is killed, then dropped as the
//! AI's `DoIwantToDropThisItemInDen` says).

use rw_assets::CreatureType;
use rw_random::UnityRandom;

use super::{RealizedRoom, World};
use crate::creature::{CreatureId, WorldCoordinate};
use crate::item::Entity;

impl World {
    /// `AbstractRoom.MoveEntityToDen` of creature `id` at its position.
    pub(crate) fn move_entity_to_den(&mut self, id: CreatureId, rng: &mut UnityRandom) {
        let room = self.creature(id).pos.room;
        self.room_move_entity_to_den(room, Entity::Creature(id), rng);
    }

    /// `MoveEntityToDen` on room `room`: `IsEnteringDen(pos)`, then out of the room's
    /// entities into its den list. An object goes in no den list (the port's lists
    /// hold creatures; the game reads only creatures from them).
    fn room_move_entity_to_den(&mut self, room: i32, e: Entity, rng: &mut UnityRandom) {
        let den = self.entity_pos(e);
        match e {
            Entity::Creature(id) => self.creature_is_entering_den(id, den, rng),
            Entity::Object(_) => self.physical_is_entering_den(e, den, rng),
        }
        let Some(l) = self.local(room) else { return };
        let lists = &mut self.lists[l];
        match e {
            Entity::Creature(id) => {
                lists.remove_creature(id);
                if !lists.in_dens.contains(&id) {
                    lists.in_dens.push(id);
                }
            }
            Entity::Object(_) => lists.remove_entity(e),
        }
    }

    /// `AbstractRoom.MoveEntityOutOfDen` of `e` in its room: `IsExitingDen`, then back
    /// among the room's entities.
    pub(crate) fn move_entity_out_of_den(&mut self, e: Entity, rng: &mut UnityRandom) {
        self.physical_is_exiting_den(e, rng);
        if let Entity::Creature(id) = e {
            // `AbstractCreature.IsExitingDen`.
            let c = self.creature_mut(id);
            c.time_spent_here = 0;
            let pos = c.pos;
            if self
                .realized(pos.room)
                .is_some_and(RealizedRoom::shortcuts_ready)
            {
                // `Realize` keeps a body it already has (one that went in realized).
                if self.realize_lizard(id, rng) || self.creature(id).lizard().is_some() {
                    if let Some(l) = self.creature_mut(id).lizard_mut() {
                        l.in_shortcut = true;
                    }
                    self.entering.push((id, pos));
                } else if !self.creature(id).is_realized() {
                    log::debug!(
                        "{} leaves its den into a realized room; it stays abstract",
                        self.creature(id).kind()
                    );
                }
            }
            if let Some(l) = self.local(pos.room) {
                self.lists[l].in_dens.retain(|&c| c != id);
            }
            self.add_entity(id);
        } else if let Entity::Object(id) = e {
            self.add_object_entity(id);
        }
    }

    /// The ends of `e`'s `stuckObjects`: its grip sticks, then its spear sticks (the
    /// game keeps one list in creation order; the port keeps the two apart).
    fn stuck_ends(&self, e: Entity) -> Vec<(Entity, Entity)> {
        let mut ends: Vec<(Entity, Entity)> = self
            .sticks_of(e)
            .iter()
            .map(|s| (Entity::Creature(s.a), s.b))
            .collect();
        ends.extend(
            self.spear_sticks_of(e)
                .iter()
                .map(|s| (Entity::Object(s.spear), Entity::Creature(s.creature))),
        );
        ends
    }

    fn set_in_den(&mut self, e: Entity, in_den: bool) {
        match e {
            Entity::Creature(id) => self.creature_mut(id).in_den = in_den,
            Entity::Object(id) => self.object_mut(id).in_den = in_den,
        }
    }

    /// `Move(den)` of a stuck object (an abstract creature's `Move` without
    /// `ChangeRooms`' realization).
    fn move_entity(&mut self, e: Entity, to: WorldCoordinate) {
        match e {
            Entity::Creature(id) => self.move_creature(id, to),
            Entity::Object(id) => self.move_object(id, to),
        }
    }

    /// `AbstractPhysicalObject.IsEnteringDen`: to the den, `InDen`, then each stuck
    /// object's ends (from the last stick) to the den and into it.
    fn physical_is_entering_den(&mut self, e: Entity, den: WorldCoordinate, rng: &mut UnityRandom) {
        if !den.same_room_and_node(self.entity_pos(e)) {
            self.move_entity(e, den);
        }
        self.set_in_den(e, true);
        let count = self.stuck_ends(e).len();
        for num in (0..count).rev() {
            let Some(&(a, b)) = self.stuck_ends(e).get(num) else {
                continue;
            };
            for end in [a, b] {
                if !den.same_room_and_node(self.entity_pos(end)) {
                    self.move_entity(end, den);
                }
            }
            for end in [a, b] {
                if !self.entity_in_den(end) {
                    self.room_move_entity_to_den(den.room, end, rng);
                }
            }
        }
    }

    /// `AbstractCreature.IsEnteringDen`: a quantified fly goes back to the swarm
    /// (`RespawnOneFly`, out of its room); otherwise the base, then for each grip stick
    /// it holds (from the last): a held creature dies (the realized one too; a creature
    /// that would leave soon then waits at least 200 ticks), and what the AI wants to
    /// drop (all without an AI) is let go. A night or `ignoreCycle` creature staying
    /// long is abstracted, which an abstract creature already is.
    fn creature_is_entering_den(
        &mut self,
        id: CreatureId,
        den: WorldCoordinate,
        rng: &mut UnityRandom,
    ) {
        let c = self.creature(id);
        if c.template.quantified && c.kind() == CreatureType::Fly {
            self.respawn_one_fly();
            self.remove_entity(id);
            return;
        }
        self.physical_is_entering_den(Entity::Creature(id), den, rng);
        let count = self.creature(id).sticks.len();
        for num in (0..count).rev() {
            let Some(&s) = self.creature(id).sticks.get(num) else {
                continue;
            };
            if s.a != id {
                continue;
            }
            if let Entity::Creature(prey) = s.b {
                // The realized AI's `preyTracker.ForgetPrey`: no realized AI here.
                self.abstract_die(prey, rng);
                if self.creature(prey).is_realized() {
                    self.creature_die(prey, rng);
                }
                let wants_to_stay = self.wants_to_stay_in_den(id);
                let c = self.creature_mut(id);
                if c.remain_in_den_counter > -1 && c.remain_in_den_counter < 200 && !wants_to_stay {
                    c.remain_in_den_counter = 200;
                }
            }
            let Some(&s) = self.creature(id).sticks.get(num) else {
                continue;
            };
            let drop = self
                .creature(id)
                .ai
                .as_ref()
                .is_none_or(|ai| ai.wants_to_drop_in_den(matches!(s.b, Entity::Creature(_))));
            if drop {
                self.drop_carried_object(id, s.grasp);
            }
        }
    }

    /// `AbstractPhysicalObject.IsExitingDen`: not `InDen`, and each stuck object's ends
    /// still in the den come out with it.
    fn physical_is_exiting_den(&mut self, e: Entity, rng: &mut UnityRandom) {
        self.set_in_den(e, false);
        let count = self.stuck_ends(e).len();
        for i in 0..count {
            let Some(&(a, b)) = self.stuck_ends(e).get(i) else {
                continue;
            };
            for end in [a, b] {
                if self.entity_in_den(end) {
                    self.move_entity_out_of_den(end, rng);
                }
            }
        }
    }
}
