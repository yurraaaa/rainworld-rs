//! The game's side of the superstructure's electric objects: `Room.Loaded` making the
//! `SSLightRod`, `ZapCoil`, `SuperStructureFuses` and `ZapCoilLight` (and `AIMapReady`
//! finding each light's coil), their turns in the room's update list, and the coil's zaps
//! over the room's `physicalObjects`: the flash, the kick away from the coil, and the
//! death of a creature. Types: [`crate::ss_light_rod`], [`crate::zap_coil`],
//! [`crate::ss_fuses`].
//!
//! Mirrors: Room.cs (Loaded: SSLightRod, ZapCoil, SuperStructureFuses, LightFixture; AIMapReady), ZapCoil.cs (Update), ZapCoilLight.cs (AIMapReady), SuperStructureFuses.cs (constructor, Update)

use std::sync::Arc;

use rw_assets::{GridRectData, RoomEffectType, SsLightRodData};

use super::RainWorldGame;
use crate::item::Entity;
use crate::ss_fuses::{FuseSetup, SuperStructureFuses};
use crate::ss_light_rod::{RodCamera, SsLightRod};
use crate::world::RealizedRoom;
use crate::zap_coil::{CoilEnv, ZapCoil, ZapCoilLight};
use crate::{ChunkOwner, LightSource, Vec2};

impl RainWorldGame {
    /// `Room.Loaded`'s `new SSLightRod(placedObject, room)`: a light for every 45 px of
    /// it (each added to the room, with its three draws), then the rod.
    pub(super) fn add_light_rod(&mut self, index: i32, pos: Vec2, data: SsLightRodData) {
        let ids: Vec<u64> = (0..SsLightRod::light_count(data.length))
            .map(|_| self.world.next_update_seq())
            .collect();
        let order = self.world.next_update_seq();
        let rod = SsLightRod::new(order, pos, data, &ids, &mut self.rng);
        if let Some(r) = self.world.realized_mut(index) {
            for &id in &ids {
                r.light_sources
                    .push(SsLightRod::light_source(id, index, order, pos));
            }
            r.fixtures.light_rods.push(rod);
        }
    }

    /// `Room.Loaded`'s `new ZapCoil(rect, room)`.
    pub(super) fn add_zap_coil(&mut self, index: i32, pos: Vec2, data: GridRectData) {
        let order = self.world.next_update_seq();
        let coil = ZapCoil::new(order, data.rect(pos.to_array()));
        if let Some(r) = self.world.realized_mut(index) {
            r.fixtures.zap_coils.push(coil);
        }
    }

    /// `Room.Loaded`'s `new SuperStructureFuses(placedObject, rect, room)`: the cell size
    /// from the region, the depth from the free tiles in the rectangle, and how broken it
    /// is (the `CorruptionSpores` amount; outside Five Pebbles' and the Underhang's
    /// regions a story session makes it 1).
    pub(super) fn add_fuses(&mut self, index: i32, pos: Vec2, data: GridRectData) {
        let region = self.world.region_name().map(str::to_owned);
        let size = region
            .as_ref()
            .map_or(10.0, |_| self.world.properties().params.fuse_size);
        let arena = !self.world.story;
        let order = self.world.next_update_seq();
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let rect = data.rect(pos.to_array());
        let mut depth = 0;
        for i in rect[0]..=rect[2] {
            for j in rect[1]..=rect[3] {
                let tile = r.room.tile(i, j);
                if !tile.is_solid() {
                    depth = depth.max(if tile.wall_behind() { 1 } else { 2 });
                }
            }
        }
        let broken = SuperStructureFuses::broken_for(
            r.settings.effect_amount(RoomEffectType::CorruptionSpores),
            arena,
            region.as_deref(),
        );
        let setup = FuseSetup {
            pos,
            rect,
            size,
            depth,
            broken,
            gravity_dependent: r.settings.effect_amount(RoomEffectType::BrokenZeroG) > 0.0,
        };
        r.fixtures.fuses.push(SuperStructureFuses::new(order, setup));
    }

