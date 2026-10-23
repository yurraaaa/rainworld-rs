//! The game's side of the depth-faded particles: `Room.Loaded` adding the
//! `SkyDandelions` and `GreenSparks` objects (and, in the same effect loop, the
//! `AboveCloudsView` and `RoofTopView` scenes of [`super::background_scene`]) (with the dandelions the spawner makes in
//! its constructor), `Room.NowViewed` seeding sparks, specks and spores when camera 0
//! comes to the room, each particle's turn in its room's update list, a spark's
//! `LightSource`, and what the dandelion stalks read of the room's bodies. Types:
//! [`crate::SkyDandelions`], [`crate::GreenSparks`], [`crate::ZeroGSpeck`].
//!
//! Mirrors: Room.cs (Loaded, NowViewed: SkyDandelions, GreenSparks, ZeroGSpecks, CorruptionSpores), GreenSparks.cs (GreenSpark.Update: its light)

use rw_assets::{PlacedObjectType, ResizableData, RoomEffectType};

use super::{RainWorldGame, RoomObject};
use crate::insect::InsectWater;
use crate::superstructure::projections::projector::SuperStructureProjector;
use crate::superstructure::projections::room_projections::RoomProjections;
use crate::superstructure::projections::scan_lines::ProjectedScanLines;
use crate::trigger::TriggerObjectKind;
use crate::world::{RealizedRoom, World};
use crate::{
    BodyChunk, ChunkOwner, ChunkView, DandelionPatch, GreenSpark, GreenSparks, LightSource,
    LightTie, MadeDandelion, RoomParticles, SkyDandelions, SparkCx, SpeckCx, SpeckType, Vec2,
    ZeroGSpeck,
};

/// A light source's radius for a green spark (`light.setRad`).
const SPARK_LIGHT_RAD: f32 = 80.0;

/// A particle in a room's update list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParticleRef {
    Dandelion(u64),
    Stalk(u64),
    DandelionSpawner(u64),
    Spark(u64),
    SparkSpawner(u64),
    Speck(u64),
}

impl World {
    /// Every body chunk of room `index` in `physicalObjects` order, as the dandelion
    /// stalks read them. `player` is the player's body in this room with its place in
    /// the update list.
    pub(crate) fn chunk_views(
        &self,
        index: i32,
        player: Option<(u64, &[BodyChunk])>,
    ) -> Vec<ChunkView> {
        let player_seq = player.map(|(seq, _)| seq);
        let mut views = Vec::new();
        for (_, _, owner) in self.physical_objects(index, player_seq) {
            let chunks = match owner {
                ChunkOwner::Player => player.map(|(_, c)| c),
                ChunkOwner::Entity(e) => self.surface_chunks(e),
                ChunkOwner::Segment(s) => self.segment_chunks(s),
            };
            views.extend(chunks.into_iter().flatten().map(|c| ChunkView {
                pos: c.pos,
                last_pos: c.last_pos,
                rad: c.rad,
            }));
        }
        views
    }
}

