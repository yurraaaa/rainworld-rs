//! The depth-faded particles a realized room's effects hold: sky dandelions with their
//! stalks and spawners, green sparks with their spawners, zero-gravity specks and
//! corruption spores. Each list is kept in update-list order (an object's
//! `update_order` grows with `AddObject`), so objects are found by binary search.
//!
//! Mirrors: Room.cs (updateList: SkyDandelions, GreenSparks, BlinkSpeck, CorruptionSpore)

use crate::effects::particles::green_sparks::{GreenSpark, GreenSparks};
use crate::effects::particles::sky_dandelion::{DandelionStalk, SkyDandelion, SkyDandelions};
use crate::effects::particles::zero_g_speck::ZeroGSpeck;

/// A realized room's particle objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomParticles {
    pub dandelions: Vec<SkyDandelion>,
    pub stalks: Vec<DandelionStalk>,
    pub dandelion_spawners: Vec<SkyDandelions>,
    pub sparks: Vec<GreenSpark>,
    pub spark_spawners: Vec<GreenSparks>,
    pub specks: Vec<ZeroGSpeck>,
}

impl RoomParticles {
    /// Drops what `Destroy` slated. A slated dandelion stays while a stalk still
    /// holds it (the stalk keeps updating it).
    pub fn remove_slated(&mut self) {
        self.stalks.retain(|s| !s.slated);
        let stalks = &self.stalks;
        self.dandelions
            .retain(|d| !d.slated || stalks.iter().any(|s| s.dandelion == Some(d.update_order)));
        self.sparks.retain(|s| !s.slated);
        self.specks.retain(|s| !s.slated);
    }

    /// The places in the update list of the objects that have sprites (the stalks and
    /// the dandelions a stalk still holds included).
    pub fn present_orders(&self) -> impl Iterator<Item = u64> + '_ {
        self.dandelions
            .iter()
            .filter(|d| !d.slated)
            .map(|d| d.update_order)
            .chain(self.stalks.iter().map(|s| s.update_order))
            .chain(
                self.sparks
                    .iter()
                    .filter(|s| !s.slated)
                    .map(|s| s.update_order),
            )
            .chain(
                self.specks
                    .iter()
                    .filter(|s| !s.slated)
                    .map(|s| s.update_order),
            )
    }

    /// The dandelion with this place in the update list.
    pub fn dandelion_mut(&mut self, order: u64) -> Option<&mut SkyDandelion> {
        let i = self
            .dandelions
            .binary_search_by_key(&order, |d| d.update_order)
            .ok()?;
        self.dandelions.get_mut(i)
    }
}
