//! The game's side of the guide's room objects: `Room.Loaded`'s placed objects (with the
//! placed lights, made in [`crate::effects::lights::turn`]),
//! the cosmetic slime molds and dead token stalks (made in [`crate::effects::particles::placed_cosmetics_turn`]),
//! the custom decals and rainbows (made in [`crate::effects::particles::decal_turn`]), the projected stars (made in
//! [`crate::superstructure::projections::turn`]), the screen-grab effects (made in [`crate::superstructure::grab::turn`]),
//! `SuperJumpInstruction`, `ScavengerTradeInstructionTrigger` and `ReliableIggyDirection`,
//! the `InputInstructionTrigger`s' updates (with the event-made
//! `PickupObjectInstruction`), and what the guide's communication reads of them. Types:
//! [`crate::trigger`]. Behaviour: `rainworld-re/notes/overseer.md` §7b.
//!
//! Mirrors: Room.cs (Loaded), SuperJumpInstruction.cs, ScavengerTradeInstructionTrigger.cs, ReliableIggyDirection.cs, InputInstructionTrigger.cs

use rw_assets::{
    AbstractObjectType, CreatureType, CustomDecalData, DeepProcessingData, GridRectData,
    IggyCondition, LightBeamData, LightFixtureData, LightSourceData, PlacedObjectType, QuadData,
    RainbowData, ReliableIggyDirectionData, ResizableData, SsLightRodData,
};

use super::RainWorldGame;
use crate::CosmeticSlimeMold;
use crate::FloatExt;
use crate::Vec2;
use crate::creature::overseer::CommunicationModule;
use crate::creature::{CreatureId, RealizedCreature};
use crate::item::{Entity, ObjectId, WeaponMode};
use crate::trigger::{
    ForcedDirectionView, InputInstructionView, InstructionTrigger, PickupObjectInstruction,
    ReliableIggyDirection, ScavengerTradeInstructionTrigger, SuperJumpInstruction,
    TriggerObjectKind,
};
use crate::world::World;

/// What `InputInstructionTrigger.Update` leaves to the subclass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstructionStep {
    /// Completed: destroyed.
    Destroyed,
    /// Activated: the guide was asked to show it.
    Asked,
    /// Not activated yet: `CheckTrigger(Players[0])`.
    Check,
}

impl World {
    /// The player guide among room `room`'s creatures with a real AI and a communication
    /// module (`InputInstructionTrigger.Update`'s search; the AI outlives the body).
    fn guide_real_ai_communication_mut(&mut self, room: i32) -> Option<&mut CommunicationModule> {
        let id = self.room_creatures(room).iter().copied().find(|&c| {
            let creature = self.creature(c);
            creature.kind() == CreatureType::Overseer
                && creature.overseer_ai().is_some_and(|a| {
                    a.is_player_guide
                        && a.real_ai
                            .as_ref()
                            .is_some_and(|ai| ai.communication.is_some())
                })
        })?;
        self.creature_mut(id)
            .overseer_ai_mut()?
            .real_ai
            .as_mut()?
            .communication
            .as_mut()
    }

    /// The realized player guide whose body is in room `room` (`ReliableIggyDirection`'s
    /// search).
    fn guide_in_room(&self, room: i32) -> Option<CreatureId> {
        self.room_creatures(room).iter().copied().find(|&c| {
            let creature = self.creature(c);
            creature.kind() == CreatureType::Overseer
                && matches!(&creature.realized, Some(RealizedCreature::Overseer(b)) if b.in_room)
                && creature.pos.room == room
                && creature.overseer_ai().is_some_and(|a| a.is_player_guide)
        })
    }
}

