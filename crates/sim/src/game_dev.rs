//! Developer tools that add to the world through the game's own code paths.

use rw_assets::CreatureType;

use super::{OverseerCall, PlayerObserver, RainWorldGame};
use crate::Vec2;
use crate::creature::{CreatureId, CreatureTemplate, EntityId, LizardBreedParams, WorldCoordinate};
use crate::item::Entity;

impl RainWorldGame {
    /// A dev spawn as the game's dev tools make one: `new AbstractCreature(world,
    /// template, null, pos, GetNewID())` at `tile` of global room `room` (its
    /// personality, state and abstract AI with their draws), `AbstractRoom.AddEntity`,
    /// then `RealizeInRoom` when the room is ready for AI and the port has the type's
    /// body (lizards, flies, overseers). `seed_id` replaces the issued ID with
    /// `EntityID(-1, seed_id)` (the personality's seed), the issuer untouched. `Err` for
    /// a type that is not a spawnable creature or a room the world lacks.
    ///
    /// # Errors
    /// See above.
    pub fn dev_spawn_creature(
        &mut self,
        kind: CreatureType,
        room: i32,
        tile: (i32, i32),
        seed_id: Option<i32>,
        observer: &mut dyn PlayerObserver,
    ) -> Result<CreatureId, String> {
        if matches!(
            kind,
            CreatureType::Slugcat
                | CreatureType::StandardGroundCreature
                | CreatureType::LizardTemplate
        ) {
            return Err(format!("{} is not a spawnable creature", kind.value()));
        }
        if self.world.abstract_room(room).is_none() {
            return Err(format!("no room {room} in the world"));
        }
        let template = CreatureTemplate::of(kind, self.world.mods());
        let id = match seed_id {
            Some(n) => EntityId::new(-1, n),
            None => self.ids.new_id(-1),
        };
        let pos = WorldCoordinate::new(room, tile.0, tile.1, -1);
        let session = self.story_session();
        let c = self
            .world
            .new_creature(template, pos, id, &session, &mut self.ids, &mut self.rng);
        let cid = self.world.insert_creature(c);
        self.world.add_entity(cid);
        if self
            .world
            .realized(room)
            .is_some_and(crate::world::RealizedRoom::ready_for_ai)
        {
            self.dev_realize_in_room(cid, room, tile, observer);
        }
        Ok(cid)
    }

    /// `RealizeInRoom` of a dev-spawned creature, as `ReadyForAI` realizes one.
    fn dev_realize_in_room(
        &mut self,
        id: CreatureId,
        room: i32,
        tile: (i32, i32),
        observer: &mut dyn PlayerObserver,
    ) {
        let kind = self.world.creature(id).kind();
        if kind == CreatureType::Fly {
            self.realize_fly_in_room(room, id, observer);
        } else if LizardBreedParams::of(kind).is_some() {
            // Not the game-level call: a shelter would put it on the door's spawn tile.
            self.world.realize_lizard_in_room(id, None, &mut self.rng);
        } else if self.world.realize_overseer(id, 0, &mut self.rng) {
            self.run_overseer(id, OverseerCall::Place, observer);
        }
        // Dev override: the creature stays on the asked tile even where the abstract
        // AI's `PlaceInRealizedRoom` moved it toward its destination.
        if self.world.creature(id).pos.tile != tile {
            self.world.creature_mut(id).pos.tile = tile;
            let middle = Vec2::middle_of_tile(tile);
            if let Some(chunks) = self.world.body_chunks_mut(Entity::Creature(id)) {
                for c in chunks {
                    c.reset_at(middle);
                }
            }
        }
    }
}
