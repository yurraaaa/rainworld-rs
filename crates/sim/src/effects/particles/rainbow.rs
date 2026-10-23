//! `Rainbow` and `RainbowNoFade`: a ring of rainbow colours that fades in and out over
//! the cycle's start-up, drawn in `GrabShaders`. `Room.Loaded` makes a `Rainbow` only
//! with a `SeededRandom` chance (no shared-stream draws) while the cycle is starting
//! up; the object's `Update` sets its fade and destroys it when the start-up is over.
//! Sprites: `rw-graphics` `rainbow.rs`.
//!
//! Mirrors: Rainbow.cs, RainbowNoFade.cs, Room.cs (Loaded: Rainbow, RainbowNoFade)

use rw_assets::RainbowData;

use crate::{FloatExt, Vec2};

/// One `Rainbow` (or `RainbowNoFade`).
#[derive(Debug, Clone, PartialEq)]
pub struct Rainbow {
    /// Its place in the room's update list (`AddObject`).
    pub update_order: u64,
    /// `pos`: the placed position less the handle.
    pub pos: Vec2,
    /// Twice the handle's length: half the side of the drawn square.
    pub rad: f32,
    /// `fades`: the corners' opacities, the inner ring fraction, the spawn chance.
    pub fades: [f32; 6],
    /// 0 until the first `Update`.
    pub fade: f32,
    /// `alwaysShow`: `RainbowNoFade`'s.
    pub always_show: bool,
    /// Destroyed (`slatedForDeletetion`): removed from the room after the update.
    pub slated: bool,
}

impl Rainbow {
    /// The constructor (`Refresh`); `no_fade` for a `RainbowNoFade`, which always shows.
    #[must_use]
    pub fn new(update_order: u64, placed: Vec2, data: &RainbowData, no_fade: bool) -> Self {
        let handle = Vec2::from(data.handle_pos);
        Self {
            update_order,
            pos: placed - handle,
            rad: handle.length() * 2.0,
            fades: data.fades,
            fade: 0.0,
            always_show: no_fade,
            slated: false,
        }
    }

    /// `Room.Loaded`'s condition for a placed `Rainbow`, given `CycleStartUp`, the
    /// `SeededRandom(rainbowSeed + room index)` value and whether this is an arena
    /// session (`ghostMode` is 0: echoes are not ported; `PrecycleModule` is a DLC).
    #[must_use]
    pub fn spawns(cycle_start_up: f32, seeded: f32, chance: f32, arena: bool) -> bool {
        (cycle_start_up < 1.0 && seeded < chance) || arena
    }

    /// `Update`: the fade follows `sin(CycleStartUp^0.75 * pi)^0.6`, and the rainbow
    /// ends with the start-up; always 1 in an arena session or when always shown.
    pub fn update(&mut self, cycle_start_up: f32, arena: bool) {
        if self.always_show || arena {
            self.fade = 1.0;
            return;
        }
        self.fade = (cycle_start_up.mathf_pow(0.75) * std::f32::consts::PI)
            .sin()
            .clamp(0.0, 1.0)
            .mathf_pow(0.6);
        if cycle_start_up >= 1.0 {
            self.slated = true;
        }
    }
}

#[cfg(test)]
#[path = "tests/rainbow.rs"]
mod tests;
