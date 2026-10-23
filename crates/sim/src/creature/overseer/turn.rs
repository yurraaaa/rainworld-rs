//! Realized overseers in the game: the player and story as an overseer reads them, an
//! overseer's turn in its room's update list and the effects it hands back.
//!
//! Mirrors: Overseer.cs (Update), RainWorldGame.cs

use super::{HudEvent, OverseerCall, PlayerObserver, RainWorldGame};
use crate::creature::overseer::hologram::ControlPreset;
use crate::creature::overseer::{
    OverseerEffect, OverseerMode, OverseerTick, OverseerUpdate, PlayerView, StoryStats,
};
use crate::creature::{CreatureId, RealizedCreature};
use crate::{LobbyVessel, VesselOwner};

impl RainWorldGame {
    /// The player as a realized overseer in room `room` reads it.
    fn player_view(&self, room: i32) -> Option<PlayerView> {
        let p = self.player.as_ref()?;
        let inputs = p.input_history();
        let chunks = &p.body.chunks;
        Some(PlayerView {
            in_room: self.pos().room == room && !p.in_shortcut() && !p.is_destroyed(),
            pos: chunks[0].pos,
            last_pos: chunks[0].last_pos,
            hips: chunks[1].pos,
            dead: p.is_dead(),
            conscious: !p.is_dead() && !p.is_stunned(),
            food: self.food_in_stomach().cast_signed(),
            jump: [inputs[0].jump, inputs[1].jump],
            entering_shortcut: p.entering_shortcut(),
            in_shortcut: p.in_shortcut(),
            standing: p.is_standing(),
            animation: p.animation(),
            hips_contact_y: chunks[1].contact().y,
            input_x: inputs[0].x,
            input_y: inputs[0].y,
            pickup: inputs[0].pickup,
            throw: inputs[0].throw,
            eat_counter: p.grab_state().eat_counter,
        })
    }

    fn story_stats(&self) -> StoryStats {
        let stats = &self.campaign.stats;
        StoryStats {
            cycle: self.story.cycle_number,
            food_to_hibernate: stats.food_to_hibernate,
            max_food: stats.max_food,
            karma_cap: self.story.karma_cap,
            yellow: self.campaign.name == rw_assets::SlugcatName::Yellow,
            how_well_is_player_doing: -1.0,
            ever_met_moon: self.story.ever_met_moon,
        }
    }

