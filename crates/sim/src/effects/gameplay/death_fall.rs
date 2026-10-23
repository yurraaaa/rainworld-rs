//! `DeathFallGraphic`'s state: whether a room has one (an open bottom), its `counter`
//! (the phase of the sprite's alpha pulse), its height and the focal points that move it.
//! The sprite is drawn by the app. The game's side (`Room.Loaded` making it, its turn in
//! the update list) is `effects/gameplay/death_fall_turn.rs`.
//!
//! Mirrors: DeathFallGraphic.cs (RoomRequiresDeathFall, InitiateSprites, Update, DrawSprites), Room.cs (Loaded: DeathFallFocus, CameraViewingPoint)

use rw_assets::{PlacedObject, PlacedObjectType, Room};

use crate::Vec2;
use crate::game::camera::CameraView;
use crate::water::Water;

/// A room's `DeathFallGraphic` in its update list.
#[derive(Debug, Clone, PartialEq)]
pub struct DeathFall {
    pub update_order: u64,
    /// The lowest camera's bottom (`defaultHeight`, set in `InitiateSprites`).
    default_height: f32,
    /// `room.deathFallFocalPoints`: the active `DeathFallFocus` objects' positions.
    focal_points: Vec<Vec2>,
    pub height: f32,
    /// Ticks updated (`Update`), the phase of the sprite's alpha pulse.
    pub counter: i32,
}

impl DeathFall {
    /// `Room.Loaded`'s `DeathFallGraphic`, when `RoomRequiresDeathFall`: not in an
    /// unbroken shelter, and some bottom-row tile open (no water) or under water below
    /// -1200 px. The `RoomWrap` (DLC) and terrain-curve checks are off in the base game.
    /// `defaultHeight` is `InitiateSprites`' value, known from the room's cameras; the
    /// height stays it until the first `Update` or [`Self::initiate_sprites`].
    #[must_use]
    pub fn new(
        room: &Room,
        water: Option<&Water>,
        unbroken_shelter: bool,
        placed: &[PlacedObject],
    ) -> Option<Self> {
        if unbroken_shelter {
            return None;
        }
        let open = (0..room.width()).any(|x| match water {
            None => !room.tile(x, 0).is_solid(),
            Some(w) => w.level_at(x as f32 * 20.0 + 10.0) < -1200.0,
        });
        if !open {
            return None;
        }
        let default_height = room.cameras().iter().map(|c| c[1]).fold(f32::MAX, f32::min);
        Some(Self {
            update_order: 0,
            default_height,
            focal_points: placed
                .iter()
                .filter(|o| o.active && o.kind == PlacedObjectType::DeathFallFocus.value())
                .map(|o| Vec2::from(o.pos))
                .collect(),
            height: default_height,
            counter: 0,
        })
    }

    #[must_use]
    pub fn default_height(&self) -> f32 {
        self.default_height
    }

    /// `InitiateSprites`' reset of the height, when a camera starts drawing the room.
    pub fn initiate_sprites(&mut self) {
        self.height = self.default_height;
    }

    /// The height `Update` sets under `view`: the default, or the bottom of the first
    /// camera (`CameraViewingPoint`, 1366x768 rects) holding the first focal point that
    /// camera 0 shows.
    #[must_use]
    pub fn height_under(&self, room: &Room, index: i32, view: &CameraView) -> f32 {
        for &p in &self.focal_points {
            if view.viewed(index, room, p, 0.0)
                && let Some(cam) = Self::camera_viewing_point(room, p)
            {
                return cam[1];
            }
        }
        self.default_height
    }

    /// `Update`: one more tick of the counter, whether or not a camera views the room.
    pub fn update(&mut self, room: &Room, index: i32, view: &CameraView) {
        self.counter += 1;
        self.height = self.height_under(room, index, view);
    }

    /// `DrawSprites`' alpha: `0.85 + 0.15 sin((counter + timeStacker) / 25)`.
    #[must_use]
    pub fn alpha(&self, time_stacker: f32) -> f32 {
        0.85 + 0.15 * ((self.counter as f32 + time_stacker) / 25.0).sin()
    }

    /// `Room.CameraViewingPoint`.
    fn camera_viewing_point(room: &Room, p: Vec2) -> Option<[f32; 2]> {
        room.cameras()
            .iter()
            .copied()
            .find(|c| p.x > c[0] && p.x < c[0] + 1366.0 && p.y > c[1] && p.y < c[1] + 768.0)
    }
}

#[cfg(test)]
#[path = "tests/death_fall.rs"]
mod tests;