impl RainWorldGame {
    /// `Room.Loaded`'s placed objects that are ported room objects (the guide's
    /// triggers and the placed lights), in the room settings' order (inactive ones
    /// skipped), each joining the update list as it is made. Data the game could not
    /// parse (it throws) is skipped.
    pub(crate) fn add_placed_room_objects(&mut self, index: i32) {
        let (timeline, mods) = (self.world.timeline(), self.world.mods());
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let mut added = Vec::new();
        for o in realized.settings.placed_objects_for(timeline, mods) {
            if !o.active {
                continue;
            }
            let pos = Vec2::from(o.pos);
            let kind = match o.object_type(mods) {
                Some(PlacedObjectType::LightSource) => {
                    LightSourceData::parse(&o.data).map(|d| PlacedRoomObject::Light(pos, d))
                }
                Some(PlacedObjectType::LightFixture) => {
                    LightFixtureData::parse(&o.data).map(|d| PlacedRoomObject::Fixture(pos, d))
                }
                Some(PlacedObjectType::LightBeam) => {
                    LightBeamData::parse(&o.data).map(|d| PlacedRoomObject::LightBeam(pos, d))
                }
                Some(PlacedObjectType::SpotLight) => {
                    QuadData::parse(&o.data).map(|d| PlacedRoomObject::SpotLight(pos, d))
                }
                Some(PlacedObjectType::ZapCoil) => {
                    GridRectData::parse(&o.data).map(|d| PlacedRoomObject::ZapCoil(pos, d))
                }
                Some(PlacedObjectType::SuperStructureFuses) => {
                    GridRectData::parse(&o.data).map(|d| PlacedRoomObject::Fuses(pos, d))
                }
                Some(PlacedObjectType::DeepProcessing) => DeepProcessingData::parse(&o.data)
                    .map(|d| PlacedRoomObject::DeepProcessing(pos, d)),
                Some(PlacedObjectType::GravityDisruptor) => {
                    Some(PlacedRoomObject::GravityDisruptor(pos))
                }
                Some(PlacedObjectType::SSLightRod) => {
                    SsLightRodData::parse(&o.data).map(|d| PlacedRoomObject::LightRod(pos, d))
                }
                Some(PlacedObjectType::SuperJumpInstruction) => QuadData::parse(&o.data).map(|q| {
                    PlacedRoomObject::Trigger(TriggerObjectKind::SuperJumpInstruction(
                        SuperJumpInstruction::new(pos, q),
                    ))
                }),
                Some(PlacedObjectType::ScavTradeInstruction) => Some(PlacedRoomObject::Trigger(
                    TriggerObjectKind::ScavTradeInstruction(ScavengerTradeInstructionTrigger::new(
                        pos,
                    )),
                )),
                Some(PlacedObjectType::ReliableIggyDirection) => {
                    ReliableIggyDirectionData::parse(&o.data, mods).map(|d| {
                        PlacedRoomObject::Trigger(TriggerObjectKind::ReliableIggyDirection(
                            Box::new(ReliableIggyDirection::new(pos, d)),
                        ))
                    })
                }
                Some(PlacedObjectType::CosmeticSlimeMold) => ResizableData::parse(&o.data)
                    .map(|d| PlacedRoomObject::SlimeMold(pos, d.rad(), false)),
                Some(PlacedObjectType::CosmeticSlimeMold2) => ResizableData::parse(&o.data)
                    .map(|d| PlacedRoomObject::SlimeMold(pos, d.rad(), true)),
                Some(PlacedObjectType::SlimeMold) => {
                    match CosmeticSlimeMold::placed_item_rad(pos, &mut self.rng) {
                        Some(rad) => Some(PlacedRoomObject::SlimeMold(pos, rad, false)),
                        None => continue,
                    }
                }
                Some(PlacedObjectType::DeadTokenStalk) => ResizableData::parse(&o.data)
                    .map(|d| PlacedRoomObject::TokenStalk(pos, pos + Vec2::from(d.handle))),
                Some(PlacedObjectType::CustomDecal) => CustomDecalData::parse(&o.data)
                    .map(|d| PlacedRoomObject::Decal(pos, Box::new(d))),
                Some(PlacedObjectType::ProjectedStars) => {
                    ResizableData::parse(&o.data).map(|d| PlacedRoomObject::Stars(pos, d.rad()))
                }
                Some(PlacedObjectType::Rainbow) => match RainbowData::parse(&o.data) {
                    Some(d) if !self.rainbow_spawns(index, &d) => continue,
                    d => d.map(|d| PlacedRoomObject::Rainbow(pos, d, false)),
                },
                Some(PlacedObjectType::RainbowNoFade) => {
                    RainbowData::parse(&o.data).map(|d| PlacedRoomObject::Rainbow(pos, d, true))
                }
                Some(
                    PlacedObjectType::CoralNeuron
                    | PlacedObjectType::CoralCircuit
                    | PlacedObjectType::CoralStem
                    | PlacedObjectType::CoralStemWithNeurons
                    | PlacedObjectType::WallMycelia,
                ) => Some(PlacedRoomObject::CoralSystem),
                _ => continue,
            };
            match kind {
                Some(kind) => added.push(kind),
                None => log::warn!("unreadable {} data {:?}", o.kind, o.data),
            }
        }
        for object in added {
            match object {
                PlacedRoomObject::CoralSystem => self.add_coral_system(index),
                PlacedRoomObject::Trigger(kind) => self.add_trigger_object(index, kind),
                PlacedRoomObject::Light(pos, data) => self.add_placed_light(index, pos, &data),
                PlacedRoomObject::Fixture(pos, data) => self.add_light_fixture(index, pos, data),
                PlacedRoomObject::LightBeam(pos, data) => self.add_light_beam(index, pos, data),
                PlacedRoomObject::SpotLight(pos, data) => self.add_spot_light(index, pos, &data),
                PlacedRoomObject::ZapCoil(pos, data) => self.add_zap_coil(index, pos, data),
                PlacedRoomObject::Fuses(pos, data) => self.add_fuses(index, pos, data),
                PlacedRoomObject::LightRod(pos, data) => self.add_light_rod(index, pos, data),
                PlacedRoomObject::DeepProcessing(pos, data) => {
                    self.add_deep_processing(index, pos, data);
                }
                PlacedRoomObject::GravityDisruptor(pos) => self.add_gravity_disruptor(index, pos),
                PlacedRoomObject::SlimeMold(pos, rad, through_walls) => {
                    self.add_slime_mold(index, pos, rad, through_walls);
                }
                PlacedRoomObject::TokenStalk(hover, base) => {
                    self.add_token_stalk(index, hover, base);
                }
                PlacedRoomObject::Decal(pos, data) => self.add_custom_decal(index, pos, *data),
                PlacedRoomObject::Rainbow(pos, data, no_fade) => {
                    self.add_rainbow(index, pos, &data, no_fade);
                }
                PlacedRoomObject::Stars(pos, rad) => self.add_star_matrix(index, pos, rad),
            }
        }
        self.link_zap_coil_lights(index);
        if !self.awaiting_first_view(index) {
            self.link_gravity_disruptors(index);
        }
    }

