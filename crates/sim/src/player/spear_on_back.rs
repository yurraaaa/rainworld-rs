//! The Hunter's `SpearOnBack` as the slugcat keeps it: the spear, the stash/retrieve
//! counter, the lean (`flip`) and the interaction lock; `Stun` and `Die` drop the spear
//! (`DropSpear`), drawing where the game draws, and [`crate::RainWorldGame`] applies the
//! drop to the spear. The update, `SpearToBack`, `SpearToHand` and the placing on the
//! back reach the world and live in `game_spear_on_back.rs`.
//!
//! Mirrors: Player.cs (SpearOnBack, Stun, Die)

use rw_random::UnityRandom;

use super::Player;
use crate::Vec2;
use crate::item::{ObjectId, SpearSpin};

/// `Player.SpearOnBack`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpearOnBack {
    pub spear: Option<ObjectId>,
    pub increment: bool,
    pub counter: i32,
    pub flip: f32,
    pub interaction_locked: bool,
    /// `abstractStick`'s serial.
    pub stick: Option<u64>,
}

impl SpearOnBack {
    #[must_use]
    pub fn has_a_spear(&self) -> bool {
        self.spear.is_some()
    }
}

/// `DropSpear` as far as the slugcat goes: its draws made, the spear let go of; the
/// world sets the velocity, the `Free` mode (with the spin drawn here) and ends the stick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackSpearDrop {
    pub spear: ObjectId,
    pub stick: Option<u64>,
    /// `mainBodyChunk.vel + RNV() * 3 * value`.
    pub vel: Vec2,
    pub spin: SpearSpin,
}

impl Player {
    /// `spearOnBack` (the Hunter's, `SlugCatClass == Red` and not a ghost).
    #[must_use]
    pub fn spear_on_back(&self) -> Option<&SpearOnBack> {
        self.spear_on_back.as_ref()
    }

    pub(crate) fn spear_on_back_mut(&mut self) -> Option<&mut SpearOnBack> {
        self.spear_on_back.as_mut()
    }

    /// The constructor's `spearOnBack = new SpearOnBack(this)`.
    pub fn give_spear_on_back(&mut self) {
        self.spear_on_back = Some(SpearOnBack::default());
    }

    /// `spearOnBack.DropSpear()` when there is a spear: velocity (two draws) and the
    /// `Free` mode's spin (two more) drawn now; `main_vel` is `mainBodyChunk.vel`.
    pub(crate) fn drop_back_spear(&mut self, main_vel: Vec2, rng: &mut UnityRandom) {
        let Some(s) = self.spear_on_back.as_mut() else {
            return;
        };
        let Some(spear) = s.spear.take() else {
            return;
        };
        let dir = Vec2::random_unit(rng);
        let vel = main_vel + dir * (3.0 * rng.value());
        let spin = SpearSpin::draw(rng);
        let stick = s.stick.take();
        self.back_spear_drop = Some(BackSpearDrop {
            spear,
            stick,
            vel,
            spin,
        });
    }

    /// The `DropSpear` since the last call, for the world.
    pub(crate) fn take_back_spear_drop(&mut self) -> Option<BackSpearDrop> {
        self.back_spear_drop.take()
    }
}
