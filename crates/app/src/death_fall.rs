//! The camera's room's `DeathFallGraphic`: whether the room has one (an open bottom) and
//! its height, which `RoomCamera.DrawUpdate` reads for `_waterLevel`. Its sprite (the
//! `DeathFall` shader in `Bloom`) is not drawn yet.
//!
//! Mirrors: DeathFallGraphic.cs (RoomRequiresDeathFall, InitiateSprites, Update), Room.cs (Loaded: DeathFallFocus)

use rw_assets::{PlacedObject, PlacedObjectType, Room};
use rw_sim::{CameraView, Vec2, Water};

/// A room's `DeathFallGraphic`.
#[derive(Debug, Clone, PartialEq)]
pub struct DeathFall {
    /// The lowest camera's bottom (`defaultHeight`, set in `InitiateSprites`).
    default_height: f32,
    /// `room.deathFallFocalPoints`: the active `DeathFallFocus` objects' positions.
    focal_points: Vec<Vec2>,
    pub height: f32,
}

impl DeathFall {
    /// `Room.Loaded`'s `DeathFallGraphic`, when `RoomRequiresDeathFall`: not in an
    /// unbroken shelter, and some bottom-row tile open (no water) or under water below
    /// -1200 px. The `RoomWrap` (DLC) and terrain-curve checks are off in the base game.
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
            default_height,
            focal_points: placed
                .iter()
                .filter(|o| o.active && o.kind == PlacedObjectType::DeathFallFocus.value())
                .map(|o| Vec2::from(o.pos))
                .collect(),
            height: default_height,
        })
    }

    /// `Update`: the default height, or the bottom of the first camera of the room
    /// (`CameraViewingPoint`, 1366x768 rects) holding the first focal point camera 0
    /// shows.
    pub fn update(&mut self, room: &Room, index: i32, view: &CameraView) {
        self.height = self.default_height;
        for &p in &self.focal_points {
            if view.viewed(index, room, p, 0.0)
                && let Some(cam) = Self::camera_viewing_point(room, p)
            {
                self.height = cam[1];
                break;
            }
        }
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
mod tests {
    use super::*;

    /// A 20 x 12 room of air, its bottom row solid if asked, with cameras at file
    /// positions (0, 0) and (0, 300): room y -560 and -860.
    fn room(solid_bottom: bool) -> Room {
        let (w, h) = (20, 12);
        let column = format!(
            "{}{}",
            "0|".repeat(h - 1),
            if solid_bottom { "1|" } else { "0|" }
        );
        let text = format!(
            "Test\n{w}*{h}|-1|0\n1*1|0|0\n0,0|0,300\nBorder: Solid\n\n\n\n\n17|0|0|\nblob\n{}\n",
            column.repeat(w)
        );
        Room::parse(&text).expect("test room parses")
    }

    fn focus(pos: [f32; 2]) -> PlacedObject {
        PlacedObject {
            kind: PlacedObjectType::DeathFallFocus.value().to_owned(),
            pos,
            data: String::new(),
            active: true,
        }
    }

    /// `RoomRequiresDeathFall`: an open bottom tile and not an unbroken shelter; the
    /// height starts at the lowest camera's bottom.
    #[test]
    fn open_bottom_rooms_have_a_death_fall() {
        let fall = DeathFall::new(&room(false), None, false, &[]).expect("open bottom");
        assert_eq!(fall.height, -860.0);
        assert!(DeathFall::new(&room(true), None, false, &[]).is_none());
        assert!(DeathFall::new(&room(false), None, true, &[]).is_none());
    }

    /// `Update`: a focal point the camera shows sets the height to the bottom of the
    /// first camera rect (1366x768) holding it; out of view, the default.
    #[test]
    fn a_viewed_focal_point_moves_the_height() {
        let r = room(false);
        let mut fall = DeathFall::new(&r, None, false, &[focus([100.0, 0.0])]).unwrap();
        let view = CameraView {
            room: 3,
            current: 0,
            next: 0,
        };
        fall.update(&r, 3, &view);
        assert_eq!(fall.height, -560.0);
        fall.update(&r, 4, &view);
        assert_eq!(fall.height, -860.0, "camera in another room");
    }
}
