//! The game's side of the light fixtures and waterfalls: `Room.Loaded`'s `LightFixture`
//! objects and the tile parse's `WaterFall`s joining the room's update list (with the
//! lights the fixtures make), each one's turn in it, and `Room.ElectricPower`. Types:
//! [`crate::light_fixture`], [`crate::waterfall`].
//!
//! Mirrors: Room.cs (Loaded: LightFixture; LoadFromDataString: WaterFall; ElectricPower), Redlight.cs (Update: the RedlightLight made on the first update)

use rw_assets::{LightFixtureData, LightFixtureType, RoomEffectType};

use super::{RainWorldGame, RoomObject};
use crate::Vec2;
use crate::light_fixture::{HolyFire, Redlight, SlimeMoldLight};
use crate::room_fixtures::FixtureRef;
use crate::waterfall::WaterFall;
use crate::world::RealizedRoom;

impl RealizedRoom {
    /// `Room.ElectricPower` (base game): 1 without `BrokenZeroG`; with it the world's
    /// `brokenAntiGrav.CurrentLightsOn` (the switch exists once such a room has loaded).
    #[must_use]
    pub fn electric_power(&self, cycle: &crate::world::RainCycle) -> f32 {
        if self.settings.effect_amount(RoomEffectType::BrokenZeroG) > 0.0 {
            cycle
                .broken_anti_grav
                .map_or(1.0, |b| b.current_lights_on())
        } else {
            1.0
        }
    }
}

impl RainWorldGame {
    /// `Room.Loaded`'s `LightFixture` of room `index`: a `Redlight` (`RedSubmersible` too),
    /// a `HolyFire` or a `SlimeMoldLight`, with the lights their constructors add first.
    /// A `ZapCoilLight` is made as well (`game_ss_electric.rs`), and a `DeepProcessingLight`
    /// (`game_ss_grab.rs`); the DLC's `GlowWeedLight` is not made.
    pub(super) fn add_light_fixture(&mut self, index: i32, pos: Vec2, data: LightFixtureData) {
        match data.kind {
            Some(LightFixtureType::RedLight) => self.add_redlight(index, pos, data, false),
            Some(LightFixtureType::RedSubmersible) => self.add_redlight(index, pos, data, true),
            Some(LightFixtureType::HolyFire) => self.add_holy_fire(index, pos),
            Some(LightFixtureType::SlimeMoldLight) => {
                let order = self.world.next_update_seq();
                if let Some(r) = self.world.realized_mut(index) {
                    let light = SlimeMoldLight::new(order, pos, data.random_seed);
                    r.fixtures.slime_lights.push(light);
                }
            }
            Some(LightFixtureType::ZapCoilLight) => {
                self.add_zap_coil_light(index, pos, data.random_seed);
            }
            Some(LightFixtureType::DeepProcessing) => {
                self.add_deep_processing_light(index, pos, data.random_seed);
            }
            Some(LightFixtureType::GlowWeedLight) | None => {}
        }
    }

    fn add_redlight(&mut self, index: i32, pos: Vec2, data: LightFixtureData, submersible: bool) {
        let Some(broken) = self
            .world
            .realized(index)
            .map(|r| r.settings.effect_amount(RoomEffectType::BrokenZeroG))
        else {
            return;
        };
        let (flat, order) = (self.world.next_update_seq(), self.world.next_update_seq());
        let red = Redlight::new(
            order,
            flat,
            pos,
            data.random_seed,
            submersible,
            broken,
            &mut self.rng,
        );
        if let Some(r) = self.world.realized_mut(index) {
            r.light_sources
                .push(Redlight::flat_light_source(flat, index, order, pos));
            r.fixtures.redlights.push(red);
        }
    }

