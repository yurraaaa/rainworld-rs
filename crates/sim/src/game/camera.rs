//! What the sim reads of camera 0 (`RoomCamera`): the room it shows, the shown camera
//! position and `mostLikelyNextCamPos`, and the visibility tests on them
//! (`PositionCurrentlyVisible`, `PositionVisibleInNextScreen`, `Room.ViewedByAnyCamera`).
//! The app's room camera chooses the cameras and hands them in every tick.
//!
//! Mirrors: RoomCamera.cs (PositionCurrentlyVisible, PositionVisibleInNextScreen), Room.cs (ViewedByAnyCamera, CameraViewingPoint)

use rw_assets::Room;

use crate::Vec2;

/// Camera 0's state as the rooms read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraView {
    /// Global index of the camera's room.
    pub room: i32,
    /// `currentCameraPosition`.
    pub current: usize,
    /// `mostLikelyNextCamPos`.
    pub next: usize,
}

impl CameraView {
    /// `CamPos(i)`: camera `i`'s image bottom-left (the origin for a missing one).
    #[must_use]
    pub fn cam_pos(room: &Room, i: usize) -> Vec2 {
        room.cameras().get(i).map_or(Vec2::ZERO, |&p| Vec2::from(p))
    }

    /// `GetVisibleRect(margin, widescreen).Contains(pos)` (base game: a 1024 px wide
    /// screen; `Rect.Contains` includes the low edges).
    #[must_use]
    pub fn position_currently_visible(
        &self,
        room: &Room,
        pos: Vec2,
        margin: f32,
        widescreen: bool,
    ) -> bool {
        let cam = Self::cam_pos(room, self.current);
        let wide = if widescreen { 190.0 } else { 0.0 };
        let x_min = cam.x - 188.0 - margin - wide;
        let x_max = cam.x + 188.0 + 1024.0 + margin + wide;
        let y_min = cam.y - 18.0 - margin;
        let y_max = cam.y + 18.0 + 768.0 + margin;
        pos.x >= x_min && pos.x < x_max && pos.y >= y_min && pos.y < y_max
    }

    /// `PositionVisibleInNextScreen`.
    #[must_use]
    pub fn position_visible_in_next_screen(
        &self,
        room: &Room,
        pos: Vec2,
        margin: f32,
        widescreen: bool,
    ) -> bool {
        if self.next >= room.cameras().len() {
            return false;
        }
        let cam = Self::cam_pos(room, self.next);
        let wide = if widescreen { 190.0 } else { 0.0 };
        pos.x > cam.x - 188.0 - margin - wide
            && pos.x < cam.x + 188.0 + 1024.0 + margin + wide
            && pos.y > cam.y - 18.0 - margin
            && pos.y < cam.y + 18.0 + 768.0 + margin
    }

    /// `Room.ViewedByAnyCamera(pos, margin)` for room `index` (one camera).
    #[must_use]
    pub fn viewed(&self, index: i32, room: &Room, pos: Vec2, margin: f32) -> bool {
        self.room == index && self.position_currently_visible(room, pos, margin, false)
    }
}

/// Room methods the cameras define.
pub trait RoomCameras {
    /// `Room.CameraViewingPoint`: the first camera whose 1366x768 rect holds `p`.
    fn camera_viewing_point(&self, p: Vec2) -> Option<usize>;
}

impl RoomCameras for Room {
    fn camera_viewing_point(&self, p: Vec2) -> Option<usize> {
        self.cameras()
            .iter()
            .position(|c| p.x > c[0] && p.x < c[0] + 1366.0 && p.y > c[1] && p.y < c[1] + 768.0)
    }
}
