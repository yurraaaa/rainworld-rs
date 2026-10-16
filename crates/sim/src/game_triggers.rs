//! The game's side of room triggers: `Room.Loaded`'s `ActiveTriggerChecker`s, their
//! updates and `FireEvent` (`MusicEvent`, `StopMusicEvent`, `ImageTrigger`,
//! `PickupObjectInstruction`, `RoomSpecificTextMessage`, `BringGuideToPlayerEvent`),
//! `RainCycle.MusicAllowed` and `RainRequestStopSong`. Types: [`crate::trigger`].

use rw_assets::{
    AbstractObjectType, CreatureType, DangerType, EventType, ShowProjectedImageEvent, Timeline,
    TriggeredEvent,
};

use super::{HudEvent, RainWorldGame, RoomObject};
use crate::creature::overseer::{CommunicationModule, TutorialMessage};
use crate::item::{Entity, ObjectId, WeaponMode};
use crate::trigger::{
    ActiveTriggerChecker, InputInstructionView, MusicRequest, PickupObjectInstruction, TriggerHud,
    TriggerObject, TriggerObjectKind, TriggerPlayer,
};
use crate::world::World;
use crate::{FloatExt, Vec2};

impl World {
    /// `RainCycle.MusicAllowed`: arena, or at least 2400 ticks before the rain; the
    /// Saint's and Rivulet's timelines; Five Pebbles (and More Slugcats' RM, DM, LC, OE).
    #[must_use]
    pub fn music_allowed(&self) -> bool {
        if !self.story || self.rain_cycle.time_until_rain() >= 2400 {
            return true;
        }
        if matches!(self.timeline(), Timeline::Saint | Timeline::Rivulet) {
            return true;
        }
        let region = self.region_name();
        region == Some("SS")
            || (self.mods().msc && matches!(region, Some("RM" | "DM" | "LC" | "OE")))
    }

    /// The realized player guide in room `room` with a communication module
    /// (`ImageTrigger`'s and `InputInstructionTrigger`'s search of the room's creatures).
    fn guide_communication_mut(&mut self, room: i32) -> Option<&mut CommunicationModule> {
        let id = self.room_creatures(room).iter().copied().find(|&c| {
            let creature = self.creature(c);
            creature.kind() == CreatureType::Overseer
                && creature.overseer_ai().is_some_and(|a| a.is_player_guide)
                && creature.is_realized()
        })?;
        self.creature_mut(id)
            .overseer_ai_mut()?
            .real_ai
            .as_mut()?
            .communication
            .as_mut()
    }
}

