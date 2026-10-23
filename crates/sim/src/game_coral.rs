//! The game's side of the superstructure's coral: `Room.Loaded` making a room's
//! `CoralNeuronSystem` (for an `SSSwarmers` effect or any coral or wall mycelia object),
//! its `AIMapReady` building the owners from the placed objects, their turns in the
//! update list, the `NeuronSpark`s strands strike, and the overseers' strands joining
//! the system's registry. Types: [`crate::coral_system`], [`crate::wall_mycelia`],
//! [`crate::mycelium`], [`crate::neuron_spark`].
//!
//! `AIMapReady` runs in the loading step, after `ReadyForAI` realized the room's
//! creatures; the session's first room waits for its `NowViewed` (the camera's first
//! tick), as the game's `RoomPreparer` does (D37). What `CoralNeuronSystem.AIMapReady`
//! builds besides the wall mycelia (their places below) is the later steps': the
//! `CoralNeuron`s in the placed-object loop, then the `WallMycelia`, then the
//! `CoralCircuit` clusters, then the `CoralStem`s, then `PlaceSwarmers`.
//!
//! Mirrors: Room.cs (Loaded: CoralNeuronSystem; ReadyForAI), CoralNeuronSystem.cs (AIMapReady), WallMycelia.cs (Update), Overseer.cs (UpdateCoralNeuronSystem)

use std::sync::Arc;

use rw_assets::{PlacedObjectType, ResizableData};

use super::{RainWorldGame, RoomObject};
use crate::creature::CreatureId;
use crate::{
    CoralNeuronSystem, CosmeticKind, MyceliaCx, MyceliumEnv, PlayerObserver, Vec2, WallMycelia,
    WallPlace,
};

/// A coral object in a room's update list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoralRef {
    /// The room's `CoralNeuronSystem`.
    System,
    /// A `WallMycelia`, by its place in the update list.
    Wall(u64),
}

impl RainWorldGame {
    /// `Room.Loaded`'s `AddObject(new CoralNeuronSystem())` unless the room has one: two
    /// draws for the wind, and an empty registry for the strands.
    pub(super) fn add_coral_system(&mut self, index: i32) {
        if self.world.realized(index).is_none_or(|r| r.coral.is_some()) {
            return;
        }
        let mut system = CoralNeuronSystem::new(0, &mut self.rng);
        system.update_order = self.world.next_update_seq();
        self.world.mycelia.create_registry(index);
        if let Some(r) = self.world.realized_mut(index) {
            r.coral = Some(system);
        }
    }

    /// `CoralNeuronSystem.AIMapReady` of room `index` (once, when the room is ready for AI).
    pub(super) fn coral_ai_map_ready(&mut self, index: i32) {
        let (timeline, mods) = (self.world.timeline(), self.world.mods());
        let Some(r) = self.world.realized(index) else {
            return;
        };
        if !r.ready_for_ai() || r.coral.as_ref().is_none_or(|s| s.built) {
            return;
        }
        let room = Arc::clone(&r.room);
        let mut places = Vec::new();
        for o in r.settings.placed_objects_for(timeline, mods) {
            if !o.active {
                continue;
            }
            // The later steps' owners join this loop in the game's order: a `CoralNeuron` is
            // made and added as the loop meets it; the stems and circuits are collected.
            if o.object_type(mods) == Some(PlacedObjectType::WallMycelia) {
                match ResizableData::parse(&o.data) {
                    Some(d) => places.push(WallPlace {
                        pos: Vec2::from(o.pos),
                        rad: d.rad(),
                    }),
                    None => log::warn!("unreadable {} data {:?}", o.kind, o.data),
                }
            }
        }
        let mut wall = (!places.is_empty()).then(|| {
            WallMycelia::new(
                0,
                places,
                &room,
                index,
                &mut self.world.mycelia,
                &mut self.rng,
            )
        });
        if let Some(w) = &mut wall {
            w.update_order = self.world.next_update_seq();
        }
        if let Some(system) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.coral.as_mut())
        {
            system.built = true;
            system.wall_mycelia.extend(wall);
        }
    }

    /// Room `index`'s coral objects in its update list.
    pub(super) fn room_coral_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.coral.as_ref())
            .map_or_else(Vec::new, |s| {
                let system = (s.update_order, RoomObject::Coral(CoralRef::System));
                std::iter::once(system)
                    .chain(s.wall_mycelia.iter().map(|w| {
                        (
                            w.update_order,
                            RoomObject::Coral(CoralRef::Wall(w.update_order)),
                        )
                    }))
                    .collect()
            })
    }

    /// The `Update` of a coral object of room `index`.
    pub(super) fn run_coral(&mut self, index: i32, which: CoralRef) {
        let camera = self.camera_view;
        let viewed = index == self.camera_room;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(mut system) = r.coral.take() else {
            return;
        };
        let (room, ai_map, tick) = (
            Arc::clone(&r.room),
            r.ai_map.clone(),
            r.cameras_changed_tick,
        );
        match which {
            CoralRef::System => system.update(viewed, &mut self.rng),
            CoralRef::Wall(order) => {
                let env = MyceliumEnv {
                    room: &room,
                    room_index: index,
                    ai_map: ai_map.as_deref(),
                    camera,
                    cameras_changed_tick: tick,
                    frozen: Some(system.frozen()),
                };
                if let Some(wall) = system
                    .wall_mycelia
                    .iter_mut()
                    .find(|w| w.update_order == order)
                {
                    wall.update(&mut self.world.mycelia, &env, &mut self.rng);
                }
            }
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.coral = Some(system);
        }
        self.flush_mycelium_sparks(index);
    }

    /// The `NeuronSpark`s strands made since the last call join room `index`'s update list.
    pub(super) fn flush_mycelium_sparks(&mut self, index: i32) {
        for spark in std::mem::take(&mut self.world.mycelia.sparks) {
            self.world
                .add_cosmetic(index, CosmeticKind::NeuronSpark(spark));
        }
    }

    /// `Room.AddObject`'s `InitiateGraphicsModule` (or `NowViewed`) of overseer `id` in
    /// room `room`: its strands are made and joins the room's system. Returns whether the
    /// observer built a module.
    pub(super) fn overseer_module_added(
        &mut self,
        id: CreatureId,
        room: i32,
        observer: &mut dyn PlayerObserver,
    ) -> bool {
        let mut pool = std::mem::take(&mut self.world.mycelia);
        let built = match (self.world.overseer_view(id), self.world.realized(room)) {
            (Some(view), Some(r)) => {
                let mut mx = MyceliaCx {
                    pool: &mut pool,
                    env: r.mycelium_env(room, self.camera_view),
                };
                observer.overseer_added(&view, &mut mx, &mut self.rng)
            }
            _ => false,
        };
        self.world.mycelia = pool;
        built
    }

    /// `RoomCamera.ApplyPositionChange`'s `camerasChangedTick++` for the room the camera
    /// now shows.
    pub(super) fn cameras_changed(&mut self, room: i32) {
        if let Some(r) = self.world.realized_mut(room) {
            r.cameras_changed_tick += 1;
        }
    }
}

#[cfg(test)]
#[path = "game_coral_tests.rs"]
mod tests;
