//! The light fixtures and waterfalls of a realized room, in the room's update list. Each
//! list is kept in update-list order (an object's `update_order` grows with `AddObject`).
//!
//! The superstructure's electric objects (`SSLightRod`, `ZapCoil` with its `ZapCoilLight`s and
//! `ZapFlash`es, `SuperStructureFuses`) are kept here too, and the screen-grab effects
//! (`DeepProcessing` with its light, `GravityDisruptor` with its `DisruptorSpeck`s).
//!
//! Mirrors: Room.cs (updateList: Redlight, HolyFire, SlimeMoldLight, WaterFall, SSLightRod, ZapCoil, ZapCoilLight, SuperStructureFuses, DeepProcessing, DeepProcessingLight, GravityDisruptor)

use crate::effects::lights::light_fixture::{HolyFire, HolyFireSprite, Redlight, SlimeMoldLight};
use crate::superstructure::electric::fuses::SuperStructureFuses;
use crate::superstructure::electric::light_rod::SsLightRod;
use crate::superstructure::electric::zap_coil::{ZapCoil, ZapCoilLight, ZapFlash};
use crate::superstructure::grab::deep_processing::{DeepProcessing, DeepProcessingLight};
use crate::superstructure::grab::gravity_disruptor::{DisruptorSpeck, GravityDisruptor};
use crate::water::waterfall::WaterFall;

/// An object of [`RoomFixtures`], by its place in the update list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureRef {
    Redlight(u64),
    HolyFire(u64),
    HolyFireSprite(u64),
    SlimeLight(u64),
    WaterFall(u64),
    LightRod(u64),
    ZapCoil(u64),
    /// An emitter of a coil's sound loops, by its place in the update list.
    ZapCoilLoop(u64),
    ZapCoilLight(u64),
    ZapFlash(u64),
    Fuses(u64),
    DeepProcessing(u64),
    DeepProcessingLight(u64),
    GravityDisruptor(u64),
    DisruptorSpeck(u64),
}

/// A realized room's `Redlight`s, `HolyFire`s (with their sprites), `SlimeMoldLight`s and
/// `WaterFall`s.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomFixtures {
    pub redlights: Vec<Redlight>,
    pub holy_fires: Vec<HolyFire>,
    pub holy_sprites: Vec<HolyFireSprite>,
    pub slime_lights: Vec<SlimeMoldLight>,
    pub waterfalls: Vec<WaterFall>,
    pub light_rods: Vec<SsLightRod>,
    pub zap_coils: Vec<ZapCoil>,
    pub zap_coil_lights: Vec<ZapCoilLight>,
    pub zap_flashes: Vec<ZapFlash>,
    pub fuses: Vec<SuperStructureFuses>,
    pub deep_processing: Vec<DeepProcessing>,
    pub deep_processing_lights: Vec<DeepProcessingLight>,
    pub gravity_disruptors: Vec<GravityDisruptor>,
    pub disruptor_specks: Vec<DisruptorSpeck>,
}

impl RoomFixtures {
    /// Drops the sprites `Destroy` slated.
    pub fn remove_slated(&mut self) {
        self.holy_sprites.retain(|s| !s.slated);
        self.zap_flashes.retain(|f| !f.slated);
    }

    /// Whether the object a light is tied to at this place in the update list is there (a
    /// light tied to it fades once it is not): a `Redlight`, `HolyFire`, `SSLightRod`,
    /// `ZapCoilLight` or `ZapFlash`.
    #[must_use]
    pub fn has_light_fixture(&self, order: u64) -> bool {
        self.redlights.iter().any(|r| r.update_order == order)
            || self.holy_fires.iter().any(|h| h.update_order == order)
            || self.light_rods.iter().any(|r| r.update_order == order)
            || self.zap_coil_lights.iter().any(|l| l.update_order == order)
            || self
                .deep_processing_lights
                .iter()
                .any(|l| l.update_order == order)
            || self
                .zap_flashes
                .iter()
                .any(|f| f.update_order == order && !f.slated)
    }

    /// Every object with its place in the update list.
    pub(crate) fn objects(&self) -> Vec<(u64, FixtureRef)> {
        let mut list: Vec<(u64, FixtureRef)> = Vec::new();
        list.extend(
            self.redlights
                .iter()
                .map(|r| (r.update_order, FixtureRef::Redlight(r.update_order))),
        );
        list.extend(
            self.holy_fires
                .iter()
                .map(|h| (h.update_order, FixtureRef::HolyFire(h.update_order))),
        );
        list.extend(
            self.holy_sprites
                .iter()
                .filter(|s| !s.slated)
                .map(|s| (s.update_order, FixtureRef::HolyFireSprite(s.update_order))),
        );
        list.extend(
            self.slime_lights
                .iter()
                .map(|s| (s.update_order, FixtureRef::SlimeLight(s.update_order))),
        );
        list.extend(
            self.waterfalls
                .iter()
                .map(|w| (w.update_order, FixtureRef::WaterFall(w.update_order))),
        );
        list.extend(
            self.light_rods
                .iter()
                .map(|r| (r.update_order, FixtureRef::LightRod(r.update_order))),
        );
        for c in &self.zap_coils {
            list.push((c.update_order, FixtureRef::ZapCoil(c.update_order)));
            list.extend(
                c.emitters()
                    .map(|e| (e.update_order, FixtureRef::ZapCoilLoop(e.update_order))),
            );
        }
        list.extend(
            self.zap_coil_lights
                .iter()
                .map(|l| (l.update_order, FixtureRef::ZapCoilLight(l.update_order))),
        );
        list.extend(
            self.zap_flashes
                .iter()
                .filter(|f| !f.slated)
                .map(|f| (f.update_order, FixtureRef::ZapFlash(f.update_order))),
        );
        list.extend(
            self.fuses
                .iter()
                .map(|f| (f.update_order, FixtureRef::Fuses(f.update_order))),
        );
        list.extend(
            self.deep_processing
                .iter()
                .map(|d| (d.update_order, FixtureRef::DeepProcessing(d.update_order))),
        );
        list.extend(self.deep_processing_lights.iter().map(|l| {
            (
                l.update_order,
                FixtureRef::DeepProcessingLight(l.update_order),
            )
        }));
        list.extend(
            self.gravity_disruptors
                .iter()
                .map(|g| (g.update_order, FixtureRef::GravityDisruptor(g.update_order))),
        );
        list.extend(
            self.disruptor_specks
                .iter()
                .map(|k| (k.update_order, FixtureRef::DisruptorSpeck(k.update_order))),
        );
        list
    }

    /// The places in the update list of the objects that have sprites.
    pub fn drawn_orders(&self) -> impl Iterator<Item = u64> + '_ {
        self.holy_sprites
            .iter()
            .filter(|s| !s.slated)
            .map(|s| s.update_order)
            .chain(self.slime_lights.iter().map(|s| s.update_order))
            .chain(self.waterfalls.iter().map(|w| w.update_order))
            .chain(self.light_rods.iter().map(|r| r.update_order))
            .chain(self.zap_coils.iter().map(|c| c.update_order))
            .chain(
                self.zap_flashes
                    .iter()
                    .filter(|f| !f.slated)
                    .map(|f| f.update_order),
            )
            .chain(self.fuses.iter().map(|f| f.update_order))
            .chain(self.deep_processing.iter().map(|d| d.update_order))
            .chain(self.gravity_disruptors.iter().map(|g| g.update_order))
            .chain(self.disruptor_specks.iter().map(|k| k.update_order))
    }
}