impl RainWorldGame {
    /// `Room.Loaded`'s checkers, after the room's scripts and before its items: in a
    /// story, only the triggers whose cycles and slugcats include this save.
    pub(super) fn add_trigger_checkers(&mut self, index: i32) {
        let world = &self.world;
        self.trigger_objects
            .retain(|o| o.room != index && world.realized(o.room).is_some());
        let story = self.is_story_session();
        let cycle = self.story.cycle_number;
        let (timeline, mods) = (self.world.timeline(), self.world.mods());
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let active: Vec<usize> = realized
            .settings
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !story
                    || (cycle >= t.active_from_cycle
                        && t.active_in(timeline, mods)
                        && (t.active_to_cycle < 0 || cycle <= t.active_to_cycle))
            })
            .map(|(i, _)| i)
            .collect();
        for trigger in active {
            self.add_trigger_object(
                index,
                TriggerObjectKind::Checker(ActiveTriggerChecker::new(trigger)),
            );
        }
    }

    /// `room.AddObject`: the end of the update list.
    fn add_trigger_object(&mut self, room: i32, kind: TriggerObjectKind) {
        let id = self.world.next_update_seq();
        self.trigger_objects.push(TriggerObject {
            id,
            room,
            slated: false,
            kind,
        });
    }

    /// Room `index`'s trigger objects by their place in its update list.
    pub(super) fn room_trigger_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.trigger_objects
            .iter()
            .filter(|o| o.room == index && !o.slated)
            .map(|o| (o.id, RoomObject::Trigger(o.id)))
            .collect()
    }

    /// Drops the trigger objects destroyed during room `index`'s update.
    pub(super) fn remove_slated_trigger_objects(&mut self) {
        self.trigger_objects.retain(|o| !o.slated);
    }

    /// The input instruction objects, as the guide's communication reads them.
    pub(super) fn input_instruction_views(&self) -> Vec<InputInstructionView> {
        self.trigger_objects
            .iter()
            .filter_map(TriggerObject::instruction_view)
            .collect()
    }

    /// The overseer marked instruction `id` as followed and counted it.
    pub(super) fn instruction_followed(&mut self, id: u64) {
        if let Some(TriggerObjectKind::PickupInstruction(p)) = self
            .trigger_objects
            .iter_mut()
            .find(|o| o.id == id)
            .map(|o| &mut o.kind)
        {
            p.mark_instruction_as_followed = false;
        }
    }

    /// Camera 0's HUD as the checkers read it (set by the app before the tick).
    pub fn set_trigger_hud(&mut self, hud: TriggerHud) {
        self.trigger_hud = hud;
    }

    /// The song requests since the last call, in order.
    pub fn take_music_requests(&mut self) -> Vec<MusicRequest> {
        std::mem::take(&mut self.music_requests)
    }

    /// The shared `UnityEngine.Random`, for the side processes that run after the game
    /// in a frame (the music player).
    pub fn rng_mut(&mut self) -> &mut rw_random::UnityRandom {
        &mut self.rng
    }

    /// The live trigger objects (for reports).
    #[must_use]
    pub fn trigger_objects(&self) -> &[TriggerObject] {
        &self.trigger_objects
    }

    /// `RainCycle.Update`'s last step: with music not allowed and rain danger in camera
    /// 0's room, the music player is asked to stop its songs.
    pub(super) fn rain_request_stop_song(&mut self) {
        let danger = self
            .world
            .realized(self.camera_room)
            .map(|r| r.settings.danger_type);
        if !self.world.music_allowed() && danger.is_some_and(|d| d != Some(DangerType::None)) {
            self.music_requests.push(MusicRequest::RainStop);
        }
    }

    /// Trigger object `id`'s `Update` in room `index`.
    pub(super) fn run_trigger_object(&mut self, index: i32, id: u64) {
        let Some(at) = self.trigger_objects.iter().position(|o| o.id == id) else {
            return;
        };
        match &self.trigger_objects[at].kind {
            TriggerObjectKind::Checker(_) => self.run_checker(index, at),
            TriggerObjectKind::PickupInstruction(_) => self.run_pickup_instruction(index, id),
            TriggerObjectKind::TextMessage => self.run_text_message(index),
        }
    }

    /// What checkers in room `index` read of `Players[0]`.
    fn trigger_player(&self, index: i32) -> TriggerPlayer {
        let pos = self.pos();
        let p = self.player.as_ref();
        TriggerPlayer {
            in_room: pos.room == index,
            realized: p.is_some(),
            node: pos.node,
            still_in_start_shelter: p.is_some_and(crate::Player::still_in_start_shelter),
            in_shortcut: p.is_some_and(crate::Player::in_shortcut),
            karma: if self.is_story_session() {
                self.story.karma
            } else {
                0
            },
            pos: p.map_or(Vec2::ZERO, |p| p.body.chunks[0].pos),
        }
    }

    /// `GameOverModeActive` as the checkers see it: the HUD's flag, or a `GameOver` this
    /// tick the HUD has not entered yet.
    fn trigger_hud_now(&self) -> TriggerHud {
        let pending = self.game_over.is_some()
            && self.is_story_session()
            && self.script_tick.hud_with_control_preset;
        TriggerHud {
            game_over_mode: self.trigger_hud.game_over_mode || pending,
            region_bumps: self.trigger_hud.region_bumps,
        }
    }

    fn run_checker(&mut self, index: i32, at: usize) {
        let player = self.trigger_player(index);
        let hud = self.trigger_hud_now();
        let Some(settings) = self.world.realized(index).map(|r| &r.settings) else {
            return;
        };
        let TriggerObjectKind::Checker(checker) = &mut self.trigger_objects[at].kind else {
            return;
        };
        let Some(trigger) = settings.triggers.get(checker.trigger) else {
            return;
        };
        if checker.update(trigger, &player, hud) {
            let trigger = checker.trigger;
            let id = self.trigger_objects[at].id;
            self.fire_event(index, id, trigger);
        }
    }

    /// `FireEvent`.
    fn fire_event(&mut self, index: i32, id: u64, trigger: usize) {
        let Some(realized) = self.world.realized(index) else {
            return;
        };
        let Some(t) = realized.settings.triggers.get(trigger).cloned() else {
            return;
        };
        let danger_none = realized.settings.danger_type == Some(DangerType::None);
        let Some(passed) = ActiveTriggerChecker::roll(&t, &mut self.rng) else {
            return;
        };
        if passed && let Some(event) = t.event {
            log::info!("trigger in room {index} fires {:?}", event.event_type());
            match event {
                TriggeredEvent::Music(m) => {
                    let red_out = self.is_story_session() && self.red_is_out_of_cycles();
                    if (self.world.music_allowed() || danger_none) && !red_out {
                        self.music_requests.push(MusicRequest::Song(m));
                    }
                }
                TriggeredEvent::StopMusic(s) => self.music_requests.push(MusicRequest::Stop(s)),
                TriggeredEvent::ShowProjectedImage(e) => self.attempt_image_trigger(index, id, e),
                TriggeredEvent::Plain(Some(EventType::PickUpObjectInstruction)) => {
                    let kind = TriggerObjectKind::PickupInstruction(PickupObjectInstruction::new());
                    self.add_trigger_object(index, kind);
                }
                TriggeredEvent::Plain(Some(EventType::RoomSpecificTextMessage)) => {
                    self.add_trigger_object(index, TriggerObjectKind::TextMessage);
                }
                TriggeredEvent::Plain(Some(EventType::BringPlayerGuideToRoom)) => {
                    self.bring_guide(-0.5);
                }
                TriggeredEvent::Plain(_) => {}
            }
        }
        if !t.multi_use {
            self.slate_trigger_object(id);
        }
    }

    fn slate_trigger_object(&mut self, id: u64) {
        if let Some(o) = self.trigger_objects.iter_mut().find(|o| o.id == id) {
            o.slated = true;
        }
    }

    /// `ImageTrigger.AttemptTriggerFire`: the guide in the room is told to show the
    /// room's pictures. Moon is not ported, so `SLOracleState.playerEncounters` is 0.
    fn attempt_image_trigger(&mut self, index: i32, id: u64, event: ShowProjectedImageEvent) {
        if self.is_story_session() {
            let encountered_moon = false;
            let name = self.room_name(index);
            if event.after_encounter != encountered_moon
                || self.guide.has_image_been_shown_in_room(&name)
            {
                self.slate_trigger_object(id);
                return;
            }
        }
        let player_room = self
            .player
            .as_ref()
            .filter(|p| !p.in_shortcut())
            .map(|_| self.pos().room);
        let realized_player = self.player.is_some();
        let karma = self.story.karma;
        let world = &self.world;
        let direction = event.only_when_showing_direction;
        let Some(comm) = self.world_guide_communication(index) else {
            return;
        };
        if direction
            && (!realized_player || !comm.any_progression_direction(world, player_room, karma))
        {
            return;
        }
        let Some(comm) = self.world.guide_communication_mut(index) else {
            return;
        };
        comm.image_to_show = Some(event);
        comm.show_image_room = index;
        self.slate_trigger_object(id);
    }

    /// The guide's communication module in room `room`, read-only.
    fn world_guide_communication(&self, room: i32) -> Option<&CommunicationModule> {
        let id = self.world.room_creatures(room).iter().copied().find(|&c| {
            let creature = self.world.creature(c);
            creature.kind() == CreatureType::Overseer
                && creature.overseer_ai().is_some_and(|a| a.is_player_guide)
                && creature.is_realized()
        })?;
        self.world
            .creature(id)
            .overseer_ai()?
            .real_ai
            .as_ref()?
            .communication
            .as_ref()
    }

    fn room_name(&self, index: i32) -> String {
        self.world
            .abstract_room(index)
            .map(|r| r.name.clone())
            .unwrap_or_default()
    }

    /// `BringGuideToPlayerEvent.BringGuide(world, minLike)`: the guide heads for the
    /// player for 400 to 1800 ticks by how much it likes them.
    fn bring_guide(&mut self, min_like: f32) {
        let Some(guide) = self.world.overseers_world_ai().and_then(|o| o.player_guide) else {
            return;
        };
        let likes = self.guide.likes_player;
        if likes < min_like {
            return;
        }
        if let Some(abs) = self.world.creature_mut(guide).overseer_ai_mut() {
            abs.go_to_player = true;
            abs.player_guide_counter = likes.lerp_map(-0.5, 1.0, 400.0, 1800.0) as i32;
            log::info!(
                "guide brought to the player for {}",
                abs.player_guide_counter
            );
        }
    }

    /// `RoomSpecificTextMessage.Update` (base game: Shoreline's `SL_C05` only).
    fn run_text_message(&mut self, index: i32) {
        if self.room_name(index) == "SL_C05"
            && self.is_story_session()
            && !self.story.dangle_fruit_in_water_message
            && self.script_tick.hud_with_control_preset
        {
            self.hud_events.push(HudEvent::Message(TutorialMessage {
                text: "Dropping food in the water might attract creatures.",
                wait: 0,
                time: 200,
                hide_hud: true,
            }));
            self.hud_events.push(HudEvent::DangleFruitInWaterMessage);
            self.story.dangle_fruit_in_water_message = true;
        }
    }

    /// `PickupObjectInstruction.Update` with its `InputInstructionTrigger` base.
    fn run_pickup_instruction(&mut self, index: i32, id: u64) {
        let Some(mut state) = self.pickup_state(id) else {
            return;
        };
        let mut slated = false;
        if state.completed {
            slated = true;
        } else if state.activated {
            state.time_since_activated += 1;
            let shown = self.guide.pickup_objects_shown;
            if let Some(comm) = self.world.guide_communication_mut(index) {
                // `PlayerNeedsInputInstruction`.
                if shown <= 0 {
                    comm.input_instruction = Some(id);
                }
            }
        } else {
            // `CheckTrigger(Players[0])`.
            state.activated = state.item.is_some();
        }
        slated |= self.pickup_step(index, &mut state);
        if let Some(o) = self.trigger_objects.iter_mut().find(|o| o.id == id) {
            o.kind = TriggerObjectKind::PickupInstruction(state);
            o.slated |= slated;
        }
    }

    fn pickup_state(&self, id: u64) -> Option<PickupObjectInstruction> {
        match &self.trigger_objects.iter().find(|o| o.id == id)?.kind {
            TriggerObjectKind::PickupInstruction(p) => Some(p.clone()),
            _ => None,
        }
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
            state.instruction_pos = main;
            let held = state
                .item
                .is_some_and(|i| !self.world.grabbed_by(Entity::Object(i)).is_empty());
            if !held || player.entering_shortcut().is_some() {
                state.completed = true;
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
                    state.instruction_pos = w.body.chunks[0].pos;
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
}

#[cfg(test)]
#[path = "game_triggers_tests.rs"]
mod tests;