    fn add_holy_fire(&mut self, index: i32, pos: Vec2) {
        let first = self.world.next_update_seq();
        let lights = [
            first,
            self.world.next_update_seq(),
            self.world.next_update_seq(),
        ];
        let flat = self.world.next_update_seq();
        let order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            for (i, &id) in lights.iter().enumerate() {
                r.light_sources
                    .push(HolyFire::light_source(i, id, index, order, pos));
            }
            r.light_sources
                .push(HolyFire::flat_light_source(flat, index, order, pos));
            r.fixtures
                .holy_fires
                .push(HolyFire::new(order, pos, lights, flat));
        }
    }

    /// The tile parse's `WaterFall`s of room `index`, before anything else of `Room.Loaded`.
    pub(super) fn add_waterfalls(&mut self, index: i32) {
        let Some(r) = self.world.realized(index) else {
            return;
        };
        let specs = WaterFall::specs(&r.room);
        let has_water = r.room.water_level().is_some();
        for spec in specs {
            let order = self.world.next_update_seq();
            if let Some(r) = self.world.realized_mut(index) {
                let fall = WaterFall::new(order, &r.room, spec, has_water);
                r.fixtures.waterfalls.push(fall);
            }
        }
    }

    /// Room `index`'s fixtures and waterfalls in its update list.
    pub(super) fn room_fixture_objects(&self, index: i32) -> Vec<(u64, RoomObject)> {
        self.world.realized(index).map_or_else(Vec::new, |r| {
            r.fixtures
                .objects()
                .into_iter()
                .map(|(order, f)| (order, RoomObject::Fixture(f)))
                .collect()
        })
    }

    /// The `Update` of fixture `f` in room `index`.
    pub(super) fn run_fixture(&mut self, index: i32, f: FixtureRef) {
        match f {
            FixtureRef::Redlight(order) => self.run_redlight(index, order),
            FixtureRef::HolyFire(order) => self.run_holy_fire(index, order),
            FixtureRef::HolyFireSprite(order) => {
                if let Some(s) = self.world.realized_mut(index).and_then(|r| {
                    r.fixtures
                        .holy_sprites
                        .iter_mut()
                        .find(|s| s.update_order == order && !s.slated)
                }) {
                    s.update(&mut self.rng);
                }
            }
            FixtureRef::SlimeLight(order) => {
                if let Some(s) = self.world.realized_mut(index).and_then(|r| {
                    r.fixtures
                        .slime_lights
                        .iter_mut()
                        .find(|s| s.update_order == order)
                }) {
                    s.update(&mut self.rng);
                }
            }
            FixtureRef::WaterFall(order) => self.run_waterfall(index, order),
            FixtureRef::LightRod(order) => self.run_light_rod(index, order),
            FixtureRef::ZapCoil(order) => self.run_zap_coil(index, order),
            FixtureRef::ZapCoilLoop(order) => self.run_zap_coil_emitter(index, order),
            FixtureRef::ZapCoilLight(order) => self.run_zap_coil_light(index, order),
            FixtureRef::ZapFlash(order) => self.run_zap_flash(index, order),
            FixtureRef::Fuses(order) => self.run_fuses(index, order),
            FixtureRef::DeepProcessing(order) => self.run_deep_processing(index, order),
            FixtureRef::DeepProcessingLight(order) => self.run_deep_processing_light(index, order),
            FixtureRef::GravityDisruptor(order) => self.run_gravity_disruptor(index, order),
            FixtureRef::DisruptorSpeck(order) => self.run_disruptor_speck(index, order),
        }
    }

    /// `Redlight.Update`: the `RedlightLight` is made on the first one.
    fn run_redlight(&mut self, index: i32, order: u64) {
        let new_light = self.world.realized(index).and_then(|r| {
            let red = r
                .fixtures
                .redlights
                .iter()
                .find(|x| x.update_order == order)?;
            red.light.is_none().then_some((red.pos, red.submersible))
        });
        if let Some((pos, submersible)) = new_light {
            let id = self.world.next_update_seq();
            if let Some(r) = self.world.realized_mut(index) {
                let light = Redlight::mask_light_source(id, index, order, pos, submersible);
                r.light_sources.push(light);
                if let Some(red) = r
                    .fixtures
                    .redlights
                    .iter_mut()
                    .find(|x| x.update_order == order)
                {
                    red.light = Some(id);
                }
            }
        }
        let sounds = if index == self.camera_room {
            &mut self.sounds
        } else {
            &mut self.silent
        };
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
        if let Some(red) = fixtures
            .redlights
            .iter_mut()
            .find(|x| x.update_order == order)
        {
            red.update(light_sources, power, sounds, &mut self.rng);
        }
    }

    /// `HolyFire.Update`: the sprite it sheds joins the room.
    fn run_holy_fire(&mut self, index: i32, order: u64) {
        let Some(RealizedRoom {
            fixtures,
            light_sources,
            ..
        }) = self.world.realized_mut(index)
        else {
            return;
        };
        let Some(fire) = fixtures
            .holy_fires
            .iter_mut()
            .find(|x| x.update_order == order)
        else {
            return;
        };
        let mut sprite = fire.update(light_sources, &mut self.rng);
        sprite.update_order = self.world.next_update_seq();
        if let Some(r) = self.world.realized_mut(index) {
            r.fixtures.holy_sprites.push(sprite);
        }
    }

    /// Drops the fixtures' sprites destroyed during room `index`'s update.
    pub(super) fn remove_slated_fixtures(&mut self, index: i32) {
        if let Some(r) = self.world.realized_mut(index) {
            r.fixtures.remove_slated();
        }
    }

    /// `LightSource.Update`'s `tiedToObject.slatedForDeletetion || room != room` for a
    /// light tied to the fixture at `order`.
    pub(super) fn fixture_tie_gone(&self, index: i32, order: u64) -> bool {
        !self
            .world
            .realized(index)
            .is_some_and(|r| r.fixtures.has_light_fixture(order))
    }
}

#[cfg(test)]
#[path = "game_light_fixtures_tests.rs"]
mod tests;
