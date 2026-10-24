//! The game's side of a room's `RoomRain`: `Room.Loaded` making it (not in an intact
//! shelter), its turn in the update list (the intensity, then `ThrowAroundObjects` while the
//! global rain pushes) and the `rainDeath` of the creatures it reaches. Type:
//! [`crate::world::RoomRain`].
//!
//! Not ported yet: the flood arm of `Update` (R4), the bullet drips (R5), the sound loops
//! (R3); the DLC branches are listed in D45.
//!
//! Mirrors: Room.cs (Loaded: RoomRain), RoomRain.cs (Update, ThrowAroundObjects)

use rw_assets::DangerType;

use crate::ChunkOwner;
use crate::creature::CreatureId;
use crate::game::{RainWorldGame, RoomObject};
use crate::item::Entity;
use crate::world::RoomRain;

impl RainWorldGame {
    /// `Room.Loaded`'s `new RoomRain(game.globalRain, this)` and `AddObject`: a room that
    /// is not an intact shelter, whose settings' `DangerType` is `Rain`, `FloodAndRain`,
    /// `Flood` or `AerieBlizzard`. (The Watcher water-cycle objects that also make one are
    /// not ported, D45.)
    pub(crate) fn add_room_rain(&mut self, index: i32) {
        let world = &self.world;
        let Some(data) = world.abstract_room(index) else {
            return;
        };
        let broken = usize::try_from(data.shelter_index)
            .ok()
            .and_then(|i| world.data().broken_shelters.get(i).copied())
            .unwrap_or(false);
        let shelter = data.shelter();
        if shelter && !broken {
            return;
        }
        let Some(realized) = world.realized(index) else {
            return;
        };
        let Some(danger) = realized.settings.danger_type.filter(|d| {
            matches!(
                d,
                DangerType::Rain
                    | DangerType::FloodAndRain
                    | DangerType::Flood
                    | DangerType::AerieBlizzard
            )
        }) else {
            return;
        };
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.room_rain = Some(RoomRain::new(order, &r.room, danger, shelter));
        }
    }

    /// Room `index`'s `RoomRain` in its update list.
    pub(crate) fn room_rain_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world
            .realized(index)
            .and_then(|r| r.room_rain.as_ref())
            .map(|r| (r.update_order, RoomObject::RoomRain(r.update_order)))
            .into_iter()
            .collect()
    }

    /// `RoomRain.Update` (base arm) of the object at `order` in room `index`.
    pub(crate) fn run_room_rain(&mut self, index: i32, order: u64) {
        let global = &self.world.rain_cycle.global_rain;
        let (global_intensity, pushes) = (global.intensity, global.any_push_around());
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let rain_intensity = r.settings.rain_intensity;
        let Some(rain) = r
            .room_rain
            .as_mut()
            .filter(|rain| rain.update_order == order)
        else {
            return;
        };
        rain.update_intensity(global_intensity, rain_intensity);
        // The bullet drips (R5), the flood (R4) and the sound loops (R3) follow here.
        if pushes {
            self.throw_around_objects(index);
        }
    }

    /// `ThrowAroundObjects`: every chunk of every physical object of the room, layer by
    /// layer, is pushed by the rain where its jittered tile is exposed (or under a ceiling
    /// once the inside push starts); a creature's chunk may stun it, its main chunk adds to
    /// `rainDeath` and any chunk may kill it once that is over 1.
    fn throw_around_objects(&mut self, index: i32) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let rain_intensity = r.settings.rain_intensity;
        if rain_intensity == 0.0 {
            return;
        }
        let room = r.room.clone();
        let Some(rain) = r.room_rain.take() else {
            return;
        };
        let push = rain.push_around(&self.world.rain_cycle.global_rain, rain_intensity);
        let player_seq = self.player_here(index).then_some(self.player_seq);
        for (_, _, owner) in self.world.physical_objects(index, player_seq) {
            let creature = self.rain_creature(owner);
            let owner_chunks = (0..).map_while(|i| self.electric_chunk(owner, i)).count();
            for i in 0..owner_chunks {
                let Some(chunk) = self.electric_chunk(owner, i) else {
                    break;
                };
                let water_level = self
                    .world
                    .realized(index)
                    .and_then(|r| r.water.as_ref())
                    .map(|w| w.level_at(chunk.pos.x));
                let Some(p) = rain.chunk_push(push, water_level, &chunk, &mut self.rng) else {
                    continue;
                };
                let mut pushed = chunk;
                rain.push_velocity(&room, &mut pushed, &p, &mut self.rng);
                self.set_rain_chunk_vel(owner, i, pushed.vel);
                if let Some(id) = creature {
                    self.rain_on_creature_chunk(id, i, owner_chunks, p.num);
                }
                if let Some(mut pushed) = self.electric_chunk(owner, i) {
                    RoomRain::final_push(&mut pushed, push.inside, &mut self.rng);
                    self.set_rain_chunk_vel(owner, i, pushed.vel);
                }
            }
        }
        if let Some(r) = self.world.realized_mut(index) {
            r.room_rain = Some(rain);
        }
    }

    /// The creature part of a pushed chunk: the stun roll, the main chunk's share of
    /// `rainDeath`, the kill roll.
    fn rain_on_creature_chunk(&mut self, id: CreatureId, chunk: usize, chunks: usize, num: f32) {
        if let Some(st) = RoomRain::stun_roll(num, chunks, &mut self.rng) {
            self.creature_stun(id, st);
        }
        if chunk == 0 {
            self.add_rain_death(id, num / 20.0);
        }
        if RoomRain::kill_roll(num, self.rain_death(id), &mut self.rng) {
            self.realized_creature_die(id);
        }
    }

    /// The creature a physical object is, if any.
    fn rain_creature(&self, owner: ChunkOwner) -> Option<CreatureId> {
        match owner {
            ChunkOwner::Player => Some(self.player_id),
            ChunkOwner::Entity(Entity::Creature(id)) => Some(id),
            ChunkOwner::Entity(Entity::Object(_)) | ChunkOwner::Segment(_) => None,
        }
    }

    fn set_rain_chunk_vel(&mut self, owner: ChunkOwner, i: usize, vel: crate::Vec2) {
        if let Some(c) = self.electric_chunk_mut(owner, i) {
            c.vel = vel;
        }
    }

    /// `Creature.rainDeath` of a realized creature.
    #[must_use]
    pub fn rain_death(&self, id: CreatureId) -> f32 {
        if id == self.player_id {
            self.player.as_ref().map_or(0.0, |p| p.rain_death)
        } else {
            self.world.creature(id).rain_death
        }
    }

    fn add_rain_death(&mut self, id: CreatureId, amount: f32) {
        if id == self.player_id {
            if let Some(p) = self.player.as_mut() {
                p.rain_death += amount;
            }
        } else {
            self.world.creature_mut(id).rain_death += amount;
        }
    }
}

#[cfg(test)]
#[path = "tests/room_rain_turn.rs"]
mod tests;
