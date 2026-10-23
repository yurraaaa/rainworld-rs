//! The game's side of a room's `ElectricDeath`: `Room.Loaded` making it (and
//! `AIMapReady` listing the wall tiles), its turn in the update list with the sounds,
//! sparks and strikes it makes over the room's `physicalObjects`, the creatures' electric
//! `Violence`, its loop emitters' turns and its `SparkFlash`es' turns. Types:
//! [`crate::ElectricDeath`], [`crate::SparkFlash`].
//!
//! Mirrors: Room.cs (Loaded: ElectricDeath), ElectricDeath.cs (Update, AIMapReady)

use std::sync::Arc;

use super::lizard_hits::Hit;
use super::{RainWorldGame, RoomObject};
use crate::chunk::BodyChunk;
use crate::creature::DamageType;
use crate::electric_death::{ElectricDeath, SparkFlash, Strike};
use crate::item::{Entity, LoopCx};
use crate::{ChunkOwner, CosmeticKind, FloatExt, RoomExt, SoundId, Vec2};

impl RainWorldGame {
    /// `Room.Loaded`'s `ElectricDeath` for one such effect, joining the update list, and
    /// `AIMapReady`'s `closeToWallTiles`: every tile with `getTerrainProximity == 1`.
    pub(super) fn add_electric_death(&mut self, index: i32, amount: f32) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let mut tiles = Vec::new();
        if let Some(map) = &r.ai_map {
            for x in 0..r.room.width() {
                for y in 0..r.room.height() {
                    if map.terrain_proximity(x, y) == 1 {
                        tiles.push((x, y));
                    }
                }
            }
        }
        let order = self.world.next_update_seq();
        let mut death = ElectricDeath::new(order, amount);
        death.close_to_wall_tiles = Some(tiles);
        if let Some(r) = self.world.realized_mut(index) {
            r.electric_deaths.push(death);
        }
    }

    /// Room `index`'s `ElectricDeath`s and their loop emitters in its update list.
    pub(super) fn room_electric_death_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        let Some(r) = self.world.realized(index) else {
            return Vec::new();
        };
        let mut objects = Vec::new();
        for d in &r.electric_deaths {
            objects.extend(d.emitters().map(|e| {
                (
                    e.update_order,
                    RoomObject::ElectricDeathLoop(e.update_order),
                )
            }));
            objects.push((d.update_order, RoomObject::ElectricDeath(d.update_order)));
        }
        objects
    }

    /// `f` with what a sound loop in room `index` reads of the game.
    pub(super) fn with_loop_cx<R>(
        &mut self,
        index: i32,
        f: impl FnOnce(&mut LoopCx<'_>) -> R,
    ) -> R {
        let triggers = Arc::clone(self.sounds.table());
        let mut seq = self.world.update_seq();
        let (camera_room, tick) = (self.camera_room, self.tick);
        let sounds = if index == camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        let mut cx = LoopCx {
            room_index: index,
            sounds,
            rng: &mut self.rng,
            triggers: &triggers,
            camera_room,
            tick,
            update_seq: &mut seq,
        };
        let out = f(&mut cx);
        self.world.set_update_seq(seq);
        out
    }

    /// The `SoundEmitter.Update` of the `ElectricDeath` loop emitter at `order`.
    pub(super) fn run_electric_death_emitter(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(at) = r
            .electric_deaths
            .iter()
            .position(|d| d.emitters().any(|e| e.update_order == order))
        else {
            return;
        };
        let mut death = std::mem::replace(&mut r.electric_deaths[at], ElectricDeath::new(0, 0.0));
        self.with_loop_cx(index, |cx| death.update_emitter(order, cx));
        if let Some(r) = self.world.realized_mut(index) {
            r.electric_deaths[at] = death;
        }
    }

    /// `ElectricDeath.Update` of the object at `order` in room `index`.
    pub(super) fn run_electric_death(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(at) = r
            .electric_deaths
            .iter()
            .position(|d| d.update_order == order)
        else {
            return;
        };
        let mut death = std::mem::replace(&mut r.electric_deaths[at], ElectricDeath::new(0, 0.0));
        self.electric_death_update(index, &mut death);
        if let Some(r) = self.world.realized_mut(index) {
            r.electric_deaths[at] = death;
        }
    }

    fn electric_death_update(&mut self, index: i32, death: &mut ElectricDeath) {
        let intensity = death.intensity(&self.world.rain_cycle);
        if intensity == 0.0 {
            return;
        }
        self.with_loop_cx(index, |cx| death.update_loops(intensity, cx));
        death.advance_sin(intensity);
        let Some(room) = self.world.realized(index).map(|r| Arc::clone(&r.room)) else {
            return;
        };
        let viewed = self.camera_room == index;
        let area = room.width() * room.height();
        if let Some(spark) = death.roll_wall_spark(intensity, viewed, area, &mut self.rng) {
            if self.camera_view.viewed(index, &room, spark.pos, 50.0) {
                let flash = SparkFlash::new(spark.pos, spark.size, &mut self.rng);
                self.world
                    .add_cosmetic(index, CosmeticKind::SparkFlash(flash));
            }
            self.play_electric_sound(
                index,
                SoundId::DeathLightningSparkSpontaneous,
                ElectricSoundAt::Position(spark.pos),
                spark.size,
            );
        }
        death.update_flashes(intensity, self.world.screen_size(), &mut self.rng);
        death.update_color(&mut self.rng);
        if let Some(chance) = ElectricDeath::strike_chance(intensity, &mut self.rng) {
            self.electric_death_strikes(index, &room, death, intensity, chance);
        }
    }

    /// The strikes: every chunk of every physical object, layer by layer, rolls the
    /// chance; a struck one touching terrain (or on a beam) is kicked away from the wall,
    /// flashes and sounds, and a creature takes an electric `Violence`.
    fn electric_death_strikes(
        &mut self,
        index: i32,
        room: &rw_assets::Room,
        death: &mut ElectricDeath,
        intensity: f32,
        chance: f32,
    ) {
        let player_here = self.player_here(index);
        let player_seq = player_here.then_some(self.player_seq);
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
                if self.rng.value() >= chance {
                    continue;
                }
                let contact = chunk.contact();
                if contact.x == 0 && contact.y == 0 && !room.tile_at_pos(chunk.pos).any_pole() {
                    continue;
                }
                let power = ElectricDeath::strike_power(intensity, &mut self.rng);
                let contact_v = Vec2::new(f32::from(contact.x), f32::from(contact.y));
                let flash = SparkFlash::new(
                    chunk.pos + contact_v * chunk.rad,
                    power.mathf_pow(0.5),
                    &mut self.rng,
                );
                self.world
                    .add_cosmetic(index, CosmeticKind::SparkFlash(flash));
                let away = -(contact_v + Vec2::random_unit(&mut self.rng)).normalized();
                let kick = away * (22.0 * power / chunk.mass);
                if let Some(c) = self.electric_chunk_mut(owner, i) {
                    c.vel += kick;
                }
                self.play_electric_sound(
                    index,
                    SoundId::DeathLightningSparkObject,
                    ElectricSoundAt::Chunk(owner, i, chunk.pos),
                    power,
                );
                let mut strike = Strike {
                    tick: self.tick,
                    creature: creature.is_some(),
                    pos: chunk.pos,
                    power,
                    damage: 0.0,
                    stun_bonus: power * 40.0,
                };
                if let Some(id) = creature {
                    let mut damage = power * 1.8;
                    if self.world.rain_cycle.time_until_rain() <= -1500 && intensity >= 0.5 {
                        damage = self.rng.value().mathf_pow(0.9) * 2.0;
                    }
                    strike.damage = damage;
                    self.creature_violence(
                        id,
                        Hit {
                            source: None,
                            direction_and_momentum: None,
                            chunk: Some(i),
                            kind: DamageType::Electric,
                            damage,
                            stun_bonus: power * 40.0,
                        },
                    );
                }
                death.record_strike(strike);
            }
        }
    }

    /// Whether the player's body is in room `index`'s `physicalObjects`.
    pub(super) fn player_here(&self, index: i32) -> bool {
        self.player
            .as_ref()
            .is_some_and(|p| self.pos().room == index && !p.in_shortcut())
    }

    /// A copy of chunk `i` of a physical object's body.
    pub(super) fn electric_chunk(&self, owner: ChunkOwner, i: usize) -> Option<BodyChunk> {
        match owner {
            ChunkOwner::Player => self.player.as_ref()?.body.chunks.get(i).cloned(),
            ChunkOwner::Entity(e) => self.world.surface_chunks(e)?.get(i).cloned(),
        }
    }

    pub(super) fn electric_chunk_mut(
        &mut self,
        owner: ChunkOwner,
        i: usize,
    ) -> Option<&mut BodyChunk> {
        match owner {
            ChunkOwner::Player => self.player.as_mut()?.body.chunks.get_mut(i),
            ChunkOwner::Entity(e) => self.world.surface_chunks_mut(e)?.get_mut(i),
        }
    }

    /// `Room.PlaySound(id, pos, volume, 1)` or `Room.PlaySound(id, chunk, false, volume, 1)`.
    fn play_electric_sound(&mut self, index: i32, id: SoundId, at: ElectricSoundAt, volume: f32) {
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
        match at {
            ElectricSoundAt::Position(pos) => {
                sounds.play_at(id, pos, volume, 1.0, &mut self.rng);
            }
            ElectricSoundAt::Chunk(ChunkOwner::Player, chunk, _) => {
                sounds.play_on_chunk(id, chunk, false, volume, 1.0, &mut self.rng);
            }
            ElectricSoundAt::Chunk(ChunkOwner::Entity(_), _, pos) => {
                sounds.play_on_object(id, pos, false, volume, 1.0, &mut self.rng);
            }
        }
    }

    /// A `SparkFlash`'s `Update` in room `index`: its spark, then its life.
    pub(super) fn run_spark_flash(&mut self, index: i32, id: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let Some(c) = r.cosmetics.iter_mut().find(|c| c.id == id) else {
            return;
        };
        let CosmeticKind::SparkFlash(flash) = &mut c.kind else {
            return;
        };
        let spark = flash.spark(&mut self.rng);
        if flash.update() {
            c.slated = true;
            r.cosmetics.retain(|c| c.id != id);
        }
        self.world.add_cosmetic(index, CosmeticKind::Spark(spark));
    }
}

/// Where an electric sound plays.
enum ElectricSoundAt {
    Position(Vec2),
    /// Chunk `i` of an object, at this position.
    Chunk(ChunkOwner, usize, Vec2),
}

#[cfg(test)]
#[path = "game_electric_death_tests.rs"]
mod tests;
