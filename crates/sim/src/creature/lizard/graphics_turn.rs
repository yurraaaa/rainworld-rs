//! The game's side of a lizard's graphics module (`LizardGraphics`, built in
//! `rw-graphics`): its `Update` after the lizard's (and once more while it enters a
//! shortcut), what it adds to the room (bubbles, drips, its head light), its belly-drag
//! loop's emitter, the bubbles of a hard landing, and the cosmetics tied to it.
//!
//! Mirrors: LizardGraphics.cs (Update)

use std::sync::Arc;

use super::{PlayerObserver, RainWorldGame, RoomObject};
use crate::creature::CreatureId;
use crate::creature::lizard::{LizardBubble, LizardGraphicsTick, LizardSpawn};
use crate::effects::cosmetic::{CosmeticKind, CosmeticTie};
use crate::effects::lights::light_source::{LightSource, LightTie};
use crate::{FloatExt, TerrainImpact, Vec2};

impl RainWorldGame {
    /// `graphicsModule.Update()` of lizard `id` in room `index`, when it has one.
    pub(super) fn lizard_graphics_update(
        &mut self,
        index: i32,
        id: CreatureId,
        observer: &mut dyn PlayerObserver,
    ) {
        if self
            .world
            .creature(id)
            .lizard()
            .is_none_or(|l| l.graphics.is_none())
        {
            return;
        }
        let Some(room) = self.world.realized(index).map(|r| Arc::clone(&r.room)) else {
            return;
        };
        let grabbed = self.lizard_grabbed_creature_pos(id);
        let (looks, forced_look) = self.lizard_look_view(id);
        let Some(mut link) = self.world.lizard_at(id).graphics.take() else {
            return;
        };
        let light_slated = link
            .light
            .is_none_or(|l| self.world.light_source(l).is_none_or(|s| s.slated));
        let water = self.world.realized_mut(index).and_then(|r| r.water.take());
        let camera_room = self
            .world
            .realized(self.camera_room)
            .map_or_else(|| Arc::clone(&room), |r| Arc::clone(&r.room));
        let triggers = Arc::clone(self.sounds.table());
        let darkness = if index == self.camera_room {
            self.camera_darkness
        } else {
            0.0
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = LizardGraphicsTick {
            room_index: index,
            room: &room,
            water: water.as_ref(),
            cameras: room.cameras().len(),
            camera_room: &camera_room,
            camera: &self.camera_view,
            rng: &mut self.rng,
            sounds,
            triggers: &triggers,
            tick: self.tick,
            update_seq: self.world.update_seq(),
            darkness,
            light_slated,
            added: Vec::new(),
            light_upkeep: None,
        };
        if let Some(mut view) = self.world.lizard_view(id, grabbed) {
            view.looks = &looks;
            view.forced_look = forced_look;
            observer.lizard_updated(&view, &mut link, &mut cx);
        }
        let (seq, added, upkeep) = (cx.update_seq, cx.added, cx.light_upkeep);
        self.world.set_update_seq(seq);
        if let Some(r) = self.world.realized_mut(index) {
            r.water = water;
        }
        for (seq, spawn) in added {
            match spawn {
                LizardSpawn::Drip(d) => {
                    self.world.add_drip_at(index, d, seq);
                }
                LizardSpawn::Bubble(b) => {
                    self.world
                        .add_cosmetic_at(index, seq, CosmeticKind::LizardBubble(Box::new(b)));
                }
                LizardSpawn::Light { pos, color } => {
                    let mut light =
                        LightSource::new(seq, index, pos, false, color, Some(LightTie::Lizard(id)));
                    light.require_up_keep = true;
                    if let Some(r) = self.world.realized_mut(index) {
                        r.light_sources.push(light);
                    }
                    link.light = Some(seq);
                }
            }
        }
        if let Some(u) = upkeep
            && let Some(light) = self.world.light_source_mut(u.id)
        {
            light.stay_alive = true;
            light.set_pos = Some(u.pos);
            light.set_rad = Some(u.rad);
            light.set_color(u.color);
            light.set_alpha = Some(u.alpha);
        }
        self.world.lizard_at(id).graphics = Some(link);
        self.lizard_apply_look_reads(id, &looks);
    }

    /// `graphicsModule.SuckedIntoShortCut(pos)` while lizard `id` enters a shortcut.
    pub(super) fn lizard_graphics_sucked(
        &mut self,
        id: CreatureId,
        pos: Vec2,
        observer: &mut dyn PlayerObserver,
    ) {
        let entity = self.world.creature(id).id;
        if let Some(link) = self
            .world
            .creature_mut(id)
            .lizard_mut()
            .and_then(|l| l.graphics.as_mut())
        {
            observer.lizard_sucked_into_shortcut(entity, link, pos);
        }
    }

    /// The position of the chunk lizard `id`'s jaws hold, when it is a creature's.
    fn lizard_grabbed_creature_pos(&self, id: CreatureId) -> Option<Vec2> {
        let g = self.world.creature(id).grasps.first().copied().flatten()?;
        g.grabbed.creature()?;
        self.chunk_of(super::lizard_hits::ChunkRef {
            owner: g.grabbed,
            index: g.chunk_grabbed,
        })
        .map(|c| c.pos)
    }

    /// `LizardGraphics.TerrainImpact` from `Lizard.TerrainImpact` (a first contact over
    /// 10 px a tick, with graphics): one roll, then on the head or 60% of the time
    /// `1 + InverseLerp(20, 90, speed) * 3` bubbles thrown off the surface.
    pub(super) fn lizard_impact_bubbles(
        &mut self,
        index: i32,
        id: CreatureId,
        impact: &TerrainImpact,
    ) {
        if !impact.first_contact || impact.speed <= 10.0 {
            return;
        }
        let holding = self.lizard_holding(id);
        let l = self.world.lizard_ref(id);
        let Some(link) = &l.graphics else {
            return;
        };
        let head = link.head.for_bubble(l.params, l.jaw_open(holding));
        let module = link.module;
        let voice = link.voice.visualization;
        let main = l.body.chunks[0].pos;
        if self.rng.value() >= 0.6 && impact.chunk != 0 {
            return;
        }
        let n = 1 + (impact.speed.inverse_lerp(20.0, 90.0) * 3.0) as i32;
        for _ in 0..n {
            let stickiness = self.rng.value();
            let params = [
                1.0,
                stickiness,
                impact.speed / 15.0 + voice.mathf_pow(2.0) * 20.0,
            ];
            let mut b = LizardBubble::new((id, module), main, head, params, &mut self.rng);
            let d = impact.direction;
            if d.x != 0 {
                b.vel.x = b.vel.x.abs() * -f32::from(d.x);
            }
            if d.y != 0 {
                b.vel.y = b.vel.y.abs() * -f32::from(d.y);
            }
            self.world
                .add_cosmetic(index, CosmeticKind::LizardBubble(Box::new(b)));
        }
    }

    /// The lizards' graphics' loop emitters, held, let go or left by dropped graphics,
    /// in room `index`'s update list.
    pub(super) fn room_lizard_loop_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .creatures()
            .filter_map(|(id, c)| Some((id, c.lizard()?)))
            .flat_map(|(owner, l)| {
                l.graphics
                    .iter()
                    .flat_map(|g| g.sound_loop.emitters())
                    .chain(l.dropped_loop_emitters.iter().filter(|e| !e.slated))
                    .filter(move |e| e.room == index)
                    .map(move |e| {
                        let order = e.update_order;
                        (order, RoomObject::LizardLoop { owner, order })
                    })
            })
            .collect()
    }

