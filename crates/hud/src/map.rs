//! What is kept of `HUD.Map` without the map (user decision, docs/DIVERGENCES.md): its
//! fade, which the karma meter and the controls page read, and the random draw its
//! update makes every tick once the map textures are loaded, which keeps the shared
//! stream in step.

use rw_random::UnityRandom;

/// `HUD.Map`'s visibility and per-tick draw.
#[derive(Debug, Clone, Default)]
pub struct MapFade {
    pub fade: f32,
    pub last_fade: f32,
    pub fade_counter: i32,
    pub visible: bool,
    pub counter: i32,
    pub visible_counter: i32,
    pub invisible_counter: i32,
    /// `mapData.activeSwarmRooms.Length`.
    pub swarm_rooms: usize,
    /// The map's swarm-circle draws were due but not made (they need the map).
    pub swarm_draws_skipped: u32,
}

impl MapFade {
    #[must_use]
    pub fn new(swarm_rooms: usize) -> Self {
        Self {
            swarm_rooms,
            ..Self::default()
        }
    }

    /// `Map.Update` for a region map whose textures load on the first update (the
    /// game reads them synchronously). `reveal_map` is the owner's `RevealMap`.
    pub fn update(&mut self, reveal_map: bool, hide_general_hud: bool, rng: &mut UnityRandom) {
        self.counter += 1;
        if self.visible {
            self.invisible_counter = 0;
            self.visible_counter += 1;
        } else {
            self.visible_counter = 0;
            self.invisible_counter += 1;
        }
        self.last_fade = self.fade;
        if reveal_map {
            self.fade_counter += 1;
            if self.fade_counter > 30 {
                self.fade = (self.fade + 1.0 / 30.0).min(1.0);
                self.fade += (1.0 - self.fade) * 0.1;
            }
        } else {
            self.fade_counter = (self.fade_counter - 5).max(0);
            self.fade = (self.fade - 0.05).max(0.0);
            self.fade -= self.fade * 0.1;
        }
        if hide_general_hud {
            self.fade = 0.0;
        }
        self.visible = self.fade > 0.0 && self.last_fade > 0.0;
        // `Random.value < activeSwarmRooms.Length / 20f && fade > 0.95f`.
        if rng.value() < self.swarm_rooms as f32 / 20.0 && self.fade > 0.95 {
            self.swarm_draws_skipped += 1;
        }
    }
}