impl RainWorldGame {
    /// The room's effects in the settings' order, the ones the particles read.
    fn particle_effects(&self, index: i32) -> Vec<(RoomEffectType, f32)> {
        let mods = self.world.mods();
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.settings
                .effects
                .iter()
                .filter_map(|e| Some((RoomEffectType::from_value(&e.kind, mods)?, e.amount)))
                .collect()
        })
    }

    /// `rainCycle.ProximityToMiddleOfCycle` and `globalRain.Intensity` (0: the rain is
    /// not ported).
    fn dandelion_weather(&self) -> (f32, f32) {
        (self.world.rain_cycle.proximity_to_middle_of_cycle(), 0.0)
    }

    /// `Room.Loaded`'s effects loop: each effect's object (the sky dandelions with the
    /// dandelions their constructor scatters, the sparks, views, `Lightning`,
    /// `ElectricDeath`, `RoomBorderPushBack`) joins the update list in the settings' order.
    pub(super) fn add_room_effect_particles(&mut self, index: i32) {
        for (kind, amount) in self.particle_effects(index) {
            match kind {
                RoomEffectType::SkyDandelions => self.add_sky_dandelions(index, amount),
                RoomEffectType::GreenSparks => self.add_green_sparks(index, amount),
                RoomEffectType::AboveCloudsView => self.add_above_clouds_view(index, amount),
                RoomEffectType::RoofTopView => self.add_roof_top_view(index),
                RoomEffectType::ZeroG | RoomEffectType::BrokenZeroG => self.add_anti_gravity(index),
                RoomEffectType::SSMusic => {
                    self.add_trigger_object(index, TriggerObjectKind::SsMusic { amount });
                }
                RoomEffectType::Lightning => self.add_lightning(index, amount, false),
                RoomEffectType::BkgOnlyLightning => self.add_lightning(index, amount, true),
                RoomEffectType::ElectricDeath => self.add_electric_death(index, amount),
                RoomEffectType::BorderPushBack => self.add_border_push_back(index),
                RoomEffectType::SSSwarmers => self.add_coral_system(index),
                _ => {}
            }
        }
    }

    fn add_sky_dandelions(&mut self, index: i32, amount: f32) {
        let weather = self.dandelion_weather();
        let mods = self.world.mods();
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let patches: Vec<DandelionPatch> = r
            .settings
            .placed_objects
            .iter()
            .filter(|o| o.object_type(mods) == Some(PlacedObjectType::DandelionPatch))
            .filter_map(|o| {
                ResizableData::parse(&o.data).map(|d| DandelionPatch {
                    pos: Vec2::from(o.pos),
                    rad: d.rad(),
                })
            })
            .collect();
        let mut spawner = SkyDandelions {
            update_order: 0,
            amount,
            wait: 0.0,
        };
        let made = spawner.generate(&r.room, index, &patches, weather, &mut self.rng);
        let (mut dandelions, mut stalks) = (Vec::new(), Vec::new());
        for m in made {
            match m {
                MadeDandelion::Free(mut d) => {
                    d.update_order = self.world.next_update_seq();
                    dandelions.push(d);
                }
                MadeDandelion::Planted(mut stalk, mut d) => {
                    stalk.update_order = self.world.next_update_seq();
                    d.update_order = self.world.next_update_seq();
                    stalk.dandelion = Some(d.update_order);
                    stalks.push(stalk);
                    dandelions.push(d);
                }
            }
        }
        spawner.update_order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.particles.dandelions.extend(dandelions);
            r.particles.stalks.extend(stalks);
            r.particles.dandelion_spawners.push(spawner);
        }
    }

    fn add_green_sparks(&mut self, index: i32, amount: f32) {
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            let mut spawner = GreenSparks::new(&r.room, amount);
            spawner.update_order = order;
            r.particles.spark_spawners.push(spawner);
        }
    }

    /// `Room.NowViewed`'s particles: sparks seeded over the room, then the specks and
    /// spores, for each effect in the settings' order. Runs when camera 0 comes to
    /// room `index` (every time, so revisiting a room seeds it again).
    pub(super) fn particles_now_viewed(&mut self, index: i32) {
        let effects = self.particle_effects(index);
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let ai_map = r.ai_map.as_deref().filter(|_| r.ready_for_ai());
        let tile_size = (r.room.width(), r.room.height());
        // Each object takes its place in the update list as it is made.
        let mut seq = self.world.update_seq();
        let (mut sparks, mut specks) = (Vec::new(), Vec::new());
        let mut projections = RoomProjections::default();
        for (kind, amount) in effects {
            match kind {
                RoomEffectType::GreenSparks => {
                    for mut s in GreenSpark::seed_room(&r.room, amount, ai_map, &mut self.rng) {
                        seq += 1;
                        s.update_order = seq;
                        sparks.push(s);
                    }
                }
                RoomEffectType::ZeroGSpecks => {
                    for _ in 0..SpeckType::Blink.count(amount) {
                        let mut s = ZeroGSpeck::new(SpeckType::Blink, &mut self.rng);
                        seq += 1;
                        s.update_order = seq;
                        specks.push(s);
                    }
                }
                RoomEffectType::CorruptionSpores => {
                    for _ in 0..SpeckType::CorruptionSpore.count(amount) {
                        let mut s = ZeroGSpeck::new(SpeckType::CorruptionSpore, &mut self.rng);
                        seq += 1;
                        s.update_order = seq;
                        specks.push(s);
                    }
                }
                RoomEffectType::SuperStructureProjector => {
                    projections.projectors.push(SuperStructureProjector::new(
                        tile_size,
                        amount,
                        &mut seq,
                        &mut self.rng,
                    ));
                }
                RoomEffectType::ProjectedScanLines => {
                    projections.scan_lines.push(ProjectedScanLines::new(
                        tile_size,
                        amount,
                        &mut seq,
                        &mut self.rng,
                    ));
                }
                _ => {}
            }
        }
        self.world.set_update_seq(seq);
        if let Some(r) = self.world.realized_mut(index) {
            r.particles.sparks.extend(sparks);
            r.particles.specks.extend(specks);
            r.projections.projectors.extend(projections.projectors);
            r.projections.scan_lines.extend(projections.scan_lines);
        }
    }

    /// The room's particles in its update list.
    pub(super) fn room_particle_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let Some(r) = self.world.realized(index) else {
            return Vec::new();
        };
        let p = &r.particles;
        let mut objects = Vec::new();
        objects.extend(
            p.dandelions
                .iter()
                .filter(|d| !d.slated)
                .map(|d| (d.update_order, ParticleRef::Dandelion(d.update_order))),
        );
        objects.extend(
            p.stalks
                .iter()
                .filter(|s| !s.slated)
                .map(|s| (s.update_order, ParticleRef::Stalk(s.update_order))),
        );
        objects.extend(p.dandelion_spawners.iter().map(|s| {
            (
                s.update_order,
                ParticleRef::DandelionSpawner(s.update_order),
            )
        }));
        objects.extend(
            p.sparks
                .iter()
                .filter(|s| !s.slated)
                .map(|s| (s.update_order, ParticleRef::Spark(s.update_order))),
        );
        objects.extend(
            p.spark_spawners
                .iter()
                .map(|s| (s.update_order, ParticleRef::SparkSpawner(s.update_order))),
        );
        objects.extend(
            p.specks
                .iter()
                .filter(|s| !s.slated)
                .map(|s| (s.update_order, ParticleRef::Speck(s.update_order))),
        );
        objects
            .into_iter()
            .map(|(order, p)| (order, RoomObject::Particle(p)))
            .collect()
    }

    /// The particle's `Update` in its slot of room `index`'s update list.
    pub(super) fn run_particle(&mut self, index: i32, particle: ParticleRef) {
        match particle {
            ParticleRef::Dandelion(order) => self.run_dandelion(index, order),
            ParticleRef::Stalk(order) => self.run_dandelion_stalk(index, order),
            ParticleRef::DandelionSpawner(order) => self.run_dandelion_spawner(index, order),
            ParticleRef::Spark(order) => self.run_green_spark(index, order),
            ParticleRef::SparkSpawner(order) => self.run_green_sparks(index, order),
            ParticleRef::Speck(order) => self.run_speck(index, order),
        }
    }

    /// Drops the particles destroyed during room `index`'s update.
    pub(super) fn remove_slated_particles(&mut self, index: i32) {
        if let Some(r) = self.world.realized_mut(index) {
            r.particles.remove_slated();
        }
    }

    fn run_dandelion(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let RealizedRoom {
            room,
            water,
            particles,
            ..
        } = r;
        let water = InsectWater::new(water.as_ref(), room);
        if let Some(d) = particles.dandelion_mut(order).filter(|d| !d.slated) {
            d.update(room, &water, &mut self.rng);
        }
    }

    fn run_dandelion_stalk(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let attached = r
            .particles
            .stalks
            .binary_search_by_key(&order, |s| s.update_order)
            .ok()
            .and_then(|i| r.particles.stalks[i].dandelion);
        let chunks = if attached.is_some() {
            self.room_chunk_views(index)
        } else {
            Vec::new()
        };
        let camera = self.camera_view;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let RealizedRoom {
            room, particles, ..
        } = r;
        let RoomParticles {
            stalks, dandelions, ..
        } = particles;
        let Ok(i) = stalks.binary_search_by_key(&order, |s| s.update_order) else {
            return;
        };
        if stalks[i].slated {
            return;
        }
        let dandelion = attached
            .and_then(|o| dandelions.binary_search_by_key(&o, |d| d.update_order).ok())
            .map(|j| &mut dandelions[j]);
        stalks[i].update(dandelion, room, index, &camera, &chunks, &mut self.rng);
    }

    /// Room `index`'s bodies' chunks, the player's among them if it is there.
    fn room_chunk_views(&self, index: i32) -> Vec<ChunkView> {
        let player_room = self.pos().room;
        let seq = self.player_seq;
        let here = self
            .player
            .as_ref()
            .filter(|p| player_room == index && !p.in_shortcut())
            .map(|p| (seq, &p.body.chunks[..]));
        self.world.chunk_views(index, here)
    }

    fn run_dandelion_spawner(&mut self, index: i32, order: u64) {
        let weather = self.dandelion_weather();
        let made = {
            let Some(r) = self.world.realized_mut(index) else {
                return;
            };
            let RealizedRoom {
                room, particles, ..
            } = r;
            particles
                .dandelion_spawners
                .iter_mut()
                .find(|s| s.update_order == order)
                .and_then(|s| s.update(room, weather, &mut self.rng))
        };
        if let Some(mut d) = made {
            d.update_order = self.world.next_update_seq();
            if let Some(r) = self.world.realized_mut(index) {
                r.particles.dandelions.push(d);
            }
        }
    }

    fn run_green_spark(&mut self, index: i32, order: u64) {
        let darkness = self.darkness(index);
        let being_viewed = index == self.camera_room;
        let camera = self.camera_view;
        let (light_wish, light) = {
            let Some(r) = self.world.realized_mut(index) else {
                return;
            };
            let RealizedRoom {
                room,
                water,
                particles,
                ..
            } = r;
            let water = InsectWater::new(water.as_ref(), room);
            let Ok(i) = particles
                .sparks
                .binary_search_by_key(&order, |s| s.update_order)
            else {
                return;
            };
            if particles.sparks[i].slated {
                return;
            }
            let cx = SparkCx {
                room,
                room_index: index,
                water: &water,
                camera: &camera,
                darkness,
                being_viewed,
            };
            let wish = particles.sparks[i].update(&cx, &mut self.rng);
            (wish, particles.sparks[i].light)
        };
        let light = self.green_spark_light_step(index, order, light_wish, light);
        if let Some(r) = self.world.realized_mut(index)
            && let Ok(i) = r
                .particles
                .sparks
                .binary_search_by_key(&order, |s| s.update_order)
        {
            r.particles.sparks[i].light = light;
        }
    }

    /// A spark's light after its update: made (with `AddObject`'s new place in the
    /// update list) and set while the spark is `lit`, destroyed when it no longer is.
    /// Returns the spark's light id.
    fn green_spark_light_step(
        &mut self,
        index: i32,
        order: u64,
        lit: Option<(Vec2, f32)>,
        light: Option<u64>,
    ) -> Option<u64> {
        let Some((pos, alpha)) = lit else {
            if let Some(id) = light
                && let Some(l) = self.world.light_source_mut(id)
            {
                l.slated = true;
            }
            return None;
        };
        let id = match light {
            Some(id) => id,
            None => {
                let id = self.world.next_update_seq();
                let mut l = LightSource::new(
                    id,
                    index,
                    pos,
                    false,
                    GreenSpark::COLOR,
                    Some(LightTie::GreenSpark(order)),
                );
                l.require_up_keep = true;
                if let Some(r) = self.world.realized_mut(index) {
                    r.light_sources.push(l);
                }
                id
            }
        };
        if let Some(l) = self.world.light_source_mut(id) {
            l.set_pos = Some(pos);
            l.set_alpha = Some(alpha);
            l.set_rad = Some(SPARK_LIGHT_RAD);
            l.stay_alive = true;
        }
        Some(id)
    }

    /// `LightSource.Update`'s `tiedToObject.slatedForDeletetion || room != room`, for a
    /// green spark's light.
    pub(super) fn green_spark_tie_gone(&self, index: i32, order: u64) -> bool {
        !self.world.realized(index).is_some_and(|r| {
            r.particles
                .sparks
                .binary_search_by_key(&order, |s| s.update_order)
                .is_ok_and(|i| !r.particles.sparks[i].slated)
        })
    }

    fn run_green_sparks(&mut self, index: i32, order: u64) {
        let camera = self.camera_view;
        let added = {
            let Some(r) = self.world.realized_mut(index) else {
                return;
            };
            let RealizedRoom {
                room, particles, ..
            } = r;
            let RoomParticles {
                sparks,
                spark_spawners,
                ..
            } = particles;
            spark_spawners
                .iter_mut()
                .find(|s| s.update_order == order)
                .and_then(|s| s.update(sparks, room, index, &camera, &mut self.rng))
        };
        if let Some(mut spark) = added {
            spark.update_order = self.world.next_update_seq();
            if let Some(r) = self.world.realized_mut(index) {
                if let Some(s) = r
                    .particles
                    .spark_spawners
                    .iter_mut()
                    .find(|s| s.update_order == order)
                {
                    s.sparks.push(spark.update_order);
                }
                r.particles.sparks.push(spark);
            }
        }
    }

    fn run_speck(&mut self, index: i32, order: u64) {
        let darkness = self.darkness(index);
        let being_viewed = index == self.camera_room;
        let camera_pos = self.camera_shake.pos;
        let current_camera = i32::try_from(self.camera_view.current).unwrap_or(0);
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let ready_for_ai = r.ready_for_ai();
        let RealizedRoom {
            room,
            ai_map,
            light_sources,
            particles,
            gravity,
            ..
        } = r;
        let Ok(i) = particles
            .specks
            .binary_search_by_key(&order, |s| s.update_order)
        else {
            return;
        };
        if particles.specks[i].slated {
            return;
        }
        let cx = SpeckCx {
            room,
            gravity: *gravity,
            ready_for_ai,
            ai_map: ai_map.as_deref(),
            camera_pos,
            current_camera,
            being_viewed,
            darkness,
            lights: light_sources,
        };
        particles.specks[i].update(&cx, &mut self.rng);
    }
}