    /// The `ChunkSoundEmitter.Update` of lizard `owner`'s graphics' loop emitter that
    /// joined room `index` at `order`.
    pub(super) fn run_lizard_loop(&mut self, index: i32, owner: CreatureId, order: u64) {
        let Some(l) = self.world.creature_mut(owner).lizard_mut() else {
            return;
        };
        let (owner_room, pos) = (l.room, l.body.chunks[0].pos);
        let Some(e) = l
            .graphics
            .as_mut()
            .and_then(|g| g.sound_loop.emitter_mut(order))
            .or_else(|| {
                l.dropped_loop_emitters
                    .iter_mut()
                    .find(|e| e.update_order == order)
            })
            .filter(|e| e.room == index && !e.slated)
        else {
            return;
        };
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        sounds.move_emitter(e.sound, pos);
        e.update(owner_room, self.camera_room, self.tick);
        if e.slated {
            sounds.stop_emitter(e.sound);
            if let Some(g) = &mut l.graphics {
                g.sound_loop.released.retain(|e| !e.slated);
            }
            l.dropped_loop_emitters.retain(|e| !e.slated);
        }
    }

    /// Cosmetic `id`'s `Update` in room `index`, reading the graphics of the lizard it
    /// belongs to.
    pub(super) fn run_cosmetic(&mut self, index: i32, id: u64) {
        let spark_flash = self
            .world
            .room_cosmetics(index)
            .iter()
            .any(|c| c.id == id && matches!(c.kind, CosmeticKind::SparkFlash(_)));
        if spark_flash {
            self.run_spark_flash(index, id);
            return;
        }
        let Some(c) = self.world.room_cosmetics(index).iter().find(|c| c.id == id) else {
            return;
        };
        let (lizard, pos) = match &c.kind {
            CosmeticKind::Spark(s) => (s.lizard, s.pos),
            CosmeticKind::LizardBubble(b) => (Some((b.lizard, b.module)), b.pos),
            CosmeticKind::Flash(f) => (f.lizard, f.pos),
            CosmeticKind::Spikes(_)
            | CosmeticKind::OverseerEffect(_)
            | CosmeticKind::Bubble(_)
            | CosmeticKind::Splash(_)
            | CosmeticKind::SparkFlash(_)
            | CosmeticKind::NeuronSpark(_) => (None, Vec2::ZERO),
        };
        let mut tie = CosmeticTie {
            water_level: self
                .world
                .realized(index)
                .and_then(|r| r.water.as_ref())
                .map_or(f32::MIN, |w| w.level_at(pos.x)),
            ..CosmeticTie::default()
        };
        // It reads only the module it was given; a dropped one no longer changes.
        if let Some((owner, module)) = lizard {
            let holding = self.lizard_holding(owner);
            if let Some(l) = self.world.creature(owner).lizard()
                && let Some(g) = &l.graphics
                && g.module == module
            {
                tie.white_flicker = Some(g.white_flicker);
                tie.head = Some(g.head.for_bubble(l.params, l.jaw_open(holding)));
                tie.voice = Some(g.voice);
            }
        }
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        self.world
            .update_cosmetic(index, id, tie, &mut self.rng, sounds);
    }
}