    /// `Room.Loaded`'s `new ZapCoilLight(room, placedObject, data)`: its light first.
    pub(super) fn add_zap_coil_light(&mut self, index: i32, pos: Vec2, random_seed: i32) {
        let light = self.world.next_update_seq();
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.light_sources.push(ZapCoilLight::light_source(
                light,
                index,
                order,
                pos,
                random_seed,
            ));
            r.fixtures.zap_coil_lights.push(ZapCoilLight {
                update_order: order,
                pos,
                random_seed,
                light,
                coil: None,
            });
        }
    }

    /// `AIMapReady` of every `ZapCoilLight` of room `index`: its coil is the first one in the
    /// update list whose rectangle holds the light.
    pub(super) fn link_zap_coil_lights(&mut self, index: i32) {
        if let Some(r) = self.world.realized_mut(index) {
            let coils = &r.fixtures.zap_coils;
            for l in &mut r.fixtures.zap_coil_lights {
                l.find_coil(coils);
            }
        }
    }

    /// `SSLightRod.Update` of the rod at `order` in room `index`.
    pub(super) fn run_light_rod(&mut self, index: i32, order: u64) {
        let cycle = self.world.rain_cycle;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let room = Arc::clone(&r.room);
        let power = r.electric_power(&cycle);
        let RealizedRoom {
            fixtures,
            light_sources,
            ..
        } = r;
        let Some(rod) = fixtures
            .light_rods
            .iter_mut()
            .find(|x| x.update_order == order)
        else {
            return;
        };
        let cam = RodCamera {
            view: self.camera_view,
            room_index: index,
            room: &room,
            image: self.camera_image.as_ref().filter(|c| c.room == index),
        };
        rod.update(light_sources, power, &cam, &mut self.rng);
    }

    /// What `ZapCoil.Update` reads of room `index` and the world.
    fn coil_env(&self, index: i32) -> Option<CoilEnv> {
        let r = self.world.realized(index)?;
        Some(CoilEnv {
            fully_loaded: r.fully_loaded(),
            gravity: r.gravity,
            broken_zero_g: r.settings.effect_amount(RoomEffectType::BrokenZeroG) > 0.0,
            switch_settled_on: self
                .world
                .rain_cycle
                .broken_anti_grav
                .is_some_and(|b| b.to == 1.0 && b.progress == 1.0),
        })
    }

    /// `ZapCoil.Update` of the coil at `order` in room `index`: its loops, the zaps while
    /// it is on, then the disruption and the rest.
    pub(super) fn run_zap_coil(&mut self, index: i32, order: u64) {
        let Some(mut coil) = self.world.realized(index).and_then(|r| {
            r.fixtures
                .zap_coils
                .iter()
                .find(|c| c.update_order == order)
                .cloned()
        }) else {
            return;
        };
        let Some(env) = self.coil_env(index) else {
            return;
        };
        self.with_loop_cx(index, |cx| coil.update_loops(index, cx));
        if coil.turned_on > 0.5 {
            self.zap_coil_zaps(index, &mut coil);
        }
        coil.step(&env, &mut self.rng);
        self.put_coil(index, coil);
    }

    fn put_coil(&mut self, index: i32, coil: ZapCoil) {
        if let Some(c) = self.world.realized_mut(index).and_then(|r| {
            r.fixtures
                .zap_coils
                .iter_mut()
                .find(|c| c.update_order == coil.update_order)
        }) {
            *c = coil;
        }
    }

    /// The `SoundEmitter.Update` of the zap coil loop emitter at `order` in room `index`.
    pub(super) fn run_zap_coil_emitter(&mut self, index: i32, order: u64) {
        let Some(mut coil) = self.world.realized(index).and_then(|r| {
            r.fixtures
                .zap_coils
                .iter()
                .find(|c| c.emitters().any(|e| e.update_order == order))
                .cloned()
        }) else {
            return;
        };
        self.with_loop_cx(index, |cx| coil.update_emitter(order, cx));
        self.put_coil(index, coil);
    }

    /// Whether creature `id` (the player included) is dead.
    fn zapped_dead(&self, id: crate::creature::CreatureId) -> bool {
        if id == self.player_id {
            self.player.as_ref().is_none_or(crate::Player::is_dead)
        } else {
            self.world.realized_dead(id)
        }
    }

    /// The zap loop of `ZapCoil.Update`: every chunk of every physical object, layer by
    /// layer, resting against a face of the coil's long side with its outer 30 px inside
    /// the rectangle is zapped: the flash, the kick off the coil, and a creature dies.
    fn zap_coil_zaps(&mut self, index: i32, coil: &mut ZapCoil) {
        let player_seq = self.player_here(index).then_some(self.player_seq);
        for (_, _, owner) in self.world.physical_objects(index, player_seq) {
            let creature = match owner {
                ChunkOwner::Player => Some(self.player_id),
                ChunkOwner::Entity(Entity::Creature(id)) => Some(id),
                ChunkOwner::Entity(Entity::Object(_)) => None,
            };
            for i in 0.. {
                let Some(chunk) = self.electric_chunk(owner, i) else {
                    break;
                };
                let contact = chunk.contact();
                let touching = if coil.horizontal {
                    contact.y != 0
                } else {
                    contact.x != 0
                };
                if !touching {
                    continue;
                }
                let vector = Vec2::new(f32::from(contact.x), f32::from(contact.y));
                if !coil.inside(chunk.pos + vector * (chunk.rad + 30.0)) {
                    continue;
                }
                self.trigger_coil_zap(index, coil, chunk.pos + vector * chunk.rad, chunk.rad);
                let kick = (vector * 6.0 + Vec2::random_unit(&mut self.rng) * self.rng.value())
                    / chunk.mass;
                if let Some(c) = self.electric_chunk_mut(owner, i) {
                    c.vel -= kick;
                }
                if let Some(id) = creature
                    && !self.zapped_dead(id)
                {
                    self.realized_creature_die(id);
                }
            }
        }
    }

    /// `ZapCoil.TriggerZap`: the flash joins the room.
    fn trigger_coil_zap(&mut self, index: i32, coil: &mut ZapCoil, at: Vec2, rad: f32) {
        let order = self.world.next_update_seq();
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let flash = coil.trigger_zap(at, rad, order, &mut self.rng, sounds);
        if let Some(r) = self.world.realized_mut(index) {
            r.fixtures.zap_flashes.push(flash);
        }
    }

    /// `ZapCoilLight.Update` of the light at `order` in room `index`.
    pub(super) fn run_zap_coil_light(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let RealizedRoom {
            fixtures,
            light_sources,
            ..
        } = r;
        let Some(light) = fixtures
            .zap_coil_lights
            .iter()
            .find(|l| l.update_order == order)
        else {
            return;
        };
        let coil = light
            .coil
            .and_then(|o| fixtures.zap_coils.iter().find(|c| c.update_order == o))
            .map(|c| (c.turned_on, c.smooth_disruption));
        light.update(light_sources, coil, &mut self.rng);
    }

    /// `ZapFlash.Update` of the flash at `order` in room `index`: the first one makes its
    /// light, and past its life the flash and its light are destroyed.
    pub(super) fn run_zap_flash(&mut self, index: i32, order: u64) {
        let needs_light = self.world.realized(index).is_some_and(|r| {
            r.fixtures
                .zap_flashes
                .iter()
                .any(|f| f.update_order == order && f.light.is_none())
        });
        let id = needs_light.then(|| self.world.next_update_seq());
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(flash) = r
            .fixtures
            .zap_flashes
            .iter_mut()
            .find(|f| f.update_order == order)
        else {
            return;
        };
        if let Some(id) = id {
            flash.light = Some(id);
            r.light_sources.push(flash.light_source(id, index));
        }
        if flash.update()
            && let Some(l) = flash
                .light
                .and_then(|id| LightSource::find_mut(&mut r.light_sources, id))
        {
            l.slated = true;
        }
    }

    /// `SuperStructureFuses.Update` of the grid at `order` in room `index`.
    pub(super) fn run_fuses(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let gravity = r.gravity;
        let room = Arc::clone(&r.room);
        let Some(fuses) = r.fixtures.fuses.iter_mut().find(|f| f.update_order == order) else {
            return;
        };
        let viewed =
            (0..4).any(|i| self.camera_view.viewed(index, &room, fuses.corner(i), 40.0));
        fuses.update(viewed, gravity, &mut self.rng);
    }
}

#[cfg(test)]
#[path = "game_ss_electric_tests.rs"]
mod tests;
