//! Developer tools that add to the world through the game's own code paths.

use rw_assets::{AbstractObjectType, CreatureType, NodeType};

use super::{OverseerCall, PlayerObserver, RainWorldGame};
use crate::creature::{CreatureId, CreatureTemplate, EntityId, LizardBreedParams, WorldCoordinate};
use crate::item::{AbstractObject, Entity, ObjectId};
use crate::world::LizardRoomCx;
use crate::{LobbyVessel, Vec2, VesselOwner};

impl RainWorldGame {
    /// Whether `dev_spawn_creature` takes `kind`: not the slugcat or a template.
    #[must_use]
    pub fn dev_spawnable(kind: CreatureType) -> bool {
        !matches!(
            kind,
            CreatureType::Slugcat
                | CreatureType::StandardGroundCreature
                | CreatureType::LizardTemplate
        )
    }

    /// The registered creature types `dev_spawn_creature` takes, in registration order.
    #[must_use]
    pub fn dev_spawnable_creatures(&self) -> Vec<CreatureType> {
        CreatureType::registered(self.world.mods())
            .into_iter()
            .filter(|&k| Self::dev_spawnable(k))
            .collect()
    }

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
        if !Self::dev_spawnable(kind) {
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

    /// The base-game items a dev spawn makes: rocks and spears, realized, and the loose
    /// items the port has no body for, which stay abstract.
    pub const DEV_SPAWN_OBJECTS: [AbstractObjectType; 24] = [
        AbstractObjectType::Rock,
        AbstractObjectType::Spear,
        AbstractObjectType::FlareBomb,
        AbstractObjectType::VultureMask,
        AbstractObjectType::PuffBall,
        AbstractObjectType::DangleFruit,
        AbstractObjectType::DataPearl,
        AbstractObjectType::WaterNut,
        AbstractObjectType::JellyFish,
        AbstractObjectType::Lantern,
        AbstractObjectType::KarmaFlower,
        AbstractObjectType::Mushroom,
        AbstractObjectType::FirecrackerPlant,
        AbstractObjectType::SlimeMold,
        AbstractObjectType::FlyLure,
        AbstractObjectType::ScavengerBomb,
        AbstractObjectType::SporePlant,
        AbstractObjectType::EggBugEgg,
        AbstractObjectType::NeedleEgg,
        AbstractObjectType::BubbleGrass,
        AbstractObjectType::NshSwarmer,
        AbstractObjectType::OverseerCarcass,
        AbstractObjectType::SlOracleSwarmer,
        AbstractObjectType::SsOracleSwarmer,
    ];

    /// A dev spawn of an object as the arena sandbox makes one (`SpawnItems`): `new
    /// AbstractPhysicalObject` (a rock, or a plain object of another kind, without its
    /// subclass's fields, as D14's spore plants) or `new AbstractSpear` at `tile` of
    /// global room `room` with `GetNewID()`, `AbstractRoom.AddEntity`, then
    /// `RealizeInRoom` once the room's shortcuts are ready (a room still loading
    /// realizes it then, with its other objects); the port realizes only rocks and
    /// spears. `Err` for a kind not in [`Self::DEV_SPAWN_OBJECTS`] or a room the world
    /// lacks.
    ///
    /// # Errors
    /// See above.
    pub fn dev_spawn_object(
        &mut self,
        kind: AbstractObjectType,
        room: i32,
        tile: (i32, i32),
        explosive: bool,
    ) -> Result<ObjectId, String> {
        if self.world.abstract_room(room).is_none() {
            return Err(format!("no room {room} in the world"));
        }
        let pos = WorldCoordinate::new(room, tile.0, tile.1, -1);
        let object = match kind {
            AbstractObjectType::Rock => AbstractObject::new_rock(self.ids.new_id(-1), pos),
            AbstractObjectType::Spear => {
                AbstractObject::new_spear(self.ids.new_id(-1), pos, explosive)
            }
            k if Self::DEV_SPAWN_OBJECTS.contains(&k) => {
                AbstractObject::new_plain(k, self.ids.new_id(-1), pos)
            }
            other => return Err(format!("{} is not a spawnable item", other.value())),
        };
        let id = self.world.add_object(object);
        if self
            .world
            .realized(room)
            .is_some_and(crate::world::RealizedRoom::shortcuts_ready)
        {
            self.world.realize_object_in_room(id, &mut self.rng);
        }
        Ok(id)
    }

    /// A dev teleport of the player to exit node `node` of global room `room` (the
    /// room's first exit when `None`), the way a room exit carries it: the player and
    /// what it carries are sucked into a vessel (`SuckInCreature`), the room is
    /// activated at once, and the vessel waits in `betweenRoomsWaitingLobby` with that
    /// entrance node; the lobby then moves the abstract creature, spits the player out
    /// of the exit once the room is `ReadyForPlayer` and moves the camera. The world
    /// and the session stay as they are. Returns the entrance node.
    ///
    /// # Errors
    /// Without a live player free in a room (dead, held, already in a shortcut), for
    /// an unknown room, the off-screen den, or a node that is not one of its exits.
    pub fn dev_teleport_player(&mut self, room: i32, node: Option<i32>) -> Result<i32, String> {
        let data = self
            .world
            .abstract_room(room)
            .ok_or_else(|| format!("no room {room} in the world"))?;
        if data.offscreen_den {
            return Err("the off-screen den is not a room to enter".to_owned());
        }
        let exits: Vec<i32> = data
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeType::Exit)
            .filter_map(|(i, _)| i32::try_from(i).ok())
            .collect();
        let node = match node {
            Some(n) if exits.contains(&n) => n,
            Some(n) => return Err(format!("node {n} is not an exit of {}", data.name)),
            None => *exits
                .first()
                .ok_or_else(|| format!("{} has no exits", data.name))?,
        };
        if self.pos().room == room {
            return Err(format!("the player is already in {}", data.name));
        }
        let player = self.player.as_ref().ok_or("there is no player")?;
        if player.is_dead() {
            return Err("the player is dead".to_owned());
        }
        if player.in_shortcut() || self.player_in_lobby() {
            return Err("the player is already in a shortcut".to_owned());
        }
        if !self.world.creature(self.player_id).grabbed_by.is_empty() {
            return Err("the player is held".to_owned());
        }
        if let Some(p) = self.player.as_mut() {
            p.dev_enter_vessel();
        }
        self.carried_sucked_in();
        if let Err(e) = self.world.activate_room(room, &mut self.ids, &mut self.rng) {
            log::warn!("cannot realize room {room}: {e}");
        }
        self.shortcuts.lobby.push(LobbyVessel {
            room,
            entrance_node: node,
            updates: 0,
            owner: VesselOwner::Player,
        });
        Ok(node)
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
            let mut cx = LizardRoomCx {
                rng: &mut self.rng,
                observer,
                camera_room: self.camera_room,
            };
            self.world.realize_lizard_in_room(id, None, &mut cx);
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
