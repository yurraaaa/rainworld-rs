//! The game's side of the superstructure's screen-grab effects: `Room.Loaded` making the
//! `DeepProcessing`, `DeepProcessingLight` and `GravityDisruptor` (with its twenty
//! `DisruptorSpeck`s, added to the room first), `AIMapReady` collecting the room's
//! zero-gravity specks for each disruptor, their turns in the room's update list, and the
//! disruptor's swirl over the room's `physicalObjects` (every body chunk within 450 px). Types:
//! [`crate::deep_processing`], [`crate::gravity_disruptor`].
//!
//! Mirrors: Room.cs (Loaded: DeepProcessing, DeepProcessingLight, GravityDisruptor; AIMapReady), GravityDisruptor.cs (constructor, Update, AIMapReady), DeepProcessingLight.cs (constructor)

use rw_assets::{DeepProcessingData, RoomEffectType};

use super::RainWorldGame;
use crate::Vec2;
use crate::deep_processing::{DeepProcessing, DeepProcessingLight};
use crate::gravity_disruptor::{GravityDisruptor, SPECKS};
use crate::world::RealizedRoom;

impl RainWorldGame {
    /// `Room.Loaded`'s `new DeepProcessing(placedObject)`.
    pub(super) fn add_deep_processing(&mut self, index: i32, pos: Vec2, data: DeepProcessingData) {
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.fixtures
                .deep_processing
                .push(DeepProcessing::new(order, pos, data));
        }
    }

    /// `Room.Loaded`'s `new DeepProcessingLight(room, placedObject, data)`: its light
    /// joins the room first.
    pub(super) fn add_deep_processing_light(&mut self, index: i32, pos: Vec2, random_seed: i32) {
        let light = self.world.next_update_seq();
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.light_sources
                .push(DeepProcessingLight::light_source(light, index, order, pos));
            r.fixtures.deep_processing_lights.push(DeepProcessingLight {
                update_order: order,
                pos,
                random_seed,
                light,
            });
        }
    }

    /// `Room.Loaded`'s `new GravityDisruptor(placedObject, room)`: the depth from the tile
    /// it sits on (0 in a solid one, 1 with a wall behind, else 2), then the twenty specks
    /// (joining the room, with their draws), then the rest of its draws.
    pub(super) fn add_gravity_disruptor(&mut self, index: i32, pos: Vec2) {
        let Some(tile) = self.world.realized(index).map(|r| {
            let (x, y) = pos.tile();
            r.room.tile(x, y)
        }) else {
            return;
        };
        let depth = if tile.is_solid() {
            0
        } else if tile.wall_behind() {
            1
        } else {
            2
        };
        let mut orders = [0; SPECKS];
        for o in &mut orders {
            *o = self.world.next_update_seq();
        }
        let order = self.world.next_update_seq();
        let specks = GravityDisruptor::make_specks(pos, &orders, order, &mut self.rng);
        let disruptor = GravityDisruptor::new(order, pos, depth, &mut self.rng);
        if let Some(r) = self.world.realized_mut(index) {
            r.fixtures.disruptor_specks.extend(specks);
            r.fixtures.gravity_disruptors.push(disruptor);
        }
    }

    /// `AIMapReady` of every `GravityDisruptor` of room `index`: `roomSpecks` is every
    /// `GenericZeroGSpeck` in the update list now.
    pub(super) fn link_gravity_disruptors(&mut self, index: i32) {
        if let Some(r) = self.world.realized_mut(index) {
            let specks: Vec<u64> = r.particles.specks.iter().map(|s| s.update_order).collect();
            for d in &mut r.fixtures.gravity_disruptors {
                d.room_specks.clone_from(&specks);
            }
        }
    }

    /// `DeepProcessing.Update` of the quad at `order` in room `index`.
    pub(super) fn run_deep_processing(&mut self, index: i32, order: u64) {
        let cycle = self.world.rain_cycle;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let power = r.electric_power(&cycle);
        if let Some(d) = r
            .fixtures
            .deep_processing
            .iter_mut()
            .find(|d| d.update_order == order)
        {
            d.update(power, &mut self.rng);
        }
    }

    /// `DeepProcessingLight.Update` of the light at `order` in room `index`.
    pub(super) fn run_deep_processing_light(&mut self, index: i32, order: u64) {
        let cycle = self.world.rain_cycle;
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let power = r.electric_power(&cycle);
        let RealizedRoom {
            fixtures,
            light_sources,
            ..
        } = r;
        if let Some(l) = fixtures
            .deep_processing_lights
            .iter()
            .find(|l| l.update_order == order)
        {
            l.update(light_sources, power, &mut self.rng);
        }
    }

    /// `GravityDisruptor.Update` of the disruptor at `order` in room `index`: it follows
    /// its placed object, swirls the room's chunks and specks while it has power, then
    /// the streaks and the power.
    pub(super) fn run_gravity_disruptor(&mut self, index: i32, order: u64) {
        let Some(mut d) = self.world.realized(index).and_then(|r| {
            r.fixtures
                .gravity_disruptors
                .iter()
                .find(|d| d.update_order == order)
                .cloned()
        }) else {
            return;
        };
        d.follow();
        if d.power > 0.0 {
            self.disruptor_swirl_chunks(index, &d);
            self.disruptor_swirl_specks(index, &d);
        }
        let broken = self
            .world
            .realized(index)
            .is_some_and(|r| r.settings.effect_amount(RoomEffectType::BrokenZeroG) > 0.0);
        let anti_gravity = self
            .world
            .rain_cycle
            .broken_anti_grav
            .filter(|_| broken)
            .map(|b| b.current_anti_gravity());
        d.update(anti_gravity, &mut self.rng);
        if let Some(slot) = self.world.realized_mut(index).and_then(|r| {
            r.fixtures
                .gravity_disruptors
                .iter_mut()
                .find(|x| x.update_order == order)
        }) {
            *slot = d;
        }
    }

    /// The swirl over every chunk of `physicalObjects`, layer by layer.
    fn disruptor_swirl_chunks(&mut self, index: i32, d: &GravityDisruptor) {
        let player_seq = self.player_here(index).then_some(self.player_seq);
        for (_, _, owner) in self.world.physical_objects(index, player_seq) {
            for i in 0.. {
                let Some(chunk) = self.electric_chunk(owner, i) else {
                    break;
                };
                if let Some((push, swirl)) = d.chunk_push(chunk.pos, chunk.vel)
                    && let Some(c) = self.electric_chunk_mut(owner, i)
                {
                    c.pos += push;
                    c.vel += push;
                    c.vel += swirl;
                }
            }
        }
    }

    /// The swirl over the room's `GenericZeroGSpeck`s `roomSpecks` holds.
    fn disruptor_swirl_specks(&mut self, index: i32, d: &GravityDisruptor) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        for &order in &d.room_specks {
            if let Some(s) = r
                .particles
                .specks
                .iter_mut()
                .find(|s| s.update_order == order)
                && let Some(push) = d.speck_push(s.pos, s.vel)
            {
                s.vel += push;
            }
        }
    }

    /// `DisruptorSpeck.Update` of the speck at `order` in room `index`.
    pub(super) fn run_disruptor_speck(&mut self, index: i32, order: u64) {
        let Some(r) = self.world.realized_mut(index) else {
            return;
        };
        let RealizedRoom { fixtures, .. } = r;
        let Some(speck) = fixtures
            .disruptor_specks
            .iter_mut()
            .find(|s| s.update_order == order)
        else {
            return;
        };
        let Some(d) = fixtures
            .gravity_disruptors
            .iter()
            .find(|d| d.update_order == speck.disruptor)
        else {
            return;
        };
        speck.update(d.pos, d.power, &mut self.rng);
    }
}

#[cfg(test)]
#[path = "game_ss_grab_tests.rs"]
mod tests;