    /// The input instruction objects, as the guide's communication reads them.
    pub(crate) fn input_instruction_views(&self) -> Vec<InputInstructionView> {
        self.trigger_objects
            .iter()
            .filter_map(crate::trigger::TriggerObject::instruction_view)
            .collect()
    }

    /// The `ReliableIggyDirection`s, as the guide reads them.
    pub(crate) fn forced_direction_views(&self) -> Vec<ForcedDirectionView> {
        self.trigger_objects
            .iter()
            .filter_map(crate::trigger::TriggerObject::forced_direction_view)
            .collect()
    }

    /// The overseer marked instruction `id` as followed and counted it.
    pub(crate) fn instruction_followed(&mut self, id: u64) {
        if let Some(TriggerObjectKind::PickupInstruction(p)) = self
            .trigger_objects
            .iter_mut()
            .find(|o| o.id == id)
            .map(|o| &mut o.kind)
        {
            p.mark_instruction_as_followed = false;
        }
    }

    /// The guide's controller set instruction `id`'s `completed`.
    pub(crate) fn instruction_completed(&mut self, id: u64) {
        if let Some(t) = self
            .trigger_objects
            .iter_mut()
            .find(|o| o.id == id)
            .and_then(crate::trigger::TriggerObject::instruction_trigger_mut)
        {
            t.completed = true;
        }
    }