    /// One call into realized overseer `id` in its room, with what it reads of the game,
    /// then what it did to the rest of the game.
    pub(super) fn run_overseer(
        &mut self,
        id: CreatureId,
        call: OverseerCall,
        observer: &mut dyn PlayerObserver,
    ) {
        let room_index = match call {
            OverseerCall::Hologram(hid) => self
                .world
                .creature(id)
                .overseer_body()
                .and_then(|b| b.holograms.iter().find(|h| h.id == hid))
                .map_or(-1, |h| h.room),
            _ => self.world.creature(id).pos.room,
        };
        let player = self.player_view(room_index);
        let edibles = self.room_edibles(room_index);
        let story = self.story_stats();
        let instructions = self.input_instruction_views();
        let forced_directions = self.forced_direction_views();
        let Some(mut parts) = self.world.take_overseer(id) else {
            return;
        };
        let mut pos = self.world.creature(id).pos;
        let entity = self.world.creature(id).id;
        let ignore_cycle = self.world.creature(id).ignore_cycle;
        let mut rain = self.world.rain_cycle;
        let mut effects = Vec::new();
        let mut pool = std::mem::take(&mut self.world.mycelia);
        if matches!(call, OverseerCall::Place | OverseerCall::TeleportIn) {
            parts.body.update_order = self.world.next_update_seq();
        }
        if let Some(realized) = self.world.realized(room_index)
            && let Some(ai_map) = realized.ai_map.as_ref()
        {
            let sounds = if room_index == self.camera_room {
                &mut self.sounds
            } else {
                &mut self.silent
            };
            let mut cx = OverseerTick {
                world: &self.world,
                room_index,
                room: &realized.room,
                ai_map,
                rng: &mut self.rng,
                sounds,
                observer,
                guide: &mut self.guide,
                control_preset: ControlPreset::Keyboard,
                hologram_ids: &mut self.hologram_ids,
                player_id: self.player_id,
                player,
                edibles,
                player_karma: self.story.karma,
                camera: self.camera_view,
                story,
                rain,
                hud: self.script_tick.hud_with_control_preset,
                viewed: room_index == self.camera_room,
                effects: Vec::new(),
                instructions,
                forced_directions,
                pool: &mut pool,
                cameras_changed_tick: realized.cameras_changed_tick,
                coral: realized.coral.as_ref().map(crate::CoralNeuronSystem::state),
            };
            let mut run = OverseerUpdate {
                me: id,
                id: entity,
                pos,
                ignore_cycle,
                body: &mut parts.body,
                ai: &mut parts.ai,
                abs: &mut parts.abs,
                base: &mut parts.base,
                cx: &mut cx,
            };
            match call {
                OverseerCall::Place => {
                    self.last_coords.remove(&id);
                    run.place_in_room();
                }
                OverseerCall::TeleportIn => run.teleport_into_room(),
                OverseerCall::Update => run.update_with_graphics(),
                OverseerCall::Hologram(hid) => run.update_hologram(hid),
                OverseerCall::FlyingWeapon(w) => run.flying_weapon(w),
                OverseerCall::HitByWeapon { by_slugcat } => run.hit_by_weapon(by_slugcat),
                #[cfg(test)]
                OverseerCall::Communication => run.communication_update(),
            }
            pos = run.pos;
            rain = cx.rain;
            effects = cx.effects;
        }
        self.world.mycelia = pool;
        self.flush_mycelium_sparks(room_index);
        for h in &mut parts.body.holograms {
            if h.update_order == 0 {
                h.update_order = self.world.next_update_seq();
            }
        }
        if let OverseerCall::Hologram(hid) = call
            && let Some(h) = parts.body.holograms.iter().find(|h| h.id == hid)
        {
            observer.hologram_updated(self.world.creature(id).id, h);
        }
        let current = parts.body.hologram;
        parts
            .body
            .holograms
            .retain(|h| !h.slated || Some(h.id) == current);
        self.world.put_overseer(id, parts, pos);
        self.world.rain_cycle = rain;
        self.apply_overseer_effects(id, effects, observer);
    }

    fn apply_overseer_effects(
        &mut self,
        id: CreatureId,
        effects: Vec<OverseerEffect>,
        observer: &mut dyn PlayerObserver,
    ) {
        let _ = observer;
        for effect in effects {
            match effect {
                OverseerEffect::Message(m) => self.hud_events.push(HudEvent::Message(m)),
                OverseerEffect::HalfTimeBlinkReset => {
                    self.hud_events.push(HudEvent::HalfTimeBlinkReset);
                }
                OverseerEffect::TeleportOut(dest) => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(id).realized
                    {
                        body.in_room = false;
                    }
                    self.shortcuts.lobby.push(LobbyVessel {
                        room: dest.room,
                        entrance_node: dest.node,
                        updates: 0,
                        owner: VesselOwner::Teleport(id),
                    });
                }
                OverseerEffect::StartConversation { other, with, delay } => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(other).realized
                    {
                        body.conversation_partner = Some(with);
                        body.mode = OverseerMode::Conversing;
                        body.conversation_delay = delay;
                    }
                }
                OverseerEffect::EndConversation { other, delay } => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(other).realized
                    {
                        body.switch_to_watching(delay);
                    }
                }
                OverseerEffect::KillGreenLizards(room) => self.world.kill_green_lizards(room),
                OverseerEffect::Cosmetic(kind) => {
                    let room = self.world.creature(id).pos.room;
                    self.world.add_cosmetic(room, kind);
                }
                OverseerEffect::Destroy => {
                    if let Some(RealizedCreature::Overseer(body)) =
                        &mut self.world.creature_mut(id).realized
                    {
                        body.in_room = false;
                        body.slated = true;
                    }
                }
                OverseerEffect::InstructionFollowed(instruction) => {
                    self.instruction_followed(instruction);
                }
                OverseerEffect::InstructionCompleted(instruction) => {
                    self.instruction_completed(instruction);
                }
                OverseerEffect::ResetSuperLaunchJump => {
                    if let Some(p) = self.player.as_mut() {
                        p.reset_super_launch_jump();
                    }
                }
            }
        }
    }
}
