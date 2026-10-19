//! The game's side of room triggers: `Room.Loaded`'s `ActiveTriggerChecker`s, their
//! updates and `FireEvent` (`MusicEvent`, `StopMusicEvent`, `ImageTrigger`,
//! `PickupObjectInstruction`, `RoomSpecificTextMessage`, `BringGuideToPlayerEvent`),
//! `RainCycle.MusicAllowed` and `RainRequestStopSong`. Types: [`crate::trigger`].
//!
//! Mirrors: ActiveTriggerChecker.cs, Room.cs (Loaded), RainCycle.cs (MusicAllowed)

use rw_assets::{
    CreatureType, DangerType, EventType, ShowProjectedImageEvent, Timeline, TriggeredEvent,
};

use super::{HudEvent, RainWorldGame, RoomObject};
use crate::creature::overseer::{CommunicationModule, TutorialMessage};
use crate::trigger::{
    ActiveTriggerChecker, MusicRequest, PickupObjectInstruction, TriggerHud, TriggerObject,
    TriggerObjectKind, TriggerPlayer,
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
        self.add_placed_triggers(index);
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
    pub(super) fn add_trigger_object(&mut self, room: i32, kind: TriggerObjectKind) {
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
            TriggerObjectKind::SuperJumpInstruction(_) => {
                self.run_super_jump_instruction(index, id);
            }
            TriggerObjectKind::ScavTradeInstruction(_) => {
                self.run_scav_trade_instruction(index, id);
            }
            TriggerObjectKind::ReliableIggyDirection(_) => self.run_iggy_direction(index, id),
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
    /// room's pictures (`afterEncounter` against the save's Moon encounters).
    fn attempt_image_trigger(&mut self, index: i32, id: u64, event: ShowProjectedImageEvent) {
        if self.is_story_session() {
            let encountered_moon = self.story.ever_met_moon;
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
    pub(super) fn bring_guide(&mut self, min_like: f32) {
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
            let counter = abs.player_guide_counter;
            let room = self.pos().room;
            if self.guide_brought_logged_room != Some(room) {
                self.guide_brought_logged_room = Some(room);
                log::info!("guide brought to {} for {counter}", self.room_name(room));
            }
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
}

#[cfg(test)]
#[path = "game_triggers_tests.rs"]
mod tests;
