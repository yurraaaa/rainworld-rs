//! Camera shake: camera 0's `screenShake`/`microShake` with `Update`'s per-tick jump and
//! decay, the camera side of `ScreenMovement`, and `Room.ScreenMovement` with its
//! ceiling drips. The window position itself is the app's room camera's; it hands its
//! `pos` in before each tick (after `Update`'s lerp and clamp) and takes the tick's
//! jump and bumps back ([`CameraShake::take_displacement`]). The per-frame micro shake
//! is drawn by the app (`DrawUpdate`, cosmetic stream). Base game: `rippleFade` is 1, no
//! sandstorm or blizzard; the rain's shake waits for the rain (`docs/OPEN-ITEMS.md`).
//!
//! Mirrors: RoomCamera.cs (Update, ScreenMovement, DistanceFromViewedScreen), Room.cs (ScreenMovement, Loaded), PhysicalObject.cs (TerrainImpact)

use rw_assets::Room;
use rw_random::UnityRandom;

use crate::game::camera::CameraView;
use crate::world::{RealizedRoom, World};
use crate::{BodyChunk, FloatExt, TerrainImpact, Vec2, WaterDrip};

/// One `room.ScreenMovement(sourcePos, bump, shake)` call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenMovement {
    /// `sourcePos`; `None` shakes at full strength wherever it comes from.
    pub source: Option<Vec2>,
    pub bump: Vec2,
    pub shake: f32,
}

/// Camera 0's shake state (`RoomCamera.screenShake`, `microShake`,
/// `screenMovementShake`) and its window position during a tick.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraShake {
    /// `pos`: the window's bottom-left in room pixels, as the app's camera last handed
    /// it in, plus this tick's jump and bumps.
    pub pos: Vec2,
    /// `options.ScreenSize.x`.
    pub screen_width: f32,
    pub screen_shake: f32,
    pub micro_shake: f32,
    pub screen_movement_shake: f32,
    /// What this tick's jump and bumps added to `pos`.
    displacement: Vec2,
    /// The last `ScreenMovement` that reached the camera, as applied (bump and shake
    /// divided by the distance factor).
    pub last: Option<ScreenMovement>,
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            screen_width: 1366.0,
            screen_shake: 0.0,
            micro_shake: 0.0,
            screen_movement_shake: 0.0,
            displacement: Vec2::ZERO,
            last: None,
        }
    }
}

impl CameraShake {
    /// A shake whose window is `ScreenSize.x` wide until the app hands in the camera.
    #[must_use]
    pub fn with_screen_width(screen_width: f32) -> Self {
        Self {
            screen_width,
            ..Self::default()
        }
    }

    /// The camera's window after `Update`'s lerp and clamp, before the jump.
    pub fn set_pos(&mut self, pos: Vec2, screen_width: f32) {
        self.pos = pos;
        self.screen_width = screen_width;
    }

    /// What the tick's jump and bumps moved the window by since the last call.
    pub fn take_displacement(&mut self) -> Vec2 {
        std::mem::take(&mut self.displacement)
    }

    fn displace(&mut self, d: Vec2) {
        self.pos += d;
        self.displacement += d;
    }

    /// `RoomCamera.Update` after the lerp and clamp: while `screenShake` is up, the window
    /// jumps `8 * screenShake` px at a random angle (one draw) and `screenShake` decays
    /// (by 0.7 down to 1, then by 0.025); `microShake` decays by 0.025 and is cut to 80 %
    /// above 2. Base game outside the rain: no rain, sandstorm or flood term.
    pub fn update(&mut self, rng: &mut UnityRandom) {
        if self.screen_shake > 0.0 {
            let jump = Vec2::from_degrees(rng.value() * 360.0) * (8.0 * self.screen_shake);
            self.displace(jump);
            self.screen_shake = Self::decay(self.screen_shake);
        }
        self.micro_shake = (self.micro_shake - 0.025).max(0.0);
        if self.micro_shake > 2.0 {
            self.micro_shake *= 0.8;
        }
    }

    fn decay(shake: f32) -> f32 {
        if shake > 1.0 {
            (shake - 0.7).max(1.0)
        } else {
            (shake - 0.025).max(0.0)
        }
    }

    /// `RoomCamera.ScreenMovement`: a movement from `source` is weakened by its distance
    /// from the window; the bump pushes the window at once, the shake raises
    /// `screenShake`, and `microShake` to the shake minus 3.
    pub fn screen_movement(&mut self, m: ScreenMovement) {
        let num = m.source.map_or(1.0, |s| {
            1.0 + self.distance_from_viewed_screen(s).lerp(1.0, 0.95)
        });
        self.displace(-(m.bump / num));
        let shake = m.shake / num;
        self.screen_movement_shake = shake;
        if self.screen_shake < shake {
            self.screen_shake = shake;
        }
        if self.micro_shake < shake - 3.0 {
            self.micro_shake = shake - 3.0;
        }
        self.last = Some(ScreenMovement {
            source: m.source,
            bump: m.bump / num,
            shake,
        });
    }

