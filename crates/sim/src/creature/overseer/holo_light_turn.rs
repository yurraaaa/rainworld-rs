//! The Monk's guide light: `Player.Update`'s call of `HologramLight.TryCreate`
//! (`Player.Update`), `TryCreate` and the light's slot in its room's update list.
//!
//! Mirrors: HologramLight.cs (TryCreate), Player.cs (Update)

use rw_assets::{CreatureType, SlugcatName};

use crate::creature::CreatureId;
use crate::creature::overseer::{HoloLightTick, HologramLight, LightSourceStep, OverseerMode};
use crate::game::{PlayerObserver, RainWorldGame, RoomObject};
use crate::{LightSource, LightTie};

/// `HologramLight`'s light: `new Color(1, 0.8, 0.3)`.
const HOLO_LIGHT_COLOR: [f32; 3] = [1.0, 0.8, 0.3];

impl RainWorldGame {
    /// `Player.Update` for the Monk in room `index`: in the dark (`Needed` above 0.5),
    /// without the glow, outside shelters and gates, in a story whose guide likes the
    /// player above -0.8 and is not angry, `HologramLight.TryCreate`.
    pub(crate) fn monk_hologram_light(&mut self, index: i32) {
        if self.campaign.name != SlugcatName::Yellow
            || self.story.the_glow
            || self.holo_light_needed() <= 0.5
            || !self.is_story_session()
            || self.guide.likes_player <= -0.8
            || self.guide.angry_with_player()
        {
            return;
        }
        let shelter_or_gate = self
            .world
            .abstract_room(index)
            .is_some_and(|r| r.shelter() || r.gate());
        if !shelter_or_gate {
            self.try_create_holo_light(index);
        }
    }

    /// `HologramLight.Needed(player)` for the player now.
    #[must_use]
    pub fn holo_light_needed(&self) -> f32 {
        let Some(p) = self.player.as_ref() else {
            return 0.0;
        };
        let in_room = !p.in_shortcut() && !p.is_destroyed();
        let room = self.pos().room;
        let in_camera_room = in_room && room == self.camera_room;
        let lights = self
            .world
            .realized(room)
            .map_or(&[][..], |r| &r.light_sources[..]);
        HologramLight::needed(
            in_camera_room,
            self.camera_darkness,
            p.is_dead(),
            p.body.chunks[0].pos,
            lights,
            self.world.rain_cycle.shader_light(),
        )
    }

    /// `HologramLight.TryCreate`: the first player guide among the room's creatures
    /// projects a light if it is realized, conscious, in the room and has a
    /// communication module (unless it already projects one); otherwise the guide is
    /// brought to the player (`BringGuide(world, -0.8)`). No draws.
    fn try_create_holo_light(&mut self, index: i32) {
        let guide = self.world.room_creatures(index).iter().copied().find(|&c| {
            let c = self.world.creature(c);
            c.template.kind == CreatureType::Overseer
                && c.overseer_ai().is_some_and(|a| a.is_player_guide)
        });
        let projector = guide.filter(|&g| self.guide_can_project(g, index));
        let Some(overseer) = projector else {
            self.bring_guide(-0.8);
            return;
        };
        let has_light = self.guide_comm_light(overseer).is_some();
        if has_light {
            return;
        }
        let id = self.world.next_update_seq();
        let Some(realized) = self.world.realized_mut(index) else {
            return;
        };
        realized
            .holo_lights
            .push(HologramLight::new(id, index, overseer));
        if let Some(comm) = self
            .world
            .creature_mut(overseer)
            .overseer_ai_mut()
            .and_then(|a| a.real_ai.as_mut())
            .and_then(|ai| ai.communication.as_mut())
        {
            comm.holo_light = Some(id);
        }
        log::info!("guide's hologram light {id} in room {index}");
    }

    /// The guide is realized, conscious (`stun < 10`, alive), in room `index` and has
    /// its communication module.
    fn guide_can_project(&self, guide: CreatureId, index: i32) -> bool {
        let c = self.world.creature(guide);
        let has_comm = c
            .overseer_ai()
            .and_then(|a| a.real_ai.as_ref())
            .is_some_and(|ai| ai.communication.is_some());
        c.overseer_body()
            .is_some_and(|b| !b.dead && b.stun < 10 && b.in_room && c.pos.room == index)
            && has_comm
    }

