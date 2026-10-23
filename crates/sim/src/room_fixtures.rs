//! The light fixtures and waterfalls of a realized room, in the room's update list. Each
//! list is kept in update-list order (an object's `update_order` grows with `AddObject`).
//!
//! Mirrors: Room.cs (updateList: Redlight, HolyFire, SlimeMoldLight, WaterFall)

use crate::light_fixture::{HolyFire, HolyFireSprite, Redlight, SlimeMoldLight};
use crate::waterfall::WaterFall;

/// An object of [`RoomFixtures`], by its place in the update list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureRef {
    Redlight(u64),
    HolyFire(u64),
    HolyFireSprite(u64),
    SlimeLight(u64),
    WaterFall(u64),
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
}

impl RoomFixtures {
    /// Drops the sprites `Destroy` slated.
    pub fn remove_slated(&mut self) {
        self.holy_sprites.retain(|s| !s.slated);
    }

    /// Whether the `Redlight` or `HolyFire` at this place in the update list is there (a
    /// light tied to it fades once it is not).
    #[must_use]
    pub fn has_light_fixture(&self, order: u64) -> bool {
        self.redlights.iter().any(|r| r.update_order == order)
            || self.holy_fires.iter().any(|h| h.update_order == order)
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
    }
}
