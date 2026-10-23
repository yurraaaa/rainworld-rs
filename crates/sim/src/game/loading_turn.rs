//! The game's side of room loading: `Room.Loaded`'s tail on the realizing call, then the
//! two stages the room's preparer hands over in the loading step, `ShortCutsReady`
//! (objects, and creatures without `AI` when none needs the AI map) and `ReadyForAI`
//! (creatures, quantified creatures, the `AIMapReady` listeners). Behaviour:
//! `rainworld-re/surveys/async-room-loading.md` 1.5, 1.6.
//!
//! Mirrors: Room.cs (Loaded, ShortCutsReady, ReadyForAI), AbstractRoom.cs (RealizeRoom), AbstractCreature.cs (AllowedToExistInRoom), RainWorldGame.cs (Update: loadingRooms)

use rw_assets::{AssetError, CreatureType};

use crate::creature::CreatureId;
use crate::game::OverseerCall;
use crate::world::PreparerStage;
use crate::{PlayerObserver, RainWorldGame};

/// An `INotifyWhenRoomIsReady` of a room's update list that the port has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiMapListener {
    ElectricDeath(u64),
    ZapCoilLight(u64),
    GravityDisruptor(u64),
    CoralNeuronSystem,
}

impl RainWorldGame {
    /// `World.ActivateRoom` (`AbstractRoom.RealizeRoom`): the room is read, joins the
    /// active and loading rooms, and runs `Room.Loaded`'s tail at once.
    ///
    /// # Errors
    /// When the room's files cannot be read.
    pub(crate) fn activate_room(&mut self, index: i32) -> Result<(), AssetError> {
        let result = self
            .world
            .activate_room(index, &mut self.ids, &mut self.rng);
        self.rooms_loaded();
        result
    }

    /// `Room.Loaded`'s tail for the rooms realized since the last call (the realizer
    /// activates through the world alone, and draws nothing after it).
    pub(crate) fn rooms_loaded(&mut self) {
        for index in std::mem::take(&mut self.world.pending_loaded) {
            self.room_loaded(index);
        }
    }

    /// The part of `Room.Loaded` made after the room's items (made in
    /// `World::activate_room`), in the port's order.
    fn room_loaded(&mut self, index: i32) {
        self.add_waterfalls(index);
        self.add_shelter_door(index);
        self.add_death_fall(index);
        self.add_trigger_checkers(index);
        self.add_dripping_sound(index);
    }

    /// The loading step: each preparer's call, newest first, with the stage it hands
    /// over run in that call.
    pub(crate) fn update_loading_rooms(&mut self, observer: &mut dyn PlayerObserver) {
        self.rooms_loaded();
        for (index, stage) in self.world.update_loading_rooms() {
            if self.world.realized(index).is_none() {
                continue;
            }
            match stage {
                PreparerStage::ShortcutsReady => self.shortcuts_ready(index, observer),
                PreparerStage::ReadyForAi => self.ready_for_ai(index, observer),
            }
        }
    }

    /// `Room.ShortCutsReady`.
    fn shortcuts_ready(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        if let Some(r) = self.world.realized_mut(index) {
            r.make_shortcuts_blinking();
        }
        self.world.realize_room_objects(index, &mut self.rng);
        let needs_ai_map = self.world.room_creatures_require_ai_map(index);
        if let Some(r) = self.world.realized_mut(index) {
            r.set_shortcuts_ready(!needs_ai_map);
        }
        if !needs_ai_map {
            self.realize_creatures_allowed_in_room(index, observer);
        }
        // `INotifyWhenRoomIsReady.ShortcutsReady`: empty in every base-game listener.
        self.add_shortcut_helper(index);
    }

    /// `ShortCutsReady`'s `AddObject(new ShortcutHelper(this))`: not ported yet (a later
    /// job of the async loading plan).
    fn add_shortcut_helper(&mut self, _index: i32) {}

    /// `Room.ReadyForAI`, after the thread made the AI map.
    fn ready_for_ai(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        self.world.build_ai_map(index);
        if let Some(r) = self.world.realized_mut(index) {
            r.begin_ready_for_ai();
        }
        self.realize_creatures_allowed_in_room(index, observer);
        self.place_quantified(index, observer);
        if let Some(r) = self.world.realized_mut(index) {
            r.end_ready_for_ai();
        }
        self.ai_map_ready(index);
        let story = self.is_story_session();
        let name = self.world.abstract_room(index).map(|d| d.name.clone());
        if let (Some(r), Some(name)) = (self.world.realized_mut(index), name) {
            r.raise_oracle_wait(&name, story);
        }
    }

    /// The stages' creature loop: each creature of the room, in list order, without a
    /// body and `AllowedToExistInRoom`, is `RealizeInRoom`ed.
    fn realize_creatures_allowed_in_room(&mut self, index: i32, observer: &mut dyn PlayerObserver) {
        for id in self.world.room_creatures(index).to_vec() {
            if self.world.allowed_to_exist_in_room(id, index) {
                self.realize_creature_in_room(index, id, observer);
            }
        }
    }

    /// `AbstractCreature.RealizeInRoom` of creature `id` in loading room `index`, for the
    /// kinds the port has bodies for.
    fn realize_creature_in_room(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        if id == self.player_id {
            if self.start_player_pending && self.player.is_none() {
                self.start_player_pending = false;
                self.realize_player(observer);
            }
        } else if self.world.creature(id).kind() == CreatureType::Fly {
            if !self.world.held_by_abstract_creature(id) {
                self.realize_fly_in_room(index, id, observer);
            }
        } else if crate::creature::LizardBreedParams::of(self.world.creature(id).kind()).is_some() {
            self.realize_lizard_in_room(index, id, observer);
        } else if self.world.realize_overseer(id, 0, &mut self.rng) {
            self.run_overseer(id, OverseerCall::Place, observer);
        }
    }

    /// `ReadyForAI`'s `AIMapReady` on every listener in the update list, in its order. The
    /// session's first room has its gravity disruptors and coral wait for the camera's
    /// first view (`apply_insect_view_change`), which the game's comes before.
    fn ai_map_ready(&mut self, index: i32) {
        let deferred = self.awaiting_first_view(index);
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let mut listeners: Vec<(u64, AiMapListener)> = Vec::new();
        listeners.extend(
            r.electric_deaths
                .iter()
                .map(|d| (d.update_order, AiMapListener::ElectricDeath(d.update_order))),
        );
        listeners.extend(
            r.fixtures
                .zap_coil_lights
                .iter()
                .map(|l| (l.update_order, AiMapListener::ZapCoilLight(l.update_order))),
        );
        if !deferred {
            listeners.extend(r.fixtures.gravity_disruptors.iter().map(|d| {
                (
                    d.update_order,
                    AiMapListener::GravityDisruptor(d.update_order),
                )
            }));
            listeners.extend(
                r.coral
                    .as_ref()
                    .map(|c| (c.update_order, AiMapListener::CoralNeuronSystem)),
            );
        }
        listeners.sort_by_key(|&(order, _)| order);
        for (_, listener) in listeners {
            match listener {
                AiMapListener::ElectricDeath(order) => {
                    self.electric_death_ai_map_ready(index, order);
                }
                AiMapListener::ZapCoilLight(order) => {
                    self.zap_coil_light_ai_map_ready(index, order)
                }
                AiMapListener::GravityDisruptor(order) => {
                    self.gravity_disruptor_ai_map_ready(index, order);
                }
                AiMapListener::CoralNeuronSystem => self.coral_ai_map_ready(index),
            }
        }
    }
}