    /// The guide's `communication.holoLight`; `None` also without the module.
    fn guide_comm_light(&self, guide: CreatureId) -> Option<u64> {
        self.world
            .creature(guide)
            .overseer_ai()
            .and_then(|a| a.real_ai.as_ref())
            .and_then(|ai| ai.communication.as_ref())
            .and_then(|c| c.holo_light)
    }

    pub(crate) fn room_holo_light_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.holo_lights
                .iter()
                .filter(|l| !l.slated)
                .map(|l| (l.id, RoomObject::HoloLight(l.id)))
                .collect()
        })
    }

    pub(crate) fn remove_slated_holo_lights(&mut self, index: i32) {
        if let Some(r) = self.world.realized_mut(index) {
            r.holo_lights.retain(|l| !l.slated);
            r.light_sources.retain(|l| !l.slated);
        }
    }

    /// The light `id`'s `Update` in its slot of room `index`'s update list.
    pub(crate) fn run_holo_light(
        &mut self,
        index: i32,
        id: u64,
        observer: &mut dyn PlayerObserver,
    ) {
        let Some(light) = self
            .world
            .realized(index)
            .and_then(|r| r.holo_lights.iter().find(|l| l.id == id && !l.slated))
        else {
            return;
        };
        let tick = self.holo_light_tick(light, index, observer);
        let Some(light) = self
            .world
            .realized_mut(index)
            .and_then(|r| r.holo_lights.iter_mut().find(|l| l.id == id))
        else {
            return;
        };
        match light.update(&tick, &mut self.rng) {
            LightSourceStep::Keep => {}
            LightSourceStep::Create(pos) => {
                let source = self.world.next_update_seq();
                let Some(r) = self.world.realized_mut(index) else {
                    return;
                };
                if let Some(light) = r.holo_lights.iter_mut().find(|l| l.id == id) {
                    light.light_source = Some(source);
                }
                let tie = Some(LightTie::HoloLight(id));
                r.light_sources.push(LightSource::new(
                    source,
                    index,
                    pos,
                    false,
                    HOLO_LIGHT_COLOR,
                    tie,
                ));
            }
            LightSourceStep::Destroy(source) => {
                let sources = self.world.realized_mut(index).map(|r| &mut r.light_sources);
                if let Some(l) = sources.and_then(|s| s.iter_mut().find(|l| l.id == source)) {
                    l.slated = true;
                }
            }
        }
    }

    /// What light `light` reads of its overseer and the player this tick.
    fn holo_light_tick(
        &self,
        light: &HologramLight,
        index: i32,
        observer: &dyn PlayerObserver,
    ) -> HoloLightTick {
        let light_source_gone = light.light_source.is_some_and(|s| {
            !self
                .world
                .realized(index)
                .is_some_and(|r| r.light_sources.iter().any(|l| l.id == s && !l.slated))
        });
        let c = self.world.creature(light.overseer);
        let body = c.overseer_body();
        let eye = body.map(|b| {
            if b.in_room {
                observer.overseer_eye(c.id, b).unwrap_or(b.chunk().pos)
            } else {
                b.chunk().pos
            }
        });
        let here = body.is_some_and(|b| b.in_room && c.pos.room == light.room);
        let player = self
            .player
            .as_ref()
            .filter(|p| !p.in_shortcut() && !p.is_destroyed());
        let player_room = player.map(|_| self.pos().room);
        let chunks = self.player.as_ref().map(|p| &p.body.chunks);
        HoloLightTick {
            eye,
            overseer_chunk: body.map_or((light.proj_pos, light.proj_pos), |b| {
                (b.chunk().pos, b.chunk().last_pos)
            }),
            overseer_extended: body.map_or(0.0, |b| b.extended),
            projector_active: here && body.is_some_and(|b| b.mode != OverseerMode::Zipping),
            still_the_guides: c
                .overseer_ai()
                .and_then(|a| a.real_ai.as_ref())
                .and_then(|ai| ai.communication.as_ref())
                .is_none_or(|m| m.holo_light == Some(light.id)),
            player_here: player_room == Some(light.room),
            player_elsewhere: player_room.is_some_and(|r| r != light.room),
            player_entering_shortcut: player.is_some_and(|p| p.entering_shortcut().is_some()),
            main_chunk: chunks.map_or_else(Default::default, |c| (c[0].pos, c[0].last_pos)),
            hips: chunks.map_or_else(Default::default, |c| c[1].pos),
            needed: self.holo_light_needed(),
            light_source_gone,
        }
    }
}