    fn trigger_kind(&self, id: u64) -> Option<TriggerObjectKind> {
        self.trigger_objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.kind.clone())
    }

    fn put_trigger_kind(&mut self, id: u64, kind: TriggerObjectKind, slated: bool) {
        if let Some(o) = self.trigger_objects.iter_mut().find(|o| o.id == id) {
            o.kind = kind;
            o.slated |= slated;
        }
    }

    /// `InputInstructionTrigger.Update`: destroyed once completed; while activated, the
    /// player guide in the room is told it is needed (`PlayerNeedsInputInstruction`,
    /// whose count check is `wanted`); else the subclass checks the player.
    fn instruction_base(
        &mut self,
        index: i32,
        id: u64,
        trigger: &mut InstructionTrigger,
        wanted: bool,
    ) -> InstructionStep {
        if trigger.completed {
            return InstructionStep::Destroyed;
        }
        if trigger.activated {
            trigger.time_since_activated += 1;
            if let Some(comm) = self.world.guide_real_ai_communication_mut(index)
                && wanted
            {
                comm.input_instruction = Some(id);
            }
            return InstructionStep::Asked;
        }
        InstructionStep::Check
    }

    /// The realized player's main chunk (last and current position) while its body is
    /// in room `index` (`realizedCreature.room == room`).
    fn player_main_chunk_in(&self, index: i32) -> Option<(Vec2, Vec2)> {
        let p = self
            .player
            .as_ref()
            .filter(|p| !p.in_shortcut() && !p.is_destroyed())?;
        (self.pos().room == index).then(|| (p.body.chunks[0].last_pos, p.body.chunks[0].pos))
    }

    /// `PickupObjectInstruction.Update` with its `InputInstructionTrigger` base.
    pub(crate) fn run_pickup_instruction(&mut self, index: i32, id: u64) {
        let Some(TriggerObjectKind::PickupInstruction(mut state)) = self.trigger_kind(id) else {
            return;
        };
        let wanted = self.guide.pickup_objects_shown() <= 0;
        let step = self.instruction_base(index, id, &mut state.trigger, wanted);
        if step == InstructionStep::Check {
            // `CheckTrigger(Players[0])`.
            state.trigger.activated = state.item.is_some();
        }
        let finished = self.pickup_step(index, &mut state);
        let slated = finished || step == InstructionStep::Destroyed;
        self.put_trigger_kind(id, TriggerObjectKind::PickupInstruction(state), slated);
    }

    /// The rest of `PickupObjectInstruction.Update`: follows the carried rock or spear,
    /// else picks the nearest free one (distances halved for one on screen and for the
    /// current one). Returns whether it destroys itself.
    fn pickup_step(&self, index: i32, state: &mut PickupObjectInstruction) -> bool {
        let Some(player) = self.player.as_ref().filter(|p| !p.in_shortcut()) else {
            return false;
        };
        if self.pos().room != index {
            return true;
        }
        let main = player.body.chunks[0].pos;
        if state.player_has_picked_up_an_item {
            state.trigger.instruction_pos = main;
            let held = state
                .item
                .is_some_and(|i| !self.world.grabbed_by(Entity::Object(i)).is_empty());
            if !held || player.entering_shortcut().is_some() {
                state.trigger.completed = true;
                return true;
            }
            return false;
        }
        for grasp in self.player_grasps().into_iter().flatten() {
            if let Entity::Object(o) = grasp
                && Self::rock_or_spear(&self.world, o)
            {
                state.item = Some(o);
                state.player_has_picked_up_an_item = true;
                state.mark_instruction_as_followed = true;
                return false;
            }
        }
        let room = self.world.realized(index).map(|r| r.room.clone());
        let mut nearest = f32::MAX;
        for o in self.world.room_objects(index) {
            if !Self::rock_or_spear(&self.world, o) {
                continue;
            }
            let Some(w) = self.world.realized_weapon(o, index) else {
                continue;
            };
            if w.mode != WeaponMode::Free || !self.world.grabbed_by(Entity::Object(o)).is_empty() {
                continue;
            }
            let at = w.body.chunks[0].pos;
            let mut d = at.distance(main);
            if room
                .as_ref()
                .is_some_and(|r| self.camera_view.viewed(index, r, at, -20.0))
            {
                d /= 2.0;
            }
            if state.item == Some(o) {
                d /= 2.0;
            }
            if d < nearest {
                nearest = d;
                state.item = Some(o);
            }
        }
        match state.item {
            None => true,
            Some(item) => {
                if let Some(w) = self.world.object(item).realized.as_deref() {
                    state.trigger.instruction_pos = w.body.chunks[0].pos;
                }
                false
            }
        }
    }

    fn rock_or_spear(world: &World, o: ObjectId) -> bool {
        matches!(
            world.object(o).kind,
            AbstractObjectType::Rock | AbstractObjectType::Spear
        )
    }

    /// `SuperJumpInstruction.Update`: activated when the player's main chunk, not
    /// rising, crosses the trigger line; shown while `superJumpsShown <= 3`.
    pub(crate) fn run_super_jump_instruction(&mut self, index: i32, id: u64) {
        let Some(TriggerObjectKind::SuperJumpInstruction(mut s)) = self.trigger_kind(id) else {
            return;
        };
        let wanted = self.guide.super_jumps_shown() <= 3;
        let step = self.instruction_base(index, id, &mut s.trigger, wanted);
        if step == InstructionStep::Check
            && let Some((last, now)) = self.player_main_chunk_in(index)
            && s.crossed(last, now)
        {
            s.trigger.activated = true;
            log::info!("Instruction Activated");
        }
        let slated = step == InstructionStep::Destroyed;
        self.put_trigger_kind(id, TriggerObjectKind::SuperJumpInstruction(s), slated);
    }

    /// `ScavengerTradeInstructionTrigger.Update`: activated by a realized data pearl
    /// whose `originRoom` is this room (none is ever realized in the port), and
    /// deactivated while the first living player is realized in another room.
    pub(crate) fn run_scav_trade_instruction(&mut self, index: i32, id: u64) {
        let Some(TriggerObjectKind::ScavTradeInstruction(mut s)) = self.trigger_kind(id) else {
            return;
        };
        let wanted = !self.guide.scav_trade_instruction_completed();
        let step = self.instruction_base(index, id, &mut s.trigger, wanted);
        if step == InstructionStep::Check && self.player_main_chunk_in(index).is_some() {
            // `CheckTrigger`: the first entity that is a realized `DataPearl` from this
            // room. Data pearls are kept as save text only, never realized, so no
            // object here qualifies; `originRoom` would be read with them.
            let pearl = self.world.room_objects(index).find(|&o| {
                let object = self.world.object(o);
                object.kind == AbstractObjectType::DataPearl && object.realized.is_some()
            });
            if let Some(p) = pearl {
                s.trigger.activated = true;
                s.pearl = Some(p);
            }
        }
        let first_alive_elsewhere = self.player.as_ref().is_some_and(|p| {
            !p.is_dead() && !p.in_shortcut() && !p.is_destroyed() && self.pos().room != index
        });
        if first_alive_elsewhere {
            s.trigger.activated = false;
        }
        let slated = step == InstructionStep::Destroyed;
        self.put_trigger_kind(id, TriggerObjectKind::ScavTradeInstruction(s), slated);
    }

    /// `ReliableIggyDirection.Update`. On its first update it destroys itself when the
    /// campaign is not among its players, its condition on Moon and Pebbles fails, or
    /// the guide has already given it `cyclesToShow` times. Then, with a player within
    /// the radius (not standing at the exit unless `pointPlayerBack`): the guide in the
    /// room is activated once (counting the cycle), after that it is told to give this
    /// direction; without the guide in the room, the guide is brought to the player.
    pub(crate) fn run_iggy_direction(&mut self, index: i32, id: u64) {
        let Some(TriggerObjectKind::ReliableIggyDirection(mut d)) = self.trigger_kind(id) else {
            return;
        };
        if d.first_update {
            d.first_update = false;
            if self.iggy_direction_unwanted(index, &d.data) {
                self.put_trigger_kind(id, TriggerObjectKind::ReliableIggyDirection(d), true);
                return;
            }
        }
        let pos = self.pos();
        let near = self.player_main_chunk_in(index).is_some_and(|(_, main)| {
            pos.room == index
                && (d.data.point_player_back || pos.node != d.data.exit)
                && d.pos.dist_less(main, d.data.rad())
        });
        let guide = self.world.guide_in_room(index);
        if near {
            match guide {
                Some(_) if !d.has_been_activated => {
                    d.has_been_activated = true;
                    if d.data.cycles_to_show > 0 {
                        self.guide
                            .increment_times_forced_direction_has_been_given(index);
                    }
                }
                Some(g) => {
                    if let Some(comm) = self
                        .world
                        .creature_mut(g)
                        .overseer_ai_mut()
                        .and_then(|a| a.real_ai.as_mut())
                        .and_then(|ai| ai.communication.as_mut())
                    {
                        comm.forced_direction_to_give = Some(id);
                    }
                }
                None => self.bring_guide_to_player_for_iggy(),
            }
        }
        self.put_trigger_kind(id, TriggerObjectKind::ReliableIggyDirection(d), false);
    }

    /// The first update's checks of a `ReliableIggyDirection`.
    fn iggy_direction_unwanted(&self, index: i32, data: &ReliableIggyDirectionData) -> bool {
        let story = self.is_story_session();
        let mut unwanted = !story || !data.available_to_players.contains(&self.campaign.name);
        let met_moon = self.story.ever_met_moon;
        unwanted |= match data.condition {
            Some(IggyCondition::BeforeMoon) => met_moon,
            Some(IggyCondition::AfterMoon) => !met_moon,
            Some(IggyCondition::AfterPebblesAndMoon) => {
                !met_moon || self.story.ss_ai_conversations_had < 1
            }
            Some(IggyCondition::AnyTime) | None => false,
        };
        unwanted
            || (data.cycles_to_show > 0
                && self
                    .guide
                    .how_many_times_has_forced_direction_been_given(index)
                    >= data.cycles_to_show)
    }

    /// `ReliableIggyDirection.BringGuideToPlayer`: the first player guide in the
    /// world's rooms heads for the player unless it loathes them.
    fn bring_guide_to_player_for_iggy(&mut self) {
        if self.world.overseers_world_ai().is_none() || !self.is_story_session() {
            return;
        }
        let first = self.world.first_room_index();
        let rooms = i32::try_from(self.world.number_of_rooms()).unwrap_or(0);
        let guide = (first..first + rooms).find_map(|room| {
            self.world.room_creatures(room).iter().copied().find(|&c| {
                let creature = self.world.creature(c);
                creature.kind() == CreatureType::Overseer
                    && creature.overseer_ai().is_some_and(|a| a.is_player_guide)
            })
        });
        let likes = self.guide.likes_player;
        if let Some(g) = guide
            && likes > -0.99
            && let Some(abs) = self.world.creature_mut(g).overseer_ai_mut()
        {
            abs.go_to_player = true;
            abs.player_guide_counter = likes.lerp_map(-0.99, 1.0, 400.0, 1800.0) as i32;
        }
    }
}

#[cfg(test)]
#[path = "tests/instructions_turn.rs"]
mod tests;

/// A placed object `Room.Loaded` turns into a ported room object.
enum PlacedRoomObject {
    /// A coral object: the room gets its `CoralNeuronSystem` (once).
    CoralSystem,
    Trigger(TriggerObjectKind),
    Light(Vec2, LightSourceData),
    Fixture(Vec2, LightFixtureData),
    LightBeam(Vec2, LightBeamData),
    SpotLight(Vec2, QuadData),
    ZapCoil(Vec2, GridRectData),
    Fuses(Vec2, GridRectData),
    LightRod(Vec2, SsLightRodData),
    DeepProcessing(Vec2, DeepProcessingData),
    GravityDisruptor(Vec2),
    /// `pos`, `rad`, `throughWalls`.
    SlimeMold(Vec2, f32, bool),
    /// Hover and base positions.
    TokenStalk(Vec2, Vec2),
    Decal(Vec2, Box<CustomDecalData>),
    /// Position, data, `RainbowNoFade`.
    Rainbow(Vec2, RainbowData, bool),
    /// A `ProjectedStars`: position and `Rad`.
    Stars(Vec2, f32),
}