    /// `DistanceFromViewedScreen`: how far `v` is outside the window
    /// (`ScreenSize.x` by 768 px from `pos`).
    #[must_use]
    pub fn distance_from_viewed_screen(&self, v: Vec2) -> f32 {
        let inside = Vec2::new(
            v.x.clamp(self.pos.x, self.pos.x + self.screen_width),
            v.y.clamp(self.pos.y, self.pos.y + 768.0),
        );
        v.distance(inside)
    }

    /// `ScreenShake`: `InverseLerp(0.01, 1.5, Max(screenShake, microShake))`.
    #[must_use]
    pub fn shake_property(&self) -> f32 {
        self.screen_shake
            .max(self.micro_shake)
            .inverse_lerp(0.01, 1.5)
    }
}

impl TerrainImpact {
    /// `PhysicalObject.TerrainImpact`'s `room.ScreenMovement`: a first contact whose speed
    /// times the chunk's mass passes 7 bumps by a tenth of it along the surface's side
    /// and shakes by its excess over 30, fiftieths.
    #[must_use]
    pub fn screen_movement(&self, chunk: &BodyChunk) -> Option<ScreenMovement> {
        let force = self.speed * chunk.mass;
        if !self.first_contact || force <= 7.0 {
            return None;
        }
        let dir = Vec2::new(f32::from(self.direction.x), f32::from(self.direction.y));
        Some(ScreenMovement {
            source: Some(chunk.pos),
            bump: dir * (force * 0.1),
            shake: ((force - 30.0) / 50.0).max(0.0),
        })
    }
}

impl RealizedRoom {
    /// `Room.Loaded`'s `ceilingTiles`: every non-Solid tile under a Solid one and over a
    /// non-Solid one, above the default water level, column by column.
    #[must_use]
    pub fn find_ceiling_tiles(room: &Room) -> Vec<(i32, i32)> {
        let water = room.water_level().unwrap_or(-1);
        let mut tiles = Vec::new();
        for x in 0..room.width() {
            for y in 0..room.height() - 1 {
                if !room.tile(x, y).is_solid()
                    && room.tile(x, y + 1).is_solid()
                    && !room.tile(x, y - 1).is_solid()
                    && y > water
                {
                    tiles.push((x, y));
                }
            }
        }
        tiles
    }

    /// The drip half of `Room.ScreenMovement`: a movement whose bump and shake pass 2
    /// (`|bump| / 4 + 3 * shake`) shakes up to 40 drips off random ceiling tiles of room
    /// `index`. Each draws its tile and x jitter; only one a camera sees within 300 px
    /// is made (its x speed, then `WaterDrip`'s life).
    pub fn shaken_ceiling_drips(
        &self,
        index: i32,
        m: &ScreenMovement,
        camera: &CameraView,
        rng: &mut UnityRandom,
    ) -> Vec<WaterDrip> {
        let force = m.bump.length() * 0.25 + m.shake * 3.0;
        if force <= 2.0 || self.ceiling_tiles.is_empty() {
            return Vec::new();
        }
        let tiles = self.ceiling_tiles.len();
        let n = (tiles as f32).lerp(150.0, 0.25) / 100.0
            * self.settings.ceiling_drips
            * force.clamp(2.0, 8.0);
        let mut drips = Vec::new();
        for _ in 0..40.min(n.round_ties_even() as i32) {
            let len = i32::try_from(tiles).unwrap_or(i32::MAX);
            let tile = self.ceiling_tiles[rng.range_i32(0, len) as usize];
            let pos = Vec2::middle_of_tile(tile) + Vec2::new((-10.0).lerp(10.0, rng.value()), 9.0);
            if camera.viewed(index, &self.room, pos, 300.0) {
                let vel = Vec2::new((-1.5).lerp(1.5, rng.value()), 0.0);
                drips.push(WaterDrip::new(pos, vel, false, rng));
            }
        }
        drips
    }
}

impl World {
    /// `Room.ScreenMovement` of room `index` (Remix's clamps left out): camera 0's side
    /// when it shows the room, then the ceiling drips, viewed or not.
    pub(crate) fn screen_movement(
        &mut self,
        index: i32,
        m: ScreenMovement,
        camera: &CameraView,
        shake: &mut CameraShake,
        rng: &mut UnityRandom,
    ) {
        if camera.room == index {
            shake.screen_movement(m);
        }
        let Some(room) = self.realized(index) else {
            return;
        };
        for drip in room.shaken_ceiling_drips(index, &m, camera, rng) {
            self.add_drip(index, drip);
        }
    }
}

#[cfg(test)]
#[path = "tests/screen_shake.rs"]
mod tests;
